import type { Page, TestInfo } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { shot } from "../_shared/shot";
import { galleryPlayers } from "../_shared/fixtures";
import { actor } from "../_shared/players";
import type { BoardView, MovementFacts, RouteFact } from "../../../src/protocol/types";

// Rules version 29 (RULES_MOVEMENT_ROUTES): a ship with more than one way to the active system is
// offered a limited list of routes, the default (the engine's old route) first. Synthetic fixtures:
// the facts have the shape of `MovementFacts.ships[].move.routes` from the host.

export const viewports = [
  { id: "desktop", width: 1440, height: 900 },
  { id: "phone", width: 390, height: 844 },
];

const tile = (system_id: string, label: string, q: number, r: number, anomalies: string[] = []) => ({
  system_id,
  label,
  q,
  r,
  anomalies,
});
const sys = (id: string, units: BoardView["systems"][string]["units"]) => ({
  system_id: id,
  command_tokens: [],
  planets: {},
  units,
});
const cruiser = { owner: actor, unit_type: "cruiser", damaged: false, planet: null, galvanized: false };

/**
 * origin 24 (0,0) - gravity rift 41 (1,0) - active 18 (2,0), and the way round it through
 * 25 (0,1) and 26 (1,1); 27 (1,-1) and 28 (2,-1) are a third way along the top.
 */
export const board = (): BoardView => ({
  active_system: "18",
  map_tiles: [
    tile("24", "#24", 0, 0),
    tile("41", "Gravity rift", 1, 0, ["gravity rift"]),
    tile("18", "Mecatol Rex", 2, 0),
    tile("25", "#25", 0, 1),
    tile("26", "#26", 1, 1),
    tile("27", "#27", 1, -1),
    tile("28", "#28", 2, -1),
  ],
  systems: {
    "24": sys("24", [cruiser]),
    "41": sys("41", []),
    "18": sys("18", []),
    "25": sys("25", []),
    "26": sys("26", []),
    "27": sys("27", []),
    "28": sys("28", []),
  },
});

const route = (path: string[], extra: Partial<RouteFact> = {}): RouteFact => ({
  path,
  default: false,
  gravity_drive: false,
  ionian: false,
  rifts: [],
  wormholes: 0,
  anomalies: [],
  ...extra,
});

export const throughRift = route(["24", "41", "18"], {
  default: true,
  rifts: ["41"],
  anomalies: ["41"],
});
export const aroundWithDrive = route(["24", "25", "26", "18"], { gravity_drive: true });
export const alongTheTop = route(["24", "27", "28", "18"]);
export const viaWormhole = route(["24", "25", "28", "18"], { wormholes: 1 });
export const another = route(["24", "27", "26", "18"]);

export const facts = (routes: RouteFact[]): MovementFacts => ({
  active: "18",
  ships: [
    {
      origin: "24",
      index: 0,
      unit: "cruiser",
      damaged: false,
      capacity: 0,
      fleet: true,
      move_value: 2,
      move: {
        gravity_drive: false,
        ionian: false,
        path: routes[0].path,
        rifts: routes[0].rifts,
        ...(routes.length > 1 ? { routes } : {}),
      },
    },
  ],
  cargo: [],
  gravity_drive: true,
  ionian: false,
  fleet_supply: { limit: 4, charged: 0 },
  moved: 0,
});

const choice = {
  prompt: "Move ships into Mecatol Rex",
  context: { subtype: "movement_step", target: { System: "18" } },
  options: [
    {
      id: "move|24|0",
      label: "Cruiser",
      kind: "move",
      payload: { origin: "24", unit: "cruiser", capacity: 0 },
    },
    { id: "done_moving", label: "Finish Movement", kind: "decline" },
  ],
};

export async function open(
  page: Page,
  routes: RouteFact[],
  v: { width: number; height: number },
) {
  await page.setViewportSize({ width: v.width, height: v.height });
  await openMockedGame(page, {
    board: board(),
    players: galleryPlayers,
    choice,
    facts: { tactical: { kind: "movement", ...facts(routes) } },
  });
  await page.getByTestId("tactical-movement-tray").waitFor();
  await page.getByTestId("rally-inc-24-cruiser").click();
}

export async function take(page: Page, testInfo: TestInfo, name: string, v: { id: string }) {
  await shot(page, testInfo, `${v.id}-${name}`);
}
