import type { Page } from "@playwright/test";
import type { ChoiceOption } from "../../../src/protocol/types";
import { openMockedGame } from "../_shared/mockGame";
import { opponent, playerWithHand } from "../_shared/players";

/** `before` is taken on the commit without the change (no asserts). */
export const prefix = process.env.TI4_SHOT_PREFIX ?? "after";
export const checking = prefix !== "before";

export const SIZES = [
  { name: "desktop", width: 1280, height: 800 },
  { name: "390x844", width: 390, height: 844 },
];

/** The Naalu seat of a Prophecy of Kings game: the engine deals it `naaluagent` (the sheet lists naaluagent-te). */
export const naalu = playerWithHand({
  faction: "naalu",
  leaders: { naaluagent: "Readied", naalucommander: "Locked", naaluhero: "Locked" },
});

export const ZEU_ACTION: ChoiceOption = {
  id: "component|leader|naaluagent",
  label: "Z'eu",
  kind: "component",
  payload: {},
};

export async function openTurnMenu(page: Page, width: number, height: number) {
  await page.setViewportSize({ width, height });
  await openMockedGame(page, {
    players: [naalu, opponent],
    choice: {
      prompt: "action phase",
      options: [
        { id: "tactical", label: "take a tactical action", kind: "action" },
        ZEU_ACTION,
        { id: "pass", label: "pass", kind: "action" },
      ],
      details: {
        kind: "turn_menu",
        closing: false,
        tokens: { tactic: 3, fleet: 3, strategy: 2 },
        strategy_cards: [],
        partners: [],
      },
    },
  });
  await page.getByTestId("turn-action-bar").waitFor();
}

/** What the engine asks in `Game::perform_zeu`: the systems Z'eu may use, and a decline. */
export const ZEU_QUESTION = {
  prompt: "Z'eu: perform a tactical action without placing a command token",
  context: { subtype: "zeu_activate_system", optional: true },
  options: [
    { id: "24", label: "activate 24 without a command token", kind: "activate", payload: {} },
    { id: "26", label: "activate 26 without a command token", kind: "activate", payload: {} },
    { id: "decline", label: "Decline", kind: "decline", payload: {} },
  ],
};
