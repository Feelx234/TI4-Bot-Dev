import { useMemo, useSyncExternalStore } from "react";
import { createAskSetting, subscribeAnyAskSetting, type AskMode } from "./askSetting.ts";
import { askStorageKey, offersOwnTurnMode, PAUSABLE_PROMPTS, type PausablePrompt } from "../presentation/pausablePrompts.ts";
import { skipsNow, type TurnContext } from "../presentation/promptTurn.ts";

/** The viewer's switch for one registered ability (shared with the I48S and Aetherpassage hooks). */
export function askSettingFor(id: string) {
  return createAskSetting(askStorageKey(id));
}

/** Whether the viewer is not skipping the ability outright (default yes; read fresh). */
export function readAskFor(id: string): boolean {
  return askSettingFor(id).read();
}

/**
 * How the viewer set the ability (read fresh). "Only in my turn" on an ability that has no such
 * choice (a stale value) counts as asking.
 */
export function readModeFor(id: string): AskMode {
  const mode = askSettingFor(id).readMode();
  if (mode !== "ownturn") return mode;
  const entry = PAUSABLE_PROMPTS.find((p) => p.id === id);
  return entry && offersOwnTurnMode(entry) ? "ownturn" : "ask";
}

/**
 * Whether the viewer's setting skips the ability now. Without `turn` only a plain "skip" counts;
 * with it, "only in my turn" skips while another seat is the active player.
 */
export function isPausedNow(id: string, turn?: TurnContext | null): boolean {
  return skipsNow(id, readModeFor(id), turn);
}

function modesKey(): string {
  return PAUSABLE_PROMPTS.map((p) => [p.id, readModeFor(p.id)] as const)
    .filter(([, mode]) => mode !== "ask")
    .map(([id, mode]) => `${id}:${mode}`)
    .join(",");
}

/** The abilities with a setting other than Ask, with the setting, live. */
export function usePromptModes(): ReadonlyMap<string, AskMode> {
  const key = useSyncExternalStore(subscribeAnyAskSetting, modesKey, () => "");
  return useMemo(() => {
    const modes = new Map<string, AskMode>();
    for (const part of key ? key.split(",") : []) {
      const [id, mode] = part.split(":");
      modes.set(id, mode as AskMode);
    }
    return modes;
  }, [key]);
}

/** The abilities the viewer has paused (skipped, or skipped in the turns of others), live (every registered ability, whatever the faction). */
export function usePausedPrompts(): PausablePrompt[] {
  const modes = usePromptModes();
  return useMemo(() => PAUSABLE_PROMPTS.filter((p) => modes.has(p.id)), [modes]);
}
