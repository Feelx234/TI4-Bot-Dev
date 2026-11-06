import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openMockedGame } from "../_shared/mockGame";
import { playerWithHand, opponent } from "../_shared/players";
import { assertNoHorizontalScroll, discardChoice, newPage } from "./scene";

// The generic multi-pick picker (here the coming hand-limit discard: exactly 2 cards, Sabotage
// held twice): a running count against what is owed, the problem as you pick, one '+'-joined answer.
for (const size of ["desktop", "390x844"] as const) {
  for (const stage of ["too-few", "legal"] as const) {
    if (size === "390x844" && stage === "too-few") continue;
    test(`discard ${stage} ${size}`, async ({ browser, baseURL }, testInfo) => {
      const { context, page } = await newPage(browser, size, baseURL);
      const sent: string[] = [];
      await openMockedGame(page, {
        players: [playerWithHand({ held_action_cards: ["sabo1", "direct_hit", "decoy", "skilled_retreat"] }), opponent],
        choice: discardChoice,
        onClientMessage: (message) => {
          const m = message as { option_id?: string };
          if (m.option_id) sent.push(m.option_id);
        },
      });
      await page.getByTestId("multi-pick-tally").waitFor();
      await page.locator('[data-option-id="direct_hit"] input').click();
      if (stage === "too-few") {
        await expect(page.getByTestId("multi-pick-problem")).toContainText("Pick 1 more");
      } else {
        await page.getByTestId("multi-pick-plus-sabo1").click();
        await expect(page.getByTestId("multi-pick-total")).toHaveText("2 of 2");
      }
      if (size === "390x844") await assertNoHorizontalScroll(page);
      await shot(page, testInfo, `5-discard-${stage}-${size}`);
      if (stage === "legal") {
        await page.getByTestId("submit-choice-button").click();
        await expect.poll(() => sent).toEqual(["sabo1+direct_hit"]);
      }
      await context.close();
    });
  }
}
