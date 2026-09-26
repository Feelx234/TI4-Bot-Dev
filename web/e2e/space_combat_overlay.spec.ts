import { test, expect, type APIRequestContext } from "@playwright/test";
import { openPlayerGame } from "./lobbyHelpers";
import type { InitialSnapshotMsg } from "../src/protocol/types";
import { getUnitBaseType } from "../src/components/UnitIcon";

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
    expect(launch.ok(), `launch scenario: ${launch.status()} ${await launch.text()}`).toBe(true);
    const { game_id: gameId, player_session: session, player_id: playerId } = await launch.json();

    // 2. Verify backend state starts with active combat already in progress
    const initial = await snapshot(request, gameId, session);
    expect(initial.view.board.combat).toBeDefined();
    expect(initial.view.board.combat?.attacker).toBe(playerId);
    expect(initial.pending_choice).toBeDefined();

    // Verify Sol has action cards for several stages of this battle.
    const solPlayer = initial.view.players.find((p) => p.id === playerId);
    for (const card of ["dh1", "sh1", "courageous", "salvage"]) {
      expect(solPlayer?.held_action_cards).toContain(card);
    }

    // 3. Open the game in browser as active player
    await openPlayerGame(page, gameId, session);

    // 4. Verify the Space Combat Overlay appears immediately
    const modal = page.getByTestId("combat-resolution-modal");
    await expect(modal).toBeVisible();

    // Verify modal panel is present
    const panel = modal.locator(".combat-arena-panel");
    await expect(panel).toBeVisible();
    await expect(panel).toHaveCSS("background-color", "rgb(11, 18, 34)");
    await expect(modal).toHaveCSS("background-color", "rgb(7, 12, 22)");

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

    for (const name of ["Direct Hit", "Shields Holding", "Courageous to the End", "Salvage"]) {
      await expect(attackerCard.getByTestId(`combat-cards-${playerId}`)).toContainText(name);
    }
    const opponent = initial.view.board.combat?.defender;
    if (opponent) {
      const count = initial.view.players.find((p) => p.id === opponent)?.action_cards_count;
      await expect(defenderCard.getByTestId(`combat-cards-${opponent}`)).toContainText(
        `Action cards: ${count}`,
      );
    }

    const firstRoll = initial.view.board.combat?.dice_rolls?.[0];
    if (firstRoll) {
      const seat = firstRoll.player ?? playerId;
      const card = seat === playerId ? attackerCard : defenderCard;
      const badge = card.getByTestId(
        `combat-roll-group-${seat}-${getUnitBaseType(firstRoll.unit)}`,
      );
      await expect(badge).toBeVisible();
      await badge.hover();
      await expect(badge.locator(".combat-unit-row__roll-tooltip")).toBeVisible();
      await expect(badge.locator(".combat-unit-row__roll-tooltip")).toContainText(
        `${firstRoll.roll} vs ${firstRoll.target}+`,
      );
    }

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
    if (
      initial.pending_choice?.choice.options.some((option) =>
        option.id.endsWith(":SUSTAIN_DAMAGE_USED:after"),
      )
    ) {
      await page.getByTestId("play-direct-hit-btn").click();
    } else if (subtype === "sustain_damage") {
      // Test integrated sustain option on the dreadnought row or decline
      const sustainBtn = attackerCard.locator('[data-testid^="sustain-opt-"]');
      if ((await sustainBtn.count()) > 0) {
        await sustainBtn.first().click();
      } else {
        await page.getByTestId("decline-sustain-btn").click();
      }
    } else if (subtype === "assign_casualty") {
      // Test clicking interactive unit row directly or casualty button
      const interactiveRow = attackerCard.locator(".combat-unit-row--interactive");
      if ((await interactiveRow.count()) > 0) {
        await interactiveRow.first().click();
      } else {
        await page.locator('[data-testid^="casualty-opt-"]').first().click();
      }
    } else {
      const stay = initial.pending_choice?.choice.options.find(
        (option) => option.id === "stay" || option.id === "decline",
      );
      expect(stay).toBeDefined();
      await page.getByTestId(`retreat-opt-${stay!.id}`).click();
    }

    // After submitting, the choice advances and game_version increments
    await expect
      .poll(async () => {
        const current = await snapshot(request, gameId, session);
        return current.game_version;
      })
      .toBeGreaterThan(initial.game_version);
  });

  test("can submit sustain damage after playing Direct Hit", async ({ page, request }) => {
    test.setTimeout(90_000);
    const launch = await request.post(`${backend}/api/dev/scenarios/launch`, {
      data: { scenario_id: "ongoing_combat", seed: 42 },
    });
    expect(launch.ok(), `launch scenario: ${launch.status()} ${await launch.text()}`).toBe(true);
    const { game_id: gameId, player_session: session, player_id: playerId } = await launch.json();
    const initial = await snapshot(request, gameId, session);
    const defender = initial.view.board.combat?.defender;
    expect(defender).toBeTruthy();
    expect(initial.pending_choice?.choice.context?.subtype).toBe("announce_retreat");
    const systemId = initial.view.board.combat!.system_id;
    const dreadnoughts = (state: InitialSnapshotMsg) =>
      state.view.board.systems[systemId].units.filter(
        (unit) => unit.owner === defender && unit.unit_type === "dreadnought",
      ).length;
    expect(dreadnoughts(initial)).toBeGreaterThan(0);
    await openPlayerGame(page, gameId, session);
    expect(initial.pending_choice?.choice.player).toBe(playerId);
    await expect(page.getByTestId("combat-resolution-modal")).toBeVisible();
    const stay = initial.pending_choice!.choice.options.find(
      (option) => option.id === "stay" || option.id === "decline",
    );
    expect(stay).toBeDefined();
    await page.getByTestId(`retreat-opt-${stay!.id}`).click();
    await expect
      .poll(async () =>
        (await snapshot(request, gameId, session)).pending_choice?.choice.options.some((option) =>
          option.id.endsWith(":SUSTAIN_DAMAGE_USED:after"),
        ),
      )
      .toBe(true);
    await page.getByTestId("play-direct-hit-btn").click();
    await expect
      .poll(async () => (await snapshot(request, gameId, session)).game_version)
      .toBeGreaterThan(initial.game_version);
    const after = await snapshot(request, gameId, session);
    expect(dreadnoughts(after)).toBeLessThan(dreadnoughts(initial));

    await expect
      .poll(async () =>
        (await snapshot(request, gameId, session)).pending_choice?.choice.options.some((option) =>
          option.id.endsWith(":HITS_TO_ASSIGN:when"),
        ),
      )
      .toBe(true);
    await page.getByRole("radio", { name: "Decline" }).click();
    await page.getByRole("button", { name: "Confirm choice" }).click();

    await expect
      .poll(
        async () =>
          (await snapshot(request, gameId, session)).pending_choice?.choice.context?.subtype,
      )
      .toBe("sustain_damage");
    const sustainChoice = await snapshot(request, gameId, session);
    expect(sustainChoice.pending_choice?.choice.player).toBe(playerId);
    const sustain = sustainChoice.pending_choice!.choice.options.find(
      (option) => option.id !== "decline" && option.kind !== "decline",
    );
    expect(sustain).toBeDefined();
    const sustainButton = page.getByTestId(`sustain-opt-${sustain!.id}`);
    await expect(sustainButton).toBeVisible();
    await sustainButton.click();
    await expect
      .poll(async () => (await snapshot(request, gameId, session)).game_version)
      .toBeGreaterThan(sustainChoice.game_version);
    await expect(page.getByTestId("combat-error-banner")).toBeHidden();
  });

  test("plays Shields Holding to cancel incoming combat hits", async ({ page, request }) => {
    test.setTimeout(90_000);
    const launch = await request.post(`${backend}/api/dev/scenarios/launch`, {
      data: { scenario_id: "ongoing_combat", seed: 42 },
    });
    expect(launch.ok()).toBe(true);
    const { game_id: gameId, player_session: session, player_id: playerId } = await launch.json();
    const initial = await snapshot(request, gameId, session);
    expect(
      initial.view.players.find((player) => player.id === playerId)?.held_action_cards,
    ).toContain("sh1");
    await openPlayerGame(page, gameId, session);
    const stay = initial.pending_choice!.choice.options.find(
      (option) => option.id === "stay" || option.id === "decline",
    );
    expect(stay).toBeDefined();
    await page.getByTestId(`retreat-opt-${stay!.id}`).click();

    // Remove the defender's sustained dreadnought, then respond to its return fire.
    await expect(page.getByTestId("play-direct-hit-btn")).toBeVisible();
    await page.getByTestId("play-direct-hit-btn").click();
    await expect
      .poll(async () =>
        (await snapshot(request, gameId, session)).pending_choice?.choice.options.some((option) =>
          option.id.endsWith(":HITS_TO_ASSIGN:when"),
        ),
      )
      .toBe(true);
    const before = await snapshot(request, gameId, session);
    await page.getByRole("radio", { name: /reaction:.*:HITS_TO_ASSIGN:when/ }).click();
    await page.getByRole("button", { name: "Confirm choice" }).click();

    await expect
      .poll(async () =>
        (await snapshot(request, gameId, session)).view.players.find((p) => p.id === playerId)
          ?.held_action_cards.includes("sh1"),
      )
      .toBe(false);
    const after = await snapshot(request, gameId, session);
    expect(after.game_version).toBeGreaterThan(before.game_version);
    expect(after.view.board.combat).toBeDefined();
  });
});
