import type { Page, TestInfo } from "@playwright/test";
import { openMockedGame, type MockChoice } from "../_shared/mockGame";
import { shot } from "../_shared/shot";
import { actor, galleryBoard } from "../_shared/fixtures";
import { opponent, playerWithHand } from "../_shared/players";
import type { BoardView, PlacedUnitView } from "../../../src/protocol/types";

// TI4_SHOT_PREFIX=before is how the "before" shots were taken (on the base commit, 6cfb3271);
// the default "after" regenerates the current layout.
export const prefix = process.env.TI4_SHOT_PREFIX ?? "after";

export const viewports = [
  { id: "desktop", width: 1440, height: 900 },
  { id: "phone", width: 390, height: 844 },
];

const RIVAL = `player_${"b793e54282b7a62cf6b1d7876eb".padEnd(64, "0")}`;
export const players = [{ ...playerWithHand(), faction: "sardakk" }, { ...opponent, id: RIVAL }];

const unit = (type: string, owner: string, planet?: string): PlacedUnitView => ({
  unit_type: type,
  owner,
  damaged: false,
  ...(planet ? { planet } : {}),
});

/**
 * The N'orr player invades Jord (system 18) with 2 infantry in space. With G'hom Sek'kus the engine also
 * offers one ground force from each planet of the active system and of adjacent systems without the
 * player's command token: here an infantry and a mech on Lodor (adjacent), an infantry on Arcturus.
 */
export function landingBoard(): BoardView {
  const base = galleryBoard.systems["18"];
  return {
    ...galleryBoard,
    combat: null,
    systems: {
      ...galleryBoard.systems,
      "18": {
        ...base,
        planets: { jord: { planet_id: "jord", controlled_by: RIVAL, exhausted: false } },
        units: [
          unit("carrier", actor),
          unit("infantry", actor),
          unit("infantry", actor),
          unit("infantry", RIVAL, "jord"),
        ],
      },
    },
    invasion: {
      system_id: "18",
      invasion_seq: 7,
      invader: actor,
      phase: "landing",
      planets: ["jord"],
      current_planet: null,
      defender: null,
      ground_round: 0,
      last_step: null,
      odds_context: {},
    },
  };
}

const commit = (index: number, unitType: string, from?: [string, string]) => ({
  id: `commit|${index}|jord`,
  label: `land ${unitType} on jord`,
  kind: "commit",
  payload: {
    planet: "jord",
    unit: unitType,
    damaged: false,
    ...(from ? { from_system: from[0], from_planet: from[1] } : {}),
  },
});

export const landingChoice: MockChoice = {
  prompt: "commit ground forces in 18",
  context: { subtype: "commit_ground_forces", target: { System: "18" } },
  options: [
    commit(0, "infantry"),
    commit(1, "infantry", ["26", "lodor"]),
    commit(2, "mech", ["26", "lodor"]),
    commit(3, "infantry", ["19", "arcturus"]),
    { id: "done_committing", label: "commit no more ground forces", kind: "decline", payload: {} },
  ] as never,
};

export async function openLanding(page: Page, viewport: (typeof viewports)[number]) {
  await page.setViewportSize({ width: viewport.width, height: viewport.height });
  const game = await openMockedGame(page, { players, board: landingBoard() as never, choice: landingChoice });
  await page.waitForTimeout(800);
  return game;
}

export async function take(page: Page, testInfo: TestInfo, id: string, viewport: (typeof viewports)[number]) {
  await shot(page, testInfo, `${prefix}-${id}-${viewport.id}`);
}
