import type { Page } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { playerWithHand, opponent } from "../_shared/players";
import { galleryBoard } from "../_shared/fixtures";

/** Which side of the change a run is: `before` is taken on the commit without the change. */
export const prefix = process.env.TI4_SHOT_PREFIX ?? "after";

export const SIZES = {
  desktop: { width: 1440, height: 900 },
  "390x844": { width: 390, height: 844 },
} as const;
export type SizeName = keyof typeof SIZES;

/**
 * A seat with one technology and no specialties. Research Grant Reallocation offers every
 * technology it does not own (its faction's own included), prerequisites or not: here a spread
 * of ones it could not research (gd, cr2, ws, x89c4, hm, dd2) and ones it could (sr, ps, st).
 */
const OFFERED = ["sr", "ps", "st", "nm", "pi", "gd", "cr2", "ws", "x89c4", "hm", "dd2", "amd"];

/** Research Grant Reallocation asking the elected seat which technology to gain. */
export async function openGrant(page: Page, size: SizeName) {
  await page.setViewportSize(SIZES[size]);
  await openMockedGame(page, {
    board: { ...galleryBoard, systems: {} },
    players: [playerWithHand({ technologies: ["sar"], trade_goods: 2 }), opponent],
    choice: {
      prompt: "gain which technology",
      context: {
        subtype: "grant_reallocation_choose_technology",
        source: { Content: "grant_reallocation" },
      },
      options: OFFERED.map((id) => ({ id, label: `gain ${id}`, kind: "technology" })),
    },
  });
  await page.waitForTimeout(900);
}
