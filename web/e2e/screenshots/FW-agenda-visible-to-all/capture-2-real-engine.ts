import { expect, test } from "@playwright/test";
import { clickOnce } from "../../offlineDriver";
import { shot } from "../_shared/shot";
import { SEED, SIZES, requireEngine } from "../DO-offline-game/offlineGame";

// The real wasm engine (the `agenda` start preset: agenda phase from round one, riders in hand)
// driven through the real UI as seat 3 against rule bots. The banner is read from the engine's own
// state: agenda_in_play, agenda_casts, agenda_log. Needs `npm run build:wasm` first.
for (const [size, viewport] of Object.entries(SIZES)) {
  test(`real engine agenda banner ${size}`, async ({ page }, testInfo) => {
    test.setTimeout(240_000);
    requireEngine();
    await page.setViewportSize(viewport);
    await page.route(/\/(api|ws|health)(\/|$)/, (route) => route.abort());
    await page.goto("/offline");
    await page.getByTestId("offline-nickname").fill("Ada");
    await page.getByTestId("offline-player-count").selectOption("4");
    await page.getByTestId("offline-seat").selectOption("3");
    await page.getByTestId("offline-seed").fill(String(SEED));
    await page.getByTestId("offline-preset").fill("agenda");
    await page.getByTestId("offline-start").click();
    await expect(page.getByTestId("offline-game")).toBeVisible({ timeout: 60_000 });

    const seen = new Set<string>();
    let sawSecond = false;
    for (let i = 0; i < 300 && !sawSecond; i++) {
      const strip = page.getByTestId("agenda-strip");
      if (await strip.count()) {
        const line = (await page.getByTestId("agenda-strip-text").innerText()).replace(/\s+/g, " ");
        const text = (await strip.innerText()).replace(/\s+/g, " ");
        expect(text).not.toMatch(/player_[0-9a-f]{8}/);
        if (!seen.has(line)) {
          seen.add(line);
          const n = seen.size;
          if (size !== "desktop") {
            const toggle = page.getByTestId("agenda-strip-toggle");
            if ((await toggle.getAttribute("aria-expanded")) !== "true") await toggle.click();
          }
          await shot(page, testInfo, `real-${size}-${String(n).padStart(2, "0")}`);
        }
        if (/Agenda 2\/2/.test(line)) sawSecond = true;
      }
      const clicked = await clickOnce(page);
      await page.waitForTimeout(clicked ? 150 : 300);
    }
    expect(seen.size).toBeGreaterThan(0);
  });
}
