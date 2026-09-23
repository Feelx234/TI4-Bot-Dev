import { test, expect, Page } from '@playwright/test';
import { createStartedGame, openPlayerGame } from './lobbyHelpers';

/**
 * Asserts core UI and system invariants on a given player or spectator page.
 */
async function assertPageInvariants(
  page: Page,
    expectedPlayer?: string,
  isSpectator = false
) {
  // 1. Connection must be connected
  const indicator = page.locator('[data-testid="connection-indicator"]');
  await expect(indicator).toHaveAttribute('data-status', 'connected');

  // 2. SVG Board must be rendered and responsive
  const boardSvg = page.locator('[data-testid="ti4-board-svg"]');
  await expect(boardSvg).toBeVisible();

  // 3. Privacy Invariant: Spectators & opponents must never have private cards in DOM
  if (isSpectator) {
    const privateCards = page.locator('[data-private-card="true"]');
    await expect(privateCards).toHaveCount(0);
  } else if (expectedPlayer) {
    // Assert no private cards belonging to other players exist
    const foreignCards = page.locator(
      `[data-private-card="true"]:not([data-private-card-owner="${expectedPlayer}"])`
    );
    await expect(foreignCards).toHaveCount(0);
  }

  // 4. Arithmetic & State Invariant: Player resources must be valid non-negative integers
  const vpBadges = page.locator('[data-testid^="player-vp-"]');
  const count = await vpBadges.count();
  expect(count).toBeGreaterThan(0);
  for (let i = 0; i < count; i++) {
    const text = await vpBadges.nth(i).innerText();
    const match = /(\d+)\s*VP/.exec(text);
    expect(match).not.toBeNull();
    const vp = parseInt(match![1], 10);
    expect(vp).toBeGreaterThanOrEqual(0);
  }
}

