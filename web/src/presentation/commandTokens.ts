import type { PendingChoiceDto } from "../protocol/types.ts";
import { suggestAutoPay, type PayablePlanet, type PaymentOffer } from "./paymentDraft.ts";

export type TokenPool = "tactic" | "fleet" | "strategic";
export const TOKEN_POOLS: readonly TokenPool[] = ["tactic", "fleet", "strategic"];
export type Pools = Record<TokenPool, number>;

export const POOL_LABEL: Record<TokenPool, string> = {
  tactic: "Tactic",
  fleet: "Fleet",
  strategic: "Strategy",
};

/** What each pool is for, shown under its name. */
export const POOL_PURPOSE: Record<TokenPool, string> = {
  tactic: "activate systems",
  fleet: "fleet supply, moves ships",
  strategic: "strategy secondaries and abilities",
};

/** A fleet pool of 0 or 1 leaves a faction unable to move; the engine never offers it. */
export const MIN_FLEET_POOL = 2;

const gainOptionId = (pool: TokenPool) => `${pool}_tokens`;

/** One ready planet that can pay influence, as the engine lists it. */
export interface PurchasePlanet {
  id: string;
  worth: number;
}

/**
 * Leadership's influence purchase, from the question's display-only `purchase` details: each
 * token costs `cost` influence, `max` is how many the seat's influence can pay for in all.
 */
export interface PurchaseView {
  cost: number;
  /** Influence the seat can spend now (planets plus trade goods). */
  influence: number;
  max: number;
  planets: PurchasePlanet[];
  tradeGoods: number;
  tradeGoodWorth: number;
}

export interface CommandTokenView {
  /** "gain": add new tokens to the pools; "redistribute": rearrange the tokens already held. */
  mode: "gain" | "redistribute";
  /** The pools as the engine has them now. */
  current: Pools;
  /** Tokens left in reinforcements. */
  reinforcements: number | null;
  /** Gain: tokens to place in this window. Redistribute: every token held. */
  total: number;
  /** Redistribute: the arrangement option ids the engine offers. */
  arrangements: ReadonlySet<string>;
  /** Gain: the influence purchase that follows (or is asked first), planned on the same screen. */
  purchase: PurchaseView | null;
}

/** Staged tokens per pool: added ones for a gain, the whole arrangement for a redistribute. */
export type TokenStaging = Pools;

const asCount = (value: unknown): number | null =>
  typeof value === "number" && Number.isFinite(value) && value >= 0 ? Math.floor(value) : null;

/**
 * A command-token decision as pools to fill, from the server's display-only details. `null` for
 * every other decision. A gain panel needs a way to send many tokens at once, so it is offered
 * only when `canBatch`; a redistribution is a single decision and always is.
 */
export function describeCommandTokens(
  choice: Pick<PendingChoiceDto, "options" | "details">,
  canBatch: boolean,
): CommandTokenView | null {
  const details = choice.details;
  if (!details) return null;
  // Leadership's secondary window keeps its strategy-secondary details and adds the purchase.
  const purchase = describePurchase(details.purchase);
  const buyWindow = details.kind === "strategy_secondary" && purchase !== null;
  if (details.kind !== "command_tokens" && !buyWindow) return null;
  const pools = details.pools as Partial<Record<TokenPool, unknown>> | undefined;
  const current = {
    tactic: asCount(pools?.tactic),
    fleet: asCount(pools?.fleet),
    strategic: asCount(pools?.strategic),
  };
  if (current.tactic === null || current.fleet === null || current.strategic === null) return null;
  const held: Pools = { tactic: current.tactic, fleet: current.fleet, strategic: current.strategic };
  const reinforcements = asCount(details.reinforcements);
  if (details.mode === "buy") {
    // The yes/no purchase question itself: nothing free to place, only what can be bought.
    const ids = new Set(choice.options.map((option) => option.id));
    if (!canBatch || !purchase || !ids.has("yes") || !ids.has("no")) return null;
    return { mode: "gain", current: held, reinforcements, total: 0, arrangements: new Set(), purchase };
  }
  if (details.mode === "gain") {
    const total = asCount(details.tokens_to_place);
    const ids = new Set(choice.options.map((option) => option.id));
    if (!canBatch || !total || !TOKEN_POOLS.every((pool) => ids.has(gainOptionId(pool)))) {
      return null;
    }
    return { mode: "gain", current: held, reinforcements, total, arrangements: new Set(), purchase };
  }
  if (details.mode === "redistribute") {
    const total = asCount(details.total);
    const arrangements = new Set(
      choice.options.filter((option) => option.kind === "redistribute").map((option) => option.id),
    );
    if (total === null || arrangements.size === 0) return null;
    return {
      mode: "redistribute",
      current: held,
      reinforcements,
      total,
      arrangements,
      purchase: null,
    };
  }
  return null;
}

