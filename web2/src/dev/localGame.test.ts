import { expect, it } from "vitest";
import { saveKey } from "../session/savedGame";
import { SINGLE_PLAYER } from "./defaultGame";
import { DEFAULT_GAME, localGame } from "./LocalApp";

it("the bare address is no game: it is the start screen", () => {
  expect(localGame(new URLSearchParams(""))).toBeNull();
});

it("single player is seed 42, eight seats, and the first seat is played here", () => {
  // The game that the bare address was: what it saved is kept under the same key.
  expect(localGame(new URLSearchParams(SINGLE_PLAYER.slice(2)))).toEqual(DEFAULT_GAME);
  expect(DEFAULT_GAME).toEqual({ seed: 42, players: 8, humans: 1 });
  expect(saveKey(DEFAULT_GAME)).toBe("ti4.local.42.8.1");
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
