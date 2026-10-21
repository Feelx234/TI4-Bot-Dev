import type { BoardView, GameEvent, PublicRoll } from "../protocol/types.ts";
import { isPublicEvent } from "../protocol/eventVisibility.ts";
import { systemTag } from "./systemFacts.ts";

/** A die roll as a short corner notification. */
export interface RollToast {
  /** Id of the (first) event this toast came from. */
  id: string;
  /** Same roller, reason and system: later rolls join the toast while it is on screen. */
  key: string;
  who: string;
  rolls: PublicRoll[];
  text: string;
}

/** The engine tags a gravity-rift die "gravity rift". It is one die per ship, not a hit roll. */
export function isRiftRoll(roll: PublicRoll): boolean {
  return roll.reason === "gravity rift";
}

/** A rift die destroys the ship on 1-3 (41.2); the engine's outcome says which happened. */
function riftDestroyed(roll: PublicRoll): boolean {
  if (roll.outcome) return /destroy/i.test(roll.outcome);
  return (roll.faces[0] ?? 10) <= 3;
}

/**
 * "rolled a 2 at the gravity rift in #67 and the ship was destroyed, with anything it carried" /
 * "rolled a 7 at the gravity rift in #67 and the ship survived" (without the roller). The die is
 * never worded as a hit: a high face is the ship getting through.
 */
export function riftSentence(roll: PublicRoll, board?: BoardView | null): string {
  const place = roll.system ? ` in ${systemTag(roll.system, board)}` : "";
  const face = roll.faces[0];
  const die = face === undefined ? "the die" : `${face === 8 ? "an" : "a"} ${face}`;
  return `rolled ${die} at the gravity rift${place} and the ship ${riftDestroyed(roll) ? "was destroyed, with anything it carried" : "survived"}`;
}

/** "Gravity rift in #67 [Gravity Rift]: 2 · ship destroyed" (without the roller). */
export function rollSentence(roll: PublicRoll, board?: BoardView | null): string {
  const place = roll.system ? ` in ${systemTag(roll.system, board)}` : "";
  const faces = roll.faces.join(", ");
  const rerolled = roll.rerolled?.length ? " (reroll)" : "";
  const outcome = roll.outcome ? ` · ${roll.outcome}` : "";
  return `${roll.label}${rerolled}${place}: ${faces}${outcome}`;
}

function build(id: string, key: string, who: string, rolls: PublicRoll[], board?: BoardView | null): RollToast {
  // Gravity rifts: one line per ship, each saying what its die did.
  if (rolls.every(isRiftRoll)) {
    return { id, key, who, rolls, text: rolls.map((r) => `${who} ${riftSentence(r, board)}`).join("\n") };
  }
  if (rolls.length === 1) return { id, key, who, rolls, text: `${who} rolled ${rollSentence(rolls[0], board)}` };
  // Several rolls of one kind: show every die, and total the hits when they are hit rolls.
  const merged: PublicRoll = {
    ...rolls[0],
    faces: rolls.flatMap((r) => r.faces),
    rerolled: undefined,
    outcome: mergedOutcome(rolls),
  };
  return { id, key, who, rolls, text: `${who} rolled ${rollSentence(merged, board)}` };
}

/**
 * One toast per roller, reason and system among the newly arrived events: a combat round rolls
 * several dice at once, and a toast per die would bury the one that mattered. Rolls the server
 * did not mark public never reach here, but are skipped all the same.
 */
export function rollToasts(
  events: readonly GameEvent[],
  seatLabel: (seat: string) => string,
  board?: BoardView | null,
): RollToast[] {
  const groups = new Map<string, { id: string; rolls: PublicRoll[] }>();
  for (const entry of events) {
    if (entry.event.kind !== "dice_rolled" || !entry.roll || !isPublicEvent(entry)) continue;
    const roll = entry.roll;
    const key = `${roll.roller ?? ""}|${roll.reason}|${roll.system ?? ""}`;
    const group = groups.get(key);
    if (group) group.rolls.push(roll);
    else groups.set(key, { id: entry.id, rolls: [roll] });
  }
  return [...groups].map(([key, { id, rolls }]) =>
    build(id, key, rolls[0].roller ? seatLabel(rolls[0].roller) : "A roll", rolls, board),
  );
}

/**
 * Adds toasts to the visible ones. A roll of the same roller, reason and system as a toast still
 * on screen joins it (the events of one combat round reach the client one by one); at most `max`
 * stay, the oldest is dropped.
 */
export function mergeRollToasts(
  list: readonly RollToast[],
  incoming: readonly RollToast[],
  max: number,
  board?: BoardView | null,
): RollToast[] {
  let next = [...list];
  for (const toast of incoming) {
    const at = next.findIndex((t) => t.key === toast.key);
    if (at < 0) {
      next.push(toast);
      continue;
    }
    const prior = next.splice(at, 1)[0];
    const known = new Set(prior.rolls.map((r) => r.index));
    const rolls = [...prior.rolls, ...toast.rolls.filter((r) => !known.has(r.index))];
    next.push(build(prior.id, prior.key, prior.who, rolls, board));
  }
  return next.slice(-max);
}

function mergedOutcome(rolls: readonly PublicRoll[]): string | undefined {
  if (rolls.every((r) => r.hits !== undefined)) {
    const hits = rolls.reduce((sum, r) => sum + (r.hits ?? 0), 0);
    return hits === 0 ? "no hits" : hits === 1 ? "1 hit" : `${hits} hits`;
  }
  const outcomes = rolls.map((r) => r.outcome).filter((o): o is string => !!o);
  return outcomes.length ? outcomes.join("; ") : undefined;
}
