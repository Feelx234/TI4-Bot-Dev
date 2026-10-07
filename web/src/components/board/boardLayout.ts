import type { CSSProperties } from "react";

/**
 * Room kept under the map while a prompt pill is showing (the choice banner sits 24px above the
 * bottom edge and is up to ~75px tall), in px. At or below 720px wide the banner is outside the
 * map, so `.board-stage` drops the inset in CSS.
 */
export const BOARD_PROMPT_INSET = 108;
/** Room kept otherwise, for the collapsed event log bar that overlays the bottom edge. */
export const BOARD_IDLE_INSET = 48;
/** The toolbar and seat legend flow in `.board-chrome` above the stage; its row is at least this tall. */
export const BOARD_TOOLBAR_INSET = 56;

/**
 * The map's drawing area: everything below the toolbar row and above the bottom inset. The SVG
 * viewBox is fitted into this box (preserveAspectRatio meet), so "reset" and the initial view
 * never place a tile row under the chrome or the prompt.
 */
export const boardStageStyle = (hasPrompt: boolean): CSSProperties => ({
  flex: "1 1 auto",
  minHeight: 0,
  boxSizing: "border-box",
  paddingBottom: hasPrompt ? BOARD_PROMPT_INSET : BOARD_IDLE_INSET,
});

/** Seat markers drawn on the map, in SVG units (about px at 1:1): 18 is the floor, up from 12. */
export const COMMAND_TOKEN_RADIUS = 9;
export const COMMAND_TOKEN_STEP = 19;
/** Control glyph next to a planet, up from 12. */
export const CONTROL_SYMBOL_FONT_SIZE = 17;