function describePurchase(raw: unknown): PurchaseView | null {
  if (!raw || typeof raw !== "object") return null;
  const source = raw as Record<string, unknown>;
  const cost = asCount(source.cost);
  const influence = asCount(source.influence_available);
  const max = asCount(source.max);
  const tradeGoods = asCount(source.trade_goods);
  const tradeGoodWorth = asCount(source.trade_good_worth);
  if (!cost || influence === null || !max || tradeGoods === null || !tradeGoodWorth) return null;
  const planets: PurchasePlanet[] = [];
  for (const entry of Array.isArray(source.planets) ? source.planets : []) {
    const planet = entry as Record<string, unknown>;
    const worth = asCount(planet?.worth);
    if (typeof planet?.id !== "string" || worth === null) return null;
    planets.push({ id: planet.id, worth });
  }
  return { cost, influence, max, planets, tradeGoods, tradeGoodWorth };
}

export const stagedTotal = (staging: TokenStaging): number =>
  TOKEN_POOLS.reduce((sum, pool) => sum + staging[pool], 0);

/** The staging before the player touches anything. */
export function initialStaging(view: CommandTokenView): TokenStaging {
  return view.mode === "gain"
    ? { tactic: 0, fleet: 0, strategic: 0 }
    : { ...view.current };
}

/** Every token to assign: the free ones plus the ones bought with influence. */
export const tokensToAssign = (view: CommandTokenView, bought = 0): number => view.total + bought;

/** Tokens still to assign. */
export const tokensRemaining = (
  view: CommandTokenView,
  staging: TokenStaging,
  bought = 0,
): number => Math.max(0, tokensToAssign(view, bought) - stagedTotal(staging));

export const canAddToken = (view: CommandTokenView, staging: TokenStaging, bought = 0): boolean =>
  tokensRemaining(view, staging, bought) > 0;

export const canRemoveToken = (staging: TokenStaging, pool: TokenPool): boolean =>
  staging[pool] > 0;

export function addToken(
  view: CommandTokenView,
  staging: TokenStaging,
  pool: TokenPool,
  bought = 0,
): TokenStaging {
  return canAddToken(view, staging, bought) ? { ...staging, [pool]: staging[pool] + 1 } : staging;
}

export function removeToken(staging: TokenStaging, pool: TokenPool): TokenStaging {
  return canRemoveToken(staging, pool) ? { ...staging, [pool]: staging[pool] - 1 } : staging;
}

/** What the pool will hold once the staging is confirmed. */
export const resultingCount = (
  view: CommandTokenView,
  staging: TokenStaging,
  pool: TokenPool,
): number => (view.mode === "gain" ? view.current[pool] + staging[pool] : staging[pool]);

export interface PoolPips {
  /** Tokens that stay as they are. */
  kept: number;
  /** Newly assigned tokens. */
  added: number;
  /** Tokens that were here and are taken out (redistribute only). */
  removed: number;
}

