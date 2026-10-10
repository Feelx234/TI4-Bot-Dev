// How ids of the wire protocol are shown. Presentation only: no rule is decided here.
import type { Force, TechColor, UnitType } from "../../model";

const SYMBOLS = ["●", "◆", "▲", "■", "✚", "✖", "♣", "★"];
const COLORS = [
  "#8cd5ef",
  "#d69a75",
  "#8dd2b0",
  "#ec7f8f",
  "#b79cf0",
  "#e6c15c",
  "#f0a6d0",
  "#b9d86a",
];
/** The shape and the colour of a seat, by its place at the table. */
export const seatMark = (index: number) => ({
  symbol: SYMBOLS[index % SYMBOLS.length],
  color: COLORS[index % COLORS.length],
});

export const titleCase = (text: string) =>
  text.replace(
    /(^|[\s_])([a-z])/g,
    (_, gap: string, letter: string) => `${gap === "_" ? " " : gap}${letter.toUpperCase()}`,
  );

/**
 * The printed name and the commodity value of a faction.
 * Cleanup: the view does not carry them yet. When it does, remove this table.
 */
const FACTIONS: Record<string, [name: string, commodities: number]> = {
  sol: ["Sol", 4],
  hacan: ["Hacan", 6],
  letnev: ["Letnev", 2],
  xxcha: ["Xxcha", 4],
  jolnar: ["Jol-Nar", 4],
  l1z1x: ["L1Z1X", 2],
  sardakk: ["Sardakk N'orr", 3],
  yin: ["Yin", 2],
};
export const factionName = (id: string) => FACTIONS[id]?.[0] ?? titleCase(id);
/** Null for a faction that the table above does not have. */
export const commodityValue = (id: string): number | null => FACTIONS[id]?.[1] ?? null;

/** "pok5trade" is card 5, "Trade". The id is the card set, the initiative and the name. */
export function strategyCard(id: string): { number: number; name: string } {
  const [, number = "0", name = id] = /(\d)([a-z]*)$/.exec(id) ?? [];
  return { number: Number(number), name: name ? titleCase(name) : id };
}

const UNITS: [suffix: string, unit: UnitType][] = [
  ["flagship", "flagship"],
  ["warsun", "warsun"],
  ["dreadnought", "dreadnought"],
  ["carrier", "carrier"],
  ["cruiser", "cruiser"],
  ["destroyer", "destroyer"],
  ["fighter", "fighter"],
  ["mech", "mech"],
  ["infantry", "infantry"],
  ["pds", "pds"],
  ["spacedock", "dock"],
];
/** The display order of unit types. */
export const UNIT_ORDER: UnitType[] = UNITS.map(([, unit]) => unit);
/**
 * A faction's unit has the base type at the end of its id ("sol_carrier"), and an upgraded unit
 * has a 2 after it ("cruiser2", "sol_carrier2").
 */
export function unitType(id: string): UnitType | null {
  const base = id.replace(/2$/, "");
  return UNITS.find(([suffix]) => base === suffix || base.endsWith(`_${suffix}`))?.[1] ?? null;
}

/** Units counted by type, in display order. */
export function forceOf(units: { unit_type: string; damaged: boolean }[]): Force {
  const counts = new Map<UnitType, { count: number; damaged: number }>();
  for (const unit of units) {
    const type = unitType(unit.unit_type);
    if (type) {
      const entry = counts.get(type) ?? { count: 0, damaged: 0 };
      entry.count++;
      entry.damaged += unit.damaged ? 1 : 0;
      counts.set(type, entry);
    }
  }
  return UNIT_ORDER.flatMap((unit) => {
    const entry = counts.get(unit);
    return entry ? [{ unit, ...entry }] : [];
  });
}

const SHIPS = new Set<UnitType>([
  "flagship",
  "warsun",
  "dreadnought",
  "carrier",
  "cruiser",
  "destroyer",
  "fighter",
]);
export const isShip = (unit: UnitType) => SHIPS.has(unit);
export const isGroundForce = (unit: UnitType) => unit === "infantry" || unit === "mech";

export const ANOMALIES = {
  "asteroid field": "asteroid",
  supernova: "supernova",
  nebula: "nebula",
  "gravity rift": "rift",
} as const;

export const WORMHOLES: Record<string, string> = { ALPHA: "α", BETA: "β", GAMMA: "γ", DELTA: "δ" };

export const TECH_COLORS: Record<string, TechColor> = {
  PROPULSION: "B",
  BIOTIC: "G",
  CYBERNETIC: "Y",
  WARFARE: "R",
};

const PHASES: Record<string, string> = {
  strategy: "Strategy phase",
  action: "Action phase",
  status: "Status phase",
  agenda: "Agenda phase",
};
export const phaseName = (phase: string) => PHASES[phase] ?? titleCase(phase);

/** The label of an option starts in lower case, as the engine writes it. */
export const sentence = (text: string) => text.charAt(0).toUpperCase() + text.slice(1);
