import type { ChoiceOption } from "../protocol/types.ts";

/**
 * Component limits (LRR 31.4) of the units a production dialog can build.
 *
 * The engine states them: the produce question carries `details.unit_limits`, one entry per capped
 * plastic (`limit`, units on the board `held`, `remaining` in reinforcements, and `units`, the
 * buildable ids that share the box). Fighters and infantry are uncapped and have no entry. This
 * module only subtracts the staged builds. Display only: the engine enforces the limit.
 */
export interface UnitLimit {
  base: string;
  /** A buildable id of this plastic, for naming the row when its option is no longer offered. */
  unit: string;
  units: string[];
  limit: number;
  held: number;
  remaining: number;
}

const whole = (v: unknown): number | null =>
  typeof v === "number" && Number.isSafeInteger(v) && v >= 0 ? v : null;

export function readUnitLimits(raw: unknown): UnitLimit[] {
  if (!raw || typeof raw !== "object") return [];
  const out: UnitLimit[] = [];
  for (const [base, value] of Object.entries(raw as Record<string, unknown>)) {
    if (!value || typeof value !== "object") continue;
    const v = value as Record<string, unknown>;
    const limit = whole(v.limit);
    const held = whole(v.held);
    const remaining = whole(v.remaining);
    const units = Array.isArray(v.units) ? v.units.filter((u): u is string => typeof u === "string") : [];
    if (limit === null || held === null || remaining === null || units.length === 0) continue;
    out.push({ base, unit: units[0], units, limit, held, remaining });
  }
  return out;
}

export const limitOfUnit = (limits: readonly UnitLimit[], unit: string): UnitLimit | undefined =>
  limits.find((l) => l.units.includes(unit));

/** Units of one plastic in the staged builds, `placed` per build (a pair of fighters is uncapped anyway). */
export function stagedOfLimit(
  limit: UnitLimit,
  options: readonly ChoiceOption[],
  draft: Readonly<Record<string, number>>,
): number {
  return options.reduce((sum, opt) => {
    const unit = String(opt.payload?.unit ?? opt.id);
    if (!limit.units.includes(unit)) return sum;
    const placed = opt.payload?.placed;
    return sum + (draft[opt.id] ?? 0) * (typeof placed === "number" && placed > 0 ? placed : 1);
  }, 0);
}

/** Units of one plastic still waiting in the shell's build queue. */
export function queuedOfLimit(limit: UnitLimit, queuedUnits: readonly string[]): number {
  return queuedUnits.filter((unit) => limit.units.includes(unit)).length;
}

export interface UnitLimitView {
  limit: number;
  /** On the board now plus staged and queued builds. */
  onBoard: number;
  left: number;
  staged: number;
  atLimit: boolean;
}

export function unitLimitView(limit: UnitLimit, staged: number, queued = 0): UnitLimitView {
  const pending = staged + queued;
  const left = Math.max(0, limit.remaining - pending);
  return { limit: limit.limit, onBoard: limit.held, left, staged: pending, atLimit: left === 0 };
}

const NAMES: Record<string, string> = { warsun: "war sun", spacedock: "space dock", pds: "PDS" };
export const limitUnitName = (base: string): string => NAMES[base] ?? base;

/** "Carrier 2 on board · 2 left (limit 4)"; staged builds are named when they use some of the rest. */
export function unitLimitText(name: string, view: UnitLimitView): string {
  const left = view.left === 0 ? "none left" : `${view.left} left`;
  const staged = view.staged > 0 ? ` · ${view.staged} in this build` : "";
  return `${name} ${view.onBoard} on board${staged} · ${left} (limit ${view.limit})`;
}

export const limitReachedText = (base: string): string =>
  `Limit reached: no ${limitUnitName(base)} left in reinforcements`;
