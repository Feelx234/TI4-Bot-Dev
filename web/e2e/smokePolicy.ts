export interface PolicyCandidate {
  desc: string;
  /** A checked checkbox or radio; clicking it again would take the selection back. */
  checked?: boolean;
}

/**
 * Narrows the clickable controls while a payment drawer is open. Random clicking toggles planets
 * on and off, which never settles a bill that needs most sources (for example 3 owed with exactly
 * 3 available). So: once the confirm button is enabled, press it, and never un-toggle a planet that
 * is already staged while something else can still be clicked.
 */
export function preferPayment<T extends PolicyCandidate>(candidates: T[]): T[] {
  const confirm = candidates.find((c) => /^confirm-payment-btn\b/.test(c.desc));
  if (confirm) return [confirm];
  const kept = candidates.filter((c) => !(c.checked && /^planet-card-/.test(c.desc)));
  return kept.length ? kept : candidates;
}

// Steering weights, first match wins; anything unmatched weighs 1. They push random play toward
// moving fleets into contested systems and Mecatol Rex instead of passing and trading.
const STEER_WEIGHTS: [RegExp, number][] = [
  [/take a tactical action/i, 30],
  [/strategic action/i, 3],
  [/open a transaction|propose-trade-btn|trade-opt-/i, 0.1],
  [/decline-trade-btn/i, 5],
  [/\| pass$/i, 0.3],
  // 27.2: lifting the custodians opens the agenda phase, which random play otherwise never sees.
  [/remove it for a victory point/i, 60],
  [/(^|\| )leave it\b/i, 0.05],
  // Carry ground forces along: a carrier that moves empty can never invade, and the batch
  // declines an unplanned cargo hold, so the infantry stays behind (27.2a then forbids the
  // custodians and Mecatol stays unlanded).
  [/^rally-inc-cargo-/, 25],
  // A raider in Mecatol leaving it gives up the custodians it was placed there to lift.
  [/^rally-inc-18-/, 0.2],
  [/^rally-inc-/, 10],
  [/ in space/i, 10],
  // Finishing while moves are staged throws them away, so a populated commit wins.
  [/^commit-moves-btn \| Commit Moves/, 50],
  [/finish-movement-btn|done committing/i, 0.05],
];

const UNSTAGE = /reset|remove|rally-dec|decrement/i;
// The invasion overlay renders the custodians options as bare buttons (no test id), so the
// description is just the label.
const CUSTODIANS_YES = /remove it for a victory point/i;

/**
 * Controls that take back a staged selection. A choice-dialog option never does, and neither does
 * the custodians option, even though its text says "remove": "remove it for a victory point" was
 * treated as one, so it was almost never clicked and "no" was submitted instead.
 */
export function isUnstage(desc: string): boolean {
  if (/^choice-option\b/.test(desc) || CUSTODIANS_YES.test(desc)) return false;
  return UNSTAGE.test(desc);
}

/** Weight at which an option is worth choosing before anything is submitted. */
const STRONG = 50;

/**
 * A strongly preferred option that is not selected yet. Choice dialogs preselect their first
 * option, so submitting before choosing takes it: the custodians dialog lists "no" first, and a
 * 35% chance of an early submit kept the custodians on Mecatol.
 */
export function strongUnselected<T extends PolicyCandidate>(stages: T[]): T | undefined {
  return stages.find((c) => !c.checked && steerWeight(c.desc) >= STRONG);
}

/** Steering weight of one control, by its description (`<testid> | <label>`). */
export function steerWeight(desc: string): number {
  return STEER_WEIGHTS.find(([pattern]) => pattern.test(desc))?.[1] ?? 1;
}

/**
 * Steering weight for activating one system. Unreachable systems are rarely worth it; Mecatol Rex
 * and systems holding other players' units are favoured. A seat whose ground forces wait in
 * Mecatol's space area (the combat start preset) can activate it in place and lift the custodians
 * without moving, so that is weighted far above everything else.
 */
export function activationWeight(
  id: string,
  reachable: boolean,
  hasEnemies: boolean,
  groundForcesInPlaceOnMecatol: boolean,
): number {
  // Dominant: among ~35 other activations a weight of 40 was rarely picked, and the raider then
  // moved its ships out of Mecatol instead.
  if (id === "18" && groundForcesInPlaceOnMecatol) return 2000;
  if (!reachable) return 0.2;
  return id === "18" ? 40 : hasEnemies ? 30 : 5;
}
