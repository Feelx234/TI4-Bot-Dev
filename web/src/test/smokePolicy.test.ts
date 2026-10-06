import { describe, expect, it } from "vitest";
import { activationWeight, preferPayment } from "../../e2e/smokePolicy.ts";

const c = (desc: string, checked = false) => ({ desc, checked });

describe("preferPayment (smoke harness payment policy)", () => {
  it("presses the confirm button as soon as it is enabled", () => {
    const confirm = c("confirm-payment-btn | Confirm payment");
    const picked = preferPayment([c("planet-card-exhaust|jord | Jord", true), confirm, c("trade-goods-inc")]);
    expect(picked).toEqual([confirm]);
  });

  it("never un-toggles a staged planet while another control is available", () => {
    const staged = c("planet-card-exhaust|jord | Jord", true);
    const open = c("planet-card-exhaust|arinam | Arinam", false);
    expect(preferPayment([staged, open])).toEqual([open]);
  });

  it("falls back to the staged planet when it is the only control", () => {
    const staged = c("planet-card-exhaust|jord | Jord", true);
    expect(preferPayment([staged])).toEqual([staged]);
  });

  it("leaves non-payment controls alone", () => {
    const list = [c("choice-option | take a tactical action"), c("submit-choice-button | Confirm choice")];
    expect(preferPayment(list)).toEqual(list);
  });
});

describe("activationWeight (smoke harness steering)", () => {
  it("favours Mecatol and enemy systems, and avoids unreachable ones", () => {
    expect(activationWeight("18", true, false, false)).toBe(40);
    expect(activationWeight("35", true, true, false)).toBe(30);
    expect(activationWeight("35", true, false, false)).toBe(5);
    expect(activationWeight("35", false, true, false)).toBe(0.2);
  });

  it("lets a seat with ground forces already in Mecatol activate it in place", () => {
    expect(activationWeight("18", false, false, true)).toBe(40);
    // Only Mecatol, and only with ground forces there.
    expect(activationWeight("35", false, false, true)).toBe(0.2);
    expect(activationWeight("18", false, false, false)).toBe(0.2);
  });
});
