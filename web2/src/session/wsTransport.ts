// The game on the server: `crates/ti4-server`, behind a websocket at the address of the page.
import type { PlanStopped, Transport, TransportEvent } from "./transport";
import {
  type Catalog,
  type ClientRequest,
  type DraftStep,
  PROTOCOL_VERSION,
  type RejectionReason,
  type ServerMessage,
  type SessionUpdate,
  type StateMsg,
} from "./wire";

/** Which game of the server, and as whom. */
export interface OnlineGame {
  gameId: string;
  /** What the server gave the player when the game was made or joined. Null: a spectator. */
  playerSession: string | null;
}

/** The part of `WebSocket` that is used, so a test can pass a socket of its own. */
export interface Socket {
  send(data: string): void;
  close(): void;
  onopen: (() => void) | null;
  onmessage: ((event: { data: unknown }) => void) | null;
  onclose: ((event: { code?: number }) => void) | null;
}

export interface OnlinePlay {
  /** Opens the socket of a game. Without it: `/ws/games/<id>` at the address of the page. */
  open?(gameId: string): Socket;
  /** How long to wait before try `attempt` (from 0) to reach the game again, in ms. */
  retryAfter?(attempt: number): number;
}

/** Where the staged work of a game on the server is kept between two visits, and its player. */
export const onlineKeys = (gameId: string) => ({
  /** The player session of this page in the game. */
  session: `ti4.online.${gameId}.session`,
  /** The staged movement, landings or production, with the nonce of its choice. */
  draft: `ti4.online.${gameId}.movement`,
  /** The private draft of a tactical action. */
  plan: `ti4.online.${gameId}.draft`,
});

const HEARTBEAT_MS = 10_000;
/** The server closes a socket with this code when the game was replaced (a restore of the host). */
const REPLACED = 4001;
/** After a stale answer the update that explains it is on its way. Without it, the game is asked again. */
const RESYNC_MS = 1_000;

/** A websocket at `url`, as the transport uses one. */
export function openSocket(url: string): Socket {
  const real = new WebSocket(url);
  const socket: Socket = {
    send: (data) => real.send(data),
    close: () => real.close(),
    onopen: null,
    onmessage: null,
    onclose: null,
  };
  real.onopen = () => socket.onopen?.();
  real.onmessage = (event) => socket.onmessage?.(event);
  real.onclose = (event) => socket.onclose?.(event);
  return socket;
}

/** The server is at the address of the page, as the page itself. */
const pageSocket = (gameId: string): Socket =>
  openSocket(
    `${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/ws/games/${encodeURIComponent(gameId)}`,
  );

const backoff = (attempt: number) => Math.min(15_000, 500 * 2 ** attempt);

const short = (digest: string | undefined) => (digest ? digest.slice(0, 12) : "none");

function rejection(reason: RejectionReason): string {
  switch (reason.reason) {
    case "stale_version":
    case "stale_nonce":
      return "The game went on before your answer arrived. Nothing was sent.";
    case "no_pending_choice":
      return "The game does not wait for this answer. Nothing was sent.";
    case "unauthorized_seat":
      return "The server does not take this answer from you. Nothing was sent.";
    case "unknown_option":
      return `The game does not offer "${reason.option_id}". Nothing was sent.`;
    case "validation_failed":
      return reason.message;
  }
}

/** The part of a message of the server that is the update. `state` is large and not read. */
const updateOf = (message: StateMsg): SessionUpdate => ({
  viewer: message.viewer,
  view: message.view,
  pending_choice: message.pending_choice,
  turn_status: message.turn_status,
  tactical: message.tactical,
  exploration: message.exploration,
  action_cards: message.action_cards,
  secret_objectives: message.secret_objectives,
  score: message.score,
  agendas: message.agendas,
});

/**
 * Plays one game of the server. The first update is the snapshot that the server sends after
 * the subscription. `catalog` is the one of the engine of this page (`Engine.catalog`): the
 * server sends ids, and the game is played only when both have the same corpus.
 */
