import { describe, expect, it } from "vitest";
import type { GameEvent, PublicRoll } from "../protocol/types.ts";
import { rollSentence, rollToasts } from "./rollToasts.ts";

const rift: PublicRoll = {
  index: 25,
  roller: "np",
  reason: "gravity rift",
  label: "Gravity rift",
  faces: [2],
  hits_on: 4,
  hits: 0,
  system: "67",
  outcome: "ship destroyed",
};

let n = 0;
const rolled = (roll: PublicRoll, extra: Partial<GameEvent> = {}): GameEvent => {
  n += 1;
  return {
    id: `r${n}`,
    timestamp: "1",
    visibility: { visibility: "public" },
    event: { kind: "dice_rolled" },
    actor: roll.roller,
    detail: "unused here",
    roll,
    ...extra,
  };
};
const label = (seat: string) => (seat === "np" ? "np" : seat);

describe("dice roll toasts", () => {
  it("names who rolled, what for, the system, the die and what it did", () => {
    const [toast] = rollToasts([rolled(rift)], label);
    expect(toast.text).toBe("np rolled Gravity rift in #67: 2 · ship destroyed");
  });

  it("merges the dice of one combat round into a single toast with the hits totalled", () => {
    const combat = (faces: number[], hits: number): PublicRoll => ({
      index: 1,
      roller: "p1",
      reason: "space combat",
      label: "Space combat",
      faces,
      hits_on: 7,
      hits,
      system: "18",
      outcome: hits === 0 ? "no hits" : hits === 1 ? "1 hit" : `${hits} hits`,
    });
    const toasts = rollToasts([rolled(combat([8, 2], 1)), rolled(combat([10], 1)), rolled({ ...combat([3], 0), roller: "p2" })], label);
    expect(toasts.map((t) => t.text)).toEqual([
      "p1 rolled Space combat in #18 [Mecatol Rex]: 8, 2, 10 \u00b7 2 hits",
      "p2 rolled Space combat in #18 [Mecatol Rex]: 3 \u00b7 no hits",
    ]);
  });

  it("marks rerolls and tolerates a roll nobody owns", () => {
    const text = rollSentence({ index: 0, reason: "x", label: "Sanctions", faces: [4], rerolled: [0] });
    expect(text).toBe("Sanctions (reroll): 4");
    const [toast] = rollToasts([rolled({ index: 0, reason: "x", label: "Sanctions", faces: [4] })], label);
    expect(toast.text).toBe("A roll rolled Sanctions: 4");
  });

  it("ignores events that are not public dice rolls", () => {
    const decision: GameEvent = { ...rolled(rift), event: { kind: "decision_resolved" } };
    const hidden: GameEvent = { ...rolled(rift), visibility: { visibility: "seat", seat: "p1" } };
    const bare: GameEvent = { ...rolled(rift), roll: undefined };
    expect(rollToasts([decision, hidden, bare], label)).toEqual([]);
  });
});
