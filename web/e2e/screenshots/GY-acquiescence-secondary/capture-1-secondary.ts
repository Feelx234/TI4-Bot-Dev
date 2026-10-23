import { expect, test, type Page } from "@playwright/test";
import { openMockedGame, type MockChoice } from "../_shared/mockGame";
import { playerWithHand, opponent } from "../_shared/players";
import { galleryBoard } from "../_shared/fixtures";
import { shot } from "../_shared/shot";

// The engine offers a held Winnu Acquiescence as an extra option `follow|waived|<module>|acq` on
// the strategy card's secondary question. Before, that third option pushed the question out of the
// secondary layout into a bare option list with the raw engine label. Now the normal secondary
// panel (Follow / Skip) shows it as "Use Acquiescence" with the printed card text.
const prefix = process.env.TI4_SHOT_PREFIX ?? "after";
const DESKTOP = { width: 1440, height: 900 };
const PHONE = { width: 390, height: 844 };
const players = [playerWithHand({}), opponent];

const acq = {
  id: "follow|waived|3|acq",
  label: "resolve the secondary using Acquiescence",
  kind: "strategy",
};

const secondary = (tokens: number): MockChoice => ({
  prompt: "spend a strategy token to research",
  context: { subtype: "strategy_secondary" },
  options: [
    { id: "no", label: "decline", kind: "strategy" },
    ...(tokens > 0 ? [{ id: "yes", label: "follow", kind: "strategy" }] : []),
    acq,
  ],
  details: { kind: "strategy_secondary", card: "pok7technology", played_by: "b", tokens_left: tokens, costs_token: true },
});

async function open(page: Page, viewport: { width: number; height: number }, choice: MockChoice) {
  await page.setViewportSize(viewport);
  await openMockedGame(page, { board: galleryBoard, players, choice });
  await page.getByTestId("pending-choice-dialog").waitFor();
  await page.waitForTimeout(700);
}

for (const [size, viewport] of [["desktop", DESKTOP], ["390x844", PHONE]] as const) {
  for (const [name, tokens] of [["with-token", 2], ["no-token", 0]] as const) {
    test(`acquiescence ${name} ${size}`, async ({ page }, testInfo) => {
      await open(page, viewport, secondary(tokens));
      if (prefix === "after") {
        await expect(page.getByTestId("strategy-secondary-panel")).toBeVisible();
        await expect(page.getByTestId("secondary-waiver-btn")).toContainText("Use Acquiescence");
        await expect(page.getByTestId("secondary-yes-btn")).toHaveCount(tokens > 0 ? 1 : 0);
      }
      await shot(page, testInfo, `${prefix}-${name}-${size}`);
    });
  }
}
