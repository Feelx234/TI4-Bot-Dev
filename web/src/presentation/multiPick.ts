import type { ChoiceOption, PendingChoiceDto } from "../protocol/types.ts";

/**
 * A multi-pick question (`Choice.multi_pick`): the answer is a SET of the listed options (the
 * "atoms"), their ids joined by `+`, an atom named in `repeat` once per unit (a trade good per good
 * spent). The engine checks the set; this module lets the player build it and checks it as they
 * type for the kinds whose rule is known:
 *
 * - `payment`: the atoms cover `owed`, and without the largest one the rest would be short;
 * - `purchase`: `buy:N` tokens of `owed` each (N within `buy`), paid by the same rule (nothing for none);
 * - `discard`: exactly `owed` atoms.
 *
 * Any other kind (`remove`, ...) is built freely and judged by the engine.
 */

export const MULTI_PICK_SEPARATOR = "+";
export const MULTI_PICK_BUY = "buy:";

export type MultiPickRule = "payment" | "purchase" | "discard" | "free";

export interface MultiPickAtom {
  id: string;
  option: ChoiceOption;
  /** What one unit of it is worth toward `owed` (payments and purchases); 1 for a discard. */
  worth: number;
  /** How many times it may be named (1 unless the engine lists it in `repeat`). */
  max: number;
}

export interface MultiPickView {
  kind: string;
  rule: MultiPickRule;
  owed: number;
  atoms: MultiPickAtom[];
  /** A purchase: the fewest and the most tokens. */
  buy: [number, number] | null;
}

/** How many of each atom are picked, by atom id. */
export type MultiPickCounts = Readonly<Record<string, number>>;

const ruleOf = (kind: string): MultiPickRule =>
  kind === "payment" || kind === "purchase" || kind === "discard" ? kind : "free";

const asWorth = (option: ChoiceOption): number => {
  const worth = Number(option.payload?.worth);
  return Number.isFinite(worth) && worth > 0 ? worth : 0;
};

/** The multi-pick of a question, or `null` for a question answered with one listed option. */
export function describeMultiPick(
  choice: Pick<PendingChoiceDto, "options" | "multi_pick">,
): MultiPickView | null {
  const multi = choice.multi_pick;
  if (!multi) return null;
  const rule = ruleOf(multi.kind);
  const repeat = multi.repeat ?? {};
  const atoms = choice.options
    .filter((option) => option.kind !== "decline")
    .map((option) => ({
      id: option.id,
      option,
      worth: rule === "payment" || rule === "purchase" ? asWorth(option) : 1,
      max: Math.max(1, Math.floor(Number(repeat[option.id] ?? 1)) || 1),
    }));
  const buy =
    multi.buy && multi.buy.length === 2 ? ([multi.buy[0], multi.buy[1]] as [number, number]) : null;
  return { kind: multi.kind, rule, owed: Math.max(0, multi.owed), atoms, buy };
}

/** The count of one atom after a tap: a single atom toggles, a repeated one steps by `delta`. */
export function stepAtom(view: MultiPickView, counts: MultiPickCounts, id: string, delta: number): Record<string, number> {
  const atom = view.atoms.find((candidate) => candidate.id === id);
  if (!atom) return { ...counts };
  const now = counts[id] ?? 0;
  const next = atom.max === 1 && delta > 0 && now === 1 ? 0 : now + delta;
  return setAtom(view, counts, id, next);
}

/** Sets the count of one atom (clamped to what it may be named). */
export function setAtom(view: MultiPickView, counts: MultiPickCounts, id: string, count: number): Record<string, number> {
  const atom = view.atoms.find((candidate) => candidate.id === id);
  const out: Record<string, number> = { ...counts };
  if (!atom) return out;
  const next = Math.max(0, Math.min(atom.max, Math.floor(count)));
  if (next === 0) delete out[id];
  else out[id] = next;
  return out;
}

/** Atoms picked in all (a trade good spent twice counts twice). */
export const pickedCount = (view: MultiPickView, counts: MultiPickCounts): number =>
  view.atoms.reduce((sum, atom) => sum + (counts[atom.id] ?? 0), 0);

