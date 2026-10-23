import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openMockedGame } from "../_shared/mockGame";
import { completedCombatBoard } from "../_shared/fixtures";
import { playerWithHand, opponent } from "../_shared/players";

// On a phone the space combat panel sits under the map and the page scrolls. The panel used to
// scroll on its own as well (a scroll box nested in the page scroll) and ended without room for
// the fixed Players / Events dock, so the last card was cut off. TI4_SHOT_PREFIX=before is how
// the "before" shots were taken, on the base commit d1005ed8; the default regenerates "after".
const prefix = process.env.TI4_SHOT_PREFIX ?? "after";

const sizes = [
  { id: "390x700", width: 390, height: 700 }, // 390 wide with the browser bars showing
  { id: "430x850", width: 430, height: 850 },
  { id: "360x640", width: 360, height: 640 }, // small phone, bars showing
];

for (const size of sizes)
  test(`${prefix} combat panel scrolled to the end ${size.id}`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width: size.width, height: size.height });
    await openMockedGame(page, {
      players: [playerWithHand(), opponent],
      board: completedCombatBoard({ phase: "pre_roll", dice_rolls: [], attacker_hits: 0, defender_hits: 0 }),
    });
    await page.getByTestId("combat-resolution-modal").waitFor({ state: "attached" });
    await page.getByTestId("combat-odds-card").waitFor({ state: "attached" });
    await page.waitForTimeout(800);

    // Scroll the way a finger does: the page, then whatever is under the finger, to its end.
    const m = await page.evaluate(async () => {
      const panel = document.querySelector<HTMLElement>(".combat-arena-panel")!;
      const roots = [document.scrollingElement!, document.getElementById("root")!, document.querySelector(".app-shell")!] as HTMLElement[];
      for (let i = 0; i < 4; i++) {
        panel.scrollTop = panel.scrollHeight;
        for (const r of roots) r.scrollTop = r.scrollHeight;
      }
      await new Promise((r) => setTimeout(r, 100));
      const last = [...panel.children].filter((c) => (c as HTMLElement).offsetHeight > 0).pop()!;
      const dock = document.querySelector(".app-shell__mobile-actions")!.getBoundingClientRect();
      return {
        lastBottom: last.getBoundingClientRect().bottom,
        dockTop: dock.top,
        panelScrolls: panel.scrollHeight > panel.clientHeight + 1,
        vh: innerHeight,
      };
    });
    console.log(`MEASURE ${size.id}: last card bottom ${Math.round(m.lastBottom)}, dock top ${Math.round(m.dockTop)}, viewport ${m.vh}, own scroll box: ${m.panelScrolls}`);
    if (prefix !== "before") {
      expect(m.panelScrolls, "the panel must not be a second scroll box").toBe(false);
      expect(m.lastBottom, "the last card ends above the dock").toBeLessThanOrEqual(m.dockTop);
    }
    await shot(page, testInfo, `${prefix}-end-${size.id}`);
  });
