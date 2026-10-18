import type { Page } from "@playwright/test";
import { PROTOCOL_VERSION, type ClientMessage } from "../../../src/protocol/types";
import { GAME_ID, openMockedGame, type MockChoice } from "../_shared/mockGame";
import { opponent, playerWithHand, actor } from "../_shared/players";

export const SIZES = {
  desktop: { width: 1440, height: 900 },
  "390x844": { width: 390, height: 844 },
} as const;
export type SizeName = keyof typeof SIZES;

/** The reaction window after a system activation, as it asks the Saar player about Captain Mendosa. */
export const mendosaOffer = (nonce: string): MockChoice => ({
  prompt: "after SYSTEM_ACTIVATED",
  nonce,
  context: { subtype: "reaction_after_SYSTEM_ACTIVATED", optional: true, source: { Reaction: "SYSTEM_ACTIVATED" } },
  options: [
    { id: "leader:saar:saaragent:SYSTEM_ACTIVATED:after", label: "Captain Mendosa (after system activated)", kind: "ability" },
    { id: "decline", label: "Pass", kind: "decline" },
  ],
});

/** An ability that asks its own question, outside a reaction window: Self-Assembly Routines. */
export const selfAssemblyOffer = (nonce: string): MockChoice => ({
  prompt: "Self-Assembly Routines: exhaust to place 1 mech from your reinforcements on a planet you control in this system",
  nonce,
  context: { subtype: "self_assembly_mech", optional: true, source: { Content: "sar" } },
  options: [
    { id: "mecatol-rex", label: "exhaust Self-Assembly Routines to place a mech on mecatol-rex", kind: "self_assembly" },
    { id: "decline", label: "decline", kind: "decline" },
  ],
});

export interface SceneOptions {
  size?: SizeName;
  faction?: string;
  technologies?: string[];
  /** Ability ids that start switched off (localStorage `player_ask_<id>` = false). */
  paused?: string[];
}

/** A quiet seat on turn; `advance` moves the history forward and optionally asks a question. */
export async function openScene(page: Page, options: SceneOptions = {}) {
  const { size = "desktop", faction = "saar", technologies, paused = [] } = options;
  await page.setViewportSize(SIZES[size]);
  if (paused.length) {
    await page.addInitScript((ids: string[]) => {
      for (const id of ids) localStorage.setItem(`player_ask_${id}`, "false");
    }, paused);
  }
  const game = await openMockedGame(page, {
    players: [playerWithHand({ faction, ...(technologies ? { technologies } : {}) } as never), opponent],
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
