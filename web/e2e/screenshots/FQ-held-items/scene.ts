import type { Page } from "@playwright/test";
import { openMockedGame, type MockChoice } from "../_shared/mockGame";
import { actor, galleryBoard } from "../_shared/fixtures";
import { me, opponent, withPlayer } from "../_shared/players";
import type { BoardView, PlayerView } from "../../../src/protocol/types";

export const SIZES = {
  desktop: { width: 1440, height: 900 },
  "390x844": { width: 390, height: 844 },
} as const;
export type SizeName = keyof typeof SIZES;

export const RIVAL = opponent.id;

/** A mid-game Sol seat: relics, fragments, exhaustable techs, a legendary planet, leaders, notes. */
export const rich: PlayerView = withPlayer(me, {
  victory_points: 5,
  technologies: ["aida", "pi", "sar", "amd", "ac2", "qdn"],
  exhausted_technologies: ["pi"],
  relics: ["heartofixth", "dynamiscore", "obsidian"],
  exhausted_relics: ["heartofixth"],
  relic_fragments: { cultural: 3, hazardous: 1, frontier: 2 },
  exhausted_legendary: ["primor"],
  leaders: { solagent: "readied", solcommander: "unlocked", solhero: "locked" },
});

export const rival: PlayerView = withPlayer(opponent, {
  technologies: ["sar", "gls"],
  exhausted_technologies: [],
  relics: ["thetriad"],
  relic_fragments: { industrial: 2 },
  leaders: { hacanagent: "exhausted", hacancommander: "locked", hacanhero: "purged" },
});

/** Primor and an attachment on a planet of the viewer; the rest of the gallery board as it is. */
function boardWith(): BoardView {
  const systems = structuredClone(galleryBoard.systems);
  for (const system of Object.values(systems)) {
    for (const [id, planet] of Object.entries(system.planets)) {
      if (id === "primor") system.planets[id] = { ...planet, controlled_by: actor };
      if (id === "hopes_end") system.planets[id] = { ...planet, controlled_by: RIVAL };
      if (id === "jord") system.planets[id] = { ...planet, controlled_by: actor, attachments: ["dysonsphere"] };
    }
  }
  return { ...galleryBoard, systems };
}
export const richBoard = boardWith();

/** Notes are keyed `alias:owner faction`; Support for the Throne has its own map. */
export const richState = {
  promissory_notes: {
    "cf:hacan": actor,
    "ta:hacan": actor,
    "an:hacan": actor,
    "ps:sol": actor,
    "cf:sol": actor,
    "ta:sol": RIVAL,
    "ps:hacan": RIVAL,
    "convoys:hacan": RIVAL,
  },
  promissory_faceup: ["an:hacan", "ta:sol", "convoys:hacan"],
  support_holders: { hacan: actor },
  faction_marks: {},
};

export const ACTION_MENU: MockChoice = {
  prompt: "Take an action",
  options: [
    { id: "component|tech|aida", label: "AI Development Algorithm", kind: "action" },
    { id: "component|leader|solagent", label: "Sol agent", kind: "action" },
    { id: "primor", label: "The Atrament", kind: "legendary" },
    { id: "purge|cultural", label: "Purge 3 cultural fragments", kind: "action" },
    { id: "pass", label: "pass", kind: "action" },
  ],
  details: {
    kind: "turn_menu",
    closing: false,
    tokens: { tactic: 3, fleet: 3, strategy: 2 },
    strategy_cards: [],
    partners: [],
  },
};

export async function openScene(
  page: Page,
  size: SizeName,
  scene: { players: PlayerView[]; state?: Record<string, unknown>; board?: BoardView; choice?: MockChoice | null },
) {
  await page.setViewportSize(SIZES[size]);
  await openMockedGame(page, {
    phase: "action",
    board: scene.board ?? galleryBoard,
    players: scene.players,
    state: scene.state,
    choice:
      scene.choice === undefined
        ? ({ prompt: "action phase", player: RIVAL, options: [{ id: "pass", label: "pass", kind: "action" }] } as MockChoice)
        : scene.choice,
  });
  await page.getByTestId("ti4-board-svg").waitFor();
  await page.waitForTimeout(400);
}

/** Opens the Players drawer on a phone, then the Held items panel for a seat. */
export async function openHeldItems(page: Page, size: SizeName, seat: 0 | 1 = 0) {
  if (size !== "desktop") {
    await page.getByTestId("player-sheet-toggle").click();
    await page.waitForTimeout(400);
  }
  await page.getByTestId(`held-items-button-${seat}`).scrollIntoViewIfNeeded();
  await page.getByTestId(`held-items-button-${seat}`).click();
  await page.getByTestId("held-items-panel").waitFor();
  await page.waitForTimeout(250);
}

export async function noHorizontalScroll(page: Page) {
  return page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
}
