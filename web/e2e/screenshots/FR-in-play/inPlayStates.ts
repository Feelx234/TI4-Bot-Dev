import type { MockGameOptions } from "../_shared/mockGame";
import { galleryBoard } from "../_shared/fixtures";
import { me, opponent, withPlayer } from "../_shared/players";
import type { Page } from "@playwright/test";
import { expect } from "@playwright/test";

export const SIZES = {
  desktop: { width: 1440, height: 900 },
  "390x844": { width: 390, height: 844 },
} as const;

const third = withPlayer(opponent, { id: "seat_three", strategy_cards: ["pok6warfare"], exhausted_strategy_cards: ["pok6warfare"] });

/** Round 4 of a game: several laws, a long discard history, the custodians token taken. */
export const midGame: MockGameOptions = {
  round: 4,
  phase: "action",
  players: [
    withPlayer(me, { strategy_cards: ["pok3politics"] }),
    withPlayer(opponent, { strategy_cards: ["pok1leadership"] }),
    third,
  ],
  table: {
    revealed_objectives: ["corner", "develop", "centralize_trade"],
    scored_objectives: { [me.id]: ["corner"], [opponent.id]: ["corner", "develop"] },
    unclaimed_strategy_cards: ["pok2diplomacy", "pok4construction", "pok5trade", "pok7technology", "pok8imperial"],
    strategy_card_goods: { pok2diplomacy: 1, pok4construction: 0, pok5trade: 3, pok7technology: 2, pok8imperial: 1 },
    laws: { sanctions: "for", minister_exploration: opponent.id, rep_govt: "for" },
  },
  state: {
    custodians_removed: true,
    vp_ledger: [[opponent.id, 1, "custodians"], [me.id, 1, "imperial_primary"]],
    galactic_event: "minor_factions",
    frontier_tokens: ["27", "31", "64"],
    expedition_slices: {},
    objective_deck: ["amass_wealth", "build_defenses", "lost_outposts", "conquer", "galvanize", "revolutionize"],
    relic_deck: ["r1", "r2", "r3", "r4", "r5"],
    agenda_log: [
      { round: 2, agenda: "sanctions", name: "Executive Sanctions", outcome: "for", votes: { for: 9, against: 4 }, law: true, enacted: true },
      { round: 2, agenda: "mutiny", name: "Mutiny", outcome: "against", votes: { for: 3, against: 8 }, law: false },
      { round: 3, agenda: "minister_exploration", name: "Minister of Exploration", outcome: opponent.id, votes: { [opponent.id]: 7, [me.id]: 5 }, law: true, enacted: true },
      { round: 3, agenda: "arbiter", name: "Arbiter", outcome: null, discarded: true, law: true, discard_reason: "no_choice" },
      { round: 4, agenda: "rep_govt", name: "Representative Government", outcome: "for", votes: { for: 6, against: 6 }, speaker_decided: true, law: true, enacted: true },
    ],
    discarded_action_cards: ["abs", "dh1", "bribery", "dh2", "cripple", "dh3", "assassin", "bunker", "courageous"],
    exploration_log: [
      { round: 1, player: me.id, deck: "CULTURAL", card: "frln1", planet: "lodor", outcome: "resolved", result: "gained 1 trade good" },
      { round: 2, player: opponent.id, deck: "HAZARDOUS", card: "cm1", planet: "jord", outcome: "resolved", result: "gained 1 command token" },
      { round: 2, player: opponent.id, deck: "HAZARDOUS", card: "hrf1", planet: "jord", outcome: "fragment" },
      { round: 3, player: me.id, deck: "FRONTIER", card: "minent", outcome: "discarded", system: "27", result: "gained 1 command token and 1 trade good" },
      { round: 3, player: "seat_three", deck: "INDUSTRIAL", card: "aw1", planet: "wellon", outcome: "resolved", result: "gained 1 trade good" },
    ],
  },
  facts: {
    action_cards: { deck: 38, discard: ["abs", "dh1", "bribery"], hand_limit: 7 },
    agendas: { deck: 41 },
    exploration: { decks: { CULTURAL: new Array(8).fill("c"), INDUSTRIAL: new Array(9).fill("i"), HAZARDOUS: new Array(8).fill("h"), FRONTIER: new Array(14).fill("f") }, drawn: [], explores: {} },
  },
};

/** Round 1: nothing has happened. */
export const earlyGame: MockGameOptions = {
  round: 1,
  phase: "strategy",
  players: [me, opponent, third].map((p) => withPlayer(p, { strategy_cards: [], exhausted_strategy_cards: [] })),
  table: {
    revealed_objectives: ["corner", "develop"],
    scored_objectives: {},
    unclaimed_strategy_cards: ["pok1leadership", "pok2diplomacy", "pok3politics", "pok4construction", "pok5trade", "pok6warfare", "pok7technology", "pok8imperial"],
    strategy_card_goods: {},
    laws: {},
  },
  state: {
    custodians_removed: false,
    frontier_tokens: ["27", "31", "64", "40"],
    objective_deck: ["amass_wealth", "build_defenses", "conquer", "galvanize"],
    relic_deck: ["r1", "r2", "r3"],
    agenda_log: [],
    discarded_action_cards: [],
    exploration_log: [],
  },
  facts: {
    action_cards: { deck: 45, discard: [], hand_limit: 7 },
    agendas: { deck: 50 },
    exploration: { decks: { CULTURAL: new Array(10).fill("c"), INDUSTRIAL: new Array(10).fill("i"), HAZARDOUS: new Array(10).fill("h"), FRONTIER: new Array(15).fill("f") }, drawn: [], explores: {} },
  },
  board: galleryBoard,
};

/** No sideways page scroll, and the panel's own scroller does not overflow sideways either. */
export async function expectNoHorizontalScroll(page: Page, width: number) {
  const widths = await page.evaluate(() => ({
    doc: document.documentElement.scrollWidth,
    body: document.body.scrollWidth,
    panel: (document.querySelector('[data-testid="in-play-scroll"]') as HTMLElement | null)?.scrollWidth ?? 0,
    panelClient: (document.querySelector('[data-testid="in-play-scroll"]') as HTMLElement | null)?.clientWidth ?? 0,
  }));
  expect(widths.doc).toBeLessThanOrEqual(width);
  expect(widths.body).toBeLessThanOrEqual(width);
  expect(widths.panel).toBeLessThanOrEqual(widths.panelClient);
}
