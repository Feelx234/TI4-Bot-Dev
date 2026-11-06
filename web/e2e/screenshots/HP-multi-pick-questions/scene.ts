import { expect, type Browser, type Page } from "@playwright/test";
import { openMockedGame, type MockChoice } from "../_shared/mockGame";
import { playerWithHand, opponent } from "../_shared/players";
import { paymentBoard } from "../D-planet-selection/payment";
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
  prompt: "discard 2 action cards (hand limit 7)",
  nonce: "hp-discard",
  context: { subtype: "discard_over_hand_limit" },
  options: [
    { id: "sabo1", label: "Sabotage", kind: "action_card", payload: { card: "sabo1" } },
    { id: "direct_hit", label: "Direct Hit", kind: "action_card", payload: { card: "direct_hit" } },
    { id: "decoy", label: "Decoy Operation", kind: "action_card", payload: { card: "decoy" } },
    { id: "skilled_retreat", label: "Skilled Retreat", kind: "action_card", payload: { card: "skilled_retreat" } },
  ],
  multi_pick: { kind: "discard", owed: 2, repeat: { sabo1: 2 } },
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
