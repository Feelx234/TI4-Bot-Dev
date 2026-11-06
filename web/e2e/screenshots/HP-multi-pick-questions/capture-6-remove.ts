import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openMockedGame } from "../_shared/mockGame";
import { playerWithHand, opponent } from "../_shared/players";
import { assertNoHorizontalScroll, newPage, removeBoard, removeChoice } from "./scene";

// Removal over the fleet supply as ONE multi-pick (kind remove): a stepper per unit kind up to the
// excess, the count against the excess, the engine checks the set (and fills up a short one).
for (const size of ["desktop", "390x844"] as const) {
  for (const stage of ["short", "picked"] as const) {
    if (size === "390x844" && stage === "short") continue;
    test(`remove ${stage} ${size}`, async ({ browser, baseURL }, testInfo) => {
      const { context, page } = await newPage(browser, size, baseURL);
      const sent: string[] = [];
      await openMockedGame(page, {
        players: [playerWithHand({ fleet_tokens: 5 }), opponent],
        board: removeBoard,
        choice: removeChoice,
        onClientMessage: (message) => {
          const m = message as { option_id?: string };
          if (m.option_id) sent.push(m.option_id);
        },
      });
      await page.getByTestId("multi-pick-tally").waitFor();
      await page.getByTestId("multi-pick-plus-remove|0").click();
      if (stage === "short") {
        await expect(page.getByTestId("multi-pick-engine-note")).toContainText("the game removes more");
      } else {
        await page.getByTestId("multi-pick-plus-remove|0").click();
        await page.getByTestId("multi-pick-plus-remove|3").click();
        await expect(page.getByTestId("multi-pick-total")).toHaveText("3 · over by 3");
      }
      if (size === "390x844") await assertNoHorizontalScroll(page);
      await shot(page, testInfo, `6-remove-${stage}-${size}`);
      if (stage === "picked") {
        await page.getByTestId("submit-choice-button").click();
        await expect.poll(() => sent).toEqual(["remove|0+remove|0+remove|3"]);
      }
      await context.close();
    });
  }
}
