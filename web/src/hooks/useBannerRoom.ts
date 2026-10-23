import { useLayoutEffect, type RefObject } from "react";

/** The CSS variable holding how far the decision banner (e.g. the ready-planets bar) reaches up from the bottom edge. */
export const BANNER_ROOM_VAR = "--decision-banner-room";

/**
 * Publishes how much room the element takes at the bottom of the screen (its distance from the
 * viewport's bottom edge to its top, plus a gap) as `--decision-banner-room` while it is mounted
 * (`active`), so the secondary-prep answer bars dock above it instead of over its Confirm button.
 */
export function useBannerRoom(ref: RefObject<HTMLElement | null>, active: boolean): void {
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el || !active) return;
    const root = document.documentElement;
    const publish = () => {
      const top = el.getBoundingClientRect().top;
      root.style.setProperty(BANNER_ROOM_VAR, `${Math.max(0, Math.ceil(window.innerHeight - top)) + 8}px`);
    };
    publish();
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(publish);
    observer?.observe(el);
    window.addEventListener("resize", publish);
    return () => {
      observer?.disconnect();
      window.removeEventListener("resize", publish);
      root.style.removeProperty(BANNER_ROOM_VAR);
    };
  }, [ref, active]);
}
