import { expect, it } from "vitest";
import menuUpdate from "../fixtures/update-menu.json";
import type { Catalog, Choice, ChoiceOption, SessionUpdate } from "../wire";
import { filterOffers, selectActionCards, withoutNever } from "./cards";

// Seat a holds `scuttle` and `rally` in this update of the real engine.
const menu = menuUpdate as unknown as SessionUpdate;
const card = (id: string, name: string, window: string) => ({
  id,
  name,
  phase: "Action",
  window,
  text: `The text of ${name}.`,
});
const catalog = {
  components: {
    action_cards: [
      card("mb1", "Morale Boost", "At the start of a combat round"),
      card("mb2", "Morale Boost", "At the start of a combat round"),
      card("mb3", "Morale Boost", "At the start of a combat round"),
      card("rally", "Rally", "Action"),
      card("sabo1", "Sabotage", "When another player plays an action card other than Sabotage"),
      card("scuttle", "Scuttle", "At the start of your turn"),
    ],
  },
} as unknown as Catalog;
const withFacts: SessionUpdate = {
  ...menu,
  action_cards: { deck: 90, discard: ["mb2", "sabo1"], salvage: ["mb3"], hand_limit: 7 },
};
const NONE = { never: [], faked: [] };

it("shows the hand as printed, against the limit", () => {
  const view = selectActionCards(withFacts, catalog, { never: ["Scuttle"], faked: [] });
  expect(view).toMatchObject({ held: 2, limit: 7 });
  expect(view?.deck).toEqual({ left: 90, total: 6, discarded: 3 });
  expect(view?.hand).toEqual([
    // A component action is never offered in a window: it has no setting.
    {
      name: "Rally",
      window: "ACTION:",
      text: "The text of Rally.",
      action: true,
      copies: 1,
      never: null,
    },
    {
      name: "Scuttle",
      window: "At the start of your turn:",
      text: "The text of Scuttle.",
      action: false,
      copies: 1,
      never: true,
    },
  ]);
});

it("counts the copies of a card by its name: discarded, held, and open", () => {
  const view = selectActionCards(withFacts, catalog, { never: [], faked: ["Sabotage", "Rally"] });
  const row = (name: string) => view?.reference.find((item) => item.name === name);
  expect(view?.reference.map((item) => item.name)).toEqual([
    "Morale Boost",
    "Rally",
    "Sabotage",
    "Scuttle",
  ]);
  // A card on Garbozia is not in a hand either.
  expect(row("Morale Boost")).toMatchObject({ total: 3, discarded: 2, held: 0, open: 1 });
  expect(row("Sabotage")).toMatchObject({ total: 1, discarded: 1, open: 0, faked: true });
  // A card that is held is not faked.
  expect(row("Rally")).toMatchObject({ total: 1, held: 1, open: 0, faked: false });
  expect(view?.faked.map((item) => item.name)).toEqual(["Sabotage"]);
});

it("has the hand by id without the catalogue, and no hand for a spectator", () => {
  const view = selectActionCards(menu, null, NONE);
  expect(view?.hand.map((item) => item.name)).toEqual(["Rally", "Scuttle"]);
  expect(view).toMatchObject({ limit: null, deck: null });
  expect(selectActionCards({ ...menu, viewer: { role: "spectator" } }, catalog, NONE)).toBeNull();
});

const option = (id: string, kind: string, payload: Record<string, unknown> = {}): ChoiceOption => ({
  id,
  kind,
  label: id,
  payload,
});
const window = (...options: ChoiceOption[]): Choice => ({ player: "a", prompt: "React?", options });
const sabotage = option("reaction:sabotage", "ability", { card_name: "Sabotage" });
const boosts = option("mb", "action_card", { card_names: ["Morale Boost", "Fire Team"] });
const decline = option("decline", "decline");

it("leaves a question alone that offers no Never card", () => {
  expect(filterOffers(window(sabotage, decline), []).kind).toBe("unchanged");
  expect(filterOffers(window(sabotage, decline), ["Morale Boost"]).kind).toBe("unchanged");
  // An option that is not an offer of a card stays, whatever its payload names.
  const other = option("choose", "target", { card_name: "Sabotage" });
  expect(filterOffers(window(other, decline), ["Sabotage"]).kind).toBe("unchanged");
  // A slot of several cards stays while one of them is offered.
  expect(filterOffers(window(boosts, decline), ["Morale Boost"]).kind).toBe("unchanged");
  // Without a decline the question is asked whole.
  expect(filterOffers(window(sabotage), ["Sabotage"]).kind).toBe("unchanged");
});

it("declines a window that offers only Never cards", () => {
  expect(filterOffers(window(sabotage, decline), ["Sabotage"])).toEqual({
    kind: "declined",
    option: decline,
    cards: ["Sabotage"],
  });
});

it("leaves the Never cards out of a window that offers another card", () => {
  const offers = filterOffers(window(sabotage, boosts, decline), ["Sabotage"]);
  expect(offers.kind === "narrowed" && offers.choice.options).toEqual([boosts, decline]);
  const update: SessionUpdate = {
    ...menu,
    pending_choice: { nonce: "1", choice: window(sabotage, boosts, decline) },
  };
  expect(withoutNever(update, ["Sabotage"]).pending_choice?.choice.options).toEqual([
    boosts,
    decline,
  ]);
  expect(withoutNever(update, [])).toBe(update);
});
