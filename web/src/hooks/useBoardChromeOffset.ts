import { useLayoutEffect, type RefObject } from "react";

/** The CSS variable holding the bottom edge of the board's toolbar row, for overlays that must sit below it. */
export const BOARD_CHROME_BOTTOM_VAR = "--board-chrome-bottom";

/**
 * Publishes the element's bottom edge (viewport px) as `--board-chrome-bottom` on the document root
 * while it is mounted, and clears it on unmount. The row wraps (toolbar, seat legend), so its height varies.
 */
export function useBoardChromeOffset(ref: RefObject<HTMLElement | null>): void {
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const root = document.documentElement;
    const publish = () =>
      root.style.setProperty(BOARD_CHROME_BOTTOM_VAR, `${Math.ceil(el.getBoundingClientRect().bottom)}px`);
    publish();
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(publish);
    observer?.observe(el);
    window.addEventListener("resize", publish);
    return () => {
      observer?.disconnect();
      window.removeEventListener("resize", publish);
      root.style.removeProperty(BOARD_CHROME_BOTTOM_VAR);
    };
  }, [ref]);
}