export function poolPips(view: CommandTokenView, staging: TokenStaging, pool: TokenPool): PoolPips {
  if (view.mode === "gain") {
    return { kept: view.current[pool], added: staging[pool], removed: 0 };
  }
  const now = view.current[pool];
  const next = staging[pool];
  return {
    kept: Math.min(now, next),
    added: Math.max(0, next - now),
    removed: Math.max(0, now - next),
  };
}

/** The option id of the redistribution that matches the staging, when the engine offers it. */
export function arrangementId(view: CommandTokenView, staging: TokenStaging): string | null {
  const id = `${staging.tactic}|${staging.fleet}|${staging.strategic}`;
  return view.mode === "redistribute" && view.arrangements.has(id) ? id : null;
}

/** Why a fully assigned staging still cannot be confirmed, or `null` when it can. */
export function confirmBlocker(
  view: CommandTokenView,
  staging: TokenStaging,
  bought = 0,
): string | null {
  const remaining = tokensRemaining(view, staging, bought);
  if (remaining > 0) {
    return `Assign ${remaining} more token${remaining === 1 ? "" : "s"} to confirm.`;
  }
  if (view.mode === "redistribute" && arrangementId(view, staging) === null) {
    return view.total >= MIN_FLEET_POOL
      ? `Keep at least ${MIN_FLEET_POOL} tokens in the fleet pool.`
      : "That arrangement is not allowed.";
  }
  return null;
}

/** A purchase question can be answered "no", so a view with a purchase confirms even at 0 tokens. */
export const canConfirmTokens = (
  view: CommandTokenView,
  staging: TokenStaging,
  bought = 0,
): boolean =>
  (tokensToAssign(view, bought) > 0 || view.purchase !== null) &&
  confirmBlocker(view, staging, bought) === null &&
  (bought === 0 || planPayment(view, bought) !== null);

/** A step of a token batch plan, as the server's `tokens` batch kind takes it. */
export type TokenStep =
  | { kind: "pool"; pool: string }
  /** Answer a "spend 3 influence for a command token" question. */
  | { kind: "purchase"; buy: boolean }
  /** Part of a purchase's payment: exhaust a planet, or spend a trade good. */
  | { kind: "exhaust"; planet: string }
  | { kind: "trade_good" };

/**
 * How many tokens can be bought: what the influence pays for, what the listed planets and trade
 * goods can actually be spent as, and what the reinforcements can still hold after the free ones.
 */
export function maxPurchases(view: CommandTokenView): number {
  const purchase = view.purchase;
  if (!purchase) return 0;
  const spendable =
    purchase.planets.reduce((sum, planet) => sum + planet.worth, 0) +
    purchase.tradeGoods * purchase.tradeGoodWorth;
  const room =
    view.reinforcements === null ? Infinity : Math.max(0, view.reinforcements - view.total);
  return Math.max(0, Math.min(purchase.max, Math.floor(spendable / purchase.cost), room));
}

/** Takes tokens back out of the pools (strategy first) until the staging fits the total. */
export function fitStaging(view: CommandTokenView, staging: TokenStaging, bought: number): TokenStaging {
  const fitted = { ...staging };
  const limit = tokensToAssign(view, bought);
  for (const pool of [...TOKEN_POOLS].reverse()) {
    while (stagedTotal(fitted) > limit && fitted[pool] > 0) fitted[pool] -= 1;
  }
  return fitted;
}

/** What paying for `bought` tokens spends. */
export interface PaymentPlan {
  planets: PurchasePlanet[];
  tradeGoods: number;
  /** Influence the spent planets and goods are worth. */
  spent: number;
  /** What the tokens cost. */
  bill: number;
  /** Worth beyond the bill; it carries over to the next token and is lost after the last. */
  extra: number;
}

/**
 * The payment for `bought` tokens, chosen with the payment drawer's Auto-pay rule: the planets
 * covering the bill with the least waste and then the fewest planets, trade goods only when the
 * planets alone fall short. The engine asks its payment questions without any automatic rule, so
 * the plan names each exhaust and trade good explicitly. `null` when it cannot be covered.
 */
