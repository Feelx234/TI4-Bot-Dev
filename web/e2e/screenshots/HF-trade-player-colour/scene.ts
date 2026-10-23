import type { Page } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { opponent, withPlayer } from "../_shared/players";
import { answerChoice, proposeChoice, tradePlayers, PARTNER } from "../AG-trade-staging-desk/_trade";

/** "after" for the current UI; "before" is taken on the base commit and committed under before/. */
export const prefix = process.env.TI4_SHOT_PREFIX ?? "after";
export const checking = prefix !== "before";

export const SIZES = [
  { name: "desktop", width: 1100, height: 1000 },
  { name: "390x844", width: 390, height: 844 },
  { name: "360x740", width: 360, height: 740 },
] as const;

/**
 * Four seats: the viewer is position 1 (blue), the trade partner sits at position 4 (red), so the
 * colour is clearly not the second seat's default.
 */
const fillers = [2, 3].map((n) =>
  withPlayer(opponent, { id: `filler_${n}`, faction: n === 2 ? "letnev" : "naalu" }),
);
export const players = [tradePlayers[0], ...fillers, tradePlayers[1]];
export const nicknames = { [PARTNER]: "Rina" };

export async function openTrade(page: Page, mode: "propose" | "answer", size: (typeof SIZES)[number]) {
  await page.setViewportSize({ width: size.width, height: size.height });
  await openMockedGame(page, {
    players,
    nicknames,
    choice: (mode === "propose" ? proposeChoice : answerChoice) as never,
  });
  await page.getByTestId("trade-desk-modal").waitFor();
}
