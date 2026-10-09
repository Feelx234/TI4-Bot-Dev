import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { GAME_ID, SESSION } from "../_shared/mockGame";
import { lobbyFixture } from "../_shared/mockLobby";

// A failed background lobby poll used to be stored as the action error: a fixed red banner with
// "TypeError: Failed to fetch Check the lobby and try again." that no later success cleared and that
// could not be dismissed. Now a single failed poll is silent, three in a row show a small chip at the
// top, the next success removes it, and a failed ACTION keeps a banner with a Dismiss button.
async function open(page: import("@playwright/test").Page, failPolls: { on: boolean }, readyStatus = 200) {
  const lobby = lobbyFixture;
  await page.route(`**/api/games/${GAME_ID}/lobby/join`, (route) =>
    failPolls.on
      ? route.abort("failed")
      : route.fulfill({ json: { player_session: SESSION, player: { id: lobby.host_player_id }, lobby } }),
  );
  await page.route(`**/api/games/${GAME_ID}/lobby/heartbeat`, (route) => route.fulfill({ json: {} }));
  await page.route(`**/api/games/${GAME_ID}/lobby/ready`, (route) =>
    readyStatus === 200
      ? route.fulfill({ json: lobby })
      : route.fulfill({ status: readyStatus, body: "Player already ready" }),
  );
  await page.route(`**/api/games/${GAME_ID}/lobby`, (route) => route.fulfill({ json: lobby }));
  await page.goto("/");
  await page.evaluate(
    ([id, credential]) => sessionStorage.setItem(`ti4.player-session:${id}`, credential),
    [GAME_ID, SESSION],
  );
  await page.goto(`/games/${GAME_ID}`);
  await expect(page.getByTestId("lobby-container")).toBeVisible();
}

test("persistent poll failure: small chip, then gone after recovery", async ({ page }, testInfo) => {
  test.setTimeout(60_000);
  const failPolls = { on: false };
  await open(page, failPolls);
  await shot(page, testInfo, "1-lobby-normal");
  failPolls.on = true;
  await expect(page.getByTestId("lobby-connection-lost")).toBeVisible({ timeout: 15_000 });
  await expect(page.locator(".session-error")).toHaveCount(0);
  await shot(page, testInfo, "2-connection-lost-chip");
  failPolls.on = false;
  await expect(page.getByTestId("lobby-connection-lost")).toHaveCount(0, { timeout: 10_000 });
  await shot(page, testInfo, "3-recovered");
});

test("a failed action keeps a banner with Dismiss", async ({ page }, testInfo) => {
  await open(page, { on: false }, 409);
  await page.getByTestId("ready-button").click();
  await expect(page.getByTestId("lobby-error")).toBeVisible();
  await shot(page, testInfo, "4-action-error-dismissable");
  await page.getByTestId("lobby-error-dismiss").click();
  await expect(page.getByTestId("lobby-error")).toHaveCount(0);
});