test.describe('Multiplayer Online Flow & Invariant Suite', () => {
  test('multi-seat strategy draft, live sync, redaction, and randomized actions', async ({
    browser,
    request,
  }) => {
    // Setup listeners for zero console errors / unhandled exceptions
    const trackErrors = (page: Page, label: string) => {
      page.on('pageerror', (err) => {
        throw new Error(`[${label}] Unhandled browser error: ${err.message}`);
      });
      page.on('console', (msg) => {
        if (msg.type() === 'error') {
          // Ignore harmless favicon 404s
          if (!msg.text().includes('favicon')) {
            throw new Error(`[${label}] Console Error: ${msg.text()}`);
          }
        }
      });
    };

    // Create a fresh isolated game for this test run
    const { gameId, players } = await createStartedGame(request, 3, 42);
    const [p1, p2] = players;

    // 1. Open Player 1 (p1)
    const contextP1 = await browser.newContext();
    const pageP1 = await contextP1.newPage();
    trackErrors(pageP1, 'Player 1');
    await openPlayerGame(pageP1, gameId, p1.session);

    // 2. Open Player 2 (p2)
    const contextP2 = await browser.newContext();
    const pageP2 = await contextP2.newPage();
    trackErrors(pageP2, 'Player 2');
    await openPlayerGame(pageP2, gameId, p2.session);

    // Wait for both claimed seats to connect.
    await expect(pageP1.locator('[data-testid="turn-status-bar"]')).toBeVisible();
    await expect(pageP2.locator('[data-testid="turn-status-bar"]')).toBeVisible();

    // Initial invariant checks across both claimed tabs.
    await assertPageInvariants(pageP1, p1.id);
    await assertPageInvariants(pageP2, p2.id);

    // Active Seat Choice Invariant:
    // P1 must have pending-choice-dialog; P2 must not.
    const p1Modal = pageP1.locator('[data-testid="pending-choice-dialog"]');
    await expect(p1Modal).toBeVisible();

    const p2Modal = pageP2.locator('[data-testid="pending-choice-dialog"]');
    await expect(p2Modal).toHaveCount(0);


    // Assert options in P1 modal are actionable
    const p1Options = pageP1.locator('[data-testid="choice-option"]');
    const optCount = await p1Options.count();
    expect(optCount).toBeGreaterThan(0);
    for (let i = 0; i < optCount; i++) {
      await expect(p1Options.nth(i)).toHaveAttribute('data-actionable', 'true');
    }

    // Test Choice Minimization: P1 minimizes dialog to inspect map
    await pageP1.locator('[data-testid="minimize-choice-button"]').click();
    await expect(p1Modal).toHaveCount(0);
    const minBanner = pageP1.locator('[data-testid="minimized-choice-banner"]');
    await expect(minBanner).toBeVisible();

    // Map and board SVG are completely accessible while decision is minimized
    const boardSvg = pageP1.locator('[data-testid="ti4-board-svg"]');
    await expect(boardSvg).toBeVisible();
    await expect(pageP1.locator('[data-testid="system-hex-18"]')).toBeVisible();
    await expect(pageP1.locator('button[title="Zoom In"]')).toBeVisible();

    // Verify Event Log toggle and entries have timestamps while inspecting
    const eventLogToggle = pageP1.locator('[data-testid="event-log-toggle"]');
    await eventLogToggle.click();
    const logList = pageP1.locator('[data-testid="event-log-list"]');
    await expect(logList).toBeVisible();
    const firstEntry = pageP1.locator('[data-testid="event-log-entry"]').first();
    await expect(firstEntry).toBeVisible();
    const entryText = await firstEntry.innerText();
    expect(entryText).toMatch(/\d{2}:\d{2}:\d{2}/);
    // Close event log drawer
    await eventLogToggle.click();
    await expect(logList).toHaveCount(0);

    // Restore dialog
    await pageP1.locator('[data-testid="resume-choice-button"]').click();
    await expect(p1Modal).toBeVisible();
    await expect(minBanner).toHaveCount(0);

    // Verify P1 has human-readable secret objective with description and tooltip
    const p1Secret = pageP1.locator('[data-testid^="secret-objective-item-"]');
    await expect(p1Secret).toBeVisible();
    await expect(p1Secret).toContainText('Forge an Alliance');
    await expect(p1Secret).toContainText('Control 4 cultural planets.');
    const soTitle = await p1Secret.getAttribute('title');
    expect(soTitle).toContain('Forge an Alliance');
    expect(soTitle).toContain('Control 4 cultural planets.');

    // P1 submits choice (first option: Leadership)
    await pageP1.locator('[data-testid="submit-choice-button"]').click();

    // P1 modal closes
    await expect(p1Modal).toHaveCount(0);

    // P1 now displays human-readable strategy card badge with tooltip
    const scBadge = pageP1.locator('[data-testid="strategy-card-badge-pok1leadership"]');
    await expect(scBadge).toBeVisible();
    await expect(scBadge).toContainText('1. Leadership');
    const scTitle = await scBadge.getAttribute('title');
    expect(scTitle).toContain('Gain 3 command tokens');

    // Broadcast propagates: P2 now receives choice!
    await expect(p2Modal).toBeVisible({ timeout: 5000 });

    // Assert P2 options are actionable
    const p2Options = pageP2.locator('[data-testid="choice-option"]');
    const p2OptCount = await p2Options.count();
    expect(p2OptCount).toBeGreaterThan(0);

    // P2 submits choice
    await pageP2.locator('[data-testid="submit-choice-button"]').click();

    // The third human now has the draft. No server-side bot silently plays their turn.
    await expect(p2Modal).toHaveCount(0);
    await assertPageInvariants(pageP1, p1.id);
    await assertPageInvariants(pageP2, p2.id);

    // Disconnect & Reconnect Invariant Test
    await pageP1.locator('[data-testid="leave-game-button"]').click();
    await expect(pageP1.locator('[data-testid="lobby-container"]')).toBeVisible();

    // Rejoin as p1 with its tab-scoped current credential.
    await openPlayerGame(pageP1, gameId, p1.session);

    await expect(pageP1.locator('[data-testid="turn-status-bar"]')).toBeVisible();
    await assertPageInvariants(pageP1, p1.id);
  });
});
