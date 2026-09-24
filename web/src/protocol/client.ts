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
} from "./types.ts";
import { decodeInitialSnapshot, decodeServerMessage, isStaleServerMessage } from "./decode.ts";

export type ConnectionStatus = "connecting" | "connected" | "disconnected" | "error";
export type SnapshotState = InitialSnapshotMsg | StateUpdateMsg;
export type GameLogEntry = import("./types.ts").GameEvent;

const MAX_EVENT_LOG_ENTRIES = 500;

export interface GameSessionState {
  status: ConnectionStatus;
  gameVersion: number;
  snapshot: SnapshotState | null;
  pendingChoice: PendingChoiceDto | null;
  turnStatus: PublicTurnStatus | null;
  lastError: string | null;
  events: GameLogEntry[];
  history: HistoryStatus;
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

/** Keeps the rendered audit log server-authored while bounding client memory use. */
export function serverEventLog(entries: readonly GameLogEntry[] | undefined): GameLogEntry[] {
  return (entries ?? []).slice(-MAX_EVENT_LOG_ENTRIES);
}

function rejectionMessage(message: Extract<ServerMessage, { type: "action_rejected" }>): string {
  switch (message.reason.reason) {
    case "stale_version":
      return `Rejected: Stale version (expected ${message.reason.expected}, server at ${message.reason.current})`;
    case "stale_nonce":
      return "Rejected: Stale decision nonce";
    case "unauthorized_seat":
      return "Rejected: Unauthorized seat";
    case "unknown_option":
      return `Rejected: Unknown option '${message.reason.option_id}'`;
    default:
      return `Action rejected: ${JSON.stringify(message.reason)}`;
  }
}

function pendingChoice(envelope: import("./types.ts").PendingChoiceEnvelope): PendingChoiceDto {
  if (!envelope.choice) return envelope as unknown as PendingChoiceDto;
  return {
    nonce: envelope.nonce,
    actor: envelope.choice.player,
    prompt: envelope.choice.prompt,
    options: envelope.choice.options,
    context: envelope.choice.context,
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
        pendingChoice: message.pending_choice ? pendingChoice(message.pending_choice) : null,
        events: message.type === "initial_snapshot" ? serverEventLog(message.events) : state.events,
        history: message.history ?? state.history,
      };
    case "event":
      if (state.events.some((entry) => entry.id === message.entry.id)) return state;
      return {
        ...state,
        events: serverEventLog([...state.events, message.entry]),
        history:
          message.entry.decision_count === undefined
            ? state.history
            : {
                ...state.history,
                cursor: Math.max(state.history.cursor, message.entry.decision_count),
                redo_count: 0,
              },
      };
    case "pending_choice":
      return {
        ...state,
        gameVersion: message.game_version,
        pendingChoice: pendingChoice({ nonce: message.nonce, choice: message.choice }),
      };
    case "turn_status":
      return { ...state, gameVersion: message.game_version, turnStatus: message.status };
    case "action_accepted":
      return { ...state, lastError: null };
    case "action_rejected":
      return { ...state, lastError: rejectionMessage(message) };
    case "error":
      return { ...state, lastError: `Server Error: ${message.message}` };
    case "game_over":
    case "pong":
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
  private heartbeat: ReturnType<typeof setInterval> | null = null;
  private retry: ReturnType<typeof setTimeout> | null = null;
  private pingSequence = 0;

  constructor(private readonly options: GameSessionClientOptions) {}

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
    this.clearTimers();
    this.detachSocket();
    this.rejectSubmission("Submission stopped");
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
      if (this.submission.nonce === pendingChoice.nonce && this.submission.optionId === optionId)
        return this.submission.promise;
      throw new Error("Another choice submission is still pending");
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
    const promise = new Promise<void>((done, fail) => {
      resolve = done;
      reject = fail;
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
    return promise;
  }

  /** The host changes the authoritative Rust timeline; all clients reconnect to it. */
  async changeHistory(action: "undo" | "redo" | { eventId: string }): Promise<void> {
    if (this.options.viewer.role !== "player" || !this.options.viewer.playerSession)
      throw new Error("A player session is required");
    const url = this.snapshotUrl().replace(/\/snapshot$/, "/history");
    const body =
      typeof action === "string" ? { action } : { action: "restore", event_id: action.eventId };
    const response = await fetch(url, {
      method: "POST",
      headers: { ...this.snapshotHeaders(), "content-type": "application/json" },
      body: JSON.stringify({ ...body, expected_version: this.state.gameVersion }),
    });
    if (!response.ok) {
      const reason = await response.text();
      const error = `History change failed (${response.status}): ${reason}`;
      this.setState({ ...this.state, lastError: error });
      throw new Error(error);
    }
    const snapshot = decodeInitialSnapshot(await response.json(), this.options.gameId);
    const expected = this.options.viewer;
    if (
      snapshot.viewer.role !== expected.role ||
      (expected.role === "player" &&
        (snapshot.viewer.role !== "player" || snapshot.viewer.seat !== expected.seat))
    ) {
      throw new Error("Server viewer identity does not match this session");
    }
    this.rejectSubmission("Game history changed");
    this.detachSocket();
    this.clearTimers();
    this.setState(
      reduceServerMessage(
        { ...this.state, pendingChoice: null, lastError: null },
        { ...snapshot, type: "initial_snapshot" },
      ),
    );
    this.openSocket();
  }

  private async loadSnapshot(): Promise<void> {
    try {
      const response = await fetch(this.snapshotUrl(), { headers: this.snapshotHeaders() });
      if (!response.ok) throw new Error(`Snapshot request failed (${response.status})`);
      this.ingestHttpSnapshot(await response.json());
    } catch (error) {
      if (!this.stopped)
        this.setState({ ...this.state, lastError: `Snapshot request failed: ${String(error)}` });
    }
  }

  private openSocket(): void {
    const socket = new WebSocket(this.webSocketUrl());
    this.socket = socket;
    socket.onopen = () => {
      if (this.stopped || this.socket !== socket) return;
      const playerSession =
        this.options.viewer.role === "player" ? this.options.viewer.playerSession : undefined;
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
    socket.onclose = () => {
      if (!this.stopped && this.socket === socket) {
        this.clearTimers();
        this.socket = null;
        this.rejectSubmission("Submission disconnected before confirmation");
        this.setState({
          ...this.state,
          status: "disconnected",
          pendingChoice: null,
          snapshot: null,
        });
        this.retry = setTimeout(() => {
          if (!this.stopped) {
            void this.loadSnapshot();
            this.openSocket();
          }
        }, 2_000);
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
      this.apply(
        decodeServerMessage(
          typeof value === "string" ? JSON.parse(value) : value,
          this.options.gameId,
        ),
      );
    } catch (error) {
      if (!this.stopped) {
        const message = `Invalid server message: ${String(error)}`;
        this.setState({ ...this.state, lastError: message });
        this.rejectSubmission(message);
      }
    }
  }

  private apply(message: ServerMessage): void {
    if (message.type === "initial_snapshot" || message.type === "state_update") {
      const expected = this.options.viewer;
      if (
        message.viewer.role !== expected.role ||
        (expected.role === "player" &&
          (message.viewer.role !== "player" || message.viewer.seat !== expected.seat))
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
    if (
      submission?.accepted &&
      this.state.gameVersion > submission.version &&
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
  }

  private detachSocket(): void {
    const socket = this.socket;
    this.socket = null;
    if (!socket) return;
    socket.onopen = null;
    socket.onmessage = null;
    socket.onerror = null;
    socket.onclose = null;
    if (socket.readyState === WebSocket.CONNECTING || socket.readyState === WebSocket.OPEN)
      socket.close();
  }

  private clearTimers(): void {
    if (this.heartbeat) clearInterval(this.heartbeat);
    if (this.retry) clearTimeout(this.retry);
    this.heartbeat = null;
    this.retry = null;
  }

  private setState(next: GameSessionState): void {
    this.state = next;
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
    return this.options.viewer.role === "player" && this.options.viewer.playerSession
      ? { "x-ti4-player-session": this.options.viewer.playerSession }
      : {};
  }
}
