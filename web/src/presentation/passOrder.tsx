import React, { createContext, useContext, useMemo, useRef } from "react";
import type { PlayerView } from "../protocol/types.ts";

/**
 * Who has passed in this round's action phase, and in which place. The engine keeps only a
 * `passed` flag per seat, not the order, so the order is what this client saw: a pass that shows
 * up alone in an update is the next place; several at once (or already there when the page
 * opened) have no known place and only the badge shows.
 */
export interface PassTracker {
  round: number;
  /** Seat to its place (1 = first to pass), or null when the place was not seen. */
  places: Record<string, number | null>;
}

export function trackPasses(
  prev: PassTracker | null,
  round: number,
  passedSeats: readonly string[],
): PassTracker {
  const sameRound = prev !== null && prev.round === round;
  const places: Record<string, number | null> = {};
  if (sameRound) {
    for (const seat of passedSeats) if (seat in prev.places) places[seat] = prev.places[seat];
  }
  const fresh = passedSeats.filter((seat) => !(seat in places));
  // The first update of a round (or of the page) with exactly one passer still knows the place.
  const known = fresh.length === 1;
  for (const seat of fresh) places[seat] = known ? Object.keys(places).length + 1 : null;
  return { round, places };
}

export function ordinal(n: number): string {
  const tens = n % 100;
  if (tens >= 11 && tens <= 13) return `${n}th`;
  return `${n}${["th", "st", "nd", "rd"][n % 10] ?? "th"}`;
}

/** What the sheet and the seat legend show for a seat. */
export interface PassStatus {
  passed: boolean;
  /** Place among the passers, when seen. */
  place: number | null;
  /** "Passed" or "Passed (2nd)". */
  label: string;
}

const NOT_PASSED: PassStatus = { passed: false, place: null, label: "" };
type PassLookup = (seat: string) => PassStatus;
const PassOrderContext = createContext<PassLookup>(() => NOT_PASSED);

export function passStatusFor(tracker: PassTracker | null, seat: string, active: boolean, passed: boolean): PassStatus {
  if (!active || !passed) return NOT_PASSED;
  const place = tracker?.places[seat] ?? null;
  return { passed: true, place, label: place ? `Passed (${ordinal(place)})` : "Passed" };
}

export const PassOrderProvider: React.FC<{
  players: readonly PlayerView[] | undefined;
  round: number | undefined;
  phase: string | undefined;
  children: React.ReactNode;
}> = ({ players, round, phase, children }) => {
  const trackerRef = useRef<PassTracker | null>(null);
  const active = phase === "action";
  const lookup = useMemo<PassLookup>(() => {
    const passedSeats = active ? (players ?? []).filter((p) => p.passed).map((p) => p.id) : [];
    // Outside the action phase nothing is tracked: the next action phase starts from nothing.
    const tracker = active && round !== undefined ? trackPasses(trackerRef.current, round, passedSeats) : null;
    trackerRef.current = tracker;
    const passed = new Set(passedSeats);
    return (seat) => passStatusFor(tracker, seat, active, passed.has(seat));
  }, [players, round, active]);
  return <PassOrderContext.Provider value={lookup}>{children}</PassOrderContext.Provider>;
};

export const usePassStatus = (): PassLookup => useContext(PassOrderContext);
