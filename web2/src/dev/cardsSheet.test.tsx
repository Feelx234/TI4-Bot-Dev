import { fireEvent, render, screen, within } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { GameProvider } from "../game/context";
import { ReferenceDrawer } from "../game/shell/Reference";
import type { Intent } from "../model";
import menuUpdate from "../session/fixtures/update-menu.json";
import { selectShell } from "../session/select/shell";
import type { Catalog, SessionUpdate } from "../session/wire";

// The Cards sheet over an update of the real engine: seat a holds `scuttle` and `rally`. The test
// is here because `dev` is the layer that may import both the session and the components.
const card = (id: string, name: string, window: string) => ({
  id,
  name,
  phase: "Action",
  window,
  text: `The text of ${name}.`,
});
const catalog = {
  exploration: { cards: [], decks: {}, attachments: [] },
  components: {
    abilities: [],
    factions: {},
    leaders: [],
    technologies: [],
    relics: [],
    legendary: [],
    action_cards: [
      card("mb1", "Morale Boost", "At the start of a combat round"),
      card("mb2", "Morale Boost", "At the start of a combat round"),
      card("rally", "Rally", "Action"),
      card("scuttle", "Scuttle", "At the start of your turn"),
    ],
  },
} as Catalog;
const update: SessionUpdate = {
  ...(menuUpdate as unknown as SessionUpdate),
  action_cards: { deck: 90, discard: ["mb2"], hand_limit: 7 },
};
const local = {
  inspected: null,
  staged: null,
  sent: null,
  error: null,
  replaying: null,
  canUndo: false,
  movement: {},
  landing: {},
  landingKind: null,
  production: { builds: [], planets: [], goods: 0 },
  productionStage: "build" as const,
  handled: [],
  step: null,
  remaining: null,
  planNote: null,
  stepping: null,
  draftLocked: null,
  draft: null,
};

function open(settings = { never: [] as string[], faked: [] as string[] }) {
  const view = selectShell(update, local, catalog, settings);
  const dispatch = vi.fn<(intent: Intent) => void>();
  render(
    <GameProvider seats={view.seats} dispatch={dispatch}>
      <ReferenceDrawer
        id="cards"
        view={view.reference}
        players={view.players.rows}
        onClose={() => {}}
      />
    </GameProvider>,
  );
  return dispatch;
}

it("shows the hand as printed, with the count against the limit", () => {
  open();
  const hand = within(screen.getByRole("region", { name: "Your action cards" }));
  expect(hand.getByText("Hand 2 / 7")).toBeInTheDocument();
  const scuttle = within(hand.getByRole("article", { name: "Scuttle" }));
  expect(scuttle.getByText("At the start of your turn:")).toBeInTheDocument();
  expect(scuttle.getByText("The text of Scuttle.")).toBeInTheDocument();
  // A component action is not offered in a window: it has no setting.
  expect(
    within(hand.getByRole("article", { name: "Rally" })).queryByRole("button"),
  ).not.toBeInTheDocument();
});

it("sets a card of the hand to Never offered", () => {
  const dispatch = open();
  fireEvent.click(screen.getByRole("button", { name: "Never offer Scuttle" }));
  expect(dispatch).toHaveBeenCalledWith({ type: "setCardOffer", card: "Scuttle", never: true });
});

it("lists the deck with what is unseen, and fakes a card that is not held", () => {
  const dispatch = open();
  const deck = within(screen.getByRole("region", { name: "Action card deck" }));
  expect(deck.getByText("90 of 4 cards left · 1 discarded")).toBeInTheDocument();
  expect(deck.getByText("1 of 2 unseen")).toBeInTheDocument();
  expect(deck.getByText("1 discarded")).toBeInTheDocument();
  expect(deck.getAllByText("1 in your hand")).toHaveLength(2);
  // A held card is not faked.
  expect(deck.queryByRole("button", { name: "Fake Rally" })).not.toBeInTheDocument();
  fireEvent.click(deck.getByRole("button", { name: "Fake Morale Boost" }));
  expect(dispatch).toHaveBeenCalledWith({ type: "setCardFake", card: "Morale Boost", fake: true });
});

it("shows a faked card with the hand, marked, and stops faking it", () => {
  const dispatch = open({ never: [], faked: ["Morale Boost"] });
  const faked = within(screen.getByRole("article", { name: "Morale Boost, faked" }));
  expect(faked.getByText("Faked · not in your hand")).toBeInTheDocument();
  fireEvent.click(faked.getByRole("button", { name: "Stop faking" }));
  expect(dispatch).toHaveBeenCalledWith({ type: "setCardFake", card: "Morale Boost", fake: false });
});
