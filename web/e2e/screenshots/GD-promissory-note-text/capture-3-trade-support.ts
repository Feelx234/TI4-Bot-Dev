import { test } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { shot } from "../_shared/shot";
import { SIZES, prefix, supportAnswer, supportPropose, tradePlayers } from "./_scene";

const modal = (page: import("@playwright/test").Page) =>
  page.getByTestId("trade-desk-modal").locator(".choice-workflow-modal");

// Exchanging Support for the Throne on the trade desk: each column names whose Support it is, the
// card text and who holds it. Before: the exchange showed a generic card with the wrong text and a
// Support note ask showed "Support (jolnar)" with "Card text is not available".
for (const [size, viewport] of Object.entries(SIZES)) {
  test(`trade: exchange Support ${size}`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width: viewport.width, height: size === "desktop" ? 1300 : viewport.height });
    await openMockedGame(page, { players: tradePlayers, choice: supportPropose as never });
    await page.getByTestId("trade-desk-modal").waitFor();
    await page.getByTestId("stage-give-support").click();
    await page.getByTestId("stage-receive-support").click();
    await page.getByTestId("stage-give-support").scrollIntoViewIfNeeded();
    await shot(page, testInfo, `${prefix}3-exchange-support-${size}`, size === "desktop" ? { of: modal(page), pad: 8 } : {});
  });

  test(`trade: ask for a Support note ${size}`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width: viewport.width, height: size === "desktop" ? 1300 : viewport.height });
    await openMockedGame(page, { players: tradePlayers, choice: supportPropose as never });
    await page.getByTestId("trade-desk-modal").waitFor();
    await page.getByTestId("stage-receive-note-support:jolnar").click();
    await page.getByTestId("stage-receive-note-support:jolnar").scrollIntoViewIfNeeded();
    await shot(page, testInfo, `${prefix}4-support-note-ask-${size}`, size === "desktop" ? { of: modal(page), pad: 8 } : {});
  });

  test(`trade: answer a Support exchange ${size}`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width: viewport.width, height: size === "desktop" ? 1100 : viewport.height });
    await openMockedGame(page, { players: tradePlayers, choice: supportAnswer as never });
    await page.getByTestId("offer-net").waitFor();
    await shot(page, testInfo, `${prefix}5-support-answer-${size}`, size === "desktop" ? { of: modal(page), pad: 8 } : {});
  });
}
