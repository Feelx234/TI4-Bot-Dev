import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import menuUpdate from "./fixtures/update-menu.json";
import type { Transport, TransportEvent } from "./transport";
import { type DraftStore, useLiveSession } from "./useLiveSession";
import type { ChoiceOption, SessionUpdate } from "./wire";

// A reaction window of seat a, over an update of the real engine.
const option = (id: string, kind: string, payload: Record<string, unknown> = {}): ChoiceOption => ({
  id,
  kind,
  label: id,
  payload,
});
const window = (nonce: string, ...options: ChoiceOption[]): SessionUpdate => ({
  ...(menuUpdate as unknown as SessionUpdate),
  pending_choice: { nonce, choice: { player: "a", prompt: "React?", options } },
});
const sabotage = option("reaction:sabotage", "ability", { card_name: "Sabotage" });
const scuttle = option("reaction:scuttle", "ability", { card_name: "Scuttle" });
const decline = option("decline", "decline");

function transport() {
  let listener: (event: TransportEvent) => void = () => {};
  const made = {
    catalog: null,
    subscribe: (to: typeof listener) => {
      listener = to;
      return () => {};
    },
    submitChoice: vi.fn(),
    undo: vi.fn(),
  };
  const tell = (update: SessionUpdate, canUndo = true) =>
    act(async () => listener({ kind: "update", update, canUndo }));
  return { made, transport: made as unknown as Transport, tell };
}

function store(never: string[]): DraftStore {
  const kept = new Map([["cards", JSON.stringify(never)]]);
  return {
    storage: {
      getItem: (key) => kept.get(key) ?? null,
      setItem: (key, value) => void kept.set(key, value),
      removeItem: (key) => void kept.delete(key),
    },
    key: "movement",
    cardsKey: "cards",
  };
}

const rows = (view: ReturnType<typeof useLiveSession>) => {
  const action = view.session?.view.action;
  return action?.kind === "flow"
    ? action.blocks.flatMap((block) => (block.kind === "menu" ? block.rows : []))
    : [];
};

it("passes a window that offers only cards that are never offered, and says so", async () => {
  const game = transport();
  const drafts = store(["Sabotage"]);
  const { result } = renderHook(() => useLiveSession(game.transport, drafts));
  await game.tell(window("7", sabotage, decline));
  expect(game.made.submitChoice).toHaveBeenCalledWith("7", "decline");
  expect(result.current.session?.view.toast?.text).toBe(
    "Sabotage: never offered. The window was passed.",
  );
  // The window is not a question of the player.
  expect(rows(result.current)).toEqual([]);
});

it("asks a window with another card, without the card that is never offered", async () => {
  const game = transport();
  const drafts = store(["Sabotage"]);
  const { result } = renderHook(() => useLiveSession(game.transport, drafts));
  await game.tell(window("7", sabotage, scuttle, decline));
  expect(game.made.submitChoice).not.toHaveBeenCalled();
  expect(rows(result.current).map((row) => row.title)).toHaveLength(2);
});

it("takes a passed window back with the answer before it", async () => {
  const game = transport();
  const drafts = store(["Sabotage"]);
  const { result } = renderHook(() => useLiveSession(game.transport, drafts));
  await game.tell(window("8", scuttle, decline));
  act(() => result.current.session?.dispatch({ type: "undo" }));
  expect(game.made.undo).toHaveBeenCalledTimes(1);
  // The undo opens the window that the page passed: it goes on.
  await game.tell(window("7", sabotage, decline));
  expect(game.made.undo).toHaveBeenCalledTimes(2);
  expect(game.made.submitChoice).not.toHaveBeenCalled();
  // The answer of the player is open again. The next window of the card is passed as before.
  await game.tell(window("6", scuttle, decline));
  await game.tell(window("9", sabotage, decline));
  expect(game.made.submitChoice).toHaveBeenCalledWith("9", "decline");
});

it("keeps the cards that are never offered for a reload, and not the faked ones", async () => {
  const game = transport();
  const drafts = store([]);
  const { result } = renderHook(() => useLiveSession(game.transport, drafts));
  await game.tell(window("7", scuttle, decline));
  act(() =>
    result.current.session?.dispatch({ type: "setCardOffer", card: "Scuttle", never: true }),
  );
  act(() =>
    result.current.session?.dispatch({ type: "setCardFake", card: "Sabotage", fake: true }),
  );
  expect(drafts.storage.getItem("cards")).toBe('["Scuttle"]');
  const cards = result.current.session?.view.reference.actionCards;
  expect(cards?.hand.find((card) => card.name === "Scuttle")?.never).toBe(true);
  expect(cards?.faked.map((card) => card.name)).toEqual(["Sabotage"]);
});
