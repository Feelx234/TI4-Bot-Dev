// The engine in this page: `crates/ti4-wasm`, built by `scripts/build-wasm.sh`. A game speaks
// the protocol of the server through the hosted exports (`ti4_host_start`, `ti4_host_command`):
// `ClientMessage` in, `ServerMessage` out, read as `wsTransport.ts` reads them
// (`serverMessages.ts`). The questions that need no game are exports of their own. The engine
// keeps the staged work of each seat in its saved game, as the server keeps it for the seat
// (`put_staged`, `seat_staged`: `seatSync`), and sends a ready draft of a secondary itself.
import { sha256Hex } from "@ti4/shared/engine/identity.ts";
import type { LocalGame, SeatSwitch } from "./savedGame";
import { seatSync } from "./seatStore";
import { modesOf, passedOf, planStoppedOf, rejection, updateOf } from "./serverMessages";
import type { Capabilities, PlanStopped, Transport, TransportEvent } from "./transport";
import {
  type Catalog,
  type DraftScope,
  type DraftStep,
  PROTOCOL_VERSION,
  type ReactionModes,
  type ServerMessage,
  type SpaceOddsRequest,
  type SpaceOddsResponse,
  type StateMsg,
  type TableDraw,
  type TableOptions,
  type TablePreview,
  type TableRequest,
} from "./wire";

interface Exports {
  memory: WebAssembly.Memory;
  // The questions that need no game: the request buffer in, the response buffer out.
  ti4_setup_options(players: number): number;
  ti4_setup_preview(length: number): number;
  ti4_setup_draw(length: number): number;
  ti4_scenarios(): number;
  ti4_catalog(): number;
  ti4_space_odds(length: number): number;
  ti4_request_ptr(): number;
  ti4_response_ptr(): number;
  // The game (`crates/ti4-wasm/src/host_api.rs`).
  ti4_host_in(length: number): number;
  ti4_host_out_ptr(): number;
  ti4_host_out_len(): number;
  ti4_host_set_time(unixMs: number, monotonicMs: number): void;
  ti4_host_start(length: number): number;
  ti4_host_command(length: number): number;
}

/**
 * What the engine keeps of a game to play it again (`HostSession::save_state`): the option id
 * of every decision of every seat, in order (`log`), the answers of the page among them, and the
 * seat changes. The rest (the redo list, the times of the events, the hash of the decisions) is
 * given back as it was.
 */
export interface HostSave {
  log: string[];
  answers: string[];
  switches?: SeatSwitch[];
  [field: string]: unknown;
}

/** A change of the history that the engine plays again from the seed (`Restart`). */
type Restart = Record<string, unknown> & { log?: string[]; answers?: string[] };

/** What the engine writes after a call (`Envelope` of `host_api.rs`). */
interface Envelope {
  kind: "ask" | "progress" | "end" | "command";
  messages: ServerMessage[];
  progress?: { done: number; total: number };
  /** An `ask` with no choice of the page: the game waits before a decision of this seat. */
  waiting?: string;
  /** The parked game goes on with this answer. */
  resume?: number;
  /** The game is played again: the parked run leaves, and a new one starts with this. */
  restart?: Restart;
  save?: HostSave;
  end?: { finished: boolean; left: boolean; error: string | null; diverged?: string | null };
}

// JSPI: the engine asks for a decision with a blocking call. `Suspending` parks its stack until
// the promise of the answer settles, and `promising` makes the export that runs the game async.
interface Jspi {
  Suspending: new (ask: () => Promise<number>) => WebAssembly.ImportValue;
  promising<Args extends number[]>(
    run: (...args: Args) => number,
  ): (...args: Args) => Promise<number>;
}

/** Leaves the game: the engine unwinds and `ti4_host_start` returns. */
const LEAVE = -1;
/** The answer to a `progress` envelope: go on. */
const CONTINUE = -5;
/** The id of every game in this page. */
const GAME_ID = "local";

