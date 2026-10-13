import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { playerWithHand, opponent } from "../_shared/players";
import { openCornerGame, thirdPlayer } from "../G-corner-notifications/corner";

const trigger = (over: Record<string, unknown>) => ({
  relation: "after",
  actor: "other_seat",
  event_id: 41,
  ...over,
});

// The viewer is asked whether to react to another player's action card: the card, its text and
// the targets the engine's public trigger names stay on screen as a toast.
for (const [name, extra] of [
  [
    "sabotage-window",
    trigger({ kind: "action_card_played", event_type: "ACTION_CARD_PLAYED", relation: "when", card: "direct_hit", system: "27", planet: "mecatol_rex", subject: "third_seat" }),
  ],
  [
    "reverse-engineer-window",
    trigger({ kind: "action_card_discarded", event_type: "ACTION_CARD_DISCARDED", card: "lucky_shot" }),
  ],
] as const) {
  test(`reaction notice: ${name}`, async ({ page }, testInfo) => {
    await openCornerGame(page, {
      players: [playerWithHand(), opponent, thirdPlayer],
      choice: {
        prompt: "reaction",
        context: { subtype: `reaction_${extra.relation}_${extra.event_type}`, trigger: extra },
        options: [
          { id: "reaction:x", kind: "ability", label: "Play Sabotage" },
          { id: "decline", kind: "decline", label: "Decline" },
        ],
      },
    });
    await expect(page.locator('[data-toast-kind="notice"]')).toBeVisible();
    await shot(page, testInfo, `4-${name}`);
  });
}
