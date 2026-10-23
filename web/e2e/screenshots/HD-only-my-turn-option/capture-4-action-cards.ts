import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openMockedGame } from "../_shared/mockGame";
import { playerWithHand, opponent } from "../_shared/players";
import { openReaction, other, trigger, outerOffer, pass } from "../E-reaction-offer/_reaction";

// Action cards keep their per-game modes (held by the server); the sheet gets a "My turn" button
// for the cards that come up in both turns, and the reaction window a second checkbox.
test("Action cards on the sheet: My turn only", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await openMockedGame(page, { players: [playerWithHand(), opponent] });
  const own = (id: string) => page.getByTestId(`reaction-inspect-ownturn-${id}`);
  // Sabotage, Direct Hit and Skilled Retreat come up in both turns; Decoy Operation ("After
  // another player activates a system ...") only in other players' turns: no choice there.
  await expect(own("sabo1")).toBeVisible();
  await expect(own("direct_hit")).toBeVisible();
  await expect(own("skilled_retreat")).toBeVisible();
  await expect(own("decoy")).toHaveCount(0);
  await own("direct_hit").click();
  await expect(own("direct_hit")).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByTestId("reaction-inspect-mode-direct_hit")).toHaveAttribute("data-reaction-mode", "ownturn");
  await page.getByTestId("reaction-inspect-mode-skilled_retreat").click();
  await shot(page, testInfo, "13-sheet-cards", { of: page.getByTestId("player-card").first(), pad: 6 });
});

for (const [size, viewport] of [["desktop", { width: 1440, height: 900 }], ["390x844", { width: 390, height: 844 }]] as const) {
test(`Action card reaction window: Only offer in my own turn ${size}`, async ({ page }, testInfo) => {
  await page.setViewportSize(viewport);
  await openReaction(
    page,
    "reaction_when_ACTION_CARD_PLAYED",
    [outerOffer("sabo1", "Sabotage", "ACTION_CARD_PLAYED", "when"), pass],
    trigger("action_card_played", "ACTION_CARD_PLAYED", "when", { actor: other, card: "uprising" }),
    { Reaction: "ACTION_CARD_PLAYED" },
    "when ACTION_CARD_PLAYED",
  );
  const box = page.getByTestId("reaction-inspect-ownturn-Sabotage");
  await expect(box).not.toBeChecked();
  await shot(page, testInfo, `14-window-${size}`, { of: page.getByTestId("reaction-status-bar"), pad: 8 });
  await box.click();
  await expect(box).toBeChecked();
  await shot(page, testInfo, `15-window-ticked-${size}`, { of: page.getByTestId("reaction-status-bar"), pad: 8 });
});
}
