import { test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openMockedGame } from "../_shared/mockGame";
import { actor } from "../_shared/fixtures";
import { playerWithHand, opponent } from "../_shared/players";

// Another player plays an action card; the viewer holds Sabotage and is offered it. The decision
// is shaped like the engine's (prompt, subtype and option fields from crates/ti4-engine reactions.rs).
test("reaction: offered Sabotage when an action card is played", async ({ page }, testInfo) => {
  await openMockedGame(page, {
    players: [playerWithHand(), opponent],
    choice: {
      prompt: "play an action card (when ACTION_CARD_PLAYED)",
      context: { subtype: "play_reaction_when_ACTION_CARD_PLAYED", actor, source: { Rule: "22.1" } },
      options: [
        {
          id: "sabo1",
          label: "play Sabotage",
          kind: "action_card",
          description: "When another player plays an action card: cancel that action card.",
          payload: { card: "sabo1", card_name: "Sabotage" },
        },
        { id: "decline", label: "Pass", kind: "decline" },
      ],
    },
  });
  await page.getByTestId("reaction-status-bar").waitFor().catch(() => page.waitForTimeout(500));
  await shot(page, testInfo, "1-action-card-played");
});
