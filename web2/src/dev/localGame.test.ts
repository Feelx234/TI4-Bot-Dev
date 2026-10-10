import { expect, it } from "vitest";
import { DEFAULT_GAME, localGame } from "./LocalApp";

it("the bare address is seed 42, eight seats, and the first seat is played here", () => {
  expect(localGame(new URLSearchParams(""))).toEqual({ seed: 42, players: 8, humans: 1 });
  expect(localGame(new URLSearchParams(""))).toEqual(DEFAULT_GAME);
});

it("the demo and the gallery keep their own addresses", () => {
  expect(localGame(new URLSearchParams("example=live-picker"))).toBeNull();
  expect(localGame(new URLSearchParams("gallery"))).toBeNull();
});

it("an explicit game is read as before", () => {
  expect(localGame(new URLSearchParams("local=3&players=8&humans=1"))).toEqual({
    seed: 3,
    players: 8,
    humans: 1,
  });
  expect(localGame(new URLSearchParams("example=live-picker&local=5"))).toEqual({
    seed: 5,
    players: 8,
    humans: 1,
  });
});

it("a scenario is a game of its own, and the engine says its seed and seats", () => {
  expect(localGame(new URLSearchParams("scenario=late-game"))).toEqual({
    ...DEFAULT_GAME,
    scenario: "late-game",
  });
  expect(localGame(new URLSearchParams("scenario=late-game&humans=3"))?.humans).toBe(3);
});
