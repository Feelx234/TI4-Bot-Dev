import { expect, test, type Page } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { opponent, playerWithHand } from "../_shared/players";
import { galleryBoard } from "../_shared/fixtures";
import { shot } from "../_shared/shot";

// The Stillness of Stars (Cabal agent), offered after another player replenishes commodities: the
// offer now says how many commodities that player holds, which is the trade goods they get and the
// cost cap of the unit the Cabal captures (alpha feedback fb_1793183942).
const BEFORE = process.env.TI4_SHOT_PREFIX === "before";
const SIZES = { desktop: { width: 1440, height: 900 }, "390x844": { width: 390, height: 844 } } as const;
const AGENT = "leader:cabal:cabalagent:COMMODITIES_REPLENISHED:after";

async function openOffer(page: Page, commodities: number) {
  await openMockedGame(page, {
    phase: "action",
    board: galleryBoard,
    players: [playerWithHand({ faction: "cabal", commodities: 2 }), { ...opponent, commodities }],
    choice: {
      prompt: "after COMMODITIES_REPLENISHED",
      context: {
        subtype: "reaction_after_COMMODITIES_REPLENISHED",
        optional: true,
        source: { Reaction: "COMMODITIES_REPLENISHED" },
        // The trigger as the engine serializes it: no amount rides along, the actor is the seat that replenished.
        trigger: {
          kind: "other",
          event_type: "COMMODITIES_REPLENISHED",
          event_id: 21,
          relation: "after",
          actor: opponent.id,
        },
      },
      options: [
        { id: AGENT, label: "Use The Stillness of Stars", kind: "ability", payload: {} },
        { id: "decline", label: "Pass", kind: "decline" },
      ],
    },
  });
  await page.getByTestId("reaction-status-bar").waitFor();
  await page.waitForTimeout(500);
}

for (const [size, viewport] of Object.entries(SIZES)) {
  for (const commodities of size === "desktop" ? [4, 1] : [4]) {
    test(`stillness of stars ${commodities} commodities ${size}`, async ({ page }, testInfo) => {
      await page.setViewportSize(viewport);
      await openOffer(page, commodities);
      const bar = page.getByTestId("reaction-status-bar");
      if (!BEFORE) {
        const noun = commodities === 1 ? "commodity" : "commodities";
        await expect(page.getByTestId("reaction-bar-prompt")).toContainText(
          `replenished their commodities to ${commodities}.`,
        );
        const effect = page.getByTestId("reaction-row-effect");
        await expect(effect).toContainText(`holds ${commodities} ${noun}`);
        await expect(effect).toContainText(`${commodities} trade good${commodities === 1 ? "" : "s"}`);
        await expect(effect).toContainText(`costing ${commodities} or less`);
        await expect(page.getByTestId(`play-reaction-btn-${AGENT}`)).toBeVisible();
        const box = (await bar.boundingBox())!;
        expect(box.x + box.width).toBeLessThanOrEqual(viewport.width);
        expect(box.y + box.height).toBeLessThanOrEqual(viewport.height);
      }
      await shot(page, testInfo, `${BEFORE ? "before-" : ""}1-stillness-${commodities}-${size}`);
    });
  }
}
