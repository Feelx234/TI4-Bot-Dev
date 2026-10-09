import {
  ClientMessage,
  InitialSnapshotMsg,
  PendingChoiceDto,
  PROTOCOL_VERSION,
  PublicTurnStatus,
  ServerMessage,
  StateUpdateMsg,
  ViewerRole,
  HistoryStatus,
  ReactionModeSetting,
  ReactionIntentStateMsg,
  PreviewRefusal,
  SecondaryPreviewBody,
} from "./types.ts";
import { readDeclaredTriggers, writeDeclaredTriggers } from "./bluffTriggers.ts";
import {
  decodeDecisionTrigger,
  decodeInitialSnapshot,
  decodeServerMessage,
  isStaleServerMessage,
} from "./decode.ts";
import { decodeSplicePreview, type SpliceEdit, type SplicePreview } from "./splice.ts";
import {
  decodeTurnRedoStatusResponse,
  type TurnRedoCommand,
  type TurnRedoStatus,
} from "./turnRedo.ts";

/** What the server answered to a `preview_secondary` request. */
export type SecondaryPreviewReply =
  | {
      kind: "preview";
      body: SecondaryPreviewBody;
      /** The game version and decision count the answer was computed at. */
      asOfVersion: number;
      asOfDecisions: number;
    }
  | { kind: "refused"; reason: PreviewRefusal; detail: string };

/** The server does not know the preview message (an older build): callers fall back to estimates. */
export class PreviewUnsupportedError extends Error {
  constructor(message = "This server cannot preview a secondary") {
    super(message);
    this.name = "PreviewUnsupportedError";
  }
}

/** How long a preview request may take before the caller falls back to its estimate. */
export const PREVIEW_TIMEOUT_MS = 4_000;

export type ConnectionStatus =
  "connecting" | "connected" | "disconnected" | "error";
export type SnapshotState = InitialSnapshotMsg | StateUpdateMsg;
export type GameLogEntry = import("./types.ts").GameEvent;
export type HistoryChange =
  | "undo"
  | "undo_batch"
  | "undo_pipeline"
  | "redo"
  | "redo_batch"
  | "redo_pipeline"
  | { eventId: string }
  | { cursor: number };

export type MovementStep =
  | { kind: "move"; origin: string; unit: string; damaged: boolean }
  | {
      kind: "load";
      origin: string;
      unit: string;
      source: string | null;
      damaged: boolean;
    }
  | { kind: "done_loading" }
  | { kind: "done_moving" };
export type BasketPlan =
  | {
      kind: "payment";
      steps: ({ kind: "exhaust"; planet: string } | { kind: "trade_good" })[];
    }
  | {
      kind: "agenda_vote_planets";
      steps: (
        { kind: "vote_planet"; planet: string } | { kind: "done_voting" }
      )[];
    }
  | {
      kind: "production";
      destination: string;
      steps: (
        | { kind: "produce"; unit: string; count: number }
        | { kind: "done_producing" }
      )[];
    }
  | {
      kind: "casualties";
      steps: import("../presentation/hitAssignment.ts").CasualtyStep[];
    }
  | {
      kind: "tokens";
      steps: import("../presentation/commandTokens.ts").TokenStep[];
    };

export type BatchPlan =
  | BasketPlan
  | { kind: "tactical_movement"; destination: string; steps: MovementStep[] };

/**
 * A plan the server stopped part-way because a reaction window opened between its steps. The
 * applied steps are committed and the window waits for its holder; `plan` is what was left. It is
 * sent again only when the player asks, and the server re-checks it against the offers then.
 */
export interface BatchResume {
  plan: BatchPlan;
  /** Planned steps the server applied before it stopped. */
  applied: number;
  /** What the engine is waiting on now. */
  waiting: { subtype: string | null; ownSeat: boolean };
}

/** Decision subtypes each plan kind is answered through. */
const PLAN_SUBTYPES: Record<BatchPlan["kind"], string[]> = {
  tactical_movement: ["movement_step"],
  payment: ["pay_resources", "pay_influence"],
  agenda_vote_planets: ["vote_exhaust_planet"],
  production: ["produce_unit"],
  casualties: ["sustain_damage", "assign_casualty", "assign_ground_casualty"],
  tokens: ["gain_command_token", "buy_token_with_influence", "pay_influence"],
};

/**
 * Keeps a paused plan only while it can still be continued: the engine is asking for a reaction,
 * or is back at a decision the plan answers. Anything else means the game moved on.
 */
export function settleBatchResume(state: GameSessionState): GameSessionState {
  const resume = state.batchResume;
  if (!resume) return state;
  const subtype = state.pendingChoice?.context?.subtype;
  if (!state.pendingChoice || !subtype) return state;
  if (subtype.startsWith("reaction_") || subtype.startsWith("play_reaction_")) return state;
  if (PLAN_SUBTYPES[resume.plan.kind].includes(subtype)) return state;
  return { ...state, batchResume: null };
}

