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

/** An ability that asks its own question and only ever in the owner's own turn: Bio-Stims. */
export const bioStimsOffer = (nonce: string): MockChoice => ({
  prompt: "Bio-Stims: exhaust to ready a planet or technology",
  nonce,
  context: { subtype: "bio_stims_ready", optional: true, source: { Content: "bs" } },
  options: [
    { id: "ready-mecatol-rex", label: "exhaust Bio-Stims to ready mecatol-rex", kind: "bio_stims" },
    { id: "decline", label: "decline", kind: "decline" },
  ],
});

export interface SceneOptions {
  size?: SizeName;
  faction?: string;
  technologies?: string[];
  /** Ability settings as stored in localStorage `player_ask_<id>`: "false" (skip), "ownturn". */
  stored?: Record<string, string>;
  /** Whose turn it is: the viewer (default) or the opponent. */
  activePlayer?: "me" | "other";
}

/** A quiet seat; `advance` moves the history forward and optionally asks a question. */
export async function openScene(page: Page, options: SceneOptions = {}) {
  const { size = "desktop", faction = "saar", technologies, stored = {}, activePlayer = "me" } = options;
  await page.setViewportSize(SIZES[size]);
  if (Object.keys(stored).length) {
    await page.addInitScript((values: Record<string, string>) => {
      for (const [id, value] of Object.entries(values)) localStorage.setItem(`player_ask_${id}`, value);
    }, stored);
  }
  const active = activePlayer === "me" ? actor : opponent.id;
  const game = await openMockedGame(page, {
    players: [playerWithHand({ faction, ...(technologies ? { technologies } : {}) } as never), opponent],
    view: { active_player: active },
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
