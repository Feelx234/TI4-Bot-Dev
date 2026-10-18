import { useMemo, useSyncExternalStore } from "react";
import { createAskSetting, subscribeAnyAskSetting } from "./askSetting.ts";
import { askStorageKey, PAUSABLE_PROMPTS, type PausablePrompt } from "../presentation/pausablePrompts.ts";

/** The viewer's switch for one registered ability (shared with the I48S and Aetherpassage hooks). */
export function askSettingFor(id: string) {
  return createAskSetting(askStorageKey(id));
}

/** Whether the viewer wants to be asked about the ability (default yes; read fresh). */
export function readAskFor(id: string): boolean {
  return askSettingFor(id).read();
}

/** Whether the viewer switched the ability off. */
export function isPausedNow(id: string): boolean {
  return !readAskFor(id);
}

function pausedKey(): string {
  return PAUSABLE_PROMPTS.filter((p) => isPausedNow(p.id))
    .map((p) => p.id)
    .join(",");
}

/** The abilities the viewer has paused, live (every registered ability, whatever the faction). */
export function usePausedPrompts(): PausablePrompt[] {
  const key = useSyncExternalStore(subscribeAnyAskSetting, pausedKey, () => "");
  return useMemo(() => {
    const ids = new Set(key ? key.split(",") : []);
    return PAUSABLE_PROMPTS.filter((p) => ids.has(p.id));
  }, [key]);
}
