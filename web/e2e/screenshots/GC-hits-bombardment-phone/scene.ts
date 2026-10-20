import type { Page, TestInfo } from "@playwright/test";
import { openMockedGame, type MockChoice } from "../_shared/mockGame";
import { shot } from "../_shared/shot";
import { actor, galleryBoard, hitAssignmentBoard } from "../_shared/fixtures";
import { opponent, playerWithHand } from "../_shared/players";
import type { BoardView, PlacedUnitView } from "../../../src/protocol/types";

// TI4_SHOT_PREFIX=before is how the "before" shots were taken (on the base commit 80ce1bf1);
// the default "after" regenerates the current layout.
export const prefix = process.env.TI4_SHOT_PREFIX ?? "after";

export const viewports = [
  { id: "desktop", width: 1440, height: 900 },
  { id: "phone", width: 390, height: 844 },
  { id: "small", width: 360, height: 740 },
  { id: "landscape", width: 844, height: 390 },
];
export type Viewport = (typeof viewports)[number];

export const RIVAL = `player_${"b793e54282b7a62cf6b1d7876eb".padEnd(64, "0")}`;

const opt = (id: string, label: string, kind: string, payload: Record<string, unknown>) => ({ id, label, kind, payload });

/** Space combat, hits owed by the viewer: 8 fighters, 2 destroyers, 2 dreadnoughts, 2 carriers. */
export function spaceBoard(): BoardView {
  return {
    ...hitAssignmentBoard,
    combat: {
      system_id: "18",
      round: 2,
      phase: "resolving_hits",
      attacker: RIVAL,
      defender: actor,
      round_start: hitAssignmentBoard.systems["18"].units,
      attacker_hits: 5,
      defender_hits: 0,
      hits_to_assign: 5,
      dice_rolls: [],
    },
  } as never;
}

export const spaceSustainChoice: MockChoice = {
  prompt: "cancel a hit at 18",
  context: {
    subtype: "sustain_damage",
    target: { System: "18" },
    space_battle: true,
    outstanding: [{ kind: "UnitsToRemove", amount: 5, paid: 0 }],
  },
  options: [
    opt("sustain|10", "sustain damage on dreadnought", "sustain", { unit: "dreadnought" }),
    { id: "decline", label: "take the hit", kind: "decline" },
  ] as never,
};

export const spaceCasualtyChoice: MockChoice = {
  prompt: "assign a hit",
  context: {
    subtype: "assign_casualty",
    target: { System: "18" },
    space_battle: true,
    outstanding: [{ kind: "UnitsToRemove", amount: 5, paid: 0 }],
    hit: { cause: "space_cannon", destroy: false, restriction: "any", producer: RIVAL },
  },
  options: [
    opt("destroy|0", "destroy fighter", "casualty", { unit: "fighter", damaged: false }),
    opt("destroy|8", "destroy destroyer", "casualty", { unit: "destroyer", damaged: false }),
    opt("destroy|10", "destroy dreadnought", "casualty", { unit: "dreadnought", damaged: false }),
    opt("destroy|12", "destroy carrier", "casualty", { unit: "carrier", damaged: false }),
  ] as never,
};

export async function openSpace(page: Page, v: Viewport, choice: MockChoice) {
  await page.setViewportSize({ width: v.width, height: v.height });
  await openMockedGame(page, {
    players: [playerWithHand(), { ...opponent, id: RIVAL }],
    board: spaceBoard() as never,
    choice,
  });
  await page.waitForTimeout(800);
}

const ground = (type: string, owner: string, damaged = false): PlacedUnitView => ({ unit_type: type, owner, damaged, planet: "jord" });
const many = (n: number, type: string, owner: string) => Array.from({ length: n }, () => ground(type, owner));

/** Ground combat on Jord: the viewer's 5 infantry, a mech and a damaged mech against 3 infantry. */
export const groundUnits = [...many(5, "infantry", actor), ground("mech", actor), ground("mech", actor, true), ...many(3, "infantry", RIVAL)];

const ships: PlacedUnitView[] = [
  { unit_type: "dreadnought", owner: actor, damaged: false },
  { unit_type: "carrier", owner: actor, damaged: false },
];

