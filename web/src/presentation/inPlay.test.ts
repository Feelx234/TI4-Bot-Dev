import { describe, expect, it } from "vitest";
import type { GameView } from "../protocol/types.ts";
import { buildInPlayModel, filterInPlay, type InPlayInput } from "./inPlay.ts";

const names: Record<string, string> = { p1: "Ada", p2: "Bo", p3: "Cy" };
const seatName = (id: string) => names[id] ?? null;

function view(overrides: Partial<GameView["table"]> = {}, players: Partial<GameView["players"][number]>[] = []): GameView {
  return {
    round: 3,
    turn: 0,
    phase: "agenda",
    speaker: "p2",
    seating_order: ["p1", "p2", "p3"],
    finished: false,
    players: ["p1", "p2", "p3"].map((id, i) => ({ id, strategy_cards: [], exhausted_strategy_cards: [], ...(players[i] ?? {}) })) as never,
    board: {
      systems: {
        "18": { system_id: "18", command_tokens: [], units: [], planets: { mecatol_rex: { planet_id: "mecatol_rex", controlled_by: "p1", exhausted: false, attachments: [] } } },
      },
      map_tiles: [],
    } as never,
    table: {
      revealed_objectives: [],
      scored_objectives: {},
      unclaimed_strategy_cards: [],
      strategy_card_goods: {},
      laws: {},
      ...overrides,
    },
  } as GameView;
}

const base = (extra: Partial<InPlayInput> = {}): InPlayInput => ({
  view: view(),
  state: {},
  seatName,
  ...extra,
});

const section = (input: InPlayInput, id: string) => buildInPlayModel(input).sections.find((s) => s.id === id)!;

describe("laws", () => {
  it("lists a For/Against law with its passed side, round and votes", () => {
    const input = base({
      view: view({ laws: { sanctions: "for" } }),
      state: {
        agenda_log: [{ round: 2, agenda: "sanctions", name: "Executive Sanctions", outcome: "for", votes: { for: 7, against: 3 }, law: true, enacted: true }],
      },
    });
    const laws = section(input, "laws");
    expect(laws.count).toBe(1);
    expect(laws.items[0].title).toBe("Executive Sanctions");
    expect(laws.items[0].meta).toEqual(expect.arrayContaining(["Law", "Passed: For", "Round 2", "Votes: For 7 · Against 3"]));
    expect(laws.items[0].text[0]).toMatch(/\S/);
  });

  it("names the elected player of a law that elects a seat", () => {
    const input = base({
      view: view({ laws: { minister_exploration: "p3" } }),
      state: { agenda_log: [{ round: 1, agenda: "minister_exploration", outcome: "p3", law: true, enacted: true, speaker_decided: true }] },
    });
    const [law] = section(input, "laws").items;
    expect(law.meta).toEqual(expect.arrayContaining(["Elected: Cy", "Round 1", "Speaker decided"]));
  });

  it("shows the Against text for a law that passed Against", () => {
    const input = base({ view: view({ laws: { rep_govt: "against" } }) });
    const [law] = section(input, "laws").items;
    expect(law.meta).toContain("Passed: Against");
  });

  it("is empty early in the game", () => {
    const model = buildInPlayModel(base());
    expect(model.laws).toBe(0);
    expect(model.discards).toBe(0);
    expect(section(base(), "laws").items).toEqual([]);
    expect(section(base(), "discard-action").empty).toMatch(/No action card/);
  });
});

