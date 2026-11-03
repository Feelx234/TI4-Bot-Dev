import { join } from "node:path";
import { expect, test, type Page } from "@playwright/test";
import { openMapLobby } from "../_shared/mapLobby";
import { shot } from "../_shared/shot";

// Thunder's Edge content is switched off (docs/D28E_RESUME.md): the table form of an online game,
// the offline setup form and the lobby no longer offer the switch. The strategy card set `te`
// (Thunder's Edge strategy cards) is not content and stays the default. The `before-*` shots show
// the forms as they were (TI4_SHOT_PREFIX=before on the commit before the change, committed
// under before/); the runner makes the `after-*` shots.
const prefix = process.env.TI4_SHOT_PREFIX ?? "after";
const onlySwitch = prefix === "before" ? 1 : 0;
const SIZES = { desktop: { width: 1440, height: 900 }, "390x844": { width: 390, height: 844 } } as const;

/** The server of the table form: no games, and the printed cards from the repository's fixture. */
async function routeLobbyApi(page: Page) {
  await page.route(/\/api\//, (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: "[]" }),
  );
  await page.route("**/api/catalog", (route) =>
    route.fulfill({ path: join(process.cwd(), "..", "fixtures", "catalog", "catalog.json"), contentType: "application/json" }),
  );
}

for (const [size, viewport] of Object.entries(SIZES)) {
  test(`online table form ${size}`, async ({ page }, testInfo) => {
    await page.setViewportSize(viewport);
    await routeLobbyApi(page);
    await page.goto("/");
    await expect(page.getByTestId("strategy-card-set-select")).toBeVisible();
    await expect(page.getByTestId("thunders-edge-toggle")).toHaveCount(onlySwitch);
    // The Thunder's Edge strategy cards are still the default set.
    await expect(page.getByTestId("strategy-card-set-select")).toHaveValue("te");
    await shot(page, testInfo, `${prefix}-1-online-table-form-${size}`);
  });

  test(`offline setup ${size}`, async ({ page }, testInfo) => {
    await page.setViewportSize(viewport);
    await page.route(/\/(api|ws|health)(\/|$)/, (route) => route.abort());
    await page.goto("/offline");
    await expect(page.getByTestId("offline-start")).toBeVisible();
    await expect(page.getByTestId("offline-thunders-edge")).toHaveCount(onlySwitch);
    await shot(page, testInfo, `${prefix}-2-offline-setup-${size}`);
  });

  test(`online lobby ${size}`, async ({ page }, testInfo) => {
    await page.setViewportSize(viewport);
    // A table whose server still reports the old switch as on (a lobby saved before the change):
    // the page shows the strategy cards and no Thunder's Edge row; starting it is refused by the
    // server with the reason (see the next capture).
    await page.route("**/api/catalog", (route) =>
      route.fulfill({ path: join(process.cwd(), "..", "fixtures", "catalog", "catalog.json"), contentType: "application/json" }),
    );
    await openMapLobby(page, { players: 4, strategyCardSet: "te", thundersEdge: true });
    await expect(page.getByTestId("lobby-card-set-row")).toBeVisible();
    await expect(page.getByTestId("lobby-thunders-edge-row")).toHaveCount(onlySwitch);
    await shot(page, testInfo, `${prefix}-3-online-lobby-${size}`);
  });
}

// An older client (or a script) that still asks the server for the content gets the reason, not a
// parse error: the form shows the server's message.
test("a refused table shows the reason", async ({ page }, testInfo) => {
  test.skip(prefix === "before", "the switch does not exist in the after UI; the before UI never got a refusal");
  await page.setViewportSize(SIZES.desktop);
  await routeLobbyApi(page);
  await page.route(/\/api\/games$/, (route) => {
    if (route.request().method() !== "POST") return route.fulfill({ status: 200, contentType: "application/json", body: "[]" });
    return route.fulfill({
      status: 400,
      contentType: "text/plain",
      body: "Thunder's Edge content (the expedition and the breakthroughs) is switched off in this version; the Thunder's Edge strategy cards can still be played",
    });
  });
  await page.goto("/");
  await page.getByTestId("strategy-card-set-select").selectOption("te");
  await page.getByLabel("Nickname").fill("Ada");
  await page.getByTestId("create-game-button").click();
  await expect(page.getByText(/Thunder's Edge content .* switched off/)).toBeVisible();
  await shot(page, testInfo, "after-4-refused-table-desktop");
});