/** A replay gives the page a turn this often, so it can show how far the replay is. */
const REPLAY_SLICE_MS = 50;

/** A prepared board that the engine can start a game from (`ti4_host::view::scenario`). */
export interface ScenarioInfo {
  id: string;
  title: string;
  description: string;
  seed: number;
  players: number;
}

/** The engine file, compiled once. Each game, and each undo that plays it again, makes an instance of it. */
export interface Engine {
  module: WebAssembly.Module;
  /** SHA-256 of the file, in hex. A saved game names the engine that played it. */
  hash: string;
  scenarios: ScenarioInfo[];
  /** The printed cards. Null for an engine that was not asked. */
  catalog: Catalog | null;
}

/**
 * Asks the engine a question that needs no game: the instance is asked nothing. With `request`
 * the question reads it from the request buffer, and is given its length.
 */
async function ask<T>(
  module: WebAssembly.Module,
  question: (exports: Exports, length: number) => number,
  request?: unknown,
) {
  const instance = await WebAssembly.instantiate(module, { host: { ask: () => LEAVE } });
  return askOn<T>(instance.exports as unknown as Exports, question, request);
}

/** Asks an instance that plays no game. */
function askOn<T>(
  exports: Exports,
  question: (exports: Exports, length: number) => number,
  request?: unknown,
) {
  const bytes = new TextEncoder().encode(request === undefined ? "" : JSON.stringify(request));
  new Uint8Array(exports.memory.buffer).set(bytes, exports.ti4_request_ptr());
  const status = question(exports, bytes.length);
  const text = new TextDecoder().decode(
    new Uint8Array(exports.memory.buffer, exports.ti4_response_ptr(), Math.abs(status)),
  );
  if (status < 0) {
    throw new Error(text);
  }
  return JSON.parse(text) as T;
}

/** The instance of a module that answers the questions that come often. It plays no game. */
const kept = new WeakMap<WebAssembly.Module, Promise<Exports>>();

function keptOf(module: WebAssembly.Module) {
  let exports = kept.get(module);
  if (!exports) {
    exports = WebAssembly.instantiate(module, { host: { ask: () => LEAVE } }).then(
      (instance) => instance.exports as unknown as Exports,
    );
    kept.set(module, exports);
  }
  return exports;
}

/**
 * The odds of a space combat (`ti4_space_odds`). It is asked for each change of a staged
 * movement, so one instance of the module answers all of them.
 */
export async function spaceOddsOf(module: WebAssembly.Module, request: SpaceOddsRequest) {
  const exports = await keptOf(module);
  try {
    return askOn<SpaceOddsResponse>(exports, (on, length) => on.ti4_space_odds(length), request);
  } catch (error) {
    // A trap may leave the instance broken: the next question makes a new one.
    if (error instanceof WebAssembly.RuntimeError) {
      kept.delete(module);
    }
    throw error;
  }
}

/** Asks the engine for its scenarios. */
export const scenariosOf = (module: WebAssembly.Module) =>
  ask<ScenarioInfo[]>(module, (exports) => exports.ti4_scenarios());

/** Asks the engine for the printed cards. */
export const catalogOf = (module: WebAssembly.Module) =>
  ask<Catalog>(module, (exports) => exports.ti4_catalog());

/**
 * What a host can choose for the table of the page, with the maps that build for `players`
 * seats (0: every map). The same options as `GET /api/setup-options` of the server.
 */
export const setupOptionsOf = (module: WebAssembly.Module, players: number) =>
  ask<TableOptions>(module, (exports) => exports.ti4_setup_options(players));

/** The board of a table before it starts, with a place for each seat. */
export const setupPreviewOf = (module: WebAssembly.Module, table: TableRequest) =>
  ask<TablePreview>(module, (exports, length) => exports.ti4_setup_preview(length), table);

