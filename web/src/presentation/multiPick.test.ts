import { describe, expect, it } from "vitest";
import type { ChoiceOption, PendingChoiceDto } from "../protocol/types.ts";
import {
  coversExactly,
  describeMultiPick,
  multiPickAnswer,
  multiPickProblem,
  pickedCount,
  pickedWorth,
  setAtom,
  stepAtom,
} from "./multiPick.ts";

const pay = (id: string, worth: number): ChoiceOption => ({
  id,
  kind: "pay",
  label: id,
  payload: { worth, kind: "resources" },
});

const payment = (owed: number, goods = 3): Pick<PendingChoiceDto, "options" | "multi_pick"> => ({
  options: [pay("exhaust|jord", 4), pay("exhaust|moll_primus", 4), pay("exhaust|abyz", 3), pay("trade_good", 1)],
  multi_pick: { kind: "payment", owed, repeat: { trade_good: goods } },
});

describe("describeMultiPick", () => {
  it("is null for a question answered with one option", () => {
    expect(describeMultiPick({ options: [pay("trade_good", 1)] })).toBeNull();
  });

  it("reads the atoms, their worth and how often each may be named", () => {
    const view = describeMultiPick(payment(5))!;
    expect(view.rule).toBe("payment");
    expect(view.owed).toBe(5);
    expect(view.atoms.map((a) => [a.id, a.worth, a.max])).toEqual([
      ["exhaust|jord", 4, 1],
      ["exhaust|moll_primus", 4, 1],
      ["exhaust|abyz", 3, 1],
      ["trade_good", 1, 3],
    ]);
  });

  it("counts every atom 1 for a discard and leaves an unknown kind free", () => {
    const discard = describeMultiPick({
      options: [
        { id: "sabotage", kind: "card", label: "Sabotage", payload: {} },
        { id: "direct_hit", kind: "card", label: "Direct Hit", payload: {} },
      ],
      multi_pick: { kind: "discard", owed: 1, repeat: { sabotage: 2 } },
    })!;
    expect(discard.rule).toBe("discard");
    expect(discard.atoms.map((a) => [a.id, a.worth, a.max])).toEqual([
      ["sabotage", 1, 2],
      ["direct_hit", 1, 1],
    ]);
    expect(describeMultiPick({ options: [], multi_pick: { kind: "something_new", owed: 0 } })!.rule).toBe("free");
  });
});

describe("picking atoms", () => {
  it("toggles a single atom and steps a repeated one within its limit", () => {
    const view = describeMultiPick(payment(5, 2))!;
    let counts = stepAtom(view, {}, "exhaust|jord", 1);
    expect(counts).toEqual({ "exhaust|jord": 1 });
    counts = stepAtom(view, counts, "exhaust|jord", 1);
    expect(counts).toEqual({});
    counts = setAtom(view, counts, "trade_good", 5);
    expect(counts).toEqual({ trade_good: 2 });
    counts = stepAtom(view, counts, "trade_good", -1);
    expect(counts).toEqual({ trade_good: 1 });
    expect(setAtom(view, counts, "nope", 1)).toEqual(counts);
  });
});

describe("the payment rule as you type", () => {
  const view = describeMultiPick(payment(5))!;

  it("asks for something first, then for the rest of the bill", () => {
    expect(multiPickProblem(view, {}, 0, "resources")).toMatch(/Pick what to pay 5 resources/);
    expect(multiPickProblem(view, { "exhaust|jord": 1 }, 0, "resources")).toMatch(/Short by 1 resources/);
  });

  it("takes a set whose largest item completes the bill, even with overshoot", () => {
    // jord 4 + abyz 3 = 7 for 5: without jord, 3 is short.
    const counts = { "exhaust|jord": 1, "exhaust|abyz": 1 };
    expect(multiPickProblem(view, counts)).toBeNull();
    expect(pickedWorth(view, counts)).toBe(7);
  });

  it("refuses an item the bill did not need", () => {
    // 4 + 4 + 3 = 11 for 5: without one 4, 7 still covers it.
    const counts = { "exhaust|jord": 1, "exhaust|moll_primus": 1, "exhaust|abyz": 1 };
    expect(multiPickProblem(view, counts)).toMatch(/More than needed/);
    // 4 + 1 = 5: exact.
    expect(multiPickProblem(view, { "exhaust|jord": 1, trade_good: 1 })).toBeNull();
    // 4 + 3 goods = 7 for 5: without the 4 the goods (3) are short; the engine takes it.
    expect(multiPickProblem(view, { "exhaust|jord": 1, trade_good: 3 })).toBeNull();
  });

  it("answers with the atom ids in the listed order, a trade good once per good", () => {
    const counts = { trade_good: 2, "exhaust|abyz": 1 };
    expect(multiPickAnswer(view, counts)).toBe("exhaust|abyz+trade_good+trade_good");
    expect(pickedCount(view, counts)).toBe(3);
  });

  it("matches the engine's set_worth on plain worths", () => {
    expect(coversExactly([4, 3], 5)).toBe(true);
    expect(coversExactly([4, 4, 3], 5)).toBe(false);
    expect(coversExactly([2], 5)).toBe(false);
    expect(coversExactly([], 0)).toBe(false);
  });
});

