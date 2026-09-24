import { test, expect, type APIRequestContext } from "@playwright/test";
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
  expect(response.ok(), `snapshot for ${gameId}: ${response.status()}`).toBe(true);
  return response.json();
}

async function restoreGameHistory(
  request: APIRequestContext,
  gameId: string,
  session: string,
  eventId: string,
) {
  for (let attempt = 0; attempt < 15; attempt++) {
    const currentSnap = await snapshot(request, gameId, session);
    const response = await request.post(`${backend}/api/games/${gameId}/history`, {
      headers: {
        "content-type": "application/json",
        "x-ti4-player-session": session,
      },
      data: {
        action: "restore",
        event_id: eventId,
        expected_version: currentSnap.game_version,
      },
    });
    if (response.ok()) {
      return response.json();
    }
    if (response.status() === 409 && attempt < 14) {
      await new Promise((r) => setTimeout(r, 200));
      continue;
    }
    const errText = await response.text();
    throw new Error(`restore history to ${eventId} failed (${response.status()}): ${errText}`);
  }
}

async function launchDevScenario(
  request: APIRequestContext,
  scenarioId: string,
  seed = 42,
): Promise<{ game_id: string; player_session: string; player_id: string; scenario_id: string }> {
  const response = await request.post(`${backend}/api/dev/scenarios/launch`, {
    data: { scenario_id: scenarioId, seed },
  });
  expect(response.ok(), `launch dev scenario: ${response.status()}`).toBe(true);
  return response.json();
}

