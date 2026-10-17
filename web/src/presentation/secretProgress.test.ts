import { describe, expect, it } from "vitest";
import { CONTENT } from "../protocol/catalogStore.ts";
import {
  EVENT_SECRETS,
  SECRET_PROGRESS_UNITS,
  secretNoProgressNote,
  secretProgressDisplay,
} from "./secretProgress.ts";

describe("secret objective progress wording", () => {
  it("words a count as have / threshold plus what counts", () => {
    const shown = secretProgressDisplay("ctr", { have: 4, threshold: 6, satisfied: false });
    expect(shown?.text).toBe("4 / 6 systems with at least one of your ships");
    expect(shown?.percent).toBe(67);
    expect(secretProgressDisplay("eh", { have: 15, threshold: 12, satisfied: true })?.percent).toBe(
      100,
    );
  });

  it("reminds that Strengthen Bonds counts play-area notes only, not the hand", () => {
    const shown = secretProgressDisplay("sb", { have: 0, threshold: 1, satisfied: false });
    expect(shown?.hint).toMatch(/face-up in your play area/);
    expect(shown?.hint).toMatch(/hand do not/);
    expect(secretProgressDisplay("ctr", { have: 1, threshold: 6, satisfied: false })?.hint).toBeNull();
  });

  it("has no bar without a count", () => {
    expect(secretProgressDisplay("ctr", undefined)).toBeNull();
    expect(secretNoProgressNote("pe")).toMatch(/event/);
    expect(secretNoProgressNote("ctr")).toBeNull();
  });

  it("covers every base secret as either a count or an event", () => {
    const aliases = Object.keys(CONTENT.secretObjectives).filter(
      (id) => id.length <= 4,
    );
    expect(aliases.length).toBe(40);
    for (const alias of aliases) {
      const counted = alias in SECRET_PROGRESS_UNITS;
      const event = EVENT_SECRETS.has(alias);
      expect(counted !== event, alias).toBe(true);
    }
  });
});
