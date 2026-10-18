import { useCallback, useSyncExternalStore } from "react";

/**
 * A viewer-local "ask me about this optional ability" switch kept in localStorage (this browser,
 * across games, until switched back on). Default ON (ask every time): only an explicit "false"
 * stops the prompt. Storage failures fall back to asking.
 */
export function createAskSetting(key: string) {
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
    }, []);
    const toggle = useCallback(() => setAsk(!read()), [setAsk]);
    return { ask, setAsk, toggle };
  }

  return { read, useSetting };
}