/** What the picked atoms are worth toward the bill (a discard: how many are picked). */
export const pickedWorth = (view: MultiPickView, counts: MultiPickCounts): number =>
  view.atoms.reduce((sum, atom) => sum + atom.worth * (counts[atom.id] ?? 0), 0);

/** What the set must cover: `owed`, or for a purchase `owed` per token bought. */
export const multiPickBill = (view: MultiPickView, bought = 0): number =>
  view.rule === "purchase" ? view.owed * bought : view.owed;

/** The largest single unit picked (the atom the rule may leave out last). */
const largestPicked = (view: MultiPickView, counts: MultiPickCounts): MultiPickAtom | null =>
  view.atoms
    .filter((atom) => (counts[atom.id] ?? 0) > 0)
    .reduce<MultiPickAtom | null>((best, atom) => (!best || atom.worth > best.worth ? atom : best), null);

const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;

/**
 * Why the engine would refuse the picked set, in plain words; `null` when it would take it (or,
 * for a kind whose rule is unknown, when something is picked: the engine judges the rest).
 */
export function multiPickProblem(
  view: MultiPickView,
  counts: MultiPickCounts,
  bought = 0,
  unit = "",
): string | null {
  const picked = pickedCount(view, counts);
  const worth = pickedWorth(view, counts);
  const u = unit ? ` ${unit}` : "";
  switch (view.rule) {
    case "discard": {
      if (picked < view.owed) return `Pick ${view.owed - picked} more (${view.owed} in all).`;
      if (picked > view.owed) return `Pick only ${view.owed}: take ${picked - view.owed} out.`;
      return null;
    }
    case "purchase": {
      if (view.buy && (bought < view.buy[0] || bought > view.buy[1])) {
        return view.buy[0] === view.buy[1]
          ? `Buy exactly ${plural(view.buy[0], "token")}.`
          : `Buy between ${view.buy[0]} and ${view.buy[1]} tokens.`;
      }
      if (bought === 0) return picked === 0 ? null : "Buying nothing spends nothing: clear the selection.";
      return coverProblem(view, counts, multiPickBill(view, bought), worth, u);
    }
    case "payment":
      if (picked === 0) return `Pick what to pay ${view.owed}${u} with.`;
      return coverProblem(view, counts, view.owed, worth, u);
    case "free":
      return picked === 0 ? "Pick at least one." : null;
  }
}

function coverProblem(
  view: MultiPickView,
  counts: MultiPickCounts,
  bill: number,
  worth: number,
  u: string,
): string | null {
  if (worth < bill) return `Short by ${bill - worth}${u}: pick more.`;
  const largest = largestPicked(view, counts);
  if (largest && worth - largest.worth >= bill) {
    return `More than needed: the bill is ${bill}${u} and this pays ${worth}. Without ${largest.option.label || largest.id} (${largest.worth}) the rest still covers it; take something out.`;
  }
  return null;
}

/** The one answer for the picked set: atom ids in the listed order, repeated per unit, joined by `+`. */
export function multiPickAnswer(view: MultiPickView, counts: MultiPickCounts, bought?: number): string {
  const parts: string[] = [];
  if (view.rule === "purchase" && bought !== undefined && bought > 0) parts.push(`${MULTI_PICK_BUY}${bought}`);
  for (const atom of view.atoms) {
    for (let i = 0; i < (counts[atom.id] ?? 0); i += 1) parts.push(atom.id);
  }
  if (view.rule === "purchase" && parts.length === 0) return `${MULTI_PICK_BUY}0`;
  return parts.join(MULTI_PICK_SEPARATOR);
}

/**
 * The payment rule on plain worths (the engine's `set_worth`): the set covers `owed` and without
 * its largest item the rest would be short. Shared by the payment drawer and the token panel.
 */
export function coversExactly(worths: readonly number[], owed: number): boolean {
  if (worths.length === 0) return false;
  const sum = worths.reduce((a, b) => a + b, 0);
  return sum >= owed && sum - Math.max(...worths) < owed;
}
