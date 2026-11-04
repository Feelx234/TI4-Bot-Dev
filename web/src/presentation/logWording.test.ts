import { describe, expect, it } from "vitest";
import { friendlyLogLine } from "./logWording.ts";

describe("friendlyLogLine", () => {
  it("says what the Committee Formation offer asked", () => {
    const line = friendlyLogLine("b resolved offer discard law")!;
    expect(line.text).toBe("b was asked whether to discard a law (Committee Formation or a ministry card)");
    expect(line.title).toContain("nobody votes on that agenda");
    expect(line.title).toContain("does not say which was chosen");
  });

  it("names the elected-player pick and the agenda tricks", () => {
    expect(friendlyLogLine("a resolved committee elect")!.text).toBe(
      "a chose the elected player with Committee Formation (nobody votes on that agenda)",
    );
    expect(friendlyLogLine("b resolved quash")!.text).toBe("b was asked whether to use Quash on the agenda");
    expect(friendlyLogLine("b resolved use quash")!.text).toBe("b was asked whether to use Quash on the agenda");
    expect(friendlyLogLine("c resolved use favor")!.text).toContain("Political Favor");
    expect(friendlyLogLine("c resolved use secret")!.text).toContain("Political Secret");
  });

  it("leaves every other line alone", () => {
    expect(friendlyLogLine("a resolved action menu")).toBeNull();
    expect(friendlyLogLine("b voted for b")).toBeNull();
    expect(friendlyLogLine("Decision resolved")).toBeNull();
  });
});
