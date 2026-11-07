import { expect, type Browser, type Page } from "@playwright/test";
import { openMockedGame, type MockChoice } from "../_shared/mockGame";
import { playerWithHand, opponent } from "../_shared/players";
import { paymentBoard } from "../D-planet-selection/payment";
import { actor, galleryBoard } from "../_shared/fixtures";
import { planetsBoard, PLANETS } from "../BC-leadership-planets-first/scene";
import { DEVICES } from "../BN-phone-detection/scene";

/**
 * The questions the multi-ask conversion turned into ONE question each (commit 49dd4af4c), as the
 * engine renders them: a payment and Leadership's purchase as a multi-pick (`multi_pick`, the
 * answer a '+'-joined set of the listed atoms), a gain of several tokens as one split question,
 * Predictive Intelligence as one redistribution, Politics' two agendas as one question over six
 * orders, and the coming hand-limit discard (same multi-pick field). Synthetic fixtures through
 * the mocked server, not captured engine states.
 */

export type Size = "desktop" | "390x844";
export const SIZES: Size[] = ["desktop", "390x844"];

export async function newPage(browser: Browser, size: Size, baseURL?: string) {
  const options = size === "desktop" ? { viewport: { width: 1440, height: 900 } } : DEVICES.mobile;
  const context = await browser.newContext({ ...options, colorScheme: "dark", baseURL });
  const page = await context.newPage();
  return { context, page };
}

/** No element is wider than the phone's viewport. */
export async function assertNoHorizontalScroll(page: Page) {
  const r = await page.evaluate(() => ({ vw: document.documentElement.clientWidth, sw: document.documentElement.scrollWidth }));
  expect(r.sw, "document is not wider than the viewport").toBeLessThanOrEqual(r.vw);
}

// ---- Payment (pay_resources as a multi-pick) -------------------------------------------------

const payAtom = (planet: string, name: string, worth: number) => ({
  id: `exhaust|${planet}`,
  label: `exhaust ${planet} for ${worth} resources`,
  kind: "pay",
  payload: { worth, owed: 5, kind: "resources", source: "resources", planet, planet_name: name },
});

/** Paying 5 resources: Jord 2, Lodor 3, Quann 2 (as the board prints them) and three trade goods, one question for the whole set. */
export const paymentChoice: MockChoice = {
  prompt: "pay 5 more resources",
  nonce: "hp-payment",
  context: { subtype: "pay_resources", outstanding: [{ kind: "resources", amount: 5, paid: 0 }] },
  options: [
    payAtom("jord", "Jord", 2),
    payAtom("lodor", "Lodor", 3),
    payAtom("quann", "Quann", 2),
    { id: "trade_good", label: "spend a trade good", kind: "pay", payload: { worth: 1, owed: 5, kind: "resources" } },
  ],
  multi_pick: { kind: "payment", owed: 5, repeat: { trade_good: 3 } },
};

export async function openPayment(page: Page, onSubmit?: (id: string) => void) {
  await openMockedGame(page, {
    players: [playerWithHand({ trade_goods: 3 }), opponent],
    board: paymentBoard,
    choice: paymentChoice,
    onClientMessage: (message) => {
      const m = message as { type?: string; option_id?: string };
      if (m.option_id) onSubmit?.(m.option_id);
    },
  });
  await page.getByTestId("payment-drawer").waitFor();
}

export async function stagePayment(page: Page, planets: string[], goods: number) {
  for (const planet of planets) await page.getByTestId(`planet-card-exhaust|${planet}`).locator("input").click();
  for (let i = 0; i < goods; i += 1) await page.getByTestId("tg-increment-btn").click();
}

// ---- Hand-limit discard (the coming multi-pick, generic picker) ------------------------------

export const discardChoice: MockChoice = {
  // As the engine renders it: one option per printed name (id = its place in the hand, label = the
  // card's name), a name held twice repeatable up to its copies.
  prompt: "over the hand limit — discard 2 of 9",
  nonce: "hp-discard",
  context: { subtype: "discard_over_hand_limit" },
  options: [
    { id: "0", label: "Sabotage", kind: "discard", payload: {} },
    { id: "2", label: "Direct Hit", kind: "discard", payload: {} },
    { id: "3", label: "Decoy Operation", kind: "discard", payload: {} },
    { id: "4", label: "Skilled Retreat", kind: "discard", payload: {} },
  ],
  multi_pick: { kind: "discard", owed: 2, repeat: { "0": 2 } },
};

