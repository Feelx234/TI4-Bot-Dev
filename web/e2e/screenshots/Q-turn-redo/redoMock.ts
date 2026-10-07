import type { Page, Route } from "@playwright/test";
import { GAME_ID } from "../_shared/mockGame";
import type { TurnRedoStatus } from "../../../src/protocol/turnRedo";

/** The seat the browser plays as, and the other seat of the mocked table. */
export const ME = "gallery_seat";
export const OTHER = "other_seat";

export const newTurn: TurnRedoStatus = {
  seat: ME,
  requested_by: ME,
  turns_back: 1,
  redo_count: 1,
  stage: "new_turn",
  original_decisions: 212,
  rewound_to: 187,
  turn_complete: false,
  handoff_len: null,
  outcome: null,
  can_control: true,
};

export const handoff: TurnRedoStatus = {
  ...newTurn,
  stage: "auto_played",
  handoff_len: 203,
  outcome: {
    kept: 11,
    tail_total: 24,
    stop: { kind: "handoff", seat: ME },
    asking_seat: ME,
    deck_offsets: [{ deck: "action_card", delta: -1 }],
  },
};

export const conflict: TurnRedoStatus = {
  ...newTurn,
  stage: "auto_played",
  handoff_len: 196,
  outcome: {
    kept: 4,
    tail_total: 24,
    stop: {
      kind: "conflict",
      conflict: {
        original_cursor: 195,
        kind: "options_changed",
        seat: OTHER,
        prompt: "choose a system to activate",
        detail: "the options on offer are different now",
        deck_deltas: [],
      },
    },
    asking_seat: OTHER,
    deck_offsets: [],
  },
};

export const deckConflict: TurnRedoStatus = {
  ...conflict,
  outcome: {
    kept: 2,
    tail_total: 24,
    stop: {
      kind: "conflict",
      conflict: {
        original_cursor: 193,
        kind: "deck_cursor",
        seat: OTHER,
        prompt: "play an action card",
        detail: "the new turn drew a different number of cards",
        deck_deltas: [{ deck: "action_card", delta: 1 }],
      },
    },
    asking_seat: OTHER,
    deck_offsets: [],
  },
};

/** Answers `GET /turn-redo` with the status. Register before opening the game. */
export async function routeTurnRedoStatus(page: Page, status: TurnRedoStatus | null) {
  await page.route(`**/api/games/${GAME_ID}/turn-redo`, (route: Route) => {
    if (route.request().method() === "GET") return route.fulfill({ json: { status } });
    return route.fallback();
  });
}

/** A POST to `/turn-redo` that is still running when the screenshot is taken. */
export async function holdTurnRedoPost(page: Page) {
  await page.route(`**/api/games/${GAME_ID}/turn-redo`, (route: Route) => {
    if (route.request().method() === "POST") return; // never answered
    return route.fallback();
  });
}

/** A POST to `/turn-redo` that the server refuses. */
export async function refuseTurnRedoPost(page: Page, status: number, body: string) {
  await page.route(`**/api/games/${GAME_ID}/turn-redo`, (route: Route) => {
    if (route.request().method() === "POST") return route.fulfill({ status, body });
    return route.fallback();
  });
}
