import { test, expect, type APIRequestContext, type Page } from "@playwright/test";
import { openPlayerGame } from "./lobbyHelpers";
import type { InitialSnapshotMsg } from "../src/protocol/types";

const backend = `http://127.0.0.1:${process.env.TI4_E2E_BACKEND_PORT ?? "8080"}`;

async function snapshot(
  request: APIRequestContext,
  gameId: string,
  session: string,
): Promise<InitialSnapshotMsg> {
  const response = await request.get(`${backend}/api/games/${gameId}/snapshot`, {
    headers: { "x-ti4-player-session": session },
  });
  expect(response.ok(), `snapshot: ${response.status()}`).toBe(true);
  return response.json();
}

async function startMovement(page: Page, request: APIRequestContext) {
  const launch = await request.post(`${backend}/api/dev/scenarios/launch`, {
    data: { scenario_id: "tactical_action", seed: 42 },
  });
  expect(launch.ok(), `launch scenario: ${launch.status()}`).toBe(true);
  const { game_id: gameId, player_session: session, player_id: playerId } = await launch.json();
  const initial = await snapshot(request, gameId, session);
  const opponent = initial.view.players.find((p) => p.faction?.toLowerCase().includes("hacan"));
  expect(opponent).toBeDefined();
  const border = Object.values(initial.view.board.systems).find(
    (system) =>
      system.system_id !== "16" && system.units.some((unit) => unit.owner === opponent!.id),
  );
  expect(border).toBeDefined();
  const forward = Object.values(initial.view.board.systems).find(
    (system) => system.system_id !== "01" && system.units.some((unit) => unit.owner === playerId),
  );
  expect(forward).toBeDefined();

  await openPlayerGame(page, gameId, session);
  await page.locator('[data-testid="choice-option"][data-option-id="tactical"]').click();
  await page.getByTestId("submit-choice-button").click();
  await page.getByTestId(`system-hex-${border!.system_id}`).click();
  await page.getByTestId("confirm-activation-btn").click();
  const tray = page.getByTestId("tactical-movement-tray");
  await expect(tray).toBeVisible();
  await expect(tray.getByRole("heading", { level: 2 })).toHaveText("Move Units");
  return {
    gameId,
    session,
    playerId,
    opponentId: opponent!.id,
    borderId: border!.system_id,
    forwardId: forward!.system_id,
    tray,
  };
}

test.describe("Tactical fleet rally", () => {
  test("moves ships from both origins and loads staged infantry in a single workflow", async ({
    page,
    request,
  }) => {
    test.setTimeout(90_000);
    const { gameId, session, playerId, opponentId, borderId, forwardId, tray } =
      await startMovement(page, request);
    await expect(page.getByTestId("rally-row-01-sol_carrier")).toBeVisible();
    await expect(page.getByTestId(`rally-row-${forwardId}-cruiser`)).toBeVisible();

    await page.getByTestId("rally-inc-01-sol_carrier").click();
    await page.getByTestId(`rally-inc-${forwardId}-cruiser`).click();
    // The same selection includes a planet-sourced load for the carrier.
    await page.getByTestId("rally-inc-cargo-01-sol_infantry-jord").click();
    await expect(page.getByTestId("cargo-capacity-gauge")).toContainText("1 /");
    await expect(page.getByTestId("fleet-supply-gauge")).not.toHaveAttribute(
      "data-warning",
      "true",
    );
    await page.getByTestId("commit-moves-btn").click();
    await expect(page.getByTestId("movement-progress")).toBeVisible();
    await expect(page.getByTestId("cargo-loading-tray")).toHaveCount(0);
    await expect(tray).toHaveCount(0, { timeout: 35_000 });

    await expect
      .poll(
        async () => {
          const current = await snapshot(request, gameId, session);
          const units = current.view.board.systems[borderId]?.units ?? [];
          return {
            carrier: units.some((u) => u.owner === playerId && u.unit_type === "sol_carrier"),
            cruiser: units.some((u) => u.owner === playerId && u.unit_type === "cruiser"),
            infantry: units.some((u) => u.owner === playerId && u.unit_type === "sol_infantry"),
            contested: units.some((u) => u.owner === opponentId),
            moving:
              current.pending_choice?.choice.context?.subtype === "movement_step" ||
              current.pending_choice?.choice.context?.subtype === "load_cargo",
          };
        },
        { timeout: 10_000 },
      )
      .toEqual({ carrier: true, cruiser: true, infantry: true, contested: true, moving: false });
    await expect(page.getByTestId("movement-error-banner")).toHaveCount(0);
  });

  test("advises about excess fleet supply without blocking a multi-origin commit", async ({
    page,
    request,
  }) => {
    test.setTimeout(90_000);
    const { gameId, session, playerId, borderId, forwardId, tray } = await startMovement(
      page,
      request,
    );
    // Fleet supply is three; stage every non-fighter ship across both origins.
    for (const origin of ["01", forwardId]) {
      const units = (await snapshot(request, gameId, session)).view.board.systems[origin].units;
      const ships = [
        ...new Set(
          units
            .filter(
              (u) =>
                u.owner === playerId &&
                !u.planet &&
                !["fighter", "infantry", "mech", "pds", "spacedock"].includes(u.unit_type),
            )
            .map((u) => u.unit_type),
        ),
      ];
      for (const type of ships) {
        const increment = page.getByTestId(`rally-inc-${origin}-${type}`);
        while (await increment.isEnabled()) await increment.click();
      }
    }
    await expect(page.getByTestId("fleet-supply-gauge")).toHaveAttribute("data-warning", "true");
    await expect(page.getByTestId("commit-moves-btn")).toBeEnabled();
    await page.getByTestId("commit-moves-btn").click();
    await expect(tray).toHaveCount(0, { timeout: 35_000 });
    await expect
      .poll(async () => {
        const current = await snapshot(request, gameId, session);
        return (current.view.board.systems[borderId]?.units ?? []).filter(
          (u) => u.owner === playerId && !u.planet && u.unit_type !== "fighter",
        ).length;
      })
      .toBeGreaterThanOrEqual(4);
  });
});