/** Whether the paused plan can be sent again now: its own seat is back at a decision it answers. */
export function canContinueBatch(
  resume: BatchResume,
  pending: { actor: string; context?: { subtype: string } } | null,
  seat: string | null | undefined,
): boolean {
  return Boolean(
    pending &&
      seat &&
      pending.actor === seat &&
      pending.context &&
      PLAN_SUBTYPES[resume.plan.kind].includes(pending.context.subtype),
  );
}

const HISTORY_RETRY_ATTEMPTS = 20;
/** A submit the server never acknowledges is abandoned after this long, so a click can re-send. */
const SUBMISSION_TIMEOUT_MS = 10_000;

export interface GameSessionState {
  status: ConnectionStatus;
  gameVersion: number;
  snapshot: SnapshotState | null;
  pendingChoice: PendingChoiceDto | null;
  turnStatus: PublicTurnStatus | null;
  lastError: string | null;
  /** This seat's own bluff settings, once the server has sent them (never for spectators). */
  reactionIntent?: ReactionIntentStateMsg | null;
  events: GameLogEntry[];
  history: HistoryStatus;
  /** A plan the server paused at a reaction window; see {@link BatchResume}. */
  batchResume?: BatchResume | null;
}

export interface GameSessionClientOptions {
  gameId: string;
  viewer: ViewerRole;
  serverUrl?: string;
}

type Listener = () => void;

const initialState: GameSessionState = {
  status: "connecting",
  gameVersion: 0,
  snapshot: null,
  pendingChoice: null,
  turnStatus: null,
  lastError: null,
  events: [],
  history: { cursor: 0, redo_count: 0 },
};

/** Keep the complete authoritative history, including early rounds and batches. */
export function serverEventLog(
  entries: readonly GameLogEntry[] | undefined,
): GameLogEntry[] {
  return [...(entries ?? [])];
}

const eventIds = new WeakMap<GameLogEntry[], Set<string>>();
function idsFor(entries: GameLogEntry[]): Set<string> {
  let ids = eventIds.get(entries);
  if (!ids) {
    ids = new Set(entries.map((entry) => entry.id));
    eventIds.set(entries, ids);
  }
  return ids;
}

function rejectionMessage(
  message: Extract<ServerMessage, { type: "action_rejected" }>,
): string {
  switch (message.reason.reason) {
    case "stale_version":
      return `Rejected: Stale version (expected ${message.reason.expected}, server at ${message.reason.current})`;
    case "stale_nonce":
      return "Rejected: Stale decision nonce";
    case "unauthorized_seat":
      return "Rejected: Unauthorized seat";
    case "unknown_option":
      return `Rejected: Unknown option '${message.reason.option_id}'`;
    case "no_pending_choice":
      return "Rejected: No decision is currently pending";
    case "validation_failed":
      return `Rejected: ${message.reason.message}`;
  }
}

function pendingChoice(
  envelope: import("./types.ts").PendingChoiceEnvelope,
): PendingChoiceDto {
  if (!envelope.choice) return envelope as unknown as PendingChoiceDto;
  const context = envelope.choice.context;
  return {
    nonce: envelope.nonce,
    actor: envelope.choice.player,
    prompt: envelope.choice.prompt,
    options: envelope.choice.options,
    // The trigger is display data: keep a well-formed one, drop a damaged one.
    context:
      context && "trigger" in context
        ? { ...context, trigger: decodeDecisionTrigger(context.trigger) }
        : context,
    ...(envelope.choice.details ? { details: envelope.choice.details } : {}),
  };
}

/** Applies only validated, non-stale protocol messages to the client projection. */
export function reduceServerMessage(
  state: GameSessionState,
  message: ServerMessage,
): GameSessionState {
  if (isStaleServerMessage(message, state.gameVersion)) return state;

  switch (message.type) {
    case "initial_snapshot":
    case "state_update":
      return {
        ...state,
        snapshot: message,
        gameVersion: message.game_version,
        turnStatus: message.turn_status,
        pendingChoice: message.pending_choice
          ? pendingChoice(message.pending_choice)
          : null,
        events:
          message.type === "initial_snapshot"
            ? serverEventLog(message.events)
            : state.events,
        history: message.history ?? state.history,
      };
    case "event":
      if (idsFor(state.events).has(message.entry.id)) return state;
      const nextEvents = [...state.events, message.entry];
      eventIds.set(nextEvents, idsFor(state.events).add(message.entry.id));
      return {
        ...state,
        events: nextEvents,
        history:
          message.entry.decision_count === undefined
            ? state.history
            : {
                ...state.history,
                cursor: Math.max(
                  state.history.cursor,
                  message.entry.decision_count,
                ),
                redo_count: 0,
              },
      };
    case "pending_choice":
      return {
        ...state,
        gameVersion: message.game_version,
        pendingChoice: pendingChoice({
          nonce: message.nonce,
          choice: message.choice,
        }),
      };
    case "turn_status":
      return {
        ...state,
        gameVersion: message.game_version,
        turnStatus: message.status,
        // The server sends TurnStatus instead of PendingChoice to every non-actor.
        // A previous actor must not retain an actionable choice during a nested window.
        pendingChoice: null,
      };
    case "action_accepted":
      return { ...state, lastError: null };
    case "action_rejected":
      return { ...state, lastError: rejectionMessage(message) };
    case "error":
      return { ...state, lastError: `Server Error: ${message.message}` };
    case "game_over":
      // The server pushes this when the game ends and then no more turn statuses. Ignoring it left
      // every open tab on the last phase banner until it was reloaded (found by the endgame smoke
      // preset).
      return {
        ...state,
        gameVersion: message.game_version,
        turnStatus: { kind: "game_over", winner: message.winner ?? null },
        pendingChoice: null,
      };
    case "pong":
      return state;
    case "reaction_intent_state":
      return { ...state, reactionIntent: message };
    case "secondary_preview":
      // Answered to the asking call, never part of the shared projection.
      return state;
  }
}

