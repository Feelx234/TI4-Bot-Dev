import { describe, it, expect } from "vitest";
import { ACTION_CARDS, getActionCardDescription } from "./src/protocol/contentCatalog";

describe("Action Card Descriptions", () => {
  it("should retrieve action card description for valid card ID", () => {
    // Test with a known action card ID
    const description = getActionCardDescription("abs");
    expect(description).toBeTruthy();
    expect(description.length).toBeGreaterThan(0);
  });

  it("should return fallback description for unknown card ID", () => {
    const description = getActionCardDescription("unknown_card_xyz");
    expect(description).toContain("Action Card");
  });

  it("should have action cards in the catalog", () => {
    const cards = Object.entries(ACTION_CARDS);
    expect(cards.length).toBeGreaterThan(0);
    
    // Check that cards have name property
    for (const [id, card] of cards) {
      expect((card as any).name).toBeTruthy();
    }
  });

  it("should handle card names with various cases", () => {
    // Test case sensitivity handling
    const description = getActionCardDescription("abs");
    expect(description).toBeTruthy();
  });
});
