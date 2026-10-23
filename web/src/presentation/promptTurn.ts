import type { AskMode } from "../hooks/askSetting.ts";
import { offersOwnTurnMode, pausablePrompt } from "./pausablePrompts.ts";

/** Whose turn it is, as the game shell knows it. */
export interface TurnContext {
  viewerSeat?: string | null;
  /** The phase name ("action", "status", ...). */
  phase?: string | null;
  /** The active player of the action phase. */
  activePlayer?: string | null;
}

/**
 * Whether another seat has the turn: the action phase, with an active player who is not the
 * viewer. Outside the action phase nobody has the turn, so "only in my turn" skips nothing (the
 * same rule as the server's per-game mode for action cards).
 */
export function isAnotherSeatsTurn(turn: TurnContext | null | undefined): boolean {
  if (!turn || !turn.viewerSeat || turn.phase !== "action" || !turn.activePlayer) return false;
  return turn.activePlayer !== turn.viewerSeat;
}

/**
 * Whether an ability set to `mode` is skipped for the viewer right now. "Only in my turn" counts
 * only for the abilities that have it (a stale stored value on any other one asks).
 */
export function skipsNow(promptId: string, mode: AskMode, turn: TurnContext | null | undefined): boolean {
  if (mode === "skip") return true;
  if (mode !== "ownturn") return false;
  const entry = pausablePrompt(promptId);
  if (!entry || !offersOwnTurnMode(entry)) return false;
  return isAnotherSeatsTurn(turn);
}