export function groundBoard(units: PlacedUnitView[], step: unknown = null): BoardView {
  const base = galleryBoard.systems["18"];
  return {
    ...galleryBoard,
    combat: null,
    systems: {
      ...galleryBoard.systems,
      "18": {
        ...base,
        planets: { jord: { planet_id: "jord", controlled_by: RIVAL, exhausted: false } },
        units: [...ships, ...units],
      },
    },
    invasion: {
      system_id: "18",
      invasion_seq: 7,
      invader: actor,
      phase: "ground_combat",
      planets: ["jord"],
      current_planet: "jord",
      defender: RIVAL,
      ground_round: 1,
      last_step: step,
      odds_context: {},
    },
  } as never;
}

export const groundHitChoice: MockChoice = {
  prompt: "assign a hit",
  context: {
    subtype: "assign_ground_casualty",
    target: { Planet: { system: "18", planet: "jord" } },
    outstanding: [{ kind: "UnitsToRemove", amount: 4, paid: 0 }],
  },
  options: [
    opt("destroy|0", "destroy infantry", "ground_casualty", { unit: "infantry", damaged: false }),
    opt("destroy|5", "destroy mech", "ground_casualty", { unit: "mech", damaged: false }),
    opt("destroy|6", "destroy damaged mech", "ground_casualty", { unit: "mech", damaged: true }),
  ] as never,
};

export async function openGround(page: Page, v: Viewport) {
  await page.setViewportSize({ width: v.width, height: v.height });
  await openMockedGame(page, {
    players: [playerWithHand(), { ...opponent, id: RIVAL }],
    board: groundBoard(groundUnits) as never,
    choice: groundHitChoice,
  });
  await page.waitForTimeout(800);
}

const die = (face: number, target: number) => ({ planet: "jord", player: actor, group: "dreadnought", face, target, hit: face >= target });
const defenders = [...many(4, "infantry", RIVAL), ground("mech", RIVAL), ground("pds", RIVAL)];
const afterBomb = [ground("pds", RIVAL), ground("infantry", RIVAL)];

/** Two of three dice hit; with X-89 Bacterial Weapon every hit counts twice, so the engine applies 4 hits. */
export const bacterialStep = {
  planet: "jord",
  kind: "bombardment",
  round: 0,
  before: defenders,
  after: afterBomb,
  dice: [die(9, 5), die(7, 5), die(2, 5)],
  hits: { [actor]: 4 },
  harrow_hits: 0,
};

export async function openBombardment(page: Page, v: Viewport) {
  await page.setViewportSize({ width: v.width, height: v.height });
  const board = groundBoard([...afterBomb], bacterialStep) as never as BoardView;
  (board.invasion as { phase: string }).phase = "commit_ground_forces";
  (board.invasion as { ground_round: number }).ground_round = 0;
  await openMockedGame(page, { players: [playerWithHand(), { ...opponent, id: RIVAL }], board: board as never, choice: null });
  await page.waitForTimeout(800);
}

export async function take(page: Page, testInfo: TestInfo, id: string, v: Viewport) {
  await shot(page, testInfo, `${prefix}-${id}-${v.id}`);
}

/** Layout facts every phone shot is checked against (after the fix). */
export async function layoutFacts(page: Page, root = "[data-testid=hit-assignment-panel]") {
  return page.evaluate((sel) => {
    const vw = document.documentElement.clientWidth;
    const panel = document.querySelector(sel);
    const buttons = [...(panel?.querySelectorAll("button") ?? [])].map((b) => {
      const r = b.getBoundingClientRect();
      return { text: (b.textContent ?? "").trim(), left: r.left, right: r.right, w: r.width, h: r.height };
    });
    return {
      vw,
      scrollW: document.documentElement.scrollWidth,
      buttons,
      clipped: buttons.filter((b) => b.right > vw + 0.5 || b.left < -0.5).map((b) => b.text),
      small: buttons.filter((b) => b.w < 43.5 || b.h < 43.5).map((b) => b.text),
    };
  }, root);
}
