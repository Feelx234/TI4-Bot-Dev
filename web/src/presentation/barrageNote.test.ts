import { describe, expect, it } from "vitest";
import { barrageLines, barrageSides } from "./barrageNote.ts";

const seats = { attacker: "a", defender: "b" };
const unit = (owner: string, unit_type: string, planet: string | null = null) => ({
  owner,
  unit_type,
  planet,
});
const name = (seat: string) => (seat === "a" ? "Ann" : "Bob");
const lines = (hits: Record<string, number>, start: ReturnType<typeof unit>[], faction = "ghost") =>
  barrageLines(
    barrageSides(hits, start, seats, (seat) => (seat === "a" ? faction : "muaat")),
    name,
  );

describe("barrageLines", () => {
  it("says the hits have no effect when the opponent has no fighters", () => {
    const [line] = lines({ a: 1 }, [unit("a", "destroyer"), unit("b", "carrier")]);
    expect(line.text).toBe("Ann: 1 hit. No enemy fighters: the hits have no effect.");
  });
  it("ignores fighters on planets and the roller's own fighters", () => {
    const [line] = lines({ a: 2 }, [unit("a", "fighter"), unit("b", "fighter", "x")]);
    expect(line.text).toContain("No enemy fighters");
  });
  it("counts the fighters destroyed and the excess", () => {
    const start = [unit("b", "fighter"), unit("b", "fighter_ii")];
    expect(lines({ a: 2 }, start)[0].text).toBe("Ann: 2 hits destroy 2 enemy fighters.");
    expect(lines({ a: 3 }, start)[0].text).toBe("Ann: 3 hits destroy 2 fighters; the 1 excess have no effect.");
    expect(lines({ a: 1 }, start)[0].text).toBe("Ann: 1 hit destroys 1 enemy fighter.");
  });
  it("Raid Formation damages sustain ships with the excess, even with no fighters", () => {
    expect(lines({ a: 2 }, [unit("b", "carrier")], "argent")[0].text).toContain("Raid Formation: each hit damages");
    expect(lines({ a: 3 }, [unit("b", "fighter")], "argent")[0].text).toContain("the 2 excess hits damage a ship");
  });
  it("reports a miss and skips a side that rolled nothing", () => {
    const result = lines({ b: 0 }, [unit("a", "fighter")]);
    expect(result).toHaveLength(1);
    expect(result[0].text).toBe("Bob: no hits.");
  });
});