describe("agenda discard", () => {
  const log = [
    { round: 1, agenda: "sanctions", outcome: "for", law: true, enacted: true },
    { round: 2, agenda: "arbiter", outcome: "against", law: true, enacted: false },
    { round: 2, agenda: "minister_peace", outcome: null, discarded: true, discard_reason: "no_choice" },
    { round: 3, agenda: "abolishment", outcome: "sanctions", law: false },
  ];

  it("keeps laws in play out of the pile and lists the rest newest first", () => {
    const pile = section(base({ view: view({ laws: { sanctions: "for" } }), state: { agenda_log: log } }), "discard-agenda");
    expect(pile.items.map((i) => i.id)).toEqual(["discard-agenda:3", "discard-agenda:2", "discard-agenda:1"]);
    expect(pile.count).toBe(3);
    expect(pile.items[1].meta.join("|")).toMatch(/Discarded: nothing to elect/);
    expect(pile.items[2].meta.join("|")).toMatch(/Not enacted/);
  });

  it("puts a repealed law into the pile", () => {
    const pile = section(base({ view: view(), state: { agenda_log: log } }), "discard-agenda");
    expect(pile.count).toBe(4);
    expect(pile.items.some((i) => i.meta.includes("Law repealed"))).toBe(true);
  });
});

describe("action card discard", () => {
  it("lists the pile newest first, grouping equal cards, and counts every card", () => {
    const pile = section(
      base({ state: { discarded_action_cards: ["sabotage1", "sabotage2", "bribery1"], salvage_yard: [] } }),
      "discard-action",
    );
    expect(pile.count).toBe(3);
    expect(pile.newestFirst).toBe(true);
    expect(pile.items.length).toBeLessThanOrEqual(3);
  });

  it("falls back to the sorted facts when the state has no pile", () => {
    const pile = section(base({ actionCards: { deck: 30, discard: ["a1", "a2"] } }), "discard-action");
    expect(pile.count).toBe(2);
  });
});

describe("exploration discard", () => {
  it("lists drawn cards that resolved, not fragments or attachments", () => {
    const pile = section(
      base({
        state: {
          exploration_log: [
            { round: 1, player: "p1", deck: "CULTURAL", card: "dwa", planet: "lodor", outcome: "resolved", result: "gained 1 trade good" },
            { round: 1, player: "p2", deck: "HAZARDOUS", card: "x", planet: "y", outcome: "fragment" },
            { round: 2, player: "p3", deck: "INDUSTRIAL", card: "z", planet: "y", outcome: "attached:foo" },
            { round: 2, player: "p3", deck: "FRONTIER", card: "minent", outcome: "discarded", system: "27" },
          ],
        },
      }),
      "discard-exploration",
    );
    expect(pile.count).toBe(2);
    expect(pile.items[0].title).toBe("Minor Entropic Field");
    expect(pile.items[0].meta).toEqual(expect.arrayContaining(["Frontier", "Round 2", "Drawn by Cy", "System 27"]));
    expect(pile.facts.map((f) => f.label)).toEqual(["Cultural", "Frontier"]);
  });
});

describe("the table", () => {
  it("shows the speaker, the custodians token and who took it", () => {
    const table = section(
      base({ state: { custodians_removed: true, vp_ledger: [["p1", 1, "custodians"], ["p2", 1, "imperial_primary"]], galactic_event: "minor_factions" } }),
      "table",
    );
    const fact = (label: string) => table.facts.find((f) => f.label === label)?.value;
    expect(fact("Speaker token")).toBe("Bo");
    expect(fact("Custodians token")).toMatch(/Removed by Ada/);
    expect(fact("Mecatol Rex")).toBe("Controlled by Ada");
    expect(fact("Points from Custodians and Imperial")).toBe("Ada +1, Bo +1");
    expect(fact("Galactic event")).toBe("Minor Factions");
  });

  it("says the custodians token is still on Mecatol early on", () => {
    const table = section(base({ state: { custodians_removed: false } }), "table");
    expect(table.facts.find((f) => f.label === "Custodians token")?.value).toBe("On Mecatol Rex");
  });

  it("shows strategy cards with trade goods and who picked them", () => {
    const strategy = section(
      base({
        view: view(
          { unclaimed_strategy_cards: ["base3"], strategy_card_goods: { base3: 2 } },
          [{ strategy_cards: ["base1"], exhausted_strategy_cards: ["base1"] } as never],
        ),
      }),
      "strategy",
    );
    expect(strategy.count).toBe(1);
    const all = strategy.items.map((i) => i.meta.join("|")).join("\n");
    expect(all).toMatch(/2 trade goods/);
    expect(all).toMatch(/Picked by Ada\|Used/);
  });

  it("counts unrevealed objectives per stage without naming them", () => {
    const objectives = section(
      base({ state: { objective_deck: ["corner", "corner", "zzz_unknown"] } }),
      "objectives",
    );
    expect(objectives.facts[0].label).toBe("Not yet revealed");
    expect(JSON.stringify(objectives)).not.toContain("zzz_unknown");
  });

  it("shows frontier tokens and Thunder's Edge on the map", () => {
    const map = section(base({ state: { frontier_tokens: ["27", "31"], thunders_edge_system: "99", expedition_slices: { slice_a: "p1" } } }), "map");
    const fact = (label: string) => map.facts.find((f) => f.label === label)?.value;
    expect(fact("Frontier tokens")).toBe("2 (System 27, System 31)");
    expect(fact("Thunder's Edge")).toBe("Planet in System 99");
    expect(fact("Expeditions")).toBe("Slice A: Ada");
  });
});

