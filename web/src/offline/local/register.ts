import { OFFLINE_CAPABILITIES } from "../capabilities.ts";
import { OFFLINE_FACTION_IDS } from "../factions.ts";
import { registerOfflineLauncher } from "../registry.ts";
import type { OfflineLauncher } from "../types.ts";
import { hasEngineAsset } from "./engineAsset.ts";
import { JSPI_MISSING, jspiSupported } from "./jspi.ts";
import { createLocalFeedbackQueue } from "./localFeedbackQueue.ts";

/**
 * The launcher as the page registers it at startup: only what the shell needs at once (flags,
 * faction list, the feedback queue, the JSPI check) lives in the main bundle. Everything that
 * stores or plays a game is behind a dynamic import and loads when "Play offline" is used.
 */
export function createLazyLauncher(): OfflineLauncher {
  const feedback = createLocalFeedbackQueue();
  let real: Promise<OfflineLauncher> | null = null;
  const launcher = () =>
    (real ??= import("./launcher.ts").then((m) => m.createLocalLauncher({ feedback })));
  return {
    capabilities: OFFLINE_CAPABILITIES,
    factions: OFFLINE_FACTION_IDS,
    supported: () => (jspiSupported() ? { ok: true } : { ok: false, reason: JSPI_MISSING }),
    list: async () => (await launcher()).list(),
    create: async (setup, onProgress, signal) => (await launcher()).create(setup, onProgress, signal),
    resume: async (gameId, onProgress, signal) => (await launcher()).resume(gameId, onProgress, signal),
    remove: async (gameId) => (await launcher()).remove(gameId),
    exportGame: async (gameId) => (await launcher()).exportGame(gameId),
    feedback,
  };
}

/**
 * Registers the real offline launcher when this build has the engine file. The dev mock
 * (screenshots, `?offline-mock`) keeps the slot when it was asked for.
 */
export function registerLocalLauncher(): void {
  const mock =
    Boolean((window as unknown as { __TI4_OFFLINE_MOCK__?: unknown }).__TI4_OFFLINE_MOCK__) ||
    new URLSearchParams(window.location.search).has("offline-mock");
  if (mock || !hasEngineAsset()) return;
  registerOfflineLauncher(createLazyLauncher());
}
