import { test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openMockedGame } from "../_shared/mockGame";
import { playerWithHand, opponent } from "../_shared/players";

// Two cards switched to Never: they are declined automatically when their window opens.
test("action cards: some set to never", async ({ page }, testInfo) => {
  await openMockedGame(page, { players: [playerWithHand(), opponent] });
  const toggle = (id: string) => page.getByTestId(`action-card-item-${id}`).locator("button").last();
  await toggle("direct_hit").click();
  await toggle("skilled_retreat").click();
  const card = page.getByTestId("player-card").first();
  await shot(page, testInfo, "2-some-never", { of: card, pad: 6 });
});