export function planPayment(view: CommandTokenView, bought: number): PaymentPlan | null {
  const purchase = view.purchase;
  if (!purchase) return null;
  const bill = purchase.cost * bought;
  if (bought === 0) return { planets: [], tradeGoods: 0, spent: 0, bill: 0, extra: 0 };
  const planets: PayablePlanet[] = purchase.planets.map((planet) => ({
    id: `exhaust|${planet.id}`,
    planetId: planet.id,
    planetName: planet.id,
    worth: planet.worth,
    label: planet.id,
  }));
  const offer: PaymentOffer = {
    planets,
    hasTradeGoodOption: purchase.tradeGoods > 0,
    tradeGoodWorth: purchase.tradeGoodWorth,
    owed: bill,
    totalAmount: bill,
    alreadyPaid: 0,
    currency: "Influence",
  };
  const pick = suggestAutoPay(offer, purchase.tradeGoods);
  if (!pick.settled) return null;
  const chosen = pick.planetIds
    .map((id) => purchase.planets.find((planet) => `exhaust|${planet.id}` === id))
    .filter((planet): planet is PurchasePlanet => planet !== undefined);
  const spent =
    chosen.reduce((sum, planet) => sum + planet.worth, 0) + pick.tradeGoods * purchase.tradeGoodWorth;
  return { planets: chosen, tradeGoods: pick.tradeGoods, spent, bill, extra: spent - bill };
}

/** The gain as one plan: a pool step per token, tactic first, then fleet, then strategy. */
export function tokenPlan(staging: TokenStaging): TokenStep[] {
  return TOKEN_POOLS.flatMap((pool) =>
    Array.from({ length: staging[pool] }, () => ({ kind: "pool" as const, pool: gainOptionId(pool) })),
  );
}

/**
 * The gain and the purchases as one plan, in the order the engine asks: the free tokens' pools,
 * then per purchase the yes, its payment (until the running bill is covered; overpayment carries
 * to the next token) and its pool, then the closing "no" the engine asks while more is affordable.
 * `null` when the payment cannot be covered.
 */
export function tokenPlanWithPurchase(
  view: CommandTokenView,
  staging: TokenStaging,
  bought: number,
): TokenStep[] | null {
  const purchase = view.purchase;
  if (!purchase) return tokenPlan(staging);
  const plan = planPayment(view, bought);
  if (!plan) return null;
  const pools = tokenPlan(staging);
  const steps: TokenStep[] = pools.slice(0, view.total);
  const items: { step: TokenStep; worth: number }[] = [
    ...plan.planets.map((planet) => ({
      step: { kind: "exhaust" as const, planet: planet.id },
      worth: planet.worth,
    })),
    ...Array.from({ length: plan.tradeGoods }, () => ({
      step: { kind: "trade_good" as const },
      worth: purchase.tradeGoodWorth,
    })),
  ];
  let covered = 0;
  let next = 0;
  for (let token = 1; token <= bought; token += 1) {
    steps.push({ kind: "purchase", buy: true });
    while (covered < purchase.cost * token && next < items.length) {
      steps.push(items[next].step);
      covered += items[next].worth;
      next += 1;
    }
    steps.push(pools[view.total + token - 1]);
  }
  // The engine asks again exactly while the influence still pays for one more.
  if (bought < purchase.max) steps.push({ kind: "purchase", buy: false });
  return steps;
}

export type TokenOutcome =
  | { kind: "plan"; steps: TokenStep[] }
  | { kind: "option"; optionId: string };

/** What confirming sends: a batch plan for a gain, the arrangement's option for a redistribute. */
export function tokenOutcome(
  view: CommandTokenView,
  staging: TokenStaging,
  bought = 0,
): TokenOutcome | null {
  if (!canConfirmTokens(view, staging, bought)) return null;
  if (view.mode === "gain") {
    const steps = tokenPlanWithPurchase(view, staging, bought);
    return steps ? { kind: "plan", steps } : null;
  }
  const optionId = arrangementId(view, staging);
  return optionId === null ? null : { kind: "option", optionId };
}
