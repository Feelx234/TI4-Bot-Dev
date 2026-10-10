import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { expect, test, type Page } from "@playwright/test";

// The address decides the app: `/` is the local game (seed 42), `?example=` is the demo.
const WASM = fileURLToPath(new URL("../src/session/ti4.wasm", import.meta.url));
test.use({ viewport: { width: 1920, height: 1080 } });

/** The engine is not there: the page must fall back to the demo, and say why. */
const withoutEngine = (page: Page) =>
  page.route("**/ti4.wasm", (route) => route.fulfill({ status: 404 }));

test("the bare address is the local game: seed 42, eight seats", async ({ page }) => {
  test.skip(!existsSync(WASM), "the engine is not built: run web2/scripts/build-wasm.sh");
  test.setTimeout(120_000);
  await page.goto("/");
  await page.getByRole("button", { name: "Dev", exact: true }).click();
  await expect(page.getByRole("complementary", { name: "Dev" })).toContainText("seed 42 · 8 seats");
  await page.keyboard.press("Escape");
  await expect(page.getByRole("button", { name: "Demo", exact: true })).toBeVisible();
});

test("without the engine the bare address shows the demo, with the reason", async ({ page }) => {
  await withoutEngine(page);
  await page.goto("/");
  await expect(page.getByText("Local game unavailable.")).toBeVisible();
  await expect(page.locator("#example")).toBeVisible();
});

test("the header switches between the demo and the local game", async ({ page }) => {
  await withoutEngine(page);
  await page.goto("/?example=live-picker");
  await page.getByRole("button", { name: "Local game", exact: true }).click();
  await page.waitForURL(
    (url) => url.pathname === "/" && url.searchParams.get("example") !== "live-picker",
  );
  await expect(page.getByText("Local game unavailable.")).toBeVisible();
});
