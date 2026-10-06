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
  [/\| leave it$/i, 0.05],
  // Carry ground forces along: a carrier that moves empty can never invade, and the batch
  // declines an unplanned cargo hold, so the infantry stays behind (27.2a then forbids the
  // custodians and Mecatol stays unlanded).
  [/^rally-inc-cargo-/, 25],
  [/^rally-inc-/, 10],
  [/ in space/i, 10],
  // Finishing while moves are staged throws them away, so a populated commit wins.
  [/^commit-moves-btn \| Commit Moves/, 50],
  [/finish-movement-btn|done committing/i, 0.05],
];

/** Steering weight of one control, by its description (`<testid> | <label>`). */
export function steerWeight(desc: string): number {
  return STEER_WEIGHTS.find(([pattern]) => pattern.test(desc))?.[1] ?? 1;
}

/**
 * Steering weight for activating one system. Unreachable systems are rarely worth it; Mecatol Rex
 * and systems holding other players' units are favoured. A seat whose ground forces already stand
 * in Mecatol (the combat start preset) can activate it in place and lift the custodians without
 * moving, so that counts as reachable even though no ship can "move" there.
 */
export function activationWeight(
  id: string,
  reachable: boolean,
  hasEnemies: boolean,
  groundForcesInPlaceOnMecatol: boolean,
): number {
  if (id === "18" && groundForcesInPlaceOnMecatol) return 40;
  if (!reachable) return 0.2;
  return id === "18" ? 40 : hasEnemies ? 30 : 5;
}
