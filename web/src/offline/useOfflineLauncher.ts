import { useSyncExternalStore } from "react";
import { getOfflineLauncher, subscribeLauncher } from "./registry.ts";
import type { OfflineLauncher } from "./types.ts";

/** The registered launcher (null when this build has none), live. */
export const useOfflineLauncher = (): OfflineLauncher | null =>
  useSyncExternalStore(subscribeLauncher, getOfflineLauncher);
