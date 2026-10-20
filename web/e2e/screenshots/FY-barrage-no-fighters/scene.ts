import type { Page, TestInfo } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { shot } from "../_shared/shot";
import { actor, hitAssignmentBoard } from "../_shared/fixtures";
import { opponent, playerWithHand } from "../_shared/players";

export const viewports = [
  { id: "desktop", width: 1440, height: 900 },
  { id: "phone", width: 390, height: 844 },
];

const unit = (unit_type: string, owner: string, planet: string | null = null) => ({
  unit_type,
  owner,
  damaged: false,
  planet,
  galvanized: false,
});

export interface BarrageScene {
  /** Units the defender (the viewer) has in space. */
  defender: string[];
  /** The attacker's ships. */
  attacker: string[];
  /** Hits per seat that rolled a barrage. */
  hits: Record<string, number>;
  /** Faction of the attacking seat. */
  attackerFaction?: string;
  /** Defender units left after the barrage's removals (default: the same as before). */
  defenderAfter?: string[];
}

/** The retreat question the defender gets right after the barrage, as in the feedback report. */
export async function openBarrageRecap(page: Page, viewport: (typeof viewports)[number], scene: BarrageScene) {
  const start = [...scene.defender.map((t) => unit(t, actor)), ...scene.attacker.map((t) => unit(t, opponent.id))];
  const now = [
    ...(scene.defenderAfter ?? scene.defender).map((t) => unit(t, actor)),
    ...scene.attacker.map((t) => unit(t, opponent.id)),
  ];
  const board = {
    ...hitAssignmentBoard,
    systems: { ...hitAssignmentBoard.systems, "18": { ...hitAssignmentBoard.systems["18"], units: now } },
    combat: {
      system_id: "18",
      round: 1,
      phase: "pre_roll",
      stage: "announce_retreat",
      attacker: opponent.id,
      defender: actor,
      round_start: start,
      barrage_start: start,
      barrage_hits: scene.hits,
      barrage_dice: Object.entries(scene.hits).flatMap(([player, hits]) =>
        [10, 4].slice(0, Math.max(hits, 1)).map((roll, i) => ({
          player,
          unit: "destroyer",
          roll: i < hits ? 10 : 4,
          target: 9,
          hit: i < hits,
        })),
      ),
      attacker_hits: 0,
      defender_hits: 0,
      dice_rolls: [],
    },
  };
  await page.setViewportSize({ width: viewport.width, height: viewport.height });
  await openMockedGame(page, {
    players: [playerWithHand(), { ...opponent, faction: scene.attackerFaction ?? opponent.faction }],
    board: board as never,
    choice: {
      prompt: "announce a retreat from 18",
      context: { subtype: "announce_retreat", target: { System: "18" }, space_battle: true },
      options: [
        { id: "stay", label: "stay and fight", kind: "retreat", payload: { system: "18" } },
        { id: "retreat", label: "announce a retreat", kind: "retreat", payload: { system: "18" } },
      ] as never,
    },
  });
  await page.waitForTimeout(800);
}

export async function take(page: Page, testInfo: TestInfo, id: string, viewport: (typeof viewports)[number]) {
  await shot(page, testInfo, `${id}-${viewport.id}`);
}

export { opponent, actor };
