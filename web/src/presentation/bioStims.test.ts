import { describe, expect, it } from "vitest";
import { bioStimsStatus } from "./technologyData.ts";
import type { BoardView, PlayerView } from "../protocol/types.ts";

const player = (technologies: string[], exhausted: string[] = []) =>
  ({ id: "p1", technologies, exhausted_technologies: exhausted }) as unknown as PlayerView;

const board = (exhausted: boolean) =>
  ({
    systems: {
      s1: {
        planets: {
          wellon: { planet_id: "wellon", controlled_by: "p1", exhausted },
          // no technology specialty: never a Bio-Stims target
          jord: { planet_id: "jord", controlled_by: "p1", exhausted: true },
        },
      },
    },
  }) as unknown as BoardView;

describe("Bio-Stims status (mirror of the engine's end-of-turn offer)", () => {
  it("is null without the card", () => {
    expect(bioStimsStatus(player(["st"]), board(true))).toBeNull();
    expect(bioStimsStatus(null, board(true))).toBeNull();
  });

  it("is usable when ready with an exhausted specialty planet or another exhausted technology", () => {
    const s = bioStimsStatus(player(["bs", "st"], ["st"]), board(true))!;
    expect(s).toMatchObject({ ready: true, usable: true });
    expect(s.planets).toEqual(["Wellon"]);
    expect(s.technologies).toEqual(["Sarween Tools"]);
  });

  it("ignores ready planets, planets without a specialty and itself", () => {
    const s = bioStimsStatus(player(["bs"], []), board(false))!;
    expect(s).toMatchObject({ ready: true, usable: false, planets: [], technologies: [] });
  });

  it("is not usable while exhausted, but lists what it could ready", () => {
    const s = bioStimsStatus(player(["bs"], ["bs"]), board(true))!;
    expect(s).toMatchObject({ ready: false, usable: false });
    expect(s.technologies).toEqual([]);
    expect(s.planets).toEqual(["Wellon"]);
  });
});
