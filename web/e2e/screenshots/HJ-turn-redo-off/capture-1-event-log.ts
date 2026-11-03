import { test, expect } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openMockedGame } from "../_shared/mockGame";
import { actionCardEventLog } from "../_shared/fixtures";
import { playerWithHand, opponent } from "../_shared/players";
import { openLogFully } from "../_shared/eventLog";
import { routeTurnRedoStatus, OTHER } from "../Q-turn-redo/redoMock";

// The server's turn redo is switched off (docs/DISABLE_XF123.md): the open event log of a seated
// player no longer has "Redo my last turn". The `before-*` shots were taken on the base commit's
// behaviour (TI4_SHOT_PREFIX=before TI4_SHOT_TURN_REDO=1) and are committed; the runner makes the
// `after-*` shots with the folder's manifest env (TI4_SHOT_TURN_REDO=0).
const prefix = process.env.TI4_SHOT_PREFIX ?? "after";
// Seated player, and the host (whose log had the extra "Whose turn" list).
const VIEWS = { player: { host: OTHER }, host: {} } as const;

for (const [name, extra] of Object.entries(VIEWS)) {
  test(`event log without turn redo, ${name}`, async ({ page }, testInfo) => {
    await routeTurnRedoStatus(page, null);
    await openMockedGame(page, {
      players: [playerWithHand(), opponent],
      events: actionCardEventLog(),
      phase: "status",
      ...extra,
    });
    await openLogFully(page);
    await expect(page.getByTestId("turn-redo-btn")).toHaveCount(prefix === "before" ? 1 : 0);
    await shot(page, testInfo, `${prefix}-event-log-${name}`, { of: page.locator("#event-log-drawer"), pad: 8 });
  });
}