/** What the start draws for a table: who sits where, and the faction of every seat. */
export const setupDrawOf = (module: WebAssembly.Module, table: TableRequest) =>
  ask<TableDraw>(module, (exports, length) => exports.ti4_setup_draw(length), table);

/** Fetches and compiles `ti4.wasm` from `wasmUrl`. */
export async function loadEngine(wasmUrl: string): Promise<Engine> {
  const jspi = WebAssembly as unknown as Partial<Jspi>;
  if (!jspi.Suspending || !jspi.promising) {
    throw new Error(
      "This browser has no WebAssembly JSPI, which local play needs. Chrome 137 or later has it.",
    );
  }
  const response = await fetch(wasmUrl);
  if (!response.ok) {
    throw new Error(
      `The engine was not found (${response.status} for ${wasmUrl}). Build it with web2/scripts/build-wasm.sh.`,
    );
  }
  const bytes = await response.arrayBuffer();
  const module = await WebAssembly.compile(bytes);
  return {
    module,
    hash: await sha256Hex(bytes),
    scenarios: await scenariosOf(module),
    catalog: await catalogOf(module),
  };
}

export interface LocalPlay {
  /** The game of an earlier visit (`HostSave`). It is played again before the first update. */
  save?: HostSave;
  /**
   * Told what plays the game again, to keep it: after each update of the game and at its end.
   * The engine saves it, as the server keeps a game: no seat decides a saved decision again.
   */
  onSave?(save: HostSave): void;
  /** The game waits before each decision of a seat that is not played here. */
  stepping?: boolean;
  /** The mode of each reaction that the seats of this page set, by its key. */
  modes?: Readonly<ReactionModes>;
  /** Told all modes after each change, to keep them. */
  onModes?(modes: ReactionModes): void;
  /**
   * The seats that the rule bot plays where this page does not, as `LocalGame.humans`. The
   * other seats decide at random. Not read with `LocalGame.table`, which names the kind of each
   * player.
   */
  ruleSeats?: number;
  /**
   * The wait of a table, in milliseconds: a seat that is not played here takes about this long
   * before each answer, and the page is shown the game while it waits. It does not wait to
   * decline a reaction window, as a seat on the server does not. The game is the same game with
   * every wait, and a game that is played again does not wait for its saved decisions.
   */
  waitMs?: number;
}

/** A turn of the event loop. A timer would be slowed down in a tab that is not shown. */
const breathe = () =>
  new Promise<void>((resume) => {
    const channel = new MessageChannel();
    channel.port1.onmessage = () => resume();
    channel.port2.postMessage(null);
  });

/** One run of the game: an instance of the engine, from its start to its end. An undo that plays the game again is a new run. */
interface Run {
  id: number;
  exports: Exports | null;
  /** The game waits for the page: this answers it. */
  parked: ((code: number) => void) | null;
  /** The game waits before a decision of this seat, which the page does not play. */
  waiting: string | null;
  /** The game is over, or was left: a command still reaches its session. */
  ended: boolean;
}

/**
 * Plays one game. The first update comes when the saved game is played again and the game asks
 * a seat of this page, or waits before a decision of another seat.
 */
