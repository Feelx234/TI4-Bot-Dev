import type { Page, TestInfo } from "@playwright/test";
import { openMockedGame, type MockChoice } from "../_shared/mockGame";
import { shot } from "../_shared/shot";
import { actor, galleryBoard, hitAssignmentBoard } from "../_shared/fixtures";
import { opponent, playerWithHand } from "../_shared/players";

// TI4_SHOT_PREFIX=before is how the "before" shots were taken (on the base commit 80ce1bf1); the
// default "after" regenerates the current layout. The choices are the engine's own: prompts,
// subtypes, option ids/kinds/labels and payloads are copied from combat.rs (Stage::Retreating,
// Stage::AnnouncingRetreat), naalu.rs (foresight), action_cards.rs (skilled retreat) and
// borrowed_commanders.rs (Ral Nel commander).
export const prefix = process.env.TI4_SHOT_PREFIX ?? "after";

export const viewports = [
  { id: "desktop", width: 1440, height: 900 },
  { id: "phone", width: 390, height: 844 },
];

const ship = (unit_type: string, owner = actor) => ({ unit_type, owner, damaged: false, planet: null, galvanized: false });

/** The viewer's two cruisers (a small fleet) in system 18, next to its ring of neighbours. */
export const smallFleet = [ship("cruiser"), ship("cruiser")];

/** The viewer's mixed fleet in system 18 (fighters, destroyers, dreadnoughts, carriers). */
export const bigFleet = hitAssignmentBoard.systems["18"].units.filter((u) => u.owner === actor);

const rival = [ship("cruiser", opponent.id), ship("destroyer", opponent.id)];

export function boardWith(units: ReturnType<typeof ship>[], combatPhase?: string) {
  const board = {
    ...galleryBoard,
    systems: { ...galleryBoard.systems, "18": { ...galleryBoard.systems["18"], units: [...units, ...(combatPhase ? rival : [])] } },
  } as never as typeof galleryBoard;
  if (!combatPhase) return board;
  return {
    ...board,
    combat: {
      system_id: "18",
      round: 1,
      phase: combatPhase,
      attacker: opponent.id,
      defender: actor,
      round_start: board.systems["18"].units,
      attacker_hits: 0,
      defender_hits: 0,
      dice_rolls: [],
    },
  } as never as typeof galleryBoard;
}

export const retreatToChoice = (): MockChoice => ({
  prompt: "retreat to which system",
  context: { subtype: "retreat_to", target: { System: "18" }, space_battle: true },
  options: ["26", "19", "22", "25"].map((id) => ({
    id,
    kind: "retreat_to",
    label: `retreat to ${id}`,
    payload: { system: id },
  })) as never,
});

export const announceChoice = (): MockChoice => ({
  prompt: "announce a retreat from 18",
  context: { subtype: "announce_retreat", target: { System: "18" }, space_battle: true },
  options: [
    { id: "stay", kind: "retreat", label: "stay and fight", payload: { system: "18" } },
    { id: "retreat", kind: "retreat", label: "announce a retreat", payload: { system: "18" } },
  ] as never,
});

export const foresightChoice = (): MockChoice => ({
  prompt: "Foresight: where do your ships go",
  context: { subtype: "foresight_destination", target: { System: "18" } },
  options: ["26", "19", "22"].map((id) => ({
    id: `system|${id}`,
    kind: "system",
    label: `move your ships from 18 to ${id}`,
  })) as never,
});

export const skilledRetreatChoice = (): MockChoice => ({
  prompt: "Skilled Retreat: withdraw to which system",
  context: { subtype: "skilled_retreat_choose_system", target: { System: "18" } },
  options: ["26", "19", "22"].map((id) => ({ id, kind: "skilled_retreat", label: `withdraw to ${id}` })) as never,
});

export const ralnelChoice = (): MockChoice => ({
  prompt: "Ral Nel commander: where do up to 2 ships retreat now?",
  context: { subtype: "ralnel_retreat_destination", target: { System: "18" } },
  options: ["26", "19", "22"].map((id) => ({ id: `to|${id}`, kind: "retreat", label: `retreat up to 2 ships to ${id}` })) as never,
});

export async function open(
  page: Page,
  viewport: (typeof viewports)[number],
  board: ReturnType<typeof boardWith>,
  choice: MockChoice,
  onClientMessage?: (message: { type: string; [key: string]: unknown }) => void,
) {
  await page.setViewportSize({ width: viewport.width, height: viewport.height });
  await openMockedGame(page, { players: [playerWithHand(), opponent], board, choice, onClientMessage: onClientMessage as never });
  await page.waitForTimeout(900);
}

export async function take(page: Page, testInfo: TestInfo, id: string, viewport: (typeof viewports)[number]) {
  await shot(page, testInfo, `${prefix}-${id}-${viewport.id}`);
}

export const scenes = {
  foresight: () => [boardWith(bigFleet), foresightChoice()] as const,
  "retreat-small": () => [boardWith(smallFleet, "retreating"), retreatToChoice()] as const,
  "retreat-multi": () => [boardWith(bigFleet, "retreating"), retreatToChoice()] as const,
  announce: () => [boardWith(smallFleet, "announce_retreat"), announceChoice()] as const,
  skilled: () => [boardWith(bigFleet, "retreating"), skilledRetreatChoice()] as const,
  ralnel: () => [boardWith(bigFleet), ralnelChoice()] as const,
};
