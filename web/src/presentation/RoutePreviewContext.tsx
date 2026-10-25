import React, { createContext, useCallback, useContext, useMemo, useState } from "react";

/** A route to draw on the map: the systems the ship enters, origin first, active system last. */
export interface RoutePreview {
  key: string;
  path: readonly string[];
  /** The route leaves a gravity rift: drawn as a warning. */
  risky: boolean;
}

export interface RoutePreviewApi {
  previews: readonly RoutePreview[];
  setPreviews: (previews: readonly RoutePreview[]) => void;
}

const RoutePreviewContext = createContext<RoutePreviewApi | null>(null);

/**
 * Lets the movement tray tell the map which routes the staged ships take, so the systems they
 * pass are highlighted. Display only; the answer to the game is the staged plan.
 */
export const RoutePreviewProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [previews, set] = useState<readonly RoutePreview[]>([]);
  const setPreviews = useCallback((next: readonly RoutePreview[]) => {
    set((prev) => {
      const same =
        prev.length === next.length &&
        prev.every(
          (p, i) =>
            p.key === next[i].key &&
            p.risky === next[i].risky &&
            p.path.join(">") === next[i].path.join(">"),
        );
      return same ? prev : next;
    });
  }, []);
  const value = useMemo(() => ({ previews, setPreviews }), [previews, setPreviews]);
  return <RoutePreviewContext.Provider value={value}>{children}</RoutePreviewContext.Provider>;
};

/** The shared route previews, or null outside a provider (then nothing is drawn). */
export const useRoutePreview = (): RoutePreviewApi | null => useContext(RoutePreviewContext);
