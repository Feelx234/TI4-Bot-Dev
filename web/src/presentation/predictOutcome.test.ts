import { describe, expect, it } from "vitest";
import { describePrediction, describeRider, isPredictOutcome } from "./predictOutcome.ts";
import type { DecisionTable } from "./politicsDecision.ts";

const predict = {
  context: { subtype: "predict_agenda_outcome" } as never,
  prompt: "Imperial Rider: predict the agenda outcome",
};
const table = {
  players: [{ id: "b", faction: "hacan", victory_points: 5 }],
  seating_order: ["b"],
  speaker: "b",
} as unknown as DecisionTable;

describe("predict outcome (the riders)", () => {
  it("recognises the decision and names the rider as printed", () => {
    expect(isPredictOutcome(predict)).toBe(true);
    const rider = describeRider(predict);
    expect(rider?.name).toBe("Imperial Rider");
    expect(rider?.text).toMatch(/predict/i);
    expect(describeRider({ ...predict, context: { subtype: "other" } as never })).toBeNull();
  });

  it("shows the Diplomacy Rider as printed when it asks which system, from its card source", () => {
    const pick = {
      context: { subtype: "diplomacy_rider_system", source: { ActionCard: "diplo_rider" } } as never,
      prompt: "Diplomacy Rider: choose a system",
    };
    const rider = describeRider(pick);
    expect(rider?.name).toBe("Diplomacy Rider");
    expect(rider?.step).toBe("reward");
    expect(rider?.text).toMatch(/choose 1 system that contains a planet you control/);
    expect(describeRider(predict)?.step).toBe("predict");
    expect(describeRider({ ...pick, context: { subtype: "x", source: { ActionCard: "sabotage" } } as never })).toBeNull();
  });

  it("puts an outcome in words: For/Against, a planet, a player", () => {
    expect(describePrediction(predict, { id: "FOR", label: "predict FOR" }, null)).toBe("For");
    expect(describePrediction(predict, { id: "against", label: "" }, null)).toBe("Against");
    expect(describePrediction(predict, { id: "jord", label: "" }, null)).toMatch(/^Jord \(\d+R\/\d+I\)$/);
    expect(describePrediction(predict, { id: "b", label: "" }, table)).toBe("Hacan (5 VP)");
    expect(describePrediction({ context: { subtype: "other" } as never }, { id: "FOR", label: "" }, null)).toBeNull();
  });
});
