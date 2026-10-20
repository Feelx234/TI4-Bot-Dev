import { describe, expect, it } from "vitest";
import { ordinal, passStatusFor, trackPasses } from "./passOrder.tsx";

describe("trackPasses", () => {
  it("numbers passes one at a time", () => {
    let t = trackPasses(null, 2, []);
    t = trackPasses(t, 2, ["b"]);
    t = trackPasses(t, 2, ["b", "d"]);
    t = trackPasses(t, 2, ["b", "d", "a"]);
    expect(t.places).toEqual({ b: 1, d: 2, a: 3 });
  });

  it("does not guess a place for passes that arrive together", () => {
    const t = trackPasses(null, 2, ["a", "b"]);
    expect(t.places).toEqual({ a: null, b: null });
    // a later single pass is still the next place
    expect(trackPasses(t, 2, ["a", "b", "c"]).places.c).toBe(3);
  });

  it("starts over in a new round", () => {
    const t = trackPasses(trackPasses(null, 2, []), 2, ["a"]);
    expect(trackPasses(t, 3, []).places).toEqual({});
    expect(trackPasses(t, 3, ["c"]).places).toEqual({ c: 1 });
  });

  it("keeps earlier places when a seat is no longer passed", () => {
    let t = trackPasses(null, 1, []);
    t = trackPasses(t, 1, ["a"]);
    t = trackPasses(t, 1, ["a", "b"]);
    t = trackPasses(t, 1, ["b"]);
    expect(t.places).toEqual({ b: 2 });
  });
});

describe("passStatusFor", () => {
  it("labels the place when known and only the badge otherwise", () => {
    const t = trackPasses(trackPasses(null, 1, []), 1, ["a"]);
    expect(passStatusFor(t, "a", true, true).label).toBe("Passed (1st)");
    expect(passStatusFor(null, "a", true, true).label).toBe("Passed");
    expect(passStatusFor(t, "a", false, true).passed).toBe(false);
    expect(passStatusFor(t, "b", true, false).passed).toBe(false);
  });
  it("words ordinals", () => {
    expect([1, 2, 3, 4, 11, 12, 21].map(ordinal)).toEqual(["1st", "2nd", "3rd", "4th", "11th", "12th", "21st"]);
  });
});
