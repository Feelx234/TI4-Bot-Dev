import { OFFLINE_CAPABILITIES } from "../capabilities.ts";
import { OFFLINE_FACTION_IDS } from "../factions.ts";
import type {
  OfflineGameHandle,
  OfflineFeedbackQueue,
  OfflineGameSummary,
  OfflineLauncher,
  OfflineProgressListener,
  OfflineSetup,
} from "../types.ts";
import type { HostSave, StartConfig } from "./hostApi.ts";
import { JSPI_MISSING, jspiSupported } from "./jspi.ts";
import { createLocalFeedbackQueue } from "./localFeedbackQueue.ts";
import type { LocalTransport } from "./LocalTransport.ts";
import { SessionGameStore, type OfflineGameStore, type StoredGame } from "./store.ts";

export { JSPI_MISSING, jspiSupported };

/** Hands out the lazily loaded session code (a dynamic import, so it is a separate chunk). */
const sessionModule = () => import("./session.ts");

/** The 24 faction ids of the engine's `describe`; a test keeps this list in step with the wasm. */
export const OFFLINE_FACTIONS: readonly string[] = OFFLINE_FACTION_IDS;

export interface LocalLauncherOptions {
  store?: OfflineGameStore;
  /** Shared with the page-level proxy so the banner and the sheet see the same queue. */
  feedback?: OfflineFeedbackQueue;
  /** For tests: a different loader of the session code. */
  session?: typeof sessionModule;
}

const nowSeconds = () => Math.floor(Date.now() / 1000);
const abortError = () => new DOMException("aborted", "AbortError");

/** 0-based `Setup.human_seat` from the shell's 1-based seat; the nickname never reaches the engine. */
export function engineSetup(setup: OfflineSetup, gameId: string): Record<string, unknown> {
  const seed = setup.seed ?? randomSeed();
  return {
    game_id: gameId,
    seed,
    players: setup.playerCount,
    ...(setup.humanSeat !== null ? { human_seat: setup.humanSeat - 1 } : {}),
    ...(setup.humanFaction !== null ? { human_faction: setup.humanFaction } : {}),
  };
}

function randomSeed(): number {
  const words = crypto.getRandomValues(new Uint32Array(2));
  // Up to 2^52: the engine takes seeds up to 2^53.
  return (words[0] % 0x100000) * 0x1_0000_0000 + words[1];
}

const newGameId = () => `off_${crypto.randomUUID().replace(/-/g, "").slice(0, 12)}`;

function summarize(game: StoredGame): OfflineGameSummary {
  const save = game.save;
  return {
    gameId: game.gameId,
    playerCount: save.players,
    humanFaction: (save.setup.human_faction as string | undefined) ?? game.setup.humanFaction,
    humanSeat: save.human_seat + 1,
    round: save.round,
    phase: save.phase,
    answers: save.answers.length,
    createdAt: game.createdAt,
    updatedAt: game.updatedAt,
    status: save.finished ? "finished" : "in_progress",
    resumeEstimateMs: game.resumeMs ?? estimateResume(save),
  };
}

/** ~14 s for a full game of about 1100 answers, from the measured replay speed. */
const estimateResume = (save: HostSave): number | null =>
  save.answers.length === 0 ? null : Math.round(save.answers.length * 12);

export function createLocalLauncher(options: LocalLauncherOptions = {}): OfflineLauncher {
  const store = options.store ?? new SessionGameStore();
  const loadSession = options.session ?? sessionModule;
  const live = new Map<string, LocalTransport>();
  const feedback = options.feedback ?? createLocalFeedbackQueue();

  /** Serialised writes: a slow store never reorders saves. */
  const writes = new Map<string, Promise<void>>();
  const persist = (base: StoredGame, save: HostSave) => {
    const next: StoredGame = { ...base, save, updatedAt: nowSeconds() };
    Object.assign(base, next);
    const chain = (writes.get(base.gameId) ?? Promise.resolve()).then(() => store.put({ ...next }));
    writes.set(base.gameId, chain.catch(() => undefined));
  };

  async function open(
    stored: StoredGame,
    config: StartConfig,
    onProgress: OfflineProgressListener,
    signal: AbortSignal,
    replaying: boolean,
  ): Promise<OfflineGameHandle> {
    const existing = live.get(stored.gameId);
    if (existing && existing.getState().status === "connected") return { gameId: stored.gameId, transport: existing };
    existing?.close();
    live.delete(stored.gameId);
    if (!jspiSupported()) throw new Error(JSPI_MISSING);
    onProgress({ phase: "loading-engine", done: 0, total: null });
    const session = await loadSession();
    const engine = await session.loadEngine();
    if (signal.aborted) throw abortError();
    stored.engineHash = engine.hash;
    const total = replaying ? stored.save.answers.length : null;
    onProgress({ phase: replaying ? "replaying" : "starting", done: 0, total });
    const started = performance.now();
    const transport = session.openTransport(engine, {
      gameId: stored.gameId,
      nickname: stored.setup.nickname,
      engineHash: engine.hash,
      config,
      onSave: (save) => persist(stored, save),
      onProgress: (progress) => onProgress({ phase: "replaying", done: progress.done, total: progress.total }),
    });
    const abort = () => transport.close();
    signal.addEventListener("abort", abort, { once: true });
    try {
      await transport.begin();
      await transport.ready;
    } catch (error) {
      transport.close();
      if (signal.aborted) throw abortError();
      throw error;
    } finally {
      signal.removeEventListener("abort", abort);
    }
    if (signal.aborted) {
      transport.close();
      throw abortError();
    }
    if (replaying) stored.resumeMs = Math.round(performance.now() - started);
    live.set(stored.gameId, transport);
    const save = transport.latestSave();
    if (save) persist(stored, save);
    return { gameId: stored.gameId, transport };
  }

  return {
    capabilities: OFFLINE_CAPABILITIES,
    factions: OFFLINE_FACTIONS,
    supported: () => (jspiSupported() ? { ok: true } : { ok: false, reason: JSPI_MISSING }),
    list: async () => (await store.list()).map(summarize),
    async create(setup, onProgress, signal) {
      const gameId = newGameId();
      const config: StartConfig = { setup: engineSetup(setup, gameId) };
      const now = nowSeconds();
      const stored = {
        gameId,
        setup,
        engineHash: "",
        createdAt: now,
        updatedAt: now,
        resumeMs: null,
        save: undefined as unknown as HostSave,
      } satisfies StoredGame;
      return open(stored, config, onProgress, signal, false);
    },
    async resume(gameId, onProgress, signal) {
      const stored = await store.get(gameId);
      if (!stored) throw new Error("This offline game is not on this device any more.");
      const config: StartConfig = { setup: stored.save.setup, resume: stored.save };
      return open(stored, config, onProgress, signal, stored.save.answers.length > 0);
    },
    async remove(gameId) {
      live.get(gameId)?.close();
      live.delete(gameId);
      await writes.get(gameId);
      await store.remove(gameId);
    },
    async exportGame(gameId) {
      const stored = await store.get(gameId);
      if (!stored) throw new Error("This offline game is not on this device any more.");
      const body = {
        format: "ti4-offline-game",
        gameId,
        seed: stored.save.seed,
        answers: stored.save.answers,
        engineHash: stored.engineHash,
        playerCount: stored.save.players,
        humanSeat: stored.save.human_seat,
        save: stored.save,
      };
      return {
        filename: `ti4-offline-${gameId}.json`,
        blob: new Blob([JSON.stringify(body, null, 2)], { type: "application/json" }),
      };
    },
    feedback,
  };
}
