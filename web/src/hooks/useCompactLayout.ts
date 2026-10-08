import { useEffect, useState } from "react";

/** A phone in portrait (narrow) or landscape (short): the layouts where floating panels must stay small. */
export const COMPACT_LAYOUT_QUERY = "(max-width: 720px), (max-height: 500px)";

/** Wider than a portrait phone: the board column sits beside the side panels (a phone in landscape). */
export const WIDE_LAYOUT_QUERY = "(min-width: 721px)";

const read = (query: string): boolean =>
  typeof window !== "undefined" && typeof window.matchMedia === "function"
    ? window.matchMedia(query).matches
    : false;

/** Whether the media query matches; follows rotation and resizing. False where matchMedia is missing. */
export function useMatchMedia(query: string): boolean {
  const [matches, setMatches] = useState(() => read(query));
  useEffect(() => {
    if (typeof window.matchMedia !== "function") return;
    const list = window.matchMedia(query);
    const update = () => setMatches(list.matches);
    update();
    list.addEventListener("change", update);
    return () => list.removeEventListener("change", update);
  }, [query]);
  return matches;
}

/** True on a phone-sized viewport (portrait, or a short landscape screen). */
export const useCompactLayout = (): boolean => useMatchMedia(COMPACT_LAYOUT_QUERY);
