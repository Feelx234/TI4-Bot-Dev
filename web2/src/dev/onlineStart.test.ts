import { expect, it } from "vitest";
import { createRequest, onlineAddress, onlineStart } from "./onlineStart";

const read = (search: string) => onlineStart(new URLSearchParams(search));

it("the bare address asks for a game of eight with random seats, and the server chooses the rest", () => {
  expect(read("online")).toEqual({ nickname: "Player" });
  expect(createRequest(read("online"))).toEqual({
    player_count: 8,
    nickname: "Player",
    fill_seats: "random",
  });
});

it("the address names the seed, the nickname and the map", () => {
  const start = read("online&seed=3&name=Ada&map=random");
  expect(start).toEqual({ nickname: "Ada", seed: 3, map: "random" });
  expect(createRequest(start)).toEqual({
    player_count: 8,
    nickname: "Ada",
    fill_seats: "random",
    seed: 3,
    map: "random",
  });
  // A map of the server is named by its alias; "random" is the board from the seed.
  expect(createRequest(read("online&map=milty8"))).toMatchObject({ map_template: "milty8" });
  expect(createRequest(read("online&map=milty8"))).not.toHaveProperty("map");
});

it("a seed that is no number is left to the server", () => {
  expect(read("online&seed=abc")).toEqual({ nickname: "Player" });
  expect(read("online&seed=-1&name=%20")).toEqual({ nickname: "Player" });
  expect(read("online&seed=0").seed).toBe(0);
});

it("the address of a start is read back as that start", () => {
  expect(onlineAddress()).toBe("?online");
  expect(onlineAddress({ seed: 42 })).toBe("?online&seed=42");
  const start = { nickname: "Ada Lovelace", seed: 7, map: "random" };
  expect(read(onlineAddress(start).slice(1))).toEqual(start);
});
