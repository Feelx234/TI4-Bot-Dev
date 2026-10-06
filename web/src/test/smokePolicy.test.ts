import { describe, expect, it } from "vitest";
import {
  activationWeight,
  preferHitConfirm,
  preferTokenConfirm,
  preferPayment,
  steerWeight,
  strongUnselected,
  isUnstage,
} from "../../e2e/smokePolicy.ts";

const c = (desc: string, checked = false) => ({ desc, checked });

describe("preferPayment (smoke harness payment policy)", () => {
  it("presses the confirm button as soon as it is enabled", () => {
    const confirm = c("confirm-payment-btn | Confirm payment");
    const picked = preferPayment([
      c("planet-card-exhaust|jord | Jord", true),
      confirm,
      c("trade-goods-inc"),
    ]);
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
    const list = [
      c("choice-option | take a tactical action"),
      c("submit-choice-button | Confirm choice"),
    ];
    expect(preferPayment(list)).toEqual(list);
  });
});

describe("steerWeight (smoke harness steering)", () => {
  it("strongly prefers lifting the custodians over leaving them", () => {
    const yes = steerWeight("choice-option | remove it for a victory point");
    const no = steerWeight("choice-option | leave it");
    expect(yes).toBeGreaterThanOrEqual(50);
    expect(no).toBeLessThan(0.1);
  });

  it("loads cargo more readily than it moves more ships", () => {
    expect(
      steerWeight("rally-inc-cargo-65-infantry-space | +"),
    ).toBeGreaterThan(steerWeight("rally-inc-65-carrier | +"));
  });

  it("keeps a raider's ships in Mecatol", () => {
    expect(steerWeight("rally-inc-18-carrier | +")).toBeLessThan(
      steerWeight("rally-inc-65-carrier | +"),
    );
    expect(steerWeight("rally-inc-cargo-18-infantry-space | +")).toBe(25);
  });

  it("rarely spends a raider's trade goods on Munitions Reserves rerolls", () => {
    expect(
      steerWeight("choice-option | reroll this round's misses TG 12 → 10"),
    ).toBeLessThan(0.2);
  });

  it("weighs unknown controls 1", () => {
    expect(steerWeight("choice-option | something else")).toBe(1);
  });
});

describe("activationWeight (smoke harness steering)", () => {
  it("favours Mecatol and enemy systems, and avoids unreachable ones", () => {
    expect(activationWeight("18", true, false, false)).toBe(40);
    expect(activationWeight("35", true, true, false)).toBe(30);
    expect(activationWeight("35", true, false, false)).toBe(5);
    expect(activationWeight("35", false, true, false)).toBe(0.2);
  });

  it("makes a seat with ground forces waiting in Mecatol activate it in place", () => {
    // Dominant over ~35 other activations, each weighing at most 30.
    expect(activationWeight("18", false, false, true)).toBeGreaterThanOrEqual(
      35 * 30,
    );
    // Only Mecatol, and only with ground forces there.
    expect(activationWeight("35", false, false, true)).toBe(0.2);
    expect(activationWeight("18", false, false, false)).toBe(0.2);
  });
});

describe("strongUnselected (smoke harness steering)", () => {
  const no = c("choice-option | leave it VP 0 → 0", true);
  const yes = c("choice-option | remove it for a victory point VP 0 → 1");

  it("chooses the custodians removal before submitting the preselected 'no'", () => {
    expect(strongUnselected([no, yes])).toEqual(yes);
  });

  it("lets the submit go ahead once the strong option is selected", () => {
    expect(
      strongUnselected([
        { ...no, checked: false },
        { ...yes, checked: true },
      ]),
    ).toBeUndefined();
  });

  it("ignores ordinary options", () => {
    expect(
      strongUnselected([
        c("choice-option | end your turn"),
        c("rally-inc-cargo-1-infantry-space | +"),
      ]),
    ).toBeUndefined();
  });
});

describe("isUnstage (smoke harness)", () => {
  it("never treats a choice option as taking staging back", () => {
    expect(
      isUnstage("choice-option | remove it for a victory point VP 0 → 1"),
    ).toBe(false);
  });

  it("treats the invasion overlay's bare custodians button as an option", () => {
    expect(isUnstage("remove it for a victory point")).toBe(false);
    expect(steerWeight("remove it for a victory point")).toBeGreaterThan(
      50 * steerWeight("leave it"),
    );
  });

  it("still recognises real take-back controls", () => {
    expect(isUnstage("rally-dec-18-carrier | −")).toBe(true);
    expect(isUnstage("produce-dec | Remove Fighter (0.5 cost)")).toBe(true);
    expect(isUnstage("reset-selection-btn | Reset selection")).toBe(true);
  });
});

describe("preferHitConfirm", () => {
  it("confirms the staged hits once every hit is assigned", () => {
    const confirm = c("hit-confirm | Confirm hits");
    expect(
      preferHitConfirm([
        c("hit-destroy-fighter|intact | Destroy Fighter"),
        confirm,
      ]),
    ).toEqual([confirm]);
  });

  it("keeps every staging control while hits are left", () => {
    const stages = [
      c("hit-destroy-fighter|intact | +"),
      c("hit-auto-assign | Auto-assign"),
    ];
    expect(preferHitConfirm(stages)).toEqual(stages);
  });

  it("counts the minus control as taking a hit back", () => {
    expect(
      isUnstage("hit-remove-fighter|intact | Remove a hit from Fighters | −"),
    ).toBe(true);
    expect(isUnstage("hit-destroy-fighter|intact | Destroy Fighters | +")).toBe(
      false,
    );
  });
});

describe("preferTokenConfirm", () => {
  it("confirms once every token is assigned", () => {
    const confirm = c("token-confirm | Confirm tokens");
    expect(
      preferTokenConfirm([c("token-minus-fleet | Remove a token from Fleet | −"), confirm]),
    ).toEqual([confirm]);
  });

  it("only adds tokens while some are left, so staging cannot loop", () => {
    const plus = c("token-plus-fleet | Add a token to Fleet | +");
    expect(
      preferTokenConfirm([
        c("token-minus-fleet | Remove a token from Fleet | −"),
        c("token-reset | Reset"),
        plus,
      ]),
    ).toEqual([plus]);
  });

  it("leaves other decisions alone", () => {
    const stages = [c("choice-option | tactic pool")];
    expect(preferTokenConfirm(stages)).toEqual(stages);
  });
});
