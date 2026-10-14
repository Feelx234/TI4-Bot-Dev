/**
 * Shared vocabulary of the view models. Ids are the ids of the wire protocol
 * (`web/src/protocol/types.ts`): seat ids, system ids, planet ids and unit type names are strings.
 */
export type SeatId = string;
export type SystemId = string;
export type PlanetId = string;

export type UnitType =
  | "flagship"
  | "warsun"
  | "dreadnought"
  | "carrier"
  | "cruiser"
  | "destroyer"
  | "fighter"
  | "mech"
  | "infantry"
  | "pds"
  | "dock";

/** Counts per unit type, in display order. Compare `PlacedUnitView[]`, grouped by type. */
export type Force = { unit: UnitType; count: number; damaged: number }[];

export interface SeatView {
  id: SeatId;
  /** Player name. */
  name: string;
  faction: string;
  /** Shape that identifies the seat without colour. */
  symbol: string;
  color: string;
}

/** Text with marked parts: a seat symbol, or a value that a modifier changed. */
export type Rich = (string | { seat: SeatId } | { accent: string })[];

export type Tone = "draft" | "live" | "done" | "alert" | "quiet";
export interface PillView {
  tone: Tone;
  label: string;
}

/** Rule or help text. It is not visible by default: the player opens it by hover or click. */
export interface HelpView {
  title: string;
  text: string;
}

export interface GaugeView {
  label: string;
  used: number;
  total: number;
}

export interface CounterView {
  /** Id of the staged value. It goes back unchanged in a `setCount` intent. */
  id: string;
  value: number;
  max: number;
  label: string;
}

/** Tokens that tie a list row to its system, planet or route on the board: `sys:27`, `pl:jord`, `rt:j-carrier`. */
export type LinkToken = string;

const UNIT_NAMES: Record<UnitType, [string, string]> = {
  flagship: ["Flagship", "Flagships"],
  warsun: ["War sun", "War suns"],
  dreadnought: ["Dreadnought", "Dreadnoughts"],
  carrier: ["Carrier", "Carriers"],
  cruiser: ["Cruiser", "Cruisers"],
  destroyer: ["Destroyer", "Destroyers"],
  fighter: ["Fighter", "Fighters"],
  mech: ["Mech", "Mechs"],
  infantry: ["Infantry", "Infantry"],
  pds: ["PDS", "PDS"],
  dock: ["Space dock", "Space docks"],
};

export const unitName = (unit: UnitType, count = 1) => UNIT_NAMES[unit][count === 1 ? 0 : 1];
export const plural = (count: number, word: string, many = word + "s") =>
  `${count} ${count === 1 ? word : many}`;
export const forceSize = (force: Force) => force.reduce((sum, item) => sum + item.count, 0);
/** "1 Carrier · 2 Fighters · 1 Dreadnought (damaged)" */
export const forceText = (force: Force) =>
  force
    .filter((item) => item.count)
    .map(
      (item) =>
        `${item.count} ${unitName(item.unit, item.count)}${item.damaged ? ` (${item.count > 1 ? item.damaged + " " : ""}damaged)` : ""}`,
    )
    .join(" · ") || "None";
