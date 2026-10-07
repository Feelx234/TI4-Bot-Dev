/**
 * Staging model for the two-column trade desk.
 *
 * The engine only accepts a deal id the server listed (a closed menu: `cc2`, `ct3:2`, `pn{note}:3`,
 * `np{note}:2`, ...). The desk lets the player stage both sides freely and maps the staged
 * combination back to that menu: a pure round trip between a `Staged` deal and a listed deal id.
 * Nothing here invents a deal; a combination that matches no listed id cannot be proposed.
 */
import { ChoiceOptionDto } from "../protocol/types.ts";
import { DecodedTradeOffer, decodeTradeOption } from "./tradeDecoder.ts";

/** One side of the table: what a seat hands over (or receives). */
export interface StagedSide {
  tradeGoods: number;
  commodities: number;
  /** At most one promissory note per side (the engine's Terms carry one). */
  note: string | null;
  /** At most one action card per side. */
  actionCard: string | null;
  secret: string | null;
  fragments: string[];
  /** The Support for the Throne swap: both sides hand over their own copy. */
  support: boolean;
}

export interface Staged {
  /** What the viewer gives. */
  give: StagedSide;
  /** What the viewer receives. */
  receive: StagedSide;
}

export const emptySide = (): StagedSide => ({
  tradeGoods: 0,
  commodities: 0,
  note: null,
  actionCard: null,
  secret: null,
  fragments: [],
  support: false,
});

export const emptyStaged = (): Staged => ({ give: emptySide(), receive: emptySide() });

export function isSideEmpty(side: StagedSide): boolean {
  return (
    side.tradeGoods === 0 &&
    side.commodities === 0 &&
    !side.note &&
    !side.actionCard &&
    !side.secret &&
    side.fragments.length === 0 &&
    !side.support
  );
}

export const isStagedEmpty = (s: Staged): boolean => isSideEmpty(s.give) && isSideEmpty(s.receive);

/** The staged combination a listed deal stands for, seen from the proposer's chair. */
export function dealToStaged(deal: DecodedTradeOffer): Staged {
  const d = deal.details;
  const give = emptySide();
  const receive = emptySide();
  if (deal.category === "mutual_support") {
    give.support = true;
    receive.support = true;
    return { give, receive };
  }
  give.tradeGoods = d.giveTradeGoods ?? 0;
  give.commodities = d.giveCommodities ?? 0;
  give.note = d.promissoryNote ?? d.givenNote ?? null;
  give.actionCard = d.actionCard ?? d.givenActionCard ?? null;
  give.secret = d.secretObjective ?? null;
  give.fragments = d.fragment ? [d.fragment] : [];
  receive.tradeGoods = d.receiveTradeGoods ?? 0;
  receive.commodities = d.receiveCommodities ?? 0;
  receive.note = d.receivedNote ?? null;
  // A sale (`pn`, `ac`, `so`, `fr`) is paid in trade goods by the partner; `np`/`cp` already carry
  // the payment on the give side, so the price only counts when something is sold.
  if (d.promissoryNote || d.actionCard || d.secretObjective || d.fragment) {
    receive.tradeGoods += d.price ?? 0;
  }
  return { give, receive };
}

const sideKey = (s: StagedSide): string =>
  JSON.stringify([
    s.tradeGoods,
    s.commodities,
    s.note,
    s.actionCard,
    s.secret,
    [...s.fragments].sort(),
    s.support,
  ]);

/** Canonical identity of a staged combination. */
export const stagedKey = (s: Staged): string => `${sideKey(s.give)}>${sideKey(s.receive)}`;

export interface StagedDeal {
  id: string;
  deal: DecodedTradeOffer;
  staged: Staged;
}

/** Decode a server deal list (decline excluded) into stageable deals. */
export function stageableDeals(options: readonly ChoiceOptionDto[]): StagedDeal[] {
  return options
    .filter((o) => o.id !== "decline" && o.kind !== "decline")
    .map((o) => {
      const deal = decodeTradeOption(o);
      return { id: deal.id, deal, staged: dealToStaged(deal) };
    });
}

/** The listed deal id a staged combination stands for, or null when the server lists none. */
export function stagedToDealId(staged: Staged, deals: readonly StagedDeal[]): string | null {
  const key = stagedKey(staged);
  return deals.find((d) => stagedKey(d.staged) === key)?.id ?? null;
}

/** How far two staged combinations are apart: amount differences plus 3 per differing item. */
export function stagedDistance(a: Staged, b: Staged): number {
  const side = (x: StagedSide, y: StagedSide): number => {
    let n = Math.abs(x.tradeGoods - y.tradeGoods) + Math.abs(x.commodities - y.commodities);
    const item = (p: unknown, q: unknown) => (p === q ? 0 : 3);
    n += item(x.note, y.note) + item(x.actionCard, y.actionCard) + item(x.secret, y.secret);
    n += item(x.support, y.support);
    n += item([...x.fragments].sort().join(","), [...y.fragments].sort().join(","));
    return n;
  };
  return side(a.give, b.give) + side(a.receive, b.receive);
}

/** The listed deals closest to a staged combination (never the exact match), nearest first. */
export function nearestDeals(
  staged: Staged,
  deals: readonly StagedDeal[],
  limit = 3,
): StagedDeal[] {
  return deals
    .map((d) => ({ d, dist: stagedDistance(staged, d.staged) }))
    .filter(({ dist }) => dist > 0)
    .sort((a, b) => a.dist - b.dist || a.d.id.localeCompare(b.d.id))
    .slice(0, limit)
    .map(({ d }) => d);
}

