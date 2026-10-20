import { describe, expect, it } from "vitest";
import table from "../../../crates/ti4-engine/testdata/tech_waiver_parity.json";
import {
  TECH_TRACKS,
  techMatches,
  hasResearchedTech,
  aidaIsReady,
  secondPickPrerequisitesMet,
  checkTechPrerequisites,
  hydrateTech,
  psychoarchaeologyFrees,
  unlockedByAida,
  visibleWaivers,
  getControlledSpecialtyPlanets,
  skipCounts,
} from "./technologyData.ts";
import type { BoardView, PlayerView } from "../protocol/types.ts";
import { PLANETS } from "../protocol/contentCatalog.ts";

const none = { PROPULSION: 0, BIOTIC: 0, CYBERNETIC: 0, WARFARE: 0 };
const player = (technologies: string[], exhausted: string[] = [], extra: Partial<PlayerView> = {}) =>
  ({
    id: "p1",
    faction: "sol",
    technologies,
    exhausted_technologies: exhausted,
    relics: [],
    exhausted_relics: [],
    ...extra,
  }) as unknown as PlayerView;

describe("AI Development Algorithm in the client prerequisite check", () => {
  // Dreadnought II needs B B Y; the player has one blue and one yellow: one short.
  const dn2 = hydrateTech("dn2");
  const owned = { ...none, PROPULSION: 1, CYBERNETIC: 1 };
  const ready = visibleWaivers(player(["aida", "amd", "st"]), 15);

  it("unlocks a unit upgrade with one missing prerequisite", () => {
    expect(checkTechPrerequisites(dn2, owned, none, "sol", {})).toBe(false);
    expect(checkTechPrerequisites(dn2, owned, none, "sol", ready)).toBe(true);
    expect(unlockedByAida(dn2, owned, none, "sol", ready)).toBe(true);
  });

  it("does not unlock a unit upgrade that is two short", () => {
    expect(checkTechPrerequisites(dn2, { ...none, CYBERNETIC: 1 }, none, "sol", ready)).toBe(false);
  });

  it("does not unlock a technology that is not a unit upgrade", () => {
    const x89 = hydrateTech("x89c4"); // G G G
    expect(checkTechPrerequisites(x89, { ...none, BIOTIC: 2 }, none, "sol", ready)).toBe(false);
    const gls = hydrateTech("gls"); // Y
    expect(checkTechPrerequisites(gls, none, none, "sol", ready)).toBe(false);
  });

  it("does nothing while the card is exhausted", () => {
    const spent = visibleWaivers(player(["aida", "amd", "st"], ["aida"]), 15);
    expect(spent.aida).toBe(false);
    expect(checkTechPrerequisites(dn2, owned, none, "sol", spent)).toBe(false);
    expect(unlockedByAida(dn2, owned, none, "sol", spent)).toBe(false);
  });

  it("does nothing before rules version 15", () => {
    expect(aidaIsReady(player(["aida"]), 14)).toBe(false);
    expect(visibleWaivers(player(["aida"]), 14).aida).toBe(false);
    expect(aidaIsReady(player(["aida"]), 15)).toBe(true);
  });

  it("is not there without the card", () => {
    expect(visibleWaivers(player(["amd", "st"]), 15).aida).toBe(false);
  });

  it("is not counted as the unlock when the player already has every prerequisite", () => {
    expect(unlockedByAida(dn2, { ...none, PROPULSION: 2, CYBERNETIC: 1 }, none, "sol", ready)).toBe(false);
  });

  it("a toggled specialty and aida together cover two missing prerequisites", () => {
    expect(checkTechPrerequisites(dn2, { ...none, PROPULSION: 1 }, { ...none, CYBERNETIC: 1 }, "sol", ready)).toBe(true);
  });

  it("Jol-Nar keeps its own budget on top (two on a plain technology)", () => {
    const x89 = hydrateTech("x89c4");
    expect(checkTechPrerequisites(x89, { ...none, BIOTIC: 1 }, none, "jolnar", {})).toBe(true);
    expect(checkTechPrerequisites(x89, none, none, "jolnar", {})).toBe(false);
    expect(checkTechPrerequisites(x89, none, none, "sol", {})).toBe(false);
  });

  it("The Prophet's Tears adds one waiver", () => {
    const tears = visibleWaivers(player([], [], { relics: ["prophetstears"] }), 10);
    expect(checkTechPrerequisites(hydrateTech("gls"), none, none, "sol", tears)).toBe(true);
  });
});

describe("Psychoarchaeology", () => {
  it("frees specialties from rules version 15 only, and only when owned", () => {
    expect(psychoarchaeologyFrees(player(["pa"]), 15)).toBe(true);
    expect(psychoarchaeologyFrees(player(["pa"]), 14)).toBe(false);
    expect(psychoarchaeologyFrees(player([]), 15)).toBe(false);
  });
});

/**
 * The same table the engine's `tech_picker_parity_table_matches_can_research` asserts against
 * `can_research`: the client's availability (owned tracks + every usable specialty + the waivers
 * it can see) must equal the engine's, for each scenario.
 */
