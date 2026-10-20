import type { Page } from "@playwright/test";
import { PROTOCOL_VERSION, type ClientMessage } from "../../../src/protocol/types";
import { GAME_ID, openMockedGame, type MockChoice } from "../_shared/mockGame";
import { actor, galleryBoard } from "../_shared/fixtures";
import { opponent, playerWithHand } from "../_shared/players";

export const SIZES = {
  desktop: { width: 1440, height: 900 },
  "390x844": { width: 390, height: 844 },
} as const;
export type SizeName = keyof typeof SIZES;

/** `before` runs on the base commit 80ce1bf1 (the engine then sends the old payloads); `after` on this change. */
export const prefix = process.env.TI4_SHOT_PREFIX ?? "after";
export const legacy = prefix === "before";

/** Engine ids of the systems offered, as a Diplomacy Rider's owner holds planets in them. */
export const SYSTEMS = ["25", "27", "28", "22"];

const systemOptions = (labels: (id: string) => string) =>
  SYSTEMS.map((id) => ({ id, kind: "system", label: labels(id), payload: {} }));

/** Diplomacy Rider after a correct prediction: "choose 1 system that contains a planet you control". */
export const diplomacyRiderPick = (nonce = "rider-1"): MockChoice => ({
  prompt: "Diplomacy Rider: choose a system",
  nonce,
  context: { subtype: "diplomacy_rider_system", optional: false, source: { ActionCard: "diplo_rider" } },
  options: systemOptions((id) => id),
});

/** Warfare Rider's reward: the system whose ships get the token (same list shape, other card). */
export const warfareRiderPick = (): MockChoice => ({
  prompt: "Warfare Rider: choose a system",
  nonce: "rider-2",
  context: { subtype: "warfare_rider_system", optional: false, source: { ActionCard: "war_rider" } },
  options: systemOptions((id) => id),
});

/** Umbat (Muaat agent): "produce in which system", options labelled "system <id>" by the engine. */
export const umbatPick = (): MockChoice => ({
  prompt: "Umbat: produce in which system",
  nonce: "umbat-1",
  context: { subtype: "umbat_system", optional: true, source: { Content: "muaatagent" } },
  options: [...systemOptions((id) => `system ${id}`), { id: "decline", kind: "decline", label: "Decline" }],
});

/** Minister of Peace's question as the engine asks it (laws::offer_discard). The base commit sent no `details`. */
export const ministerOfPeaceOffer = (nonce = "peace-1", system = "27"): MockChoice => ({
  prompt: "Minister of Peace: discard to end the active player's turn",
  nonce,
  context: { subtype: "offer_discard_law", optional: false, source: { Content: "minister_peace" } },
  options: [
    { id: "discard", kind: "law", label: "discard minister_peace" },
    { id: "decline", kind: "decline", label: "Decline" },
  ],
  ...(legacy
    ? {}
    : {
        details: {
          kind: "offer",
          card: {
            title: "Minister of Peace",
            tag: "law",
            window: null,
            text: "After a player activates a system that contains 1 or more of a different player's units, you may discard this card - immediately end the active player's turn.",
          },
          facts: [
            { label: "Activated system", system },
            { label: "Activated by", seat: "other_seat" },
          ],
          captions: {
            discard: { label: "Discard Minister of Peace", hint: "Ends the active player's turn at once" },
            decline: { label: "Keep Minister of Peace", hint: null },
          },
        },
      }),
});

/** Opens a quiet game as the owner of the law; `advance` moves the history on and asks a question. */
export async function openScene(page: Page, size: SizeName = "desktop", choice: MockChoice | null = null) {
  await page.setViewportSize(SIZES[size]);
  const game = await openMockedGame(page, {
    players: [playerWithHand({ faction: "hacan" }), opponent],
    board: galleryBoard,
    state: { laws: { minister_peace: actor } },
    choice,
  });
  await page.getByTestId("ti4-board-svg").waitFor();
  const submissions: ClientMessage[] = [];
  game.socket.onMessage((data) => {
    const message = JSON.parse(String(data)) as ClientMessage;
    if (message.type === "submit_choice") submissions.push(message);
  });
  const { events: _events, ...base } = game.snapshot;
  let version = game.snapshot.game_version;
  const advance = (cursor: number, next: MockChoice | null, redoCount = 0) => {
    version += 1;
    game.send({
      ...base,
      type: "state_update",
      protocol_version: PROTOCOL_VERSION,
      game_id: GAME_ID,
      game_version: version,
      history: { cursor, redo_count: redoCount },
      pending_choice: next
        ? {
            nonce: next.nonce ?? "n",
            choice: {
              player: next.player ?? actor,
              prompt: next.prompt,
              context: next.context as never,
              options: next.options,
              ...(next.details ? { details: next.details } : {}),
            },
          }
        : null,
    });
  };
  return { game, submissions, advance };
}

export async function openSheet(page: Page, size: SizeName) {
  if (size !== "desktop") {
    await page.getByTestId("player-sheet-toggle").click();
    await page.waitForTimeout(400);
  }
}
