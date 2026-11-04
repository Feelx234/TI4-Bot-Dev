import type { Page } from "@playwright/test";
import type { ObjectiveProgressView, PlayerView } from "../../../src/protocol/types";
import { openMockedGame, type MockChoice } from "../_shared/mockGame";
import { opponent, playerWithHand, withPlayer } from "../_shared/players";
import { galleryBoard } from "../_shared/fixtures";

export const SIZES = {
  desktop: { width: 1440, height: 900 },
  "390x844": { width: 390, height: 844 },
} as const;
export type SizeName = keyof typeof SIZES;

/** "before" runs on the commit without the change: same scenes, no assertions on the new elements. */
export const BEFORE = process.env.TI4_SHOT_PREFIX === "before";
export const prefixed = (name: string) => (BEFORE ? `before-${name}` : name);

// Technologies by colour: Blue (propulsion) amd, det; Green (biotic) nm, pa; Yellow (cybernetic)
// st, sdn; Red (warfare) ps, aida. cr2 is a unit upgrade and has no colour.
export const SCENES = {
  // The viewer holds 2 Blue and 1 Green: one colour met of the 2 Diversify Research needs.
  "one-colour": {
    me: ["amd", "det", "nm", "cr2"],
    other: ["st", "sdn", "ps", "aida"],
    third: ["amd", "nm", "st", "ps"],
    diversify: { me: 1, other: 2, third: 0 },
    master: { me: 1, other: 2, third: 0 },
  },
  // The viewer reaches the second colour, so Diversify Research reads 2 of 2.
  "two-colours": {
    me: ["amd", "det", "nm", "pa", "cr2"],
    other: ["st", "sdn", "ps", "aida", "nm", "pa"],
    third: ["amd", "nm", "st", "ps"],
    diversify: { me: 2, other: 3, third: 0 },
    master: { me: 2, other: 3, third: 0 },
  },
} as const;
export type SceneName = keyof typeof SCENES;

const view = (have: number, threshold: number): ObjectiveProgressView => ({
  have,
  threshold,
  satisfied: have >= threshold,
});

export function scene(name: SceneName) {
  const spec = SCENES[name];
  const hand = playerWithHand({ technologies: [...spec.me] });
  const other: PlayerView = { ...opponent, technologies: [...spec.other] };
  const third = withPlayer(opponent, { id: "seat_c", faction: "jolnar", strategy_cards: [], technologies: [...spec.third] });
  const players = [hand, other, third];
  const keys: ("me" | "other" | "third")[] = ["me", "other", "third"];
  const table = {
    revealed_objectives: ["diversify", "master_science"],
    scored_objectives: {},
    objective_progress: Object.fromEntries(
      players.map((player, index) => [
        player.id,
        {
          diversify: view(spec.diversify[keys[index]], 2),
          master_science: view(spec.master[keys[index]], 4),
        },
      ]),
    ),
  };
  return { hand, players, table };
}

export async function openScene(page: Page, size: SizeName, name: SceneName) {
  const { players, table } = scene(name);
  await page.setViewportSize(SIZES[size]);
  await openMockedGame(page, {
    phase: "action",
    board: galleryBoard,
    players,
    table,
    choice: {
      prompt: "action phase",
      player: opponent.id,
      options: [{ id: "pass", label: "pass", kind: "action" }],
    } as MockChoice,
  });
  await page.getByTestId("ti4-board-svg").waitFor();
  await page.waitForTimeout(400);
}

export async function openObjectives(page: Page, size: SizeName) {
  await page
    .getByTestId(size === "desktop" ? "objectives-modal-button" : "objectives-mobile-button")
    .click();
  await page.getByTestId("objectives-modal").waitFor();
}

export async function noHorizontalPageScroll(page: Page) {
  return page.evaluate(
    () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
  );
}
