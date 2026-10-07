import { test, expect } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openMockedGame } from "../_shared/mockGame";
import { actionCardEventLog } from "../_shared/fixtures";
import { playerWithHand, opponent } from "../_shared/players";
import { openLogFully } from "../_shared/eventLog";
import { routeTurnRedoStatus, deckConflict } from "./redoMock";

// The redone turn drew a different number of cards: later draws would shift, so it is a conflict.
test("turn redo: deck cursor conflict", async ({ page }, testInfo) => {
  await routeTurnRedoStatus(page, deckConflict);
  await openMockedGame(page, { players: [playerWithHand(), opponent], events: actionCardEventLog(), phase: "action" });
  await expect(page.getByTestId("turn-redo-bar")).toContainText("different number of cards");
  await shot(page, testInfo, "8-deck-conflict");
});
