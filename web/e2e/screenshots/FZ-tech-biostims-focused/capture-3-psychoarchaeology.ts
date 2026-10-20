import { expect, test } from "@playwright/test";
import { clickOnce } from "../../offlineDriver";
import { shot } from "../_shared/shot";
import { SIZES } from "../DO-offline-game/offlineGame";
import { openTechnologyPrimary } from "./realPa";

// Real engine, real UI (offline game, preset `research` / `research_nopa`). Ada (Letnev, who owns
// Antimass Deflectors) holds two EXHAUSTED specialty planets: Wellon (yellow) and Meer (red).
// Technologies whose prerequisites only those planets can give: Graviton Laser System (Y) and
// Non-Euclidean Shielding (R R, with her Plasma Scoring). With Psychoarchaeology the engine offers them and
// the dialog lets her pick both (the primary's second pick, 6 resources); without it, neither.
for (const [size, viewport] of Object.entries(SIZES)) {
  test(`psychoarchaeology ${size}`, async ({ page }, testInfo) => {
    test.setTimeout(300_000);
    await page.setViewportSize(viewport);
    await openTechnologyPrimary(page, "research");
    await page.waitForTimeout(800);
    const legal = async () =>
      page.locator('[data-testid^="tech-card-"][data-selectable="true"]').evaluateAll((els) =>
        els.map((e) => e.getAttribute("data-testid")!.replace("tech-card-", "")),
      );
    const ids = await legal();
    console.log(`LEGAL research ${size} ${JSON.stringify(ids)}`);
    console.log(`SKIPS research ${size} ${(await page.getByTestId("tech-skips-list").innerText()).replace(/\s+/g, " ")}`);
    expect(ids).toContain("gls");
    expect(ids).toContain("nes");
    await expect(page.getByTestId("tech-skips-list")).toContainText("Exhausted, free with Psychoarchaeology");
    await shot(page, testInfo, `real-pa-${size}-1-open`);
    await page.getByTestId("tech-card-gls").scrollIntoViewIfNeeded();
    await page.getByTestId("tech-card-gls").click();
    console.log(`PICK1 ${size} confirm-enabled=${await page.getByTestId("confirm-research-btn").isEnabled()} ${(await page.getByTestId("confirm-research-btn").innerText()).replace(/\s+/g, " ")}`);
    await expect(page.getByTestId("confirm-research-btn")).toBeEnabled();
    await shot(page, testInfo, `real-pa-${size}-2-first`);
    await page.getByTestId("tech-card-nes").scrollIntoViewIfNeeded();
    await page.getByTestId("tech-card-nes").click();
    console.log(`PICK2 ${size} confirm-enabled=${await page.getByTestId("confirm-research-btn").isEnabled()} ${(await page.getByTestId("confirm-research-btn").innerText()).replace(/\s+/g, " ")}`);
    await shot(page, testInfo, `real-pa-${size}-3-second`);
    // Confirm: the engine accepts both picks (the second one is a separate decision, answered by the
    // same dialog) and Ada owns both afterwards.
    await page.getByTestId("confirm-research-btn").click();
    for (let i = 0; i < 40 && (await page.getByTestId("technology-modal").count()); i++) {
      if (await page.getByTestId("confirm-research-btn").count()) {
        const btn = page.getByTestId("confirm-research-btn");
        if (await btn.isEnabled()) await btn.click({ timeout: 1_000 }).catch(() => undefined);
      }
      await page.waitForTimeout(300);
    }
    await expect(page.getByTestId("technology-modal")).toHaveCount(0, { timeout: 30_000 });
    const owned = page.getByTestId("technology-modal-button");
    const menu = page.getByTestId("top-menu-button");
    for (let i = 0; i < 80; i++) {
      const open = (await page.getByTestId("decision-modal").count()) + (await page.getByTestId("pending-choice-dialog").count());
      if (!open && (await page.getByTestId("turn-bar-pass").count())) break;
      const auto = page.locator("button:text-matches('^Auto-pay$')");
      if (await auto.count()) await auto.click({ timeout: 1_000 }).catch(() => undefined);
      const pay = page.locator("button:text-matches('^Pay [(]')");
      if (await pay.count()) await pay.click({ timeout: 1_000 }).catch(() => undefined);
      const clicked = await clickOnce(page);
      await page.waitForTimeout(clicked ? 150 : 300);
    }
    if (await menu.count()) await menu.click();
    await owned.click();
    await page.getByTestId("technology-modal").waitFor();
    await page.getByTestId("tech-filter-available").click();
    const left = await page
      .locator('[data-testid^="tech-card-"]')
      .evaluateAll((els) => els.map((e) => e.getAttribute("data-testid")!.replace("tech-card-", "")));
    console.log(`AFTER ${size} ${JSON.stringify(left)}`);
    expect(left).not.toContain("gls");
    expect(left).not.toContain("nes");
  });

  test(`no psychoarchaeology ${size}`, async ({ page }, testInfo) => {
    test.setTimeout(300_000);
    await page.setViewportSize(viewport);
    await openTechnologyPrimary(page, "research_nopa");
    await page.waitForTimeout(800);
    const ids = await page
      .locator('[data-testid^="tech-card-"][data-selectable="true"]')
      .evaluateAll((els) => els.map((e) => e.getAttribute("data-testid")!.replace("tech-card-", "")));
    console.log(`LEGAL nopa ${size} ${JSON.stringify(ids)}`);
    console.log(`SKIPS nopa ${size} ${(await page.getByTestId("tech-skips-list").innerText()).replace(/\s+/g, " ")}`);
    expect(ids).not.toContain("gls");
    expect(ids).not.toContain("nes");
    await expect(page.getByTestId("tech-skips-list")).toContainText("Exhausted");
    await shot(page, testInfo, `real-nopa-${size}`);
  });
}
