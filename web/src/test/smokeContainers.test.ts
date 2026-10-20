import { describe, expect, it } from "vitest";
import type { PendingChoiceDto } from "../protocol/types.ts";
import { isRetreatPickChoice } from "../presentation/retreatPick.ts";
import { DECISION_CONTAINERS } from "../../e2e/smokePolicy.ts";

describe("DECISION_CONTAINERS (smoke harness, every retreat pick)", () => {
  it("scans the retreat bar for each question it stands in for", () => {
    // Runs o3-01-2331, o2-04-2336, o3-03-2339 (retreat_to) and o4-06-2338 (Skilled Retreat):
    // all four subtypes render RetreatPickBar, whose Confirm the driver never saw.
    for (const subtype of ["retreat_to", "skilled_retreat_choose_system", "foresight_destination", "ralnel_retreat_destination"]) {
      const choice = { options: [], context: { subtype } } as unknown as PendingChoiceDto;
      expect(isRetreatPickChoice(choice), subtype).toBe(true);
    }
    expect(DECISION_CONTAINERS).toContain("retreat-pick-bar");
  });
});
