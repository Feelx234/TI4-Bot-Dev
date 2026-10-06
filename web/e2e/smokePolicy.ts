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