// ---- Fleet supply removal (multi-pick kind remove) -------------------------------------------

const ship = (unit_type: string) => ({ unit_type, owner: actor, damaged: false });

/** System 18 holds 8 of the viewer's ships against a fleet supply of 5: 3 over. */
export const removeBoard = {
  ...galleryBoard,
  systems: {
    ...galleryBoard.systems,
    "18": {
      ...galleryBoard.systems["18"],
      units: [
        ...[1, 2, 3].map(() => ship("destroyer")),
        ...[1, 2].map(() => ship("cruiser")),
        ...[1, 2].map(() => ship("dreadnought")),
        ship("carrier"),
      ],
    },
  },
};

/** One question for the whole removal: an option per unit kind, a '+'-joined answer (the engine checks it). */
export const removeChoice: MockChoice = {
  prompt: "remove a unit: over fleet supply in 18",
  nonce: "hp-remove",
  context: { subtype: "remove_unit" },
  options: [
    { id: "remove|0", label: "remove destroyer", kind: "remove", payload: {} },
    { id: "remove|3", label: "remove cruiser", kind: "remove", payload: {} },
    { id: "remove|5", label: "remove dreadnought", kind: "remove", payload: {} },
    { id: "remove|7", label: "remove carrier", kind: "remove", payload: {} },
  ],
  details: { reason: "fleet supply", system: "18", fleet_charged: 8, fleet_limit: 5, capacity_consumed: 0, capacity_transport: 0 },
  multi_pick: { kind: "remove", owed: 3 },
};

// ---- Leadership ------------------------------------------------------------------------------

const pools = { tactic: 3, fleet: 4, strategic: 2 };
const purchase = {
  cost: 3,
  influence_available: 9,
  max: 3,
  trade_goods: 0,
  trade_good_worth: 1,
  planets: PLANETS,
};

/** Every `{tactic}|{fleet}|{strategic}` split of `left` tokens, most tactic first (the engine's order). */
export function gainSplits(left: number) {
  const out = [];
  for (let tactic = left; tactic >= 0; tactic -= 1)
    for (let fleet = left - tactic; fleet >= 0; fleet -= 1) {
      const strategic = left - tactic - fleet;
      const parts = [
        [tactic, "tactic"],
        [fleet, "fleet"],
        [strategic, "strategy"],
      ]
        .filter(([n]) => (n as number) > 0)
        .map(([n, p]) => `${n} ${p}`);
      out.push({
        id: `${tactic}|${fleet}|${strategic}`,
        kind: "gain",
        label: parts.join(" + "),
        payload: { tactic, fleet, strategic },
      });
    }
  return out;
}

const gainPrompt = (left: number) =>
  left === 1 ? "gain a command token into which pool" : `gain ${left} command tokens into which pools`;

/** Leadership primary: one question for the three free tokens, the purchase planned on the same screen. */
export const leadershipPrimary: MockChoice = {
  prompt: gainPrompt(3),
  nonce: "hp-leadership-primary",
  context: { subtype: "gain_command_token", source: { StrategyCard: { card: "pok1leadership", secondary: false } } },
  options: gainSplits(3),
  details: { kind: "command_tokens", mode: "gain", pools, reinforcements: 8, tokens_to_place: 3, purchase },
};

/** Leadership's purchase question alone (the secondary after following): one multi-pick, at least one token. */
export const leadershipPurchase: MockChoice = {
  prompt: "buy command tokens for 3 influence each (up to 3)",
  nonce: "hp-leadership-purchase",
  context: { subtype: "buy_token_with_influence", source: { StrategyCard: { card: "pok1leadership", secondary: true } } },
  options: PLANETS.map((planet) => ({
    id: `exhaust|${planet.id}`,
    kind: "pay",
    label: `exhaust ${planet.id} for ${planet.worth} influence`,
    payload: { worth: planet.worth, owed: 3, kind: "influence", source: "influence" },
  })),
  details: { kind: "command_tokens", mode: "buy", pools, reinforcements: 8, tokens_to_place: 0, purchase },
  multi_pick: { kind: "purchase", owed: 3, buy: [1, 3] },
};

/**
 * Leadership's PRIMARY purchase question alone, after its three free tokens (core-multiask-1-rebase): the question lists the decline
 * ("buy no command tokens") FIRST and may be answered with nothing bought, `buy: [0, 3]`. The token panel plans it; "No purchase" answers
 * with the empty set.
 */
