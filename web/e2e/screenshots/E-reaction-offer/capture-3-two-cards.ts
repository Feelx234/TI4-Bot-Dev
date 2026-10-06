import { test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openMockedGame } from "../_shared/mockGame";
import { actor } from "../_shared/fixtures";
import { playerWithHand, opponent } from "../_shared/players";

// Two cards can answer the same window: both are offered, and Spacebar passes.
test("reaction: two cards offered for one window", async ({ page }, testInfo) => {
  await openMockedGame(page, {
    players: [playerWithHand(), opponent],
    choice: {
      prompt: "play an action card (when ACTION_CARD_PLAYED)",
      context: { subtype: "play_reaction_when_ACTION_CARD_PLAYED", actor, source: { Rule: "22.1" } },
      options: [
        { id: "sabo1", label: "play Sabotage", kind: "action_card", payload: { card: "sabo1", card_name: "Sabotage" } },
        { id: "sabo2", label: "play Sabotage", kind: "action_card", payload: { card: "sabo2", card_name: "Sabotage" } },
        { id: "decline", label: "Pass", kind: "decline" },
      ],
    },
  });
  await page.waitForTimeout(800);
  await shot(page, testInfo, "3-two-cards");
});