describe("a purchase", () => {
  const view = describeMultiPick({
    options: [pay("exhaust|wellon", 2), pay("exhaust|mehar_xull", 3), pay("trade_good", 1)],
    multi_pick: { kind: "purchase", owed: 3, repeat: { trade_good: 2 }, buy: [0, 2] },
  })!;

  it("buying nothing spends nothing", () => {
    expect(multiPickProblem(view, {}, 0)).toBeNull();
    expect(multiPickAnswer(view, {}, 0)).toBe("buy:0");
    expect(multiPickProblem(view, { trade_good: 1 }, 0)).toMatch(/clear the selection/);
  });

  it("pays owed per token by the payment rule, and names the count first", () => {
    const counts = { "exhaust|wellon": 1, "exhaust|mehar_xull": 1, trade_good: 1 };
    expect(multiPickProblem(view, counts, 2)).toBeNull();
    expect(multiPickAnswer(view, counts, 2)).toBe("buy:2+exhaust|wellon+exhaust|mehar_xull+trade_good");
    expect(multiPickProblem(view, { "exhaust|wellon": 1, "exhaust|mehar_xull": 1 }, 2)).toMatch(/Short by 1/);
  });

  it("keeps the count within the question's range", () => {
    expect(multiPickProblem(view, {}, 3)).toMatch(/between 0 and 2/);
  });
});

describe("a discard", () => {
  const view = describeMultiPick({
    options: [
      { id: "sabotage", kind: "card", label: "Sabotage", payload: {} },
      { id: "direct_hit", kind: "card", label: "Direct Hit", payload: {} },
      { id: "skilled_retreat", kind: "card", label: "Skilled Retreat", payload: {} },
    ],
    multi_pick: { kind: "discard", owed: 2, repeat: { sabotage: 2 } },
  })!;

  it("needs exactly owed atoms, a duplicate card counted per copy", () => {
    expect(multiPickProblem(view, { sabotage: 1 })).toMatch(/Pick 1 more/);
    expect(multiPickProblem(view, { sabotage: 2 })).toBeNull();
    expect(multiPickProblem(view, { sabotage: 2, direct_hit: 1 })).toMatch(/take 1 out/);
    expect(multiPickAnswer(view, { sabotage: 1, skilled_retreat: 1 })).toBe("sabotage+skilled_retreat");
  });
});

describe("a removal", () => {
  it("names a unit kind up to the excess, any non-empty set goes to the engine", () => {
    const view = describeMultiPick({
      options: [
        { id: "remove|0", kind: "remove", label: "remove fighter", payload: {} },
        { id: "remove|1", kind: "remove", label: "remove destroyer", payload: {} },
      ],
      multi_pick: { kind: "remove", owed: 3 },
    })!;
    expect(view.rule).toBe("remove");
    expect(view.atoms.map((a) => a.max)).toEqual([3, 3]);
    expect(multiPickProblem(view, {})).toMatch(/at least one/);
    expect(multiPickProblem(view, { "remove|0": 1 })).toBeNull();
    expect(multiPickAnswer(view, { "remove|0": 2, "remove|1": 1 })).toBe("remove|0+remove|0+remove|1");
  });
});

describe("an unknown kind", () => {
  it("lets the player pick anything and leaves the rest to the engine", () => {
    const view = describeMultiPick({
      options: [{ id: "fighter|1", kind: "x", label: "a fighter", payload: {} }],
      multi_pick: { kind: "something_new", owed: 0 },
    })!;
    expect(multiPickProblem(view, {})).toMatch(/at least one/);
    expect(multiPickProblem(view, { "fighter|1": 1 })).toBeNull();
    expect(multiPickAnswer(view, { "fighter|1": 1 })).toBe("fighter|1");
  });
});