test.describe("Tactical Movement & Combat Multi-Fleet Integration", () => {
  test("stages units from multiple systems exceeding fleet limit, resolves moves to combat, and undos cleanly", async ({
    page,
    request,
  }) => {
    test.setTimeout(90_000);
    // 1. Launch the extended tactical scenario
    const launched = await launchDevScenario(request, "tactical_action", 42);
    const { game_id: gameId, player_session: session, player_id: solPlayerId } = launched;

    // Inspect initial layout from snapshot
    const initialSnap = await snapshot(request, gameId, session);
    const systems = initialSnap.view.board.systems;

    // Find Hacan player ID
    const hacanPlayer = initialSnap.view.players.find(
      (p) => p.faction?.toLowerCase() === "hacan" || p.faction?.toLowerCase() === "emirates_of_hacan",
    );
    expect(hacanPlayer, "Hacan player must exist in scenario").toBeDefined();
    const hacanId = hacanPlayer!.id;

    // Find the border system where Hacan's fleet is stationed (not Hacan's home system #16)
    const borderSystemEntry = Object.values(systems).find(
      (sys) => sys.system_id !== "16" && sys.units.some((u) => u.owner === hacanId),
    );
    expect(borderSystemEntry, "border system with Hacan fleet must exist").toBeDefined();
    const borderSysId = borderSystemEntry!.system_id;

    // Find Sol's second forward system (not Sol's home system #01)
    const secondSystemEntry = Object.values(systems).find(
      (sys) => sys.system_id !== "01" && sys.units.some((u) => u.owner === solPlayerId),
    );
    expect(secondSystemEntry, "Sol forward system must exist").toBeDefined();
    const secondSysId = secondSystemEntry!.system_id;

    // 2. Open game UI
    page.on("console", (msg) => console.log("[BROWSER]", msg.type(), msg.text()));
    page.on("pageerror", (err) => console.log("[PAGEERROR]", err.message));
    await openPlayerGame(page, gameId, session);

    // Verify Sol starts in Action Phase with Tactical Action available
    const tacticalOption = page.locator(
      '[data-testid="choice-option"][data-option-id="tactical"]',
    );
    await expect(tacticalOption).toBeVisible({ timeout: 10_000 });
    await tacticalOption.click();
    await page.getByTestId("submit-choice-button").click();

    // 3. System activation step: click the border system hex on the board
    const borderHex = page.locator(`[data-testid="system-hex-${borderSysId}"]`);
    await expect(borderHex).toBeVisible({ timeout: 5_000 });
    await borderHex.click();

    const confirmActivationBtn = page.getByTestId("confirm-activation-btn");
    await expect(confirmActivationBtn).toBeVisible({ timeout: 5_000 });
    await confirmActivationBtn.click();

    // 4. Tactical movement tray opens
    const movementTray = page.getByTestId("tactical-movement-tray");
    await expect(movementTray).toBeVisible({ timeout: 8_000 });

    // Verify casing and subtitle fixes
    await expect(movementTray.getByRole("heading", { level: 2 })).toHaveText("Move Units");
    await expect(movementTray.getByText("movement", { exact: true })).not.toBeVisible();

    // Verify both origin systems are represented in the rally tray
    const homeRows = page.locator(`[data-testid^="rally-row-01-"]`);
    const secondRows = page.locator(`[data-testid^="rally-row-${secondSysId}-"]`);
    await expect(homeRows.first()).toBeVisible();
    await expect(secondRows.first()).toBeVisible();

    // -------------------------------------------------------------
    // SCENARIO 1: Stage ships exceeding Sol's fleet supply (3 tokens)
    // -------------------------------------------------------------
    // Click increment buttons for all non-fighter ships in both systems
    // Home system (01): 2 Carriers, 1 Destroyer
    const incHome = page.locator(`[data-testid^="rally-inc-01-"]`);
    const incHomeCount = await incHome.count();
    for (let i = 0; i < incHomeCount; i++) {
      const btn = incHome.nth(i);
      while (await btn.isEnabled()) {
        await btn.click();
      }
    }

    // Forward system: 1 Cruiser, 1 Dreadnought
    const incSecond = page.locator(`[data-testid^="rally-inc-${secondSysId}-"]`);
    const incSecondCount = await incSecond.count();
    for (let i = 0; i < incSecondCount; i++) {
      const btn = incSecond.nth(i);
      while (await btn.isEnabled()) {
        await btn.click();
      }
    }

    // Gauge should reflect exceeding Sol's fleet limit of 3
    const fleetGauge = page.getByTestId("fleet-supply-gauge");
    await expect(fleetGauge).toBeVisible();
    await expect(fleetGauge).toHaveAttribute("data-warning", "true");
    await expect(fleetGauge).toContainText("Ships");

    // LRR 37.4 soft advisory rule: commit moves must NOT be disabled even when exceeding limit
    const commitBtn = page.getByTestId("commit-moves-btn");
    await expect(commitBtn).toBeEnabled();

    // Also stage ground forces / fighters (cargo)
    const incCargo = page.locator('[data-testid^="rally-inc-"]');
    const cargoCount = await incCargo.count();
    for (let i = 0; i < cargoCount; i++) {
      const btn = incCargo.nth(i);
      if (await btn.isEnabled()) {
        await btn.click();
      }
    }

    // Commit moves and watch execution complete
    await commitBtn.click();

    // Tray closes once movement pipeline completes and transitions to combat
    await expect(movementTray).not.toBeVisible({ timeout: 35_000 });

    // Verify game advanced to combat in the border system
    await expect
      .poll(
        async () => {
          const s = await snapshot(request, gameId, session);
          const borderUnits = s.view.board.systems[borderSysId]?.units ?? [];
          const hasSol = borderUnits.some((u) => u.owner === solPlayerId);
          return hasSol;
        },
        { timeout: 10_000 },
      )
      .toBe(true);

    // -------------------------------------------------------------
    // UNDO: Roll back to initial state using the undo API
    // -------------------------------------------------------------
    const initialEventId = initialSnap.events[0]?.id;
    expect(initialEventId, "Initial event must exist").toBeDefined();

    // Call the undo/restore history API to roll back directly to the initial state
    await restoreGameHistory(request, gameId, session, initialEventId!);

    // Reload the page so the web client loads the restored timeline
    await page.reload();

    // -------------------------------------------------------------
    // SCENARIO 2: Re-stage a balanced fleet within Sol's fleet limit
    // -------------------------------------------------------------
    // Sol re-initiates Tactical Action
    const tacticalOptionAfterUndo = page.locator(
      '[data-testid="choice-option"][data-option-id="tactical"]',
    );
    await expect(tacticalOptionAfterUndo).toBeVisible({ timeout: 5_000 });
    await tacticalOptionAfterUndo.click();
    await page.getByTestId("submit-choice-button").click();

    // Re-activate border system
    await borderHex.click();
    await confirmActivationBtn.click();
    await expect(movementTray).toBeVisible({ timeout: 8_000 });

    // Now stage a smaller fleet within Sol's fleet limit (e.g. 1 Carrier from 01, 1 Cruiser from second system)
    // First ensure 0 staged units
    const finishBtn = page.getByTestId("finish-movement-btn");
    await expect(finishBtn).toBeVisible();

    // Increment 1 Carrier from 01
    const firstHomeInc = page.locator(`[data-testid^="rally-inc-01-"]`).first();
    await firstHomeInc.click();

    // Increment 1 ship from second system
    const firstSecondInc = page.locator(`[data-testid^="rally-inc-${secondSysId}-"]`).first();
    await firstSecondInc.click();

    // 2 ships total <= 3 fleet supply: warning must NOT be set
    await expect(fleetGauge).not.toHaveAttribute("data-warning", "true");

    // Commit the balanced fleet
    await page.getByTestId("commit-moves-btn").click();

    // Tray completes and space combat initiates again
    await expect(movementTray).not.toBeVisible({ timeout: 20_000 });

    // Verify Sol units arrived in border system for space combat
    await expect
      .poll(
        async () => {
          const s = await snapshot(request, gameId, session);
          const borderUnits = s.view.board.systems[borderSysId]?.units ?? [];
          const solCount = borderUnits.filter((u) => u.owner === solPlayerId).length;
          return solCount >= 2;
        },
        { timeout: 10_000 },
      )
      .toBe(true);
  });
});