/** What the listed deals allow on each side: the most that can be staged and which items exist. */
export interface StagingLimits {
  give: SideLimits;
  receive: SideLimits;
}
export interface SideLimits {
  maxTradeGoods: number;
  maxCommodities: number;
  notes: string[];
  actionCards: string[];
  secrets: string[];
  fragments: string[];
  support: boolean;
}

const uniq = (xs: (string | null)[]): string[] => [...new Set(xs.filter((x): x is string => !!x))];

export function stagingLimits(deals: readonly StagedDeal[]): StagingLimits {
  const side = (pick: (s: Staged) => StagedSide): SideLimits => {
    const sides = deals.map((d) => pick(d.staged));
    return {
      maxTradeGoods: Math.max(0, ...sides.map((s) => s.tradeGoods)),
      maxCommodities: Math.max(0, ...sides.map((s) => s.commodities)),
      notes: uniq(sides.map((s) => s.note)),
      actionCards: uniq(sides.map((s) => s.actionCard)),
      secrets: uniq(sides.map((s) => s.secret)),
      fragments: uniq(sides.flatMap((s) => s.fragments)),
      support: sides.some((s) => s.support),
    };
  };
  return { give: side((s) => s.give), receive: side((s) => s.receive) };
}

/** A plain-words reason why a staged combination matches no listed deal. */
export function whyNoDeal(staged: Staged, deals: readonly StagedDeal[]): string[] {
  const reasons: string[] = [];
  if (isStagedEmpty(staged)) return ["Nothing is staged yet."];
  const limits = stagingLimits(deals);
  const amount = (label: string, n: number, max: number, who: string) => {
    if (n > max) reasons.push(`${who} can list at most ${max} ${label} right now.`);
  };
  amount("trade goods", staged.give.tradeGoods, limits.give.maxTradeGoods, "You");
  amount("commodities", staged.give.commodities, limits.give.maxCommodities, "You");
  amount("trade goods", staged.receive.tradeGoods, limits.receive.maxTradeGoods, "The partner");
  amount("commodities", staged.receive.commodities, limits.receive.maxCommodities, "The partner");
  const single = (a: StagedSide, b: StagedSide) => {
    const count = (s: StagedSide) =>
      [s.note, s.actionCard, s.secret, s.fragments.length ? "f" : null, s.support ? "s" : null].filter(
        Boolean,
      ).length;
    return count(a) + count(b);
  };
  if (single(staged.give, staged.receive) > 1 && reasons.length === 0) {
    reasons.push("The server lists no deal that combines these items.");
  }
  if (reasons.length === 0) {
    reasons.push("The server lists no deal with exactly these amounts.");
  }
  return reasons;
}

/** Parse one `Terms::describe` part into a side (notes are anything not a known phrase). */
function addPart(side: StagedSide, raw: string): void {
  const part = raw.trim();
  let m: RegExpMatchArray | null;
  if ((m = part.match(/^(\d+) trade goods$/))) side.tradeGoods += parseInt(m[1], 10);
  else if ((m = part.match(/^(\d+) commodities$/))) side.commodities += parseInt(m[1], 10);
  else if ((m = part.match(/^(\d+) relic fragments$/)))
    side.fragments.push(...Array.from({ length: parseInt(m[1], 10) }, () => "relic"));
  else if ((m = part.match(/^the action card (.+)$/))) side.actionCard = m[1];
  else if ((m = part.match(/^the secret objective (.+)$/))) side.secret = m[1];
  else if (part && part !== "nothing") side.note = part;
}

function parseSide(text: string): StagedSide {
  const side = emptySide();
  for (const part of text.split(", ")) addPart(side, part);
  return side;
}

/**
 * The offer named in an answer prompt ("`Faction` gives `terms` for `terms` -- accept?"), seen from
 * the answerer's chair: what they give is what the proposer asked for. Null when the prompt is not
 * of that shape, so the desk degrades to the sentence it has today.
 */
export function parseOfferPrompt(prompt: string): { proposer: string; staged: Staged } | null {
  const text = prompt.replace(/\s+--\s+accept\?\s*$/, "");
  const at = text.indexOf(" gives ");
  if (at <= 0) return null;
  const proposer = text.slice(0, at);
  const rest = text.slice(at + " gives ".length);
  // Terms never contain " for " themselves, except in an odd card name: take the first split.
  const cut = rest.indexOf(" for ");
  if (cut < 0) return null;
  const proposerGives = parseSide(rest.slice(0, cut));
  const proposerGets = parseSide(rest.slice(cut + " for ".length));
  return { proposer, staged: { give: proposerGets, receive: proposerGives } };
}

/** Plain-text lines for a side, for summaries ("2 trade goods", "the note ra:jolnar"). */
export function sideLines(side: StagedSide): string[] {
  const lines: string[] = [];
  if (side.tradeGoods) lines.push(`${side.tradeGoods} trade goods`);
  if (side.commodities) lines.push(`${side.commodities} commodities`);
  if (side.support) lines.push("Support for the Throne");
  if (side.note) lines.push(`Promissory Note: ${side.note}`);
  if (side.actionCard) lines.push(`Action Card: ${side.actionCard}`);
  if (side.secret) lines.push(`Secret Objective: ${side.secret}`);
  if (side.fragments.length) lines.push(`${side.fragments.length} relic fragments`);
  return lines;
}