export function createWasmTransport(
  engine: Engine,
  game: LocalGame,
  play: LocalPlay = {},
): Transport {
  type Update = Extract<TransportEvent, { kind: "update" }>;
  const listeners = new Set<(event: TransportEvent) => void>();
  /** The game as the engine told it last, and its version. */
  let latest: (Update & { version: number }) | null = null;
  let failed: string | null = null;
  /** The modes that the seats of this page set. The engine of each run is told. */
  const modes: ReactionModes = { ...play.modes };
  let stepping = play.stepping ?? false;
  let waitMs = play.waitMs ?? 0;
  /** What the engine is told: it asks before each decision of another seat, or before each that takes the wait, or not at all. */
  const pauses = () => (stepping ? 1 : waitMs > 0 ? 2 : 0);
  /** The run that is the game. */
  let run: Run | null = null;
  let runs = 0;
  /** The wait of the seat that the game waits for. */
  let timer: ReturnType<typeof setTimeout> | null = null;
  /** The script that is being sent: its result is counted in entries, a plan in steps. */
  let sent: { nonce: string; destination: string; lead: number } | null = null;
  /** Told with the next update: where the last plan stopped. */
  let stopped: PlanStopped | null = null;
  /** The newest draft that was asked. */
  let asked: { id: string; script: DraftStep[]; scope?: DraftScope } | null = null;
  let requests = 0;
  /** The game is played again from the seed when the run that is parked has left. */
  let restart: Restart | null = null;
  /** The notes of the engine that were told. */
  const noted = new Set<string>();
  /** The reactions whose mode changed while the game did not wait: the engine is told at the next question. */
  const unsent = new Set<string>();
  /** Why the engine did not play the draft that is asked. */
  let refusal: string | null = null;
  /**
   * The staged work of the seat that the page is shown as, kept by the engine. A write reaches
   * the engine while it waits for the page (or after the end), and the save is kept after it.
   */
  const staged = seatSync((slot, base, value) => {
    const on = run;
    if (!on?.exports || !(on.parked || on.ended)) {
      return false;
    }
    send(on, {
      type: "put_staged",
      slot,
      base_revision: base,
      ...(value === null ? {} : { value }),
    });
    keep(on);
    return true;
  });

  const say = (event: TransportEvent) => {
    for (const listener of listeners) {
      listener(event);
    }
  };
  const fail = (message: string) => {
    failed = message;
    say({ kind: "error", message });
  };
  const settle = () => {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
  };
  /** Tells the game as it is now, once more: what was staged against it starts again. */
  const again = (plan?: PlanStopped, undone?: Update["undone"]) => {
    if (latest) {
      const { version: _, ...event } = latest;
      say({ ...event, ...(plan ? { plan } : {}), ...(undone?.length ? { undone } : {}) });
    }
  };

  const text = (on: Run) => {
    const exports = on.exports!;
    return new TextDecoder().decode(
      new Uint8Array(exports.memory.buffer, exports.ti4_host_out_ptr(), exports.ti4_host_out_len()),
    );
  };
  const write = (on: Run, request: unknown) => {
    const exports = on.exports!;
    const bytes = new TextEncoder().encode(JSON.stringify(request));
    // Reserving the input may grow the memory: its buffer is read after.
    const pointer = exports.ti4_host_in(bytes.length);
    new Uint8Array(exports.memory.buffer, pointer, bytes.length).set(bytes);
    return bytes.length;
  };

  const onState = (message: StateMsg, snapshot: boolean, waiting: string | null) => {
    const told = modesOf(message.reaction_modes ?? {});
    if (JSON.stringify(told) !== JSON.stringify(modes)) {
      for (const key of Object.keys(modes)) {
        delete modes[key];
      }
      Object.assign(modes, told);
      say({ kind: "modes", modes: { ...modes } });
    }
    for (const note of passedOf(message)) {
      if (!noted.has(note.id)) {
        noted.add(note.id);
        // One note for the windows of one reason: each window is told.
        for (let count = 0; count < note.count; count += 1) {
          say({ kind: "passed", cards: note.cards });
        }
      }
    }
    const update = updateOf(message);
    // The engine tells the game again when only a card setting changed. The page then keeps
    // what is staged: nothing of the game is new.
    const same =
      !snapshot &&
      !stopped &&
      latest?.version === message.game_version &&
      latest.update.pending_choice?.nonce === update.pending_choice?.nonce;
    latest = {
      kind: "update",
      update,
      canUndo: message.history?.can_undo === true,
      version: message.game_version,
      ...(waiting !== null && stepping ? { stepping: { seat: waiting } } : {}),
    };
    if (!same) {
      const plan = stopped ?? undefined;
      stopped = null;
      again(plan, message.undone);
    }
  };

  const receive = (messages: ServerMessage[], waiting: string | null) => {
    for (const [at, message] of messages.entries()) {
      switch (message.type) {
        case "initial_snapshot":
          onState(message, true, waiting);
          break;
        case "state_update":
          onState(message, false, waiting);
          break;
        case "action_rejected":
          sent = null;
          say({ kind: "rejected", reason: rejection(message.reason) });
          again();
          break;
        case "error":
          if (asked) {
            refusal = message.message;
          } else if (latest) {
            say({ kind: "rejected", reason: message.message });
          } else {
            fail(message.message);
          }
          break;
        case "seat_staged":
          staged.receive(message);
          break;
        case "draft_outcome":
          if (asked?.id === message.request_id) {
            const { script, scope } = asked;
            asked = null;
            say({ kind: "draft", script, scope, outcome: message.outcome });
          }
          break;
        case "draft_applied": {
          if (sent?.nonce !== message.nonce) {
            break;
          }
          const script = sent;
          sent = null;
          if (message.stopped) {
            const plan = planStoppedOf(message.stopped, script);
            const told = messages
              .slice(at + 1)
              .some((later) => later.type === "initial_snapshot" || later.type === "state_update");
            if (plan.interrupted || told) {
              // The game went on, or went back to the choice of the plan: the update of where
              // it is now comes next.
              stopped = plan;
            } else {
              // Nothing was sent: the same choice is open.
              again(plan);
            }
          }
          break;
        }
        default:
          break;
      }
    }
  };

  /** A command to the session of `on`. Its messages are read, and a parked game goes on when it says so. */
  const command = (on: Run, request: Record<string, unknown>): Envelope | null => {
    if (!on.exports || on.id !== run?.id) {
      return null;
    }
    const length = write(on, request);
    const status = on.exports.ti4_host_command(length);
    const reply = text(on);
    if (status < 0) {
      throw new Error(reply);
    }
    const envelope: Envelope = JSON.parse(reply);
    receive(envelope.messages, on.waiting);
    if (envelope.restart) {
      restart = envelope.restart;
    }
    if (envelope.resume !== undefined && on.parked) {
      const answer = on.parked;
      on.parked = null;
      on.waiting = null;
      settle();
      on.exports.ti4_host_set_time(Date.now(), performance.now());
      answer(envelope.resume);
    } else if (restart && on.ended) {
      // The game is over: the history change is played at once.
      const resume = restart;
      restart = null;
      start(resume);
    }
    return envelope;
  };
  /** A message of the protocol, from the seat that the game is shown as. */
  const send = (on: Run, message: Record<string, unknown>) =>
    command(on, { protocol_version: PROTOCOL_VERSION, game_id: GAME_ID, ...message });

  /** The app was told a save: an undo back to the start is a save too. */
  let kept = play.save !== undefined;
  /** Tells the app what plays the game again. A new game with no decision yet has nothing to keep. */
  const keep = (on: Run) => {
    const save = command(on, { type: "save_state" })?.save;
    if (save && (save.log.length > 0 || kept)) {
      kept = true;
      play.onSave?.(save);
    }
  };

  /** Lets the seat decide that the game waits for. */
  const step = () => {
    if (run?.waiting && run.parked) {
      command(run, { type: "step" });
    }
  };
  /** The game waits before a decision of another seat: with Step for the page, else for the wait of the table. */
  const waitFor = (on: Run) => {
    settle();
    if (stepping) {
      return;
    }
    if (waitMs === 0) {
      return step();
    }
    timer = setTimeout(
      () => {
        timer = null;
        if (run === on) {
          step();
        }
      },
      waitMs * (0.6 + 0.8 * Math.random()),
    );
  };

  const start = (resume?: Record<string, unknown>) => {
    const on: Run = { id: ++runs, exports: null, parked: null, waiting: null, ended: false };
    run = on;
    settle();
    const total = (resume?.log as string[] | undefined)?.length ?? 0;
    if (total > 0) {
      say({ kind: "replaying", done: 0, total });
    }
    let shown = performance.now();
    const jspi = WebAssembly as unknown as Jspi;
    const ask = new jspi.Suspending(async () => {
      if (run !== on) {
        return LEAVE;
      }
      const envelope: Envelope = JSON.parse(text(on));
      if (envelope.kind === "progress") {
        if (envelope.progress && performance.now() - shown > REPLAY_SLICE_MS) {
          say({ kind: "replaying", ...envelope.progress });
          await breathe();
          shown = performance.now();
        }
        return run === on ? CONTINUE : LEAVE;
      }
      return new Promise<number>((answer) => {
        on.parked = answer;
        // The stack of the game is parked from here on: commands may reach it.
        queueMicrotask(() => {
          if (run !== on) {
            on.parked = null;
            return answer(LEAVE);
          }
          on.waiting = envelope.waiting ?? null;
          for (const key of unsent) {
            send(on, { type: "set_reaction_mode", card: key, mode: modes[key] ?? "always" });
          }
          unsent.clear();
          receive(envelope.messages, on.waiting);
          keep(on);
          if (on.waiting !== null && on.parked) {
            waitFor(on);
          }
        });
      });
    });
    (async () => {
      const instance = await WebAssembly.instantiate(engine.module, { host: { ask } });
      on.exports = instance.exports as unknown as Exports;
      on.exports.ti4_host_set_time(Date.now(), performance.now());
      const request = {
        ...(game.table
          ? { table: game.table }
          : {
              local: {
                seed: game.seed,
                players: game.players,
                humans: game.humans,
                ...(game.scenario === undefined ? {} : { scenario: game.scenario }),
                rule_seats: play.ruleSeats ?? 0,
              },
            }),
        ...(resume ? { resume } : {}),
        modes,
        stepping: pauses(),
        staged: true,
      };
      const status = await jspi.promising(on.exports.ti4_host_start)(write(on, request));
      if (run !== on) {
        return;
      }
      on.ended = true;
      on.parked = null;
      on.waiting = null;
      const reply = text(on);
      if (status < 0) {
        throw new Error(reply);
      }
      const end: Envelope = JSON.parse(reply);
      receive(end.messages, null);
      if (end.end?.diverged) {
        throw new Error(
          `The saved game does not fit this engine: ${end.end.diverged}. Start a new game.`,
        );
      }
      if (end.end?.error) {
        throw new Error(end.end.error);
      }
      if (restart) {
        // The parked run left for a history change: the game is played again.
        const next = restart;
        restart = null;
        return start(next);
      }
      keep(on);
    })().catch((error: unknown) => {
      if (run === on) {
        fail(error instanceof Error ? error.message : String(error));
      }
    });
  };

  /** The run that is the game now is no longer it. Where it waits for an answer, it leaves. */
  const leave = () => {
    const on = run;
    run = null;
    settle();
    if (on?.parked) {
      const answer = on.parked;
      on.parked = null;
      answer(LEAVE);
    }
  };
  start(play.save);

  // A reload, or a tab that is put away, does not wait for the next change: what was staged
  // last is written now.
  const flush = () => staged.flushNow();
  const hidden = () => document.visibilityState === "hidden" && staged.flushNow();
  if (typeof window !== "undefined") {
    window.addEventListener("pagehide", flush);
    document.addEventListener("visibilitychange", hidden);
  }

  const isPlayer = () => latest?.update.viewer.role === "player";
  /** Tells the engine whether it waits for each seat. Only a flag: it may be set at any time. */
  const tellStepping = () => {
    if (run?.exports) {
      command(run, { type: "stepping", mode: pauses() });
    }
  };

  // The tools of a local game. The saved game and the scenarios are of the app that made this
  // transport (`onSave`, `Engine.scenarios`).
  const capabilities: Capabilities = {
    stepping: {
      set(on) {
        stepping = on;
        tellStepping();
        if (run?.waiting !== null && run?.parked) {
          if (on) {
            // The seat waits for Step now, not for its time.
            settle();
            if (latest) {
              latest = { ...latest, stepping: { seat: run.waiting } };
              again();
            }
          } else {
            step();
          }
        }
      },
      step,
    },
    switchSeat(seat) {
      if (!run?.parked || sent || !isPlayer()) {
        return;
      }
      command(run, { type: "switch_seat", seat });
    },
    wait(ms) {
      waitMs = Math.max(0, ms);
      tellStepping();
      if (!stepping) {
        step();
      }
    },
    scenarios: true,
    save: true,
  };

  return {
    catalog: engine.catalog,
    odds: (request) => spaceOddsOf(engine.module, request),
    capabilities,
    staged: staged.store,
    subscribe(listener) {
      listeners.add(listener);
      listener({ kind: "modes", modes: { ...modes } });
      if (failed !== null) {
        listener({ kind: "error", message: failed });
      } else if (latest) {
        const { version: _, ...event } = latest;
        listener(event);
      }
      return () => listeners.delete(listener);
    },
    submitChoice(nonce, optionId) {
      if (!run?.parked || !latest || latest.update.pending_choice?.nonce !== nonce) {
        return;
      }
      send(run, {
        type: "submit_choice",
        nonce,
        expected_version: latest.version,
        option_id: optionId,
      });
    },
    submitPlan(nonce, plan) {
      this.applyDraft(nonce, { choices: [], plan });
    },
    applyDraft(nonce, draft) {
      if (!run?.parked || !latest || latest.update.pending_choice?.nonce !== nonce || !isPlayer()) {
        return;
      }
      const script: DraftStep[] = [
        ...draft.choices.map((option_id): DraftStep => ({ kind: "choose", option_id })),
        ...(draft.plan?.steps ?? []),
      ];
      // The answer may be in the reply: what was sent is known before.
      sent = { nonce, destination: draft.plan?.destination ?? "", lead: draft.choices.length };
      send(run, { type: "apply_draft", nonce, expected_version: latest.version, script });
    },
    draft(script, scope) {
      // Later, not now: the answer of the server is later too, and the page has one path.
      queueMicrotask(() => {
        if (!listeners.size) {
          return;
        }
        const on = run;
        if (!on?.parked || !latest) {
          say({ kind: "draft", script, scope, outcome: null });
          return;
        }
        const id = `${++requests}`;
        asked = { id, script, scope };
        refusal = null;
        try {
          send(on, { type: "run_draft", request_id: id, script, ...(scope ? { scope } : {}) });
        } catch (error) {
          refusal = error instanceof Error ? error.message : String(error);
        }
        if (asked?.id === id) {
          // The engine could not play it: the reason is the outcome.
          asked = null;
          const reason = refusal ?? "The engine did not play the draft.";
          say({
            kind: "draft",
            script,
            scope,
            outcome: { update: latest.update, consumed: 0, stop: { kind: "failed", reason } },
          });
        }
      });
    },
    setReactionMode(key, mode) {
      if ((modes[key] ?? "always") === mode) {
        return;
      }
      if (mode === "always") {
        delete modes[key];
      } else {
        modes[key] = mode;
      }
      play.onModes?.({ ...modes });
      say({ kind: "modes", modes: { ...modes } });
      // A window that is open stays as it was asked: the mode is of the windows after it.
      if (run?.parked || run?.ended) {
        send(run, { type: "set_reaction_mode", card: key, mode });
      } else {
        unsent.add(key);
      }
    },
    undo() {
      if (!latest?.canUndo || !run || !(run.parked || run.ended)) {
        return;
      }
      send(run, { type: "undo", expected_version: latest.version });
    },
    close() {
      staged.flushNow();
      leave();
      listeners.clear();
      staged.close();
      if (typeof window !== "undefined") {
        window.removeEventListener("pagehide", flush);
        document.removeEventListener("visibilitychange", hidden);
      }
    },
  };
}
