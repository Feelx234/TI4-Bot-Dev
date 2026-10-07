import { describe, expect, it } from "vitest";
import { BoardView, PendingChoiceDto } from "../protocol/types.ts";
import {
  describeSystem,
  isSystemPickChoice,
  planetOccupants,
  summarizeUnits,
} from "./systemFacts.ts";
import { buildBoardPresentationModel } from "./boardPresentation.ts";
import { resolveMapTargetSelection } from "./planetSelection.ts";

const board: BoardView = {
  systems: {
    "14": {
      system_id: "14",
      command_tokens: ["p2"],
      units: [
        { unit_type: "sol_carrier2", owner: "p1", damaged: false },
        { unit_type: "fighter", owner: "p1", damaged: false },
        { unit_type: "fighter", owner: "p1", damaged: false },
        { unit_type: "infantry", owner: "p1", planet: "arinam", damaged: false },
      ],
      planets: { arinam: { planet_id: "arinam", controlled_by: "p1", exhausted: false } },
    },
    "18": { system_id: "18", command_tokens: [], units: [], planets: {} },
  },
  map_tiles: [
    {
      system_id: "14",
      label: "Arinam",
      q: 0,
      r: 0,
      planets: [{ id: "arinam", label: "Arinam", resources: 1, influence: 2 }],
    },
    { system_id: "18", label: "Mecatol Rex", q: 1, r: 0, planets: [] },
  ],
};

const diplomacy: PendingChoiceDto = {
  nonce: "d1",
  actor: "p1",
  prompt: "choose a system",
  context: { subtype: "diplomacy_choose_system", source: { StrategyCard: { card: "Diplomacy", secondary: false } } },
  options: [
    { id: "14", kind: "system", label: "14" },
    { id: "18", kind: "system", label: "18" },
  ],
};

describe("system facts", () => {
  it("summarizes planets, ships and tokens of a system", () => {
    const facts = describeSystem("14", board);
    expect(facts.title).toBe("Arinam (#14)");
    expect(facts.planets[0]).toMatchObject({ name: "Arinam", resources: 1, influence: 2 });
    expect(facts.ships).toEqual([{ owner: "p1", summary: "1 carrier, 2 fighters" }]);
    expect(facts.commandTokens).toEqual(["p2"]);
    expect(describeSystem("18", board).title).toBe("Mecatol Rex (#18)");
  });

  it("names units and groups planet occupants", () => {
    expect(summarizeUnits([{ unit_type: "pds" }, { unit_type: "pds" }])).toBe("2 PDS");
    expect(planetOccupants("14", "arinam", board)).toEqual([{ owner: "p1", summary: "1 infantry" }]);
  });

  it("recognises a bare system pick, only when every option is a board system", () => {
    expect(isSystemPickChoice(diplomacy, board)).toBe(true);
    expect(isSystemPickChoice(diplomacy, null)).toBe(false);
    const bogus = { ...diplomacy, options: [{ id: "999", kind: "system", label: "999" }] };
    expect(isSystemPickChoice(bogus, board)).toBe(false);
    const other = { ...diplomacy, context: { subtype: "gain_command_token" } };
    expect(isSystemPickChoice(other, board)).toBe(false);
    // Unexpected Action: recall your own token from one of the systems that hold it.
    const unexpected = { ...diplomacy, context: { subtype: "unexpected_pick_recall" } };
    expect(isSystemPickChoice(unexpected, board)).toBe(true);
  });

  it("highlights the candidate systems and a map click selects the same option id", () => {
    const model = buildBoardPresentationModel(board, ["p1", "p2"], [], diplomacy, "p1");
    expect(model.tiles.filter((t) => t.isCandidateTarget).map((t) => t.systemId).sort()).toEqual([
      "14",
      "18",
    ]);
    expect(resolveMapTargetSelection(diplomacy, "14", undefined, board)).toEqual({
      kind: "select",
      optionId: "14",
      planetId: null,
    });
  });
});
