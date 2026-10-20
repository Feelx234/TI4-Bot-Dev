import type { Page } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { playerWithHand, opponent } from "../_shared/players";
import { galleryBoard } from "../_shared/fixtures";

export const SIZES = {
  desktop: { width: 1440, height: 900 },
  "390x844": { width: 390, height: 844 },
} as const;
export type SizeName = keyof typeof SIZES;

/** What the seat could research with nothing owned (no prerequisites). */
export const RESEARCHABLE = ["amd", "ps", "sar", "st", "nm", "pi", "dd2"];

/** Focused Research as the engine asks it from rules version 18 (the Technology primary's question). */
export async function openFocusedAfter(page: Page, size: SizeName) {
  await page.setViewportSize(SIZES[size]);
  await openMockedGame(page, {
    board: { ...galleryBoard, systems: {} },
    players: [playerWithHand({ technologies: [], trade_goods: 2 }), opponent],
    choice: {
      prompt: "research a technology",
      context: { subtype: "research_technology", source: { ActionCard: "f_researched" } },
      options: RESEARCHABLE.map((id) => ({ id, label: `research ${id}`, kind: "research" })),
    },
  });
  await page.getByTestId("technology-modal").waitFor();
  await page.waitForTimeout(700);
}

/** Focused Research below rules version 18: a bare `technology` pick. */
export async function openFocusedBefore(page: Page, size: SizeName) {
  await page.setViewportSize(SIZES[size]);
  await openMockedGame(page, {
    board: { ...galleryBoard, systems: {} },
    players: [playerWithHand({ technologies: [], trade_goods: 6 }), opponent],
    choice: {
      prompt: "Focused Research: research a technology",
      context: { subtype: "f_researched_pick_technology", source: { ActionCard: "f_researched" } },
      options: RESEARCHABLE.map((id) => ({ id, label: `research ${id}`, kind: "technology" })),
    },
  });
  await page.waitForTimeout(900);
}

/** The header's Technologies button for a seat that owns Bio-Stims. */
export async function openBioStims(
  page: Page,
  size: SizeName,
  opts: { exhaustedBs: boolean; exhaustedTech: boolean; exhaustedPlanet: boolean },
) {
  await page.setViewportSize(SIZES[size]);
  const board = JSON.parse(JSON.stringify(galleryBoard));
  for (const system of Object.values(board.systems) as any[]) {
    const wellon = system.planets?.wellon;
    if (wellon) wellon.exhausted = opts.exhaustedPlanet;
  }
  const exhausted_technologies = [
    ...(opts.exhaustedBs ? ["bs"] : []),
    ...(opts.exhaustedTech ? ["st"] : []),
  ];
  await openMockedGame(page, {
    board,
    players: [playerWithHand({ technologies: ["bs", "st", "amd"], exhausted_technologies }), opponent],
  });
  const menu = page.getByTestId("top-menu-button");
  if (await menu.count()) await menu.click();
  await page.getByTestId("technology-modal-button").click();
  await page.getByTestId("technology-modal").waitFor();
  await page.waitForTimeout(700);
}
