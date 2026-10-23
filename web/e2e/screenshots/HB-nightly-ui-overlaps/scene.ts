import type { Page } from "@playwright/test";
import { expect } from "@playwright/test";
import { push, openWaiting, secondaryQuestion } from "../AH-secondary-prep-real-ui/prep";
import { board, EXHAUSTED } from "../BO-diplomacy-ready/scene";

/** Which side of the change a run is: `before` is taken on the commit without the fixes. */
export const prefix = process.env.TI4_SHOT_PREFIX ?? "after";

/** What is on top at the centre of an element: its own test id, or whatever covers it. */
export const topmost = (page: Page, testId: string) =>
  page.getByTestId(testId).evaluate((el) => {
    const r = el.getBoundingClientRect();
    const top = document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2);
    if (top === el || el.contains(top)) return "itself";
    return top?.closest("[data-testid]")?.getAttribute("data-testid") ?? top?.tagName ?? "nothing";
  });

/**
 * A prepared Diplomacy secondary arrives: the secondary window opens (the "Prepared ... Confirm /
 * Choose myself" bar), and when `dismiss` the player picks "Choose myself" before the engine asks
 * which planets to ready (the follow-up question of the same secondary).
 */
export async function arriveAtReadyStep(page: Page, dismiss: boolean) {
  const game = await openWaiting(page, { name: "Diplomacy", card: "pok2diplomacy", board });
  await page.getByTestId("secondary-prep-chip").click();
  await page.getByTestId("secondary-yes-btn").click();
  await page.getByTestId("planet-jord").click();
  await page.getByTestId("ready-planet-chip-tarmann").dispatchEvent("click"); // the banner overlaps it on a phone
  await page.getByTestId("ready-bar-confirm").dispatchEvent("click");
  await page.getByTestId("secondary-prep-chip").waitFor();
  push(game, {
    version: 42,
    history: { cursor: 41, redo_count: 0, generation: 0 },
    choice: secondaryQuestion("pok2diplomacy", "spend a strategy token to ready two planets", "ready"),
  });
  await page.getByTestId("secondary-prepared-bar").waitFor();
  if (dismiss) {
    await page.getByTestId("secondary-prepared-dismiss").click();
    await expect(page.getByTestId("secondary-prepared-bar")).toHaveCount(0);
  }
  push(game, {
    version: 43,
    history: { cursor: 42, redo_count: 0, generation: 0 },
    choice: {
      prompt: "ready which planet",
      subtype: "ready_planet",
      options: EXHAUSTED.map((planet) => ({ id: planet, kind: "ready", label: `ready ${planet}`, payload: { planet } })),
    },
  });
  await page.getByTestId("ready-planets-bar").waitFor();
  await page.waitForTimeout(300);
  return game;
}
