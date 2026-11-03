import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

describe("paused plan banner placement", () => {
  const css = readFileSync(resolve(__dirname, "../index.css"), "utf8");
  const block = (selector: string) => {
    const start = css.indexOf(`\n${selector} {`);
    expect(start).toBeGreaterThan(-1);
    return css.slice(start, css.indexOf("}", start));
  };
  it("sits below the game header like the turn redo bar (run o2-23-0017: the header covered its Dismiss)", () => {
    // The overlay layer is below the header, so a banner at the top of it is hidden behind the bar.
    const top = (selector: string) => block(selector).match(/\n\s*top: (\d+)px;/)?.[1];
    expect(top(".turn-redo")).toBe("56");
    expect(top(".paused-plan")).toBe(top(".turn-redo"));
  });
});
