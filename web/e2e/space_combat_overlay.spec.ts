import { test, expect, type APIRequestContext } from "@playwright/test";
import { openPlayerGame } from "./lobbyHelpers";
import type { InitialSnapshotMsg } from "../src/protocol/types";

const backend = `http://127.0.0.1:${process.env.TI4_E2E_BACKEND_PORT ?? "8180"}`;

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

test.describe("Space Combat Overlay", () => {
  test("loads ongoing combat scenario, displays side-by-side fleets with dreadnoughts and odds, shows per-round hits, and supports docking and integrated decision execution", async ({
    page,
    request,
  }) => {
    test.setTimeout(60_000);

    // 1. Launch the ongoing combat dev scenario
    const launch = await request.post(`${backend}/api/dev/scenarios/launch`, {
      data: { scenario_id: "ongoing_combat", seed: 42 },
    });
    expect(launch.ok(), `launch scenario: ${launch.status()}`).toBe(true);
    const { game_id: gameId, player_session: session, player_id: playerId } = await launch.json();

    // 2. Verify backend state starts with active combat already in progress
    const initial = await snapshot(request, gameId, session);
    expect(initial.view.board.combat).toBeDefined();
    expect(initial.view.board.combat?.attacker).toBe(playerId);
    expect(initial.pending_choice).toBeDefined();

    // Verify Sol has direct_hit in held action cards
    const solPlayer = initial.view.players.find((p) => p.id === playerId);
    expect(solPlayer?.held_action_cards).toContain("direct_hit");

    // 3. Open the game in browser as active player
    await openPlayerGame(page, gameId, session);

    // 4. Verify the Space Combat Overlay appears immediately
    const modal = page.getByTestId("combat-resolution-modal");
    await expect(modal).toBeVisible();

    // Verify modal panel is present
    const panel = modal.locator(".combat-arena-panel");
    await expect(panel).toBeVisible();

    // Verify both attacker and defender have Dreadnoughts
    const attackerCard = page.getByTestId("attacker-fleet-card");
    await expect(attackerCard).toBeVisible();
    await expect(attackerCard.getByTestId("unit-row-dreadnought")).toBeVisible();
    await expect(page.getByTestId("attacker-fleet-supply-gauge")).toBeVisible();
    await expect(page.getByTestId("attacker-capacity-gauge")).toBeVisible();

    const defenderCard = page.getByTestId("defender-fleet-card");
    await expect(defenderCard).toBeVisible();
    await expect(defenderCard.getByTestId("unit-row-dreadnought")).toBeVisible();
    await expect(page.getByTestId("defender-fleet-supply-gauge")).toBeVisible();
    await expect(page.getByTestId("defender-capacity-gauge")).toBeVisible();

    // Verify combat odds card
    const oddsCard = page.getByTestId("combat-odds-card");
    await expect(oddsCard).toBeVisible();
    await expect(oddsCard.getByText("Combat Odds Analysis")).toBeVisible();

    // Verify round hits scorecard is visible
    const roundHits = page.getByTestId("combat-round-hits");
    await expect(roundHits).toBeVisible();
    await expect(page.getByTestId("attacker-round-hits")).toBeVisible();
    await expect(page.getByTestId("defender-round-hits")).toBeVisible();

    // 5. Test dockable minimizing behavior
    const minimizeBtn = page.getByTestId("close-combat-modal");
    await expect(minimizeBtn).toBeVisible();
    await minimizeBtn.click();

    // Modal should close and the docked pill should be visible at bottom
    await expect(modal).toHaveCount(0);
    const dockedPill = page.getByTestId("combat-docked-pill");
    await expect(dockedPill).toBeVisible();
    await expect(dockedPill).toContainText("SPACE COMBAT");

    // Click resume button on docked pill to restore the full overlay
    const resumeBtn = page.getByTestId("resume-combat-btn");
    await expect(resumeBtn).toHaveText("Resume Decision");
    await resumeBtn.click();

    // Overlay is restored
    await expect(modal).toBeVisible();

    // 6. Test submitting an integrated combat decision
    const subtype = initial.pending_choice?.choice?.context?.subtype;
    if (subtype === "sustain_damage") {
      // Test integrated sustain option on the dreadnought row or decline
      const sustainBtn = attackerCard.locator('[data-testid^="sustain-opt-"]');
      if (await sustainBtn.count() > 0) {
        await sustainBtn.first().click();
      } else {
        await page.getByTestId("decline-sustain-btn").click();
      }
    } else if (subtype === "assign_casualty") {
      // Test clicking interactive unit row directly or casualty button
      const interactiveRow = attackerCard.locator(".combat-unit-row--interactive");
      if (await interactiveRow.count() > 0) {
        await interactiveRow.first().click();
      } else {
        await page.locator('[data-testid^="casualty-opt-"]').first().click();
      }
    } else {
      // In case retreat stage is presented
      const anyOpt = page.locator('[data-testid^="retreat-opt-"]').first();
      await anyOpt.click();
    }

    // After submitting, the choice advances and game_version increments
    await expect
      .poll(async () => {
        const current = await snapshot(request, gameId, session);
        return current.game_version;
      })
      .toBeGreaterThan(initial.game_version);
  });
});