export const leadershipPurchasePrimary: MockChoice = {
  ...leadershipPurchase,
  nonce: "hp-leadership-purchase-primary",
  context: { subtype: "buy_token_with_influence", source: { StrategyCard: { card: "pok1leadership", secondary: false } } },
  options: [{ id: "decline", label: "buy no command tokens", kind: "decline", payload: {} }, ...leadershipPurchase.options],
  multi_pick: { kind: "purchase", owed: 3, buy: [0, 3] },
};

export async function openLeadership(page: Page, choice: MockChoice) {
  await openMockedGame(page, { players: [playerWithHand({ trade_goods: 0 }), opponent], board: planetsBoard, choice });
  await page.getByTestId("command-token-panel").waitFor();
}

// ---- Gains and redistributions ----------------------------------------------------------------

/** A gain of `left` tokens outside Leadership (an exploration card, Summit, ...): one split question. */
export const gainChoice = (left: number): MockChoice => ({
  prompt: gainPrompt(left),
  nonce: `hp-gain-${left}`,
  context: { subtype: "gain_command_token" },
  options: gainSplits(left),
  details: { kind: "command_tokens", mode: "gain", pools, reinforcements: 8, tokens_to_place: left },
});

/** Predictive Intelligence: one question over every final split of the nine tokens (no fleet floor). */
export const predictiveChoice: MockChoice = {
  prompt: "Predictive Intelligence: redistribute command tokens",
  nonce: "hp-pi",
  context: { subtype: "predictive_intelligence_redistribute", source: { Content: "pi" } },
  options: Array.from({ length: 10 }, (_, tactic) => tactic).flatMap((tactic) =>
    Array.from({ length: 10 - tactic }, (_, fleet) => fleet).map((fleet) => {
      const strategic = 9 - tactic - fleet;
      return {
        id: `${tactic}|${fleet}|${strategic}`,
        kind: "redistribute",
        label: `tactic ${tactic} / fleet ${fleet} / strategy ${strategic}`,
        payload: {},
      };
    }),
  ),
  details: { kind: "command_tokens", mode: "redistribute", pools, reinforcements: 7, total: 9 },
};

export async function openTokens(page: Page, choice: MockChoice) {
  await openMockedGame(page, { players: [playerWithHand(), opponent], choice });
  await page.getByTestId("command-token-panel").waitFor();
}

// ---- Politics ----------------------------------------------------------------------------------

const agendas = [
  {
    id: "minister_commerce",
    name: "Minister of Commerce",
    type: "Law",
    target: "Elect Player",
    text1: "The elected player places this card faceup in their play area; after they replenish commodities, they gain 1 trade good for each player that is their neighbor.",
    text2: "",
  },
  {
    id: "mutiny",
    name: "Mutiny",
    type: "Directive",
    target: "For/Against",
    text1: "For: Each player who voted \"For\" gains 1 victory point.",
    text2: "Against: Each player who voted \"For\" loses 1 victory point.",
  },
];

const order = (top: string[], bottom: string[], label: string) => ({
  id: `${top.join(",")}/${bottom.join(",")}`,
  kind: "agenda_order",
  label,
  payload: { top, bottom },
});

/** Politics primary: both looked-at agendas in one question, the six orders as options. */
export const politicsChoice: MockChoice = {
  prompt: "place minister_commerce and mutiny on the top or the bottom of the agenda deck",
  nonce: "hp-politics",
  context: { subtype: "politics_place_agenda", source: { StrategyCard: { card: "pok3politics", secondary: false } } },
  details: { agendas },
  options: [
    order(["minister_commerce"], ["mutiny"], "minister_commerce on top, mutiny on the bottom"),
    order(["mutiny"], ["minister_commerce"], "mutiny on top, minister_commerce on the bottom"),
    order(["minister_commerce", "mutiny"], [], "both on top: minister_commerce first, then mutiny"),
    order(["mutiny", "minister_commerce"], [], "both on top: mutiny first, then minister_commerce"),
    order([], ["minister_commerce", "mutiny"], "both on the bottom: minister_commerce, then mutiny as the bottom card"),
    order([], ["mutiny", "minister_commerce"], "both on the bottom: mutiny, then minister_commerce as the bottom card"),
  ],
};

export async function openGeneric(page: Page, choice: MockChoice) {
  await openMockedGame(page, { players: [playerWithHand(), opponent], choice });
  await page.getByTestId("pending-choice-dialog").waitFor();
}