describe("parity with the engine's can_research", () => {
  type Scenario = {
    name: string;
    rules_version: number;
    owned: string[];
    exhausted?: string[];
    specialties?: {
      colour: "propulsion" | "biotic" | "cybernetic" | "warfare";
      exhausted: boolean;
      /** A research facility on a bare planet, or a facility's +1/+1 on a planet printing the colour. */
      via?: "facility" | "facility_on_specialty";
    }[];
    target: string;
    researchable: boolean;
    then_target?: string;
    then_researchable?: boolean;
  };
  const client = (sc: Scenario, second = false): boolean => {
    const me = player(sc.owned, sc.exhausted ?? []);
    if (hasResearchedTech(me, sc.target)) return false;
    const owned = { ...none };
    for (const track of TECH_TRACKS) {
      owned[track.id] = sc.owned.filter((id) => track.techIds.some((t) => techMatches(id, t))).length;
    }
    const free = psychoarchaeologyFrees(me, sc.rules_version);
    // The skips come from the board the picker reads: planets with their printed specialty and
    // attachments, through the same functions the picker uses.
    const used = new Set<string>();
    const planets: Record<string, unknown> = {};
    for (const spec of sc.specialties ?? []) {
      const printed = spec.via !== "facility";
      const meta = Object.values(PLANETS).find(
        (p) =>
          !used.has(p.id) &&
          (printed
            ? p.techSpecialties?.length === 1 && p.techSpecialties[0].toLowerCase() === spec.colour
            : !p.techSpecialties?.length),
      )!;
      used.add(meta.id);
      planets[meta.id] = {
        planet_id: meta.id,
        controlled_by: "p1",
        exhausted: spec.exhausted,
        attachments: spec.via === "facility" ? [spec.colour] : spec.via ? ["warfarestat"] : [],
      };
    }
    const board = { systems: { s: { planets } } } as unknown as BoardView;
    const skips = skipCounts(getControlledSpecialtyPlanets(board, "p1"), (p) => free || !p.isExhausted);
    const waivers = visibleWaivers(me, sc.rules_version);
    if (second) {
      return secondPickPrerequisitesMet(
        sc.target,
        hydrateTech(sc.then_target!),
        owned,
        skips,
        skips,
        "sol",
        waivers,
        sc.rules_version,
      );
    }
    return checkTechPrerequisites(hydrateTech(sc.target), owned, skips, "sol", waivers);
  };
  for (const sc of table.scenarios as Scenario[]) {
    it(sc.name, () => {
      expect(client(sc)).toBe(sc.researchable);
      if (sc.then_target) expect(client(sc, true)).toBe(sc.then_researchable);
    });
  }
});

describe("technology skips of a planet with a research facility", () => {
  const bare = Object.values(PLANETS).find((p) => !p.techSpecialties?.length)!;
  const blue = Object.values(PLANETS).find((p) => p.techSpecialties?.length === 1 && p.techSpecialties[0].toLowerCase() === "propulsion")!;
  const boardOf = (planets: Record<string, { attachments: string[]; exhausted?: boolean }>) =>
    ({
      systems: {
        s: {
          planets: Object.fromEntries(
            Object.entries(planets).map(([id, p]) => [id, { planet_id: id, controlled_by: "p1", exhausted: false, ...p }]),
          ),
        },
      },
    }) as unknown as BoardView;

  it("every facility colour is a skip of that colour on a planet without a specialty", () => {
    for (const [attachment, track] of [
      ["warfare", "WARFARE"],
      ["propulsion", "PROPULSION"],
      ["cybernetic", "CYBERNETIC"],
      ["biotic", "BIOTIC"],
    ] as const) {
      const found = getControlledSpecialtyPlanets(boardOf({ [bare.id]: { attachments: [attachment] } }), "p1");
      expect(found).toHaveLength(1);
      expect(found[0].skips).toEqual([{ specialty: track, fromAttachment: attachment }]);
    }
  });

  it("shows the facility's colour next to the planet's own specialty when both are present", () => {
    const found = getControlledSpecialtyPlanets(boardOf({ [blue.id]: { attachments: ["warfare"] } }), "p1");
    expect(found[0].skips).toEqual([
      { specialty: "PROPULSION", fromAttachment: undefined },
      { specialty: "WARFARE", fromAttachment: "warfare" },
    ]);
    const counts = skipCounts(found, () => true);
    expect(counts.PROPULSION).toBe(1);
    expect(counts.WARFARE).toBe(1);
  });

  it("the +1/+1 facility record on a planet with a specialty adds no skip", () => {
    const found = getControlledSpecialtyPlanets(boardOf({ [blue.id]: { attachments: ["warfarestat"] } }), "p1");
    expect(found[0].skips.map((s) => s.specialty)).toEqual(["PROPULSION"]);
  });

  it("an exhausted planet gives nothing unless Psychoarchaeology frees it", () => {
    const found = getControlledSpecialtyPlanets(
      boardOf({ [bare.id]: { attachments: ["warfare"], exhausted: true } }),
      "p1",
    );
    expect(found[0].isExhausted).toBe(true);
    expect(skipCounts(found, (p) => !p.isExhausted).WARFARE).toBe(0);
    expect(skipCounts(found, () => true).WARFARE).toBe(1);
  });
});
