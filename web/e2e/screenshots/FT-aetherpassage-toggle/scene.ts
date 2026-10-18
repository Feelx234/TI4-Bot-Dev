import type { Page } from "@playwright/test";
import { PROTOCOL_VERSION, type ClientMessage } from "../../../src/protocol/types";
import { GAME_ID, openMockedGame, type MockChoice } from "../_shared/mockGame";
import { opponent, playerWithHand, actor } from "../_shared/players";

export const SIZES = {
  desktop: { width: 1440, height: 900 },
  "390x844": { width: 390, height: 844 },
} as const;
export type SizeName = keyof typeof SIZES;

/** The reaction window after a system activation, as it asks Empyrean about Aetherpassage. */
export const aetherOffer = (nonce: string): MockChoice => ({
  prompt: "after SYSTEM_ACTIVATED",
  nonce,
  context: { subtype: "reaction_after_SYSTEM_ACTIVATED", optional: true, source: { Reaction: "SYSTEM_ACTIVATED" } },
  options: [
    { id: "ability:empyrean:aetherpassage:SYSTEM_ACTIVATED:after", label: "Aetherpassage (after system activated)", kind: "ability" },
    { id: "decline", label: "Pass", kind: "decline" },
  ],
});

/** A quiet Empyrean seat on turn; `advance` moves the history forward and optionally asks a question. */
export async function openScene(page: Page, size: SizeName = "desktop") {
  await page.setViewportSize(SIZES[size]);
  const game = await openMockedGame(page, {
    players: [playerWithHand({ faction: "empyrean" }), opponent],
    choice: null,
  });
  await page.getByTestId("ti4-board-svg").waitFor();
  const submissions: ClientMessage[] = [];
  game.socket.onMessage((data) => {
    const message = JSON.parse(String(data)) as ClientMessage;
    if (message.type === "submit_choice") submissions.push(message);
  });
  const { events: _events, ...base } = game.snapshot;
  let version = game.snapshot.game_version;
  const advance = (cursor: number, choice: MockChoice | null, redoCount = 0) => {
    version += 1;
    game.send({
      ...base,
      type: "state_update",
      protocol_version: PROTOCOL_VERSION,
      game_id: GAME_ID,
      game_version: version,
      history: { cursor, redo_count: redoCount },
      pending_choice: choice
        ? {
            nonce: choice.nonce ?? "n",
            choice: {
              player: choice.player ?? actor,
              prompt: choice.prompt,
              context: choice.context as never,
              options: choice.options,
            },
          }
        : null,
    });
  };
  return { game, submissions, advance };
}

/** Phone layouts keep the player sheet behind a toggle. */
export async function openSheet(page: Page, size: SizeName) {
  if (size !== "desktop") {
    await page.getByTestId("player-sheet-toggle").click();
    await page.waitForTimeout(400);
  }
}
