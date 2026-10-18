import { useCallback, useSyncExternalStore } from "react";

/**
 * A viewer-local "ask me about this optional ability" switch kept in localStorage (this browser,
 * across games, until switched back on). Default ON (ask every time): only an explicit "false"
 * stops the prompt. Storage failures fall back to asking.
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

  function read(): boolean {
    try {
      return localStorage.getItem(key) !== "false";
    } catch {
      return true;
    }
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
    const ask = useSyncExternalStore(subscribe, read, () => true);
    const setAsk = useCallback((next: boolean) => {
      try {
        localStorage.setItem(key, String(next));
      } catch {
        // Storage unavailable: the choice then only lasts until the next read.
      }
      listeners.forEach((l) => l());
      anyListeners.forEach((l) => l());
    }, []);
    const toggle = useCallback(() => setAsk(!read()), [setAsk]);
    return { ask, setAsk, toggle };
  }

  return { read, useSetting };
}
