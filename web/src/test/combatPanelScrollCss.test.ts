import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

describe("phone combat panel scrolling", () => {
  const css = readFileSync(resolve(__dirname, "../index.css"), "utf8");
  it("is one page scroll with room under the last card for the dock and browser bar", () => {
    // Two classes: it must beat the phone rule that pins every .choice-workflow-modal to 100dvh.
    const start = css.indexOf(".combat-dialog .combat-arena-panel {\n    width: 100%");
    expect(start).toBeGreaterThan(-1);
    const block = css.slice(start, css.indexOf("}", start));
    expect(block).toContain("max-height: none");
    expect(block).toContain("overflow: visible");
    expect(block).toContain("height: auto");
    expect(block).toMatch(/padding-bottom: calc\(\d+px \+ env\(safe-area-inset-bottom/);
  });
});
