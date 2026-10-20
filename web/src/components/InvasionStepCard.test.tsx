import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { InvasionStepView, PlacedUnitView } from "../protocol/types.ts";
import { InvasionStepCard } from "./InvasionStepCard.tsx";

const unit = (owner: string, unit_type: string): PlacedUnitView => ({ owner, unit_type, damaged: false, planet: "jord", galvanized: false });
const die = (face: number) => ({ planet: "jord", player: "inv", group: "dreadnought", face, target: 5, hit: face >= 5 });

const step = (applied: number): InvasionStepView => ({
  planet: "jord",
  kind: "bombardment",
  round: 0,
  before: [unit("rival", "infantry"), unit("rival", "infantry"), unit("rival", "infantry"), unit("rival", "infantry")],
  after: [unit("rival", "infantry")],
  dice: [die(9), die(7), die(2)],
  hits: { inv: applied },
  harrow_hits: 0,
});

describe("InvasionStepCard bombardment hits", () => {
  it("explains X-89 Bacterial Weapon doubling", () => {
    render(<InvasionStepCard step={step(4)} invader="inv" />);
    expect(screen.getByTestId("invasion-step-multiplier").textContent).toContain("each hit counts as 2");
    expect(screen.getByTestId("invasion-step-multiplier").textContent).toContain("2 rolled hits = 4 hits to assign");
    expect(screen.getByText("2 × 2 = 4 hits")).toBeTruthy();
  });

  it("says nothing extra without the technology", () => {
    render(<InvasionStepCard step={step(2)} invader="inv" />);
    expect(screen.queryByTestId("invasion-step-multiplier")).toBeNull();
  });
});