export function createWsTransport(
  engine: { catalog: Catalog | null },
  game: OnlineGame,
  play: OnlinePlay = {},
): Transport {
  type Update = Extract<TransportEvent, { kind: "update" }>;
  const listeners = new Set<(event: TransportEvent) => void>();
  const open = play.open ?? pageSocket;
  const retryAfter = play.retryAfter ?? backoff;
  /** The game as the server told it last, and its version. */
  let latest: (Update & { version: number }) | null = null;
  /** Why the game cannot be played. Nothing is tried after it. */
  let failed: string | null = null;
  let offline = false;
  let closed = false;
  let never: string[] = [];
  let socket: Socket | null = null;
  /** The socket is open and the server was asked for the game. */
  let ready = false;
  let attempt = 0;
  let retry: ReturnType<typeof setTimeout> | null = null;
  let heartbeat: ReturnType<typeof setInterval> | null = null;
  let resync: ReturnType<typeof setTimeout> | null = null;
  let sequence = 0;
  /** The newest draft that was asked. An answer for another request is late. */
  let asked: { id: string; script: DraftStep[] } | null = null;
  let requests = 0;
  /** The script that is being sent: its result is counted in entries, a plan in steps. */
  let sent: { nonce: string; destination: string; lead: number } | null = null;
  /** Told with the next update: where the last plan stopped. */
  let stopped: PlanStopped | null = null;
  /** The notes of the server that were told. A note has an id so that a repeat can be dropped. */
  const noted = new Set<string>();

  const say = (event: TransportEvent) => {
    for (const listener of listeners) {
      listener(event);
    }
  };
  const send = (message: ClientRequest) => {
    if (!socket || !ready) {
      return false;
    }
    socket.send(JSON.stringify({ protocol_version: PROTOCOL_VERSION, ...message }));
    return true;
  };
  const stop = () => {
    for (const timer of [retry, resync]) {
      if (timer !== null) {
        clearTimeout(timer);
      }
    }
    if (heartbeat !== null) {
      clearInterval(heartbeat);
    }
    retry = resync = heartbeat = null;
    ready = false;
    const old = socket;
    socket = null;
    if (old) {
      old.onopen = old.onmessage = old.onclose = null;
      old.close();
    }
  };
  /** The game cannot be played from here on. */
  const fail = (message: string) => {
    failed = message;
    stop();
    say({ kind: "error", message });
  };
  /** Tells the game as it is now, once more: what was staged against it starts again. */
  const again = (plan?: PlanStopped) => {
    if (latest) {
      const { version: _, ...event } = latest;
      say(plan ? { ...event, plan } : event);
    }
  };

  const onState = (message: StateMsg, snapshot: boolean) => {
    if (snapshot) {
      const here = engine.catalog?.content;
      if (engine.catalog && here !== message.content) {
        return fail(
          `The server plays with other cards than this page (${short(message.content)} there, ${short(here)} here). Reload the page; if that does not help, the page and the server are of different builds.`,
        );
      }
      attempt = 0;
    }
    if (resync !== null) {
      clearTimeout(resync);
      resync = null;
    }
    const modes = message.reaction_modes ?? {};
    const cards = Object.keys(modes)
      .filter((card) => modes[card] === "never")
      .sort();
    if (cards.join("\n") !== never.join("\n")) {
      never = cards;
      say({ kind: "cards", never: [...never] });
    }
    for (const note of message.auto_resolved ?? []) {
      if (note.passed?.length && !noted.has(note.id)) {
        noted.add(note.id);
        say({ kind: "passed", cards: note.passed });
      }
    }
    const update = updateOf(message);
    // The server tells the game again when only a card setting changed. The page then keeps
    // what is staged: nothing of the game is new.
    const same =
      !snapshot &&
      !offline &&
      !stopped &&
      latest?.version === message.game_version &&
      latest.update.pending_choice?.nonce === update.pending_choice?.nonce;
    // The undo of a seat is not on the server yet (phase 3 of the plan).
    latest = { kind: "update", update, canUndo: false, version: message.game_version };
    offline = false;
    if (!same) {
      const plan = stopped ?? undefined;
      stopped = null;
      again(plan);
    }
  };

  const receive = (text: string) => {
    let message: ServerMessage;
    try {
      message = JSON.parse(text);
    } catch {
      return fail("The server sent a message that this page cannot read.");
    }
    if (message.protocol_version !== PROTOCOL_VERSION) {
      return fail(
        `The server speaks protocol ${message.protocol_version} and this page protocol ${PROTOCOL_VERSION}. Reload the page.`,
      );
    }
    switch (message.type) {
      case "initial_snapshot":
        return onState(message, true);
      case "state_update":
        return onState(message, false);
      case "action_rejected": {
        sent = null;
        say({ kind: "rejected", reason: rejection(message.reason) });
        again();
        const { reason } = message.reason;
        if (
          reason === "stale_version" ||
          reason === "stale_nonce" ||
          reason === "no_pending_choice"
        ) {
          // This side is behind the game. The update that says so is usually on its way; when
          // none comes, the server is asked for the game again.
          resync ??= setTimeout(() => {
            resync = null;
            stop();
            connect();
          }, RESYNC_MS);
        }
        return;
      }
      case "error":
        // Before the first update the server refused the subscription: there is no game to show.
        if (!latest) {
          return fail(
            message.kind === "unauthorized"
              ? "The server does not know the player of this page in this game."
              : `The server refused the game: ${message.message}`,
          );
        }
        return say({ kind: "rejected", reason: message.message });
      case "draft_outcome":
        if (asked?.id === message.request_id) {
          const { script } = asked;
          asked = null;
          say({ kind: "draft", script, outcome: message.outcome });
        }
        return;
      case "draft_applied": {
        if (sent?.nonce !== message.nonce) {
          return;
        }
        const { destination, lead } = sent;
        sent = null;
        if (!message.stopped) {
          return;
        }
        // The server counts in entries of the script; a plan in the steps after the answers by id.
        const { applied, remaining, interrupted, reason } = message.stopped;
        const plan: PlanStopped = {
          destination,
          applied: Math.max(0, applied - lead),
          remaining: remaining.slice(Math.max(0, lead - applied)),
          interrupted,
          reason,
        };
        if (interrupted) {
          // The game went on: the update of where it is now comes next.
          stopped = plan;
        } else {
          // Nothing was sent, so the server has no update: the same choice is open.
          again(plan);
        }
        return;
      }
      default:
        return;
    }
  };

  const connect = () => {
    if (closed || failed !== null) {
      return;
    }
    const made = open(game.gameId);
    socket = made;
    made.onopen = () => {
      if (socket !== made) {
        return;
      }
      ready = true;
      send({
        type: "subscribe",
        game_id: game.gameId,
        ...(game.playerSession === null ? {} : { player_session: game.playerSession }),
      });
      // The server counts a player as present while its connection pings.
      heartbeat = setInterval(() => send({ type: "ping", sequence: ++sequence }), HEARTBEAT_MS);
    };
    made.onmessage = (event) => {
      if (socket === made) {
        receive(String(event.data));
      }
    };
    made.onclose = (event) => {
      if (socket !== made) {
        return;
      }
      stop();
      // What was on its way is lost. The snapshot after the next subscription says where the game is.
      sent = null;
      asked = null;
      if (!offline) {
        offline = true;
        say({ kind: "offline" });
      }
      retry = setTimeout(
        () => {
          retry = null;
          connect();
        },
        event.code === REPLACED ? 0 : retryAfter(attempt++),
      );
    };
  };
  connect();

  const isPlayer = () => latest?.update.viewer.role === "player";

  return {
    catalog: engine.catalog,
    // The tools of a local game are not on the server.
    capabilities: {},
    subscribe(listener) {
      listeners.add(listener);
      listener({ kind: "cards", never: [...never] });
      if (failed !== null) {
        listener({ kind: "error", message: failed });
      } else {
        if (latest) {
          const { version: _, ...event } = latest;
          listener(event);
        }
        if (offline) {
          listener({ kind: "offline" });
        }
      }
      return () => listeners.delete(listener);
    },
    submitChoice(nonce, optionId) {
      const pending = latest?.update.pending_choice;
      if (!latest || pending?.nonce !== nonce) {
        return;
      }
      send({
        type: "submit_choice",
        game_id: game.gameId,
        nonce,
        expected_version: latest.version,
        option_id: optionId,
      });
    },
    submitPlan(nonce, plan) {
      this.applyDraft(nonce, { choices: [], plan });
    },
    applyDraft(nonce, draft) {
      if (!latest || latest.update.pending_choice?.nonce !== nonce || !isPlayer()) {
        return;
      }
      const script: DraftStep[] = [
        ...draft.choices.map((option_id): DraftStep => ({ kind: "choose", option_id })),
        ...(draft.plan?.steps ?? []),
      ];
      const asks = send({
        type: "apply_draft",
        game_id: game.gameId,
        nonce,
        expected_version: latest.version,
        script,
      });
      if (asks) {
        sent = { nonce, destination: draft.plan?.destination ?? "", lead: draft.choices.length };
      }
    },
    draft(script) {
      if (latest && !isPlayer()) {
        // A spectator has no turn to draft. Later, not now, as every answer.
        queueMicrotask(() => say({ kind: "draft", script, outcome: null }));
        return;
      }
      // While the game cannot be reached nothing is asked: the page asks again with the next update.
      const id = `${++requests}`;
      if (send({ type: "run_draft", game_id: game.gameId, request_id: id, script })) {
        asked = { id, script };
      }
    },
    setReactionMode(card, mode) {
      if (never.includes(card) !== (mode === "never")) {
        // The server tells the change with the game (`reaction_modes`): a `cards` event follows.
        send({ type: "set_reaction_mode", game_id: game.gameId, card, mode });
      }
    },
    undo() {
      // `canUndo` is never true: the undo of a seat is phase 3 of the plan.
    },
    close() {
      closed = true;
      stop();
      listeners.clear();
    },
  };
}
