import { test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openMockedGame } from "../_shared/mockGame";
import { actor } from "../_shared/fixtures";
import { playerWithHand, opponent } from "../_shared/players";

// After an opponent moves ships into the viewer's system, a card with that trigger is offered.
test("reaction: offered a card after ships move", async ({ page }, testInfo) => {
  await openMockedGame(page, {
    players: [playerWithHand(), opponent],
    choice: {
      prompt: "play an action card (after SHIP_MOVED)",
      context: { subtype: "play_reaction_after_SHIP_MOVED", actor, source: { Rule: "22.1" }, target: { System: "26" } },
      options: [
        {
          id: "decoy",
          label: "play Decoy Operation",
          kind: "action_card",
          description: "After another player activates a system that contains 1 or more of your structures.",
          payload: { card: "decoy", card_name: "Decoy Operation" },
        },
        { id: "decline", label: "Pass", kind: "decline" },
      ],
    },
  });
  await page.waitForTimeout(800);
  await shot(page, testInfo, "2-after-movement");
});
