// The action cards: the hand of the viewer as printed, and the deck card by card. The engine says
// what the viewer holds, what the discard pile has and what the hand limit is; the catalogue has
// the printed cards and the deck that a game begins with. Copies of a card have ids of their own
// and one name: a card here is a printed name.
import type { ActionCardView, ActionCardsView } from "../../model";
import type { Catalog, Choice, ChoiceOption, SessionUpdate } from "../wire";
import { plain, timing } from "./components";
import { titleCase } from "./names";

/** What the player set for cards in this page. Only `never` changes what is sent. */
export interface CardSettings {
  /** The printed names of the cards that the viewer is never offered. */
  never: string[];
  /** The printed names of the cards that the viewer acts as if holding. Not wired to anything yet. */
  faked: string[];
}

export const NO_CARD_SETTINGS: CardSettings = { never: [], faked: [] };

/** A card with the window "Action" is a component action: no window ever offers it. */
const isAction = (window: string) => window.trim().toLowerCase() === "action";

export function selectActionCards(
  update: SessionUpdate,
  catalog: Catalog | null,
  settings: CardSettings,
): ActionCardsView | null {
  const viewer = update.viewer;
  const me =
    viewer.role === "player"
      ? update.view.players.find((player) => player.id === viewer.seat)
      : null;
  if (!me) {
    return null;
  }
  const deck = catalog?.components.action_cards ?? [];
  const printed = new Map(deck.map((card) => [card.id, card]));
  const nameOf = (id: string) => printed.get(id)?.name ?? titleCase(id);
  const count = (ids: string[]) => {
    const counts = new Map<string, number>();
    for (const id of ids) {
      counts.set(nameOf(id), (counts.get(nameOf(id)) ?? 0) + 1);
    }
    return counts;
  };
  const facts = update.action_cards;
  const held = count(me.held_action_cards ?? []);
  const gone = [...(facts?.discard ?? []), ...(facts?.salvage ?? [])];
  const discarded = count(gone);
  const total = count(deck.map((card) => card.id));
  // The first copy of a name has its text: the copies are printed the same.
  const byName = new Map<string, ActionCardView>();
  for (const card of deck) {
    if (!byName.has(card.name)) {
      const action = isAction(card.window);
      byName.set(card.name, {
        name: card.name,
        window: action ? "ACTION:" : timing(card.window),
        text: plain(card.text),
        action,
      });
    }
  }
  const card = (name: string): ActionCardView =>
    byName.get(name) ?? { name, window: "", text: "", action: false };
  const names = [...new Set([...byName.keys(), ...held.keys(), ...discarded.keys()])].sort();
  return {
    held: me.action_cards_count,
    limit: facts?.hand_limit ?? null,
    deck: facts ? { left: facts.deck, total: deck.length, discarded: gone.length } : null,
    hand: [...held.keys()].sort().map((name) => ({
      ...card(name),
      copies: held.get(name) ?? 0,
      never: card(name).action ? null : settings.never.includes(name),
    })),
    faked: settings.faked.filter((name) => !held.has(name)).map(card),
    reference: names.map((name) => {
      const all = total.get(name) ?? 0;
      const mine = held.get(name) ?? 0;
      const out = discarded.get(name) ?? 0;
      return {
        ...card(name),
        total: all,
        discarded: out,
        held: mine,
        open: Math.max(0, all - out - mine),
        faked: mine === 0 && settings.faked.includes(name),
      };
    }),
  };
}

/** The printed names of the cards that an option offers to play. Compare `reaction_modes::card_names`. */
function cardNames(option: ChoiceOption): string[] {
  const listed = option.payload?.card_names;
  if (Array.isArray(listed) && listed.length) {
    return listed.filter((name): name is string => typeof name === "string");
  }
  const one = option.payload?.card_name;
  return typeof one === "string" ? [one] : [];
}

const isOffer = (option: ChoiceOption) =>
  (option.kind === "ability" && option.id.startsWith("reaction:")) || option.kind === "action_card";
const isDecline = (option: ChoiceOption) => option.kind === "decline";

/** What "never offer" does to one question. */
export type Offers =
  | { kind: "unchanged" }
  /** Some options were Never cards; the rest still needs an answer. */
  | { kind: "narrowed"; choice: Choice }
  /** Every card of the question is a Never card: the page answers with the decline. */
  | { kind: "declined"; option: ChoiceOption; cards: string[] };

/**
 * Applies the Never cards of the viewer to a question. The same rule as
 * `ti4_engine::reaction_modes::filter`, which the server applies for a seat; the engine of this
 * page is asked every question, so the page answers.
 */
export function filterOffers(choice: Choice, never: string[]): Offers {
  if (!never.length) {
    return { kind: "unchanged" };
  }
  const dropped: string[] = [];
  const kept = choice.options.filter((option) => {
    const names = cardNames(option);
    if (!isOffer(option) || !names.length || !names.every((name) => never.includes(name))) {
      return true;
    }
    dropped.push(...names.filter((name) => !dropped.includes(name)));
    return false;
  });
  if (!dropped.length) {
    return { kind: "unchanged" };
  }
  if (kept.every(isDecline)) {
    // Nothing left and nothing to decline with: the question is asked whole.
    return kept.length
      ? { kind: "declined", option: kept[0], cards: dropped }
      : { kind: "unchanged" };
  }
  return { kind: "narrowed", choice: { ...choice, options: kept } };
}

/** The update as the viewer is asked it: without the options of the Never cards. */
export function withoutNever(update: SessionUpdate, never: string[]): SessionUpdate {
  const pending = update.pending_choice;
  const offers = pending ? filterOffers(pending.choice, never) : null;
  return pending && offers?.kind === "narrowed"
    ? { ...update, pending_choice: { ...pending, choice: offers.choice } }
    : update;
}
