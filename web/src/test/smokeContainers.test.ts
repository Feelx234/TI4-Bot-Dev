import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

// smokePlaythrough.ts imports @playwright/test, which does not load under Vitest: read the list
// from its source.
const decisionContainers = (): string[] => {
  const source = readFileSync(path.resolve(__dirname, "../../e2e/smokePlaythrough.ts"), "utf8");
  const body = /export const DECISION_CONTAINERS = \[([^\]]*)\]/.exec(source)?.[1] ?? "";
  return Array.from(body.matchAll(/"([^"]+)"/g), (m) => m[1]);
};

describe("DECISION_CONTAINERS (smoke harness)", () => {
  it("covers the map-pick retreat bar, so its Confirm button is clicked", () => {
    // Run o3-01-2331: the retreat destination moved to RetreatPickBar; the driver only clicked
    // the highlighted hexes and never Confirm, and the game stuck on retreat_to.
    const source = readFileSync(path.resolve(__dirname, "../components/RetreatPickBar.tsx"), "utf8");
    const root = /data-testid="(retreat-pick-bar)"/.exec(source)?.[1];
    expect(root).toBe("retreat-pick-bar");
    expect(decisionContainers()).toContain("turn-action-bar");
    expect(decisionContainers()).toContain(root);
  });
});