/** Owns every network ingress point and the lifecycle of one game-session connection. */
export class GameSessionClient {
  private state = initialState;
  private readonly listeners = new Set<Listener>();
  private socket: WebSocket | null = null;
  private stopped = false;
  private submission: {
    nonce: string;
    optionId: string;
    version: number;
    accepted: boolean;
    promise: Promise<void>;
    resolve: () => void;
    reject: (error: Error) => void;
  } | null = null;
  // An engine step may offer another human reaction before acknowledging the
  // previous choice. Keep its promise until the step commits, but allow the
  // newly offered choice to be submitted meanwhile.
  private priorSubmissions: NonNullable<GameSessionClient["submission"]>[] = [];
  private heartbeat: ReturnType<typeof setInterval> | null = null;
  private retry: ReturnType<typeof setTimeout> | null = null;
  private pingSequence = 0;
  private pendingBatch: {
    nonce: string;
    plan: string;
    requestId: string;
  } | null = null;
  /** The batch being sent now (see `submitBatch`); cleared when its request settles. */
  private batchInFlight: {
    nonce: string;
    plan: string;
    promise: Promise<void>;
  } | null = null;
  private previewSeq = 0;
  /** Set once an older server answered the preview message with "unknown message type". */
  private previewUnsupported = false;
  private readonly previews = new Map<
    number,
    {
      resolve: (reply: SecondaryPreviewReply) => void;
      reject: (error: Error) => void;
      timer: ReturnType<typeof setTimeout>;
    }
  >();

  constructor(private readonly options: GameSessionClientOptions) {}

