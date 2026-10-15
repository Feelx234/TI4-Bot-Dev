import { describe, expect, it } from "vitest";
import { leaderUnlockToasts, readLeaderStatuses } from "./leaderUnlocks.ts";

const seat = (leaders: Record<string, string>) => [{ id: "d", leaders }];

describe("leader unlock toasts (fb_1792039768)", () => {
  it("announces an unlocked commander with its printed ability", () => {
    const before = readLeaderStatuses(seat({ solagent: "readied", solcommander: "locked", solhero: "locked" }));
    const after = readLeaderStatuses(seat({ solagent: "readied", solcommander: "unlocked", solhero: "locked" }));
    const toasts = leaderUnlockToasts(before, after);
    expect(toasts).toHaveLength(1);
    expect(toasts[0].actor).toBe("d");
    expect(toasts[0].parts[0]).toBe(
      "unlocked the commander Claire Gibson: At the start of a ground combat on a planet you control: You may place 1 infantry from your reinforcements on that planet.",
    );
  });

  it("is quiet on the first state, for agents and for leaders that stay unlocked", () => {
    const after = readLeaderStatuses(seat({ solagent: "exhausted", solcommander: "unlocked" }));
    expect(leaderUnlockToasts(null, after)).toEqual([]);
    expect(leaderUnlockToasts(after, after)).toEqual([]);
    expect(leaderUnlockToasts(readLeaderStatuses(seat({ solagent: "readied", solcommander: "unlocked" })), after)).toEqual([]);
  });
});
