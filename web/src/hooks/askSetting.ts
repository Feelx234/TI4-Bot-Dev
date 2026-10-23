import { useCallback, useSyncExternalStore } from "react";

/**
 * How the viewer wants to be asked about an optional ability: every time (`ask`), never (`skip`,
 * the client declines for them), or only while it is their own turn (`ownturn`, declined while
 * another seat is the active player of the action phase).
 */
export type AskMode = "ask" | "skip" | "ownturn";

/** The value kept in localStorage for each mode; "true" and "false" are those of the first version. */
const STORED: Record<AskMode, string> = { ask: "true", skip: "false", ownturn: "ownturn" };

/** Reads a stored value: only an explicit "false" skips, only "ownturn" is the third mode, anything else asks. */
export function modeOfStored(value: string | null): AskMode {
  if (value === "false") return "skip";
  if (value === "ownturn") return "ownturn";
  return "ask";
}

/**
 * A viewer-local "ask me about this optional ability" switch kept in localStorage (this browser,
 * across games, until switched back on). Default ASK (every time): only an explicit "false"
 * stops the prompt, and the stored "true" / "false" of the first version still mean ask / skip.
 * A third value, "ownturn", asks only while it is the viewer's own turn; to a reader that only
 * knows the boolean (`read`) it counts as asking. Storage failures fall back to asking.
 */
export function createAskSetting(key: string) {
  const known = settings.get(key);
  if (known) return known;
  const created = buildAskSetting(key);
  settings.set(key, created);
  return created;
}

/** One instance per storage key, so every hook on the same key hears the same switch. */
const settings = new Map<string, ReturnType<typeof buildAskSetting>>();

/** Called after any setting changed, for views over all of them (the Prompts list, the chip). */
const anyListeners = new Set<() => void>();

export function subscribeAnyAskSetting(listener: () => void) {
  anyListeners.add(listener);
  window.addEventListener("storage", listener);
  return () => {
    anyListeners.delete(listener);
    window.removeEventListener("storage", listener);
  };
}

function buildAskSetting(key: string) {
  const listeners = new Set<() => void>();

  function readMode(): AskMode {
    try {
      return modeOfStored(localStorage.getItem(key));
    } catch {
      return "ask";
    }
  }

  /** Whether the viewer is not skipping the ability outright ("only in my turn" still counts as asking). */
  function read(): boolean {
    return readMode() !== "skip";
  }

  function subscribe(listener: () => void) {
    listeners.add(listener);
    window.addEventListener("storage", listener);
    return () => {
      listeners.delete(listener);
      window.removeEventListener("storage", listener);
    };
  }

  function useSetting() {
    const mode = useSyncExternalStore(subscribe, readMode, () => "ask" as AskMode);
    const setMode = useCallback((next: AskMode) => {
      try {
        localStorage.setItem(key, STORED[next]);
      } catch {
        // Storage unavailable: the choice then only lasts until the next read.
      }
      listeners.forEach((l) => l());
      anyListeners.forEach((l) => l());
    }, []);
    const setAsk = useCallback((next: boolean) => setMode(next ? "ask" : "skip"), [setMode]);
    // The two-state switch: skipped goes back to asking, anything else goes to skipped.
    const toggle = useCallback(() => setMode(readMode() === "skip" ? "ask" : "skip"), [setMode]);
    // Ask, then skip, then (where the ability has it) only in my turn, then ask again.
    const cycle = useCallback(
      (withOwnTurn: boolean) => {
        const now = readMode();
        setMode(now === "ask" ? "skip" : now === "skip" && withOwnTurn ? "ownturn" : "ask");
      },
      [setMode],
    );
    return { ask: mode !== "skip", mode, setMode, setAsk, toggle, cycle };
  }

  return { read, readMode, useSetting };
}