  /**
   * Read-only: what this seat would be asked for the secondary in progress (the strategy card
   * `card`, played by `primary`) if its window opened right now, after the `answers` already given
   * (the first answers the window question, the rest the follow-up questions in order). The server
   * changes nothing and answers this connection only. Rejects with {@link PreviewUnsupportedError}
   * for an older server (remembered for the life of this client) and with an Error on a timeout or
   * a lost connection; callers then use their own estimate.
   */
  previewSecondary(
    card: string,
    primary: string,
    answers: readonly string[],
  ): Promise<SecondaryPreviewReply> {
    if (this.options.viewer.role !== "player") return Promise.reject(new PreviewUnsupportedError());
    if (this.previewUnsupported) return Promise.reject(new PreviewUnsupportedError());
    if (!this.socket || this.socket.readyState !== WebSocket.OPEN)
      return Promise.reject(new Error("Not connected to the server"));
    const requestId = ++this.previewSeq;
    const message: ClientMessage = {
      type: "preview_secondary",
      protocol_version: PROTOCOL_VERSION,
      game_id: this.options.gameId,
      request_id: requestId,
      card,
      primary,
      answers: [...answers],
    };
    return new Promise<SecondaryPreviewReply>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.previews.delete(requestId);
        reject(new Error("The server did not answer the preview in time"));
      }, PREVIEW_TIMEOUT_MS);
      this.previews.set(requestId, { resolve, reject, timer });
      try {
        this.socket?.send(JSON.stringify(message));
      } catch (error) {
        clearTimeout(timer);
        this.previews.delete(requestId);
        reject(error instanceof Error ? error : new Error(String(error)));
      }
    });
  }

  private settlePreviews(error: Error): void {
    for (const pending of this.previews.values()) {
      clearTimeout(pending.timer);
      pending.reject(error);
    }
    this.previews.clear();
  }

  getState(): GameSessionState {
    return this.state;
  }

  subscribe(listener: Listener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  start(): void {
    this.stopped = false;
    this.setState({ ...this.state, status: "connecting", lastError: null });
    void this.loadSnapshot();
    this.openSocket();
  }

  stop(): void {
    this.stopped = true;
    this.batchInFlight = null;
    this.clearTimers();
    this.detachSocket();
    this.rejectSubmission("Submission stopped");
    this.settlePreviews(new Error("Session stopped"));
  }

  async submitChoice(optionId: string): Promise<void> {
    const { pendingChoice, gameVersion } = this.state;
    if (!this.socket || this.socket.readyState !== WebSocket.OPEN) {
      const message = "Cannot submit choice: not connected to server";
      this.setState({ ...this.state, lastError: message });
      throw new Error(message);
    }
    if (!pendingChoice) {
      const message = "No decision currently pending";
      this.setState({ ...this.state, lastError: message });
      throw new Error(message);
    }
    if (this.submission) {
      if (
        this.submission.nonce === pendingChoice.nonce &&
        this.submission.optionId === optionId
      )
        return this.submission.promise;
      if (this.submission.nonce === pendingChoice.nonce)
        throw new Error("Another choice submission is still pending");
      this.priorSubmissions.push(this.submission);
      this.submission = null;
    }

    const message: ClientMessage = {
      type: "submit_choice",
      protocol_version: PROTOCOL_VERSION,
      game_id: this.options.gameId,
      nonce: pendingChoice.nonce,
      expected_version: gameVersion,
      option_id: optionId,
    };
    let resolve!: () => void;
    let reject!: (error: Error) => void;
    let timeout: ReturnType<typeof setTimeout> | undefined;
    const promise = new Promise<void>((done, fail) => {
      resolve = () => {
        clearTimeout(timeout);
        done();
      };
      reject = (error) => {
        clearTimeout(timeout);
        fail(error);
      };
    });
    this.submission = {
      nonce: pendingChoice.nonce,
      optionId,
      version: gameVersion,
      accepted: false,
      promise,
      resolve,
      reject,
    };
    try {
      this.socket.send(JSON.stringify(message));
    } catch (error) {
      this.rejectSubmission(`Could not send choice: ${String(error)}`);
    }
    // Without this, a submit that never gets an answer keeps returning the same dead promise to
    // every later click for the same option, and the decision looks frozen.
    timeout = setTimeout(() => {
      if (this.submission?.promise === promise && !this.submission.accepted)
        this.rejectSubmission("No response from the server; try again");
    }, SUBMISSION_TIMEOUT_MS);
    return promise;
  }

  /**
   * Asks the server to stop (or resume) offering one action card to this seat for the rest of
   * the game. The answer is the seat's next state update, which carries the modes; nothing is
   * assumed locally. Rejected when not connected or when watching.
   */
  setReactionMode(card: string, mode: ReactionModeSetting): void {
    if (this.options.viewer.role !== "player") {
      this.setState({ ...this.state, lastError: "Only a seated player can change reaction modes" });
      return;
    }
    if (!this.socket || this.socket.readyState !== WebSocket.OPEN) {
      this.setState({
        ...this.state,
        lastError: "Cannot change the setting: not connected to server",
      });
      return;
    }
    const message: ClientMessage = {
      type: "set_reaction_mode",
      protocol_version: PROTOCOL_VERSION,
      game_id: this.options.gameId,
      card,
      mode,
    };
    try {
      this.socket.send(JSON.stringify(message));
    } catch (error) {
      this.setState({ ...this.state, lastError: `Could not send the setting: ${String(error)}` });
    }
  }

  /**
   * Declares which kinds of reaction window this seat bluffs about (the whole set; empty clears).
   * Kept in this browser per game and seat and sent again on every connect; the server forgets it
   * on restart. The server's answer is the seat's next `reaction_intent_state`.
   */
  setReactionIntent(triggers: string[]): void {
    const viewer = this.options.viewer;
    if (viewer.role !== "player") return;
    writeDeclaredTriggers(this.options.gameId, viewer.seat, triggers);
    this.sendIntent(triggers);
  }

  /** Ends the bluff hold early. Silent when nothing is held. */
  passReactionHold(): void {
    if (this.options.viewer.role !== "player") return;
    if (!this.socket || this.socket.readyState !== WebSocket.OPEN) return;
    try {
      this.socket.send(
        JSON.stringify({
          type: "pass_reaction_hold",
          protocol_version: PROTOCOL_VERSION,
          game_id: this.options.gameId,
        } satisfies ClientMessage),
      );
    } catch {
      // The hold ends by itself within seconds.
    }
  }

  private sendIntent(triggers: string[]): void {
    if (!this.socket || this.socket.readyState !== WebSocket.OPEN) return;
    try {
      this.socket.send(
        JSON.stringify({
          type: "set_reaction_intent",
          protocol_version: PROTOCOL_VERSION,
          game_id: this.options.gameId,
          triggers,
        } satisfies ClientMessage),
      );
    } catch (error) {
      this.setState({ ...this.state, lastError: `Could not send the setting: ${String(error)}` });
    }
  }

  async submitMovementBatch(
    destination: string,
    steps: MovementStep[],
  ): Promise<void> {
    return this.submitBatch({ kind: "tactical_movement", destination, steps });
  }

  /** Sends what is left of a plan the server paused at a reaction window. */
  async resumeBatch(): Promise<void> {
    const resume = this.state.batchResume;
    if (!resume) throw new Error("There is no paused plan to continue");
    const seat =
      this.options.viewer.role === "player" ? this.options.viewer.seat : null;
    if (!canContinueBatch(resume, this.state.pendingChoice, seat))
      throw new Error("Workflow is no longer pending");
    try {
      await this.submitBatch(resume.plan);
    } catch (error) {
      // The server checked the remainder against the current offers and refused it: stage again.
      if (this.state.batchResume === resume)
        this.setState({
          ...this.state,
          batchResume: null,
          lastError: error instanceof Error ? error.message : String(error),
        });
      throw error;
    }
  }

  dismissBatchResume(): void {
    if (this.state.batchResume) this.setState({ ...this.state, batchResume: null });
  }

  async submitBatch(plan: BatchPlan): Promise<void> {
    if (
      this.options.viewer.role !== "player" ||
      !this.options.viewer.playerSession
    )
      throw new Error("A player session is required");
    const pending = this.state.pendingChoice;
    if (
      !pending ||
      pending.actor !== this.options.viewer.seat ||
      !pending.context
    )
      throw new Error("Decision is no longer pending");
    const expected = PLAN_SUBTYPES[plan.kind];
    if (!expected.includes(pending.context.subtype))
      throw new Error("Workflow is no longer pending");
    const serialized = JSON.stringify(plan);
    // One confirmation per decision at a time. The answer of a batch is only applied to the
    // session when it comes back, so until then this state still shows the decision as open at
    // the old version: a second send (a repeated click, or a prepared Auto answer racing a click)
    // would carry that old version and be refused as stale. The same plan shares the running
    // request; a different one waits for the answer by being refused.
    const running = this.batchInFlight;
    if (running && running.nonce === pending.nonce) {
      if (running.plan === serialized) return running.promise;
      throw new Error("Another confirmation for this decision is still being sent");
    }
    const entry: NonNullable<GameSessionClient["batchInFlight"]> = {
      nonce: pending.nonce,
      plan: serialized,
      promise: Promise.resolve(),
    };
    entry.promise = (async () => {
      try {
        await this.sendBatch(plan, pending, serialized);
      } finally {
        if (this.batchInFlight === entry) this.batchInFlight = null;
      }
    })();
    this.batchInFlight = entry;
    return entry.promise;
  }

  private async sendBatch(
    plan: BatchPlan,
    pending: NonNullable<GameSessionClient["state"]["pendingChoice"]>,
    serialized: string,
  ): Promise<void> {
    if (
      this.pendingBatch?.nonce !== pending.nonce ||
      this.pendingBatch.plan !== serialized
    )
      this.pendingBatch = {
        nonce: pending.nonce,
        plan: serialized,
        requestId: crypto.randomUUID(),
      };
    const sentAtVersion = this.state.gameVersion;
    const response = await fetch(
      this.snapshotUrl().replace(/\/snapshot$/, "/batches"),
      {
        method: "POST",
        headers: {
          ...this.snapshotHeaders(),
          "content-type": "application/json",
        },
        body: JSON.stringify({
          request_id: this.pendingBatch.requestId,
          expected_version: this.state.gameVersion,
          nonce: pending.nonce,
          plan,
        }),
      },
    );
    if (!response.ok) {
      if (
        response.status !== 500 &&
        response.status !== 502 &&
        response.status !== 503
      )
        this.pendingBatch = null;
      const body = await response.text();
      let failure: {
        failed_step?: number;
        reason?: string;
        expected?: string;
        message?: string;
      };
      try {
        failure = JSON.parse(body);
      } catch {
        failure = { message: body || `HTTP ${response.status}` };
      }
      // A refusal as stale that a newer ACCEPTED version has already overtaken (the answer that
      // beat this one is applied: a prepared Auto answer racing a click) has nothing left to
      // report: the decision this request targeted is gone. No error; the live state is current.
      if (
        response.status === 409 &&
        /stale decision boundary/.test(body) &&
        this.state.gameVersion > sentAtVersion
      )
        return;
      // The server explains the rejection in `message`; older servers only send the reason.
      throw new Error(
        failure.message
          ? `Batch rejected: ${failure.message}`
          : `Batch step ${(failure.failed_step ?? 0) + 1}: ${failure.reason ?? "batch rejected"}${failure.expected ? ` (${failure.expected})` : ""}`,
      );
    }
    this.pendingBatch = null;
    const result = (await response.json()) as {
      snapshot: unknown;
      active?: boolean;
      interrupted?: {
        applied_steps: number;
        remaining_steps: unknown[];
        offered?: { subtype?: string | null; own_seat?: boolean };
      };
    };
    if (result.active === false)
      throw new Error(
        "This confirmation was already committed but is now undone. Refresh the decision before confirming again.",
      );
    const snapshot = decodeInitialSnapshot(
      { type: "initial_snapshot", ...(result.snapshot as object) },
      this.options.gameId,
    );
    // A reaction window opened between planned steps: what was applied is kept, the window waits
    // for its holder, and the rest of the plan is offered again once it resolves.
    const stopped = result.interrupted?.remaining_steps.length
      ? result.interrupted
      : undefined;
    this.rejectSubmission("Game history changed");
    this.detachSocket();
    this.clearTimers();
    this.setState(
      reduceServerMessage(
        {
          ...this.state,
          pendingChoice: null,
          lastError: null,
          batchResume: stopped
            ? {
                plan: { ...plan, steps: stopped.remaining_steps } as BatchPlan,
                applied: stopped.applied_steps,
                waiting: {
                  subtype: stopped.offered?.subtype ?? null,
                  ownSeat: stopped.offered?.own_seat ?? false,
                },
              }
            : null,
        },
        { ...snapshot, type: "initial_snapshot" },
      ),
    );
    this.openSocket();
  }

  /** Any seated player: the live game history (seed, seats, decisions, events) as pretty JSON. */
  async fetchReplay(): Promise<{ text: string; filename: string }> {
    if (this.options.viewer.role !== "player" || !this.options.viewer.playerSession)
      throw new Error("A player session is required");
    const response = await fetch(this.snapshotUrl().replace(/\/snapshot$/, "/replay"), {
      headers: this.snapshotHeaders(),
    });
    if (!response.ok) {
      const reason = (await response.text().catch(() => "")).trim();
      throw new Error(reason || `The server refused the replay (${response.status})`);
    }
    const replay: unknown = await response.json();
    return {
      text: JSON.stringify(replay, null, 2),
      filename: `ti4-replay-${this.options.gameId}.json`,
    };
  }

  /**
   * Host only, read-only: how would the game fare if one past decision were removed or changed
   * and the rest replayed from the same seed? Changes nothing on the server.
   */
  async previewSplice(edit: SpliceEdit): Promise<SplicePreview> {
    if (this.options.viewer.role !== "player" || !this.options.viewer.playerSession)
      throw new Error("A player session is required");
    const url = this.snapshotUrl().replace(/\/snapshot$/, "/history/splice-preview");
    const response = await fetch(url, {
      method: "POST",
      headers: { ...this.snapshotHeaders(), "content-type": "application/json" },
      body: JSON.stringify({ edit }),
    });
    if (!response.ok) {
      const reason = (await response.text().catch(() => "")).trim();
      throw new Error(reason || `The server refused the splice preview (${response.status})`);
    }
    return decodeSplicePreview(await response.json());
  }

  /** The host changes the authoritative Rust timeline; all clients reconnect to it. */
  async changeHistory(action: HistoryChange): Promise<void> {
    const body =
      typeof action === "string"
        ? { action }
        : "cursor" in action
          ? { action: "restore_cursor", cursor: action.cursor }
          : { action: "restore", event_id: action.eventId };
    await this.replaceHistory("/history", body, "History change");
  }

  /**
   * Any seated player (the host for another seat): where a turn redo stands, or null when none is
   * in flight. The server drops an original that can no longer be restored when this is read.
   */
  async fetchTurnRedoStatus(): Promise<TurnRedoStatus | null> {
    if (this.options.viewer.role !== "player" || !this.options.viewer.playerSession)
      throw new Error("A player session is required");
    const response = await fetch(this.snapshotUrl().replace(/\/snapshot$/, "/turn-redo"), {
      headers: this.snapshotHeaders(),
    });
    if (!response.ok) {
      const reason = (await response.text().catch(() => "")).trim();
      throw new Error(reason || `The server refused the turn redo status (${response.status})`);
    }
    return decodeTurnRedoStatusResponse(await response.json());
  }

  /**
   * Turn redo: rewind a seat's last turn(s), replay the round after the new turn, restore the
   * original timeline, or keep the new one. Every action but `keep` replaces the timeline for
   * everyone, so all clients reconnect to it.
   */
  async turnRedoCommand(command: TurnRedoCommand): Promise<void> {
    if (command.action === "keep") {
      if (this.options.viewer.role !== "player" || !this.options.viewer.playerSession)
        throw new Error("A player session is required");
      const response = await fetch(this.snapshotUrl().replace(/\/snapshot$/, "/turn-redo"), {
        method: "POST",
        headers: { ...this.snapshotHeaders(), "content-type": "application/json" },
        body: JSON.stringify({ action: "keep", expected_version: this.state.gameVersion }),
      });
      if (!response.ok) {
        const reason = (await response.text().catch(() => "")).trim();
        throw new Error(
          reason || `The server refused to keep the new timeline (${response.status})`,
        );
      }
      return;
    }
    await this.replaceHistory("/turn-redo", { ...command }, "Turn redo");
  }

  /**
   * POST a timeline replacement (`/history` or `/turn-redo`) and switch to the snapshot of the
   * replacement session. Retries while the server is still advancing toward its next human choice.
   */
  private async replaceHistory(
    path: string,
    body: Record<string, unknown>,
    label: string,
  ): Promise<void> {
    if (
      this.options.viewer.role !== "player" ||
      !this.options.viewer.playerSession
    )
      throw new Error("A player session is required");
    const url = this.snapshotUrl().replace(/\/snapshot$/, path);
    let version = this.state.gameVersion;
    const cursor = this.state.history.cursor;
    let response!: Response;
    let conflictReason: string | undefined;
    for (let attempt = 0; attempt < HISTORY_RETRY_ATTEMPTS; attempt++) {
      conflictReason = undefined;
      response = await fetch(url, {
        method: "POST",
        headers: {
          ...this.snapshotHeaders(),
          "content-type": "application/json",
        },
        body: JSON.stringify({ ...body, expected_version: version }),
      });
      if (response.ok || response.status !== 409) break;
      conflictReason = await response.text();
      if (
        attempt === HISTORY_RETRY_ATTEMPTS - 1 ||
        !conflictReason.includes("Game advanced or a decision is in flight")
      )
        break;
      // The worker may still be advancing automatically toward its next human choice.
      // Refresh the version, but never rewind a different decision if someone acted meanwhile.
      await new Promise((resolve) => setTimeout(resolve, 100));
      const latest = await fetch(this.snapshotUrl(), {
        headers: this.snapshotHeaders(),
      });
      if (!latest.ok) break;
      const snapshot = decodeInitialSnapshot(
        await latest.json(),
        this.options.gameId,
      );
      if (snapshot.history?.cursor !== cursor) break;
      version = snapshot.game_version;
    }
    if (!response.ok) {
      const reason = conflictReason ?? (await response.text());
      const error = `${label} failed (${response.status}): ${reason}`;
      this.setState({ ...this.state, lastError: error });
      throw new Error(error);
    }
    const snapshot = decodeInitialSnapshot(
      await response.json(),
      this.options.gameId,
    );
    const expected = this.options.viewer;
    if (
      snapshot.viewer.role !== expected.role ||
      (expected.role === "player" &&
        (snapshot.viewer.role !== "player" ||
          snapshot.viewer.seat !== expected.seat))
    ) {
      throw new Error("Server viewer identity does not match this session");
    }
    this.rejectSubmission("Game history changed");
    this.detachSocket();
    this.clearTimers();
    this.setState(
      reduceServerMessage(
        { ...this.state, pendingChoice: null, lastError: null, batchResume: null },
        { ...snapshot, type: "initial_snapshot" },
      ),
    );
    this.openSocket();
  }

  private async loadSnapshot(): Promise<void> {
    try {
      const response = await fetch(this.snapshotUrl(), {
        headers: this.snapshotHeaders(),
      });
      if (!response.ok)
        throw new Error(
          `Snapshot request failed (${response.status}): ${await response.text().catch(() => "")}`,
        );
      this.ingestHttpSnapshot(await response.json());
    } catch (error) {
      if (!this.stopped)
        this.setState({
          ...this.state,
          lastError: `Snapshot request failed: ${String(error)}`,
        });
    }
  }

  private openSocket(): void {
    const socket = new WebSocket(this.webSocketUrl());
    this.socket = socket;
    socket.onopen = () => {
      if (this.stopped || this.socket !== socket) return;
      const playerSession =
        this.options.viewer.role === "player"
          ? this.options.viewer.playerSession
          : undefined;
      const message: ClientMessage = {
        type: "subscribe",
        protocol_version: PROTOCOL_VERSION,
        game_id: this.options.gameId,
        player_session: playerSession,
      };
      socket.send(JSON.stringify(message));
      if (playerSession)
        this.heartbeat = setInterval(() => {
          if (socket.readyState === WebSocket.OPEN)
            socket.send(
              JSON.stringify({
                type: "ping",
                protocol_version: PROTOCOL_VERSION,
                sequence: ++this.pingSequence,
              } satisfies ClientMessage),
            );
        }, 10_000);
      this.setState({ ...this.state, status: "connected", lastError: null });
    };
    socket.onmessage = (event) => this.ingestWebSocket(event.data);
    socket.onerror = () => {
      if (!this.stopped && this.socket === socket) {
        this.setState({
          ...this.state,
          status: "error",
          lastError: "WebSocket network error occurred",
        });
      }
    };
    socket.onclose = (event) => {
      if (!this.stopped && this.socket === socket) {
        this.clearTimers();
        this.socket = null;
        this.rejectSubmission("Submission disconnected before confirmation");
        this.settlePreviews(new Error("Disconnected"));
        this.setState({
          ...this.state,
          status: "disconnected",
          pendingChoice: null,
          snapshot: null,
          reactionIntent: null,
        });
        this.retry = setTimeout(
          () => {
            if (!this.stopped) {
              void this.loadSnapshot();
              this.openSocket();
            }
          },
          event?.code === 4001 ? 0 : 2_000,
        );
      }
    };
  }

  private ingestHttpSnapshot(value: unknown): void {
    this.apply(
      decodeInitialSnapshot(value, this.options.gameId) as Extract<
        ServerMessage,
        { type: "initial_snapshot" }
      >,
    );
  }

  private ingestWebSocket(value: unknown): void {
    try {
      const raw: unknown = typeof value === "string" ? JSON.parse(value) : value;
      // An older server does not know the preview message and answers it with a protocol error
      // that names it. That is "no preview available", never an error to show the player.
      if (
        this.previews.size > 0 &&
        typeof raw === "object" &&
        raw !== null &&
        (raw as { type?: unknown }).type === "error" &&
        String((raw as { message?: unknown }).message ?? "").includes("preview_secondary")
      ) {
        this.previewUnsupported = true;
        this.settlePreviews(new PreviewUnsupportedError());
        return;
      }
      const message = decodeServerMessage(raw, this.options.gameId);
      if (message.type === "secondary_preview") {
        const pending = this.previews.get(message.request_id);
        if (pending) {
          clearTimeout(pending.timer);
          this.previews.delete(message.request_id);
          pending.resolve(
            message.outcome.result === "preview"
              ? {
                  kind: "preview",
                  body: message.outcome.preview,
                  asOfVersion: message.as_of_version,
                  asOfDecisions: message.as_of_decisions,
                }
              : { kind: "refused", reason: message.outcome.reason, detail: message.outcome.detail },
          );
        }
        return;
      }
      this.apply(message);
      if (message.type === "reaction_intent_state") {
        // The server's answer is the truth (it may have refused, or forgotten in a restart).
        const viewer = this.options.viewer;
        if (viewer.role === "player")
          writeDeclaredTriggers(this.options.gameId, viewer.seat, message.triggers);
      }
      if (message.type === "initial_snapshot" && this.options.viewer.role === "player") {
        // Tell the server again what this browser declared; only when something is declared, so
        // a server that predates the feature never sees the message.
        const declared = readDeclaredTriggers(this.options.gameId, this.options.viewer.seat);
        if (declared.length > 0) this.sendIntent(declared);
      }
    } catch (error) {
      if (!this.stopped) {
        const message = `Invalid server message: ${String(error)}`;
        this.setState({ ...this.state, lastError: message });
        this.rejectSubmission(message);
      }
    }
  }

  private apply(message: ServerMessage): void {
    if (
      message.type === "initial_snapshot" ||
      message.type === "state_update"
    ) {
      const expected = this.options.viewer;
      if (
        message.viewer.role !== expected.role ||
        (expected.role === "player" &&
          (message.viewer.role !== "player" ||
            message.viewer.seat !== expected.seat))
      ) {
        this.setState({
          ...initialState,
          status: "error",
          lastError: "Server viewer identity does not match this session",
        });
        this.stop();
        return;
      }
    }
    // A state update may precede its acknowledgement on the broadcast channel.
    // Do not discard a late acknowledgement just because its version is older.
    if (message.type === "action_accepted") {
      const index = this.priorSubmissions.findIndex(
        (pending) =>
          pending.optionId === message.option_id &&
          message.game_version >= pending.version,
      );
      if (index !== -1) {
        this.priorSubmissions.splice(index, 1)[0].resolve();
        this.setState(reduceServerMessage(this.state, message));
        return;
      }
    }
    if (message.type === "action_rejected") {
      const index = this.priorSubmissions.findIndex(
        (pending) => pending.version === message.game_version,
      );
      if (index !== -1) {
        const reason = rejectionMessage(message);
        this.priorSubmissions.splice(index, 1)[0].reject(new Error(reason));
        this.setState({ ...this.state, lastError: reason });
        return;
      }
    }
    if (
      message.type === "action_accepted" &&
      this.submission &&
      message.option_id === this.submission.optionId &&
      message.game_version >= this.submission.version
    ) {
      this.submission.accepted = true;
    }
    if (message.type === "action_rejected" && this.submission) {
      const reason = rejectionMessage(message);
      this.setState({ ...this.state, lastError: reason });
      this.rejectSubmission(reason);
      return;
    }
    this.setState(reduceServerMessage(this.state, message));
    const submission = this.submission;
    // The worker can announce the next choice at a newer version without sending
    // a state update at that version. That choice is itself confirmation of progress.
    if (
      submission?.accepted &&
      this.state.gameVersion > submission.version &&
      (this.state.snapshot?.game_version === this.state.gameVersion ||
        this.state.pendingChoice !== null) &&
      this.state.pendingChoice?.nonce !== submission.nonce
    ) {
      this.submission = null;
      submission.resolve();
    }
  }

  private rejectSubmission(reason: string): void {
    const submission = this.submission;
    this.submission = null;
    submission?.reject(new Error(reason));
    for (const prior of this.priorSubmissions.splice(0))
      prior.reject(new Error(reason));
  }

  private detachSocket(): void {
    const socket = this.socket;
    this.socket = null;
    if (!socket) return;
    socket.onopen = null;
    socket.onmessage = null;
    socket.onerror = null;
    socket.onclose = null;
    if (
      socket.readyState === WebSocket.CONNECTING ||
      socket.readyState === WebSocket.OPEN
    )
      socket.close();
  }

  private clearTimers(): void {
    if (this.heartbeat) clearInterval(this.heartbeat);
    if (this.retry) clearTimeout(this.retry);
    this.heartbeat = null;
    this.retry = null;
  }

  private setState(next: GameSessionState): void {
    this.state = settleBatchResume(next);
    this.listeners.forEach((listener) => listener());
  }

  private webSocketUrl(): string {
    if (this.options.serverUrl) return this.options.serverUrl;
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    return `${protocol}//${window.location.host}/ws/games/${encodeURIComponent(this.options.gameId)}`;
  }

  private snapshotUrl(): string {
    if (!this.options.serverUrl)
      return `/api/games/${encodeURIComponent(this.options.gameId)}/snapshot`;
    const url = new URL(this.options.serverUrl, window.location.href);
    url.protocol = url.protocol === "wss:" ? "https:" : "http:";
    url.pathname = `/api/games/${encodeURIComponent(this.options.gameId)}/snapshot`;
    url.search = "";
    return url.toString();
  }

  private snapshotHeaders(): HeadersInit {
    return this.options.viewer.role === "player" &&
      this.options.viewer.playerSession
      ? { "x-ti4-player-session": this.options.viewer.playerSession }
      : {};
  }
}