describe("no hidden information", () => {
  const secrets = {
    agenda_deck: ["SECRET_AGENDA_TOP", "SECRET_AGENDA_2"],
    action_card_deck: ["SECRET_ACTION_TOP"],
    secret_deck: ["SECRET_OBJECTIVE_TOP"],
    relic_deck: ["SECRET_RELIC_TOP", "r2"],
    exploration_decks: { CULTURAL: ["SECRET_EXPLORE_TOP"] },
    players: [{ id: "p2", action_cards: ["SECRET_HAND_CARD"], secret_objectives: ["SECRET_HAND_OBJECTIVE"] }],
    scored_secret_objectives: ["SECRET_SCORED"],
    pending: "SECRET_PENDING",
  };

  it("never copies a draw pile, a hand or the secret deck into the model", () => {
    const model = buildInPlayModel(
      base({
        state: { ...secrets, custodians_removed: false, discarded_action_cards: ["sabotage1"] },
        actionCards: { deck: 2, discard: ["sabotage1"] },
        exploration: { decks: { CULTURAL: ["SECRET_EXPLORE_TOP"] }, drawn: [], explores: {} } as never,
        agendas: { deck: 2 },
      }),
    );
    expect(JSON.stringify(model)).not.toMatch(/SECRET_/);
  });

  it("shows draw piles only as counts", () => {
    const table = section(
      base({
        state: secrets,
        actionCards: { deck: 31, discard: [] },
        exploration: { decks: { CULTURAL: ["a", "b", "c"] }, drawn: [], explores: {} } as never,
        agendas: { deck: 44 },
      }),
      "table",
    );
    expect(table.facts.find((f) => f.label === "Cards left in draw piles")?.value).toBe("Agendas 44 · Action cards 31 · Relics 2 · Cultural 3");
  });

  it("offers no section for the secret objective discard or for hands", () => {
    const ids = buildInPlayModel(base({ state: secrets })).sections.map((s) => s.id);
    expect(ids).toEqual(["laws", "effects", "table", "objectives", "strategy", "map", "discard-action", "discard-agenda", "discard-exploration"]);
  });
});

describe("filtering", () => {
  const input = base({
    view: view({ laws: { sanctions: "for" } }),
    state: { discarded_action_cards: ["sabotage1"], agenda_log: [{ round: 1, agenda: "sanctions", outcome: "for", law: true, enacted: true }] },
  });
  const model = buildInPlayModel(input);

  it("narrows by group", () => {
    expect(filterInPlay(model, { query: "", group: "discard" }).every((s) => s.group === "discard")).toBe(true);
    expect(filterInPlay(model, { query: "", group: "play" }).some((s) => s.id === "laws")).toBe(true);
  });

  it("searches names and drops sections with no match", () => {
    const hits = filterInPlay(model, { query: "sanction", group: "all" });
    expect(hits.map((s) => s.id)).toEqual(["laws"]);
    expect(filterInPlay(model, { query: "no such thing", group: "all" })).toEqual([]);
  });
});
