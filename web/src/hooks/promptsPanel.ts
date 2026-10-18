import { useSyncExternalStore } from "react";

/** Whether the Prompts list is open: opened from the player sheet and from the paused-prompts chip. */
let open = false;
const listeners = new Set<() => void>();

function set(next: boolean) {
  if (open === next) return;
  open = next;
  listeners.forEach((l) => l());
}

export const openPromptsPanel = () => set(true);
export const closePromptsPanel = () => set(false);

export function usePromptsPanelOpen(): boolean {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    () => open,
    () => false,
  );
}
