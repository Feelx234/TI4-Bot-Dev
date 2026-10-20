import { describe, expect, it } from "vitest";
import type { BoardView, PendingChoiceDto } from "../protocol/types.ts";
import { describeRetreatAnnouncement, describeRetreatPick, isRetreatPickChoice, retreatOptionSystem } from "./retreatPick.ts";
import { isSystemPickChoice } from "./systemFacts.ts";
import { resolveMapTargetSelection } from "./planetSelection.ts";

const unit = (unit_type: string, owner = "me", planet: string | null = null) => ({ unit_type, owner, damaged: false, planet });
const board = {
  map_tiles: [
    { system_id: "18", label: "Mecatol Rex", planets: [{ id: "mr" }] },
    { system_id: "26", label: "Lodor", planets: [{ id: "lodor" }], anomalies: [] },
    { system_id: "41", label: "Gravity Rift", planets: [], anomalies: ["gravity_rift"] },
  ],
  systems: {
    "18": { planets: {}, command_tokens: [], units: [unit("cruiser"), unit("cruiser"), unit("infantry", "me", "x"), unit("destroyer", "rival")] },
    "26": { planets: { lodor: { controlled_by: "me" } }, command_tokens: [], units: [unit("carrier")] },
    "41": { planets: {}, command_tokens: [], units: [] },
  },
} as unknown as BoardView;

const choice = (subtype: string, options: PendingChoiceDto["options"], target?: string): PendingChoiceDto =>
  ({
    prompt: "x",
    actor: "me",
    nonce: "n",
    options,
    context: { subtype, ...(target ? { target: { System: target } } : {}) },
  }) as never;

const retreatTo = choice(
  "retreat_to",
  [
    { id: "26", kind: "retreat_to", label: "retreat to 26", payload: { system: "26" } },
    { id: "41", kind: "retreat_to", label: "retreat to 41", payload: { system: "41" } },
  ],
  "18",
);
const foresight = choice(
  "foresight_destination",
  [{ id: "system|26", kind: "system", label: "move your ships from 18 to 26" }],
);
const ralnel = choice("ralnel_retreat_destination", [{ id: "to|26", kind: "retreat", label: "retreat up to 2 ships to 26" }], "18");

describe("retreat destination picks", () => {
  it("names the ships and the system they leave in the heading", () => {
    const pick = describeRetreatPick(retreatTo, board)!;
    expect(pick.kind).toBe("combat");
    expect(pick.fromId).toBe("18");
    expect(pick.title).toBe("Retreat 2 cruisers from Mecatol Rex (System 18)");
    expect(pick.fleet).toBe("2 cruisers");
  });

  it("counts a mixed fleet instead of listing it in the heading", () => {
    const big = {
      ...board,
      systems: { ...board.systems, "18": { ...board.systems["18"], units: [unit("cruiser"), unit("carrier"), unit("fighter"), unit("dreadnought")] } },
    } as BoardView;
    expect(describeRetreatPick(retreatTo, big)!.title).toBe("Retreat your fleet (4 units) from Mecatol Rex (System 18)");
  });

  it("lists the destinations with the facts the engine's offer rests on", () => {
    const pick = describeRetreatPick(retreatTo, board)!;
    expect(pick.destinations.map((d) => [d.optionId, d.systemId])).toEqual([["26", "26"], ["41", "41"]]);
    expect(pick.destinations[0].notes).toEqual(["your ships there: 1 carrier", "you control Lodor"]);
    expect(pick.destinations[1].notes).toEqual(["gravity rift"]);
    expect(pick.rule).toContain("asteroid field");
  });

  it("reads Foresight's origin from its label when the question carries no target", () => {
    const pick = describeRetreatPick(foresight, board)!;
    expect(pick.kind).toBe("foresight");
    expect(pick.fromId).toBe("18");
    expect(pick.title).toBe("Foresight: move 2 cruisers from Mecatol Rex (System 18)");
    expect(pick.destinations[0]).toMatchObject({ optionId: "system|26", systemId: "26" });
  });

  it("covers the Ral Nel commander and Skilled Retreat", () => {
    expect(describeRetreatPick(ralnel, board)!.title).toBe("Ral Nel commander: retreat up to 2 ships from Mecatol Rex (System 18)");
    const skilled = choice("skilled_retreat_choose_system", [{ id: "26", kind: "skilled_retreat", label: "withdraw to 26" }], "18");
    expect(describeRetreatPick(skilled, board)!.title).toBe("Skilled Retreat: withdraw 2 cruisers from Mecatol Rex (System 18)");
  });

  it("is not any other decision", () => {
    expect(describeRetreatPick(choice("activate_system", []), board)).toBeNull();
    expect(isRetreatPickChoice(choice("announce_retreat", []))).toBe(false);
    expect(isRetreatPickChoice(foresight)).toBe(true);
  });

  it("maps prefixed option ids to their system", () => {
    expect(retreatOptionSystem({ id: "system|26", kind: "system", label: "" })).toBe("26");
    expect(retreatOptionSystem({ id: "to|26", kind: "retreat", label: "" })).toBe("26");
    expect(retreatOptionSystem({ id: "26", kind: "retreat_to", label: "" })).toBe("26");
    expect(retreatOptionSystem({ id: "decline", kind: "decline", label: "" })).toBeNull();
  });

  it("makes the destinations a map pick whose click selects the option", () => {
    for (const c of [retreatTo, foresight, ralnel]) expect(isSystemPickChoice(c, board)).toBe(true);
    expect(resolveMapTargetSelection(foresight, "26", undefined, board)).toEqual({ kind: "select", optionId: "system|26", planetId: null });
    expect(resolveMapTargetSelection(ralnel, "26", undefined, board)).toEqual({ kind: "select", optionId: "to|26", planetId: null });
    expect(resolveMapTargetSelection(retreatTo, "41", undefined, board)).toEqual({ kind: "select", optionId: "41", planetId: null });
  });

  it("titles the retreat announcement with the fleet and the system", () => {
    const announce = choice("announce_retreat", [], "18");
    expect(describeRetreatAnnouncement(announce, board)).toEqual({
      title: "Announce a retreat from Mecatol Rex (System 18)?",
      fleet: "2 cruisers",
    });
    expect(describeRetreatAnnouncement(retreatTo, board)).toBeNull();
  });
});
