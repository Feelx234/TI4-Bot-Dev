import { test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openWaiting, push } from "./prep";

// The prepared answer no longer validates when the window opens: no yes is offered any more.
test("secondary prep: needs review", async ({ page }, testInfo) => {
  const game = await openWaiting(page, { name: "Technology", card: "pok7technology" });
  await page.getByTestId("secondary-prep-chip").click();
  await page.getByTestId("prep-follow").click();
  await page.getByTestId("prep-tech-amd").click();
  push(game, {
    version: 42,
    history: { cursor: 41, redo_count: 0, generation: 0 },
    choice: {
      prompt: "spend a strategy token and 4 resources to research",
      subtype: "strategy_secondary",
      options: [{ id: "no", kind: "strategy", label: "decline" }],
      details: { kind: "strategy_secondary", card: "pok7technology", played_by: "other_seat", tokens_left: 0, costs_token: true },
    },
  });
  await page.getByTestId("secondary-needs-review").waitFor();
  await shot(page, testInfo, "5-needs-review");
});
