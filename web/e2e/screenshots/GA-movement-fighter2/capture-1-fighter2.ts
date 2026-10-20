import { expect, test } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { shot } from "../_shared/shot";
import { galleryPlayers } from "../_shared/fixtures";
import { actor } from "../_shared/players";
import type { BoardView } from "../../../src/protocol/types";

// Fighter II next to the active system 18. The engine offers it a move of its own (payload capacity
// 0, move 2) and reports the destination's fleet inputs in details.fleet_supply (tactical.rs
// preview_moves). Before the fix the overlay filed it under cargo: it needed a carrier, was never
// staged as a ship, and the fleet supply gauge ignored it.
const prefix = process.env.TI4_SHOT_PREFIX ?? "after";
const SIZES = { desktop: { width: 1440, height: 900 }, "390x844": { width: 390, height: 844 } } as const;

const tile = (system_id: string, label: string, q: number, r: number) => ({ system_id, label, q, r });
const unit = (unit_type: string, n: number) =>
  Array.from({ length: n }, () => ({ owner: actor, unit_type, damaged: false }));
const boardOf = (origin: ReturnType<typeof unit>, destination: ReturnType<typeof unit> = []): BoardView => ({
  active_system: "18",
  map_tiles: [tile("20", "#20", 0, 0), tile("18", "Mecatol Rex", 1, 0)],
  systems: {
    "20": { system_id: "20", command_tokens: [], planets: {}, units: origin },
    "18": { system_id: "18", command_tokens: [], planets: {}, units: destination },
  },
});
const supply = (over: Record<string, unknown> = {}) => ({
  used: 0, limit: 3, ships: 0, capacity: 0, carried: 0, fighters: 0,
  fighter_support: 0, fighters_upgraded: true, fighters_half: false, ...over,
});
const moves = (units: string[], details: Record<string, unknown>) => ({
  prompt: "Move ships into Mecatol Rex",
  context: { subtype: "movement_step", target: { System: "18" } },
  options: [
    ...units.map((u, i) => ({
      id: `move|20|${i}`,
      label: `move ${u} from 20`,
      kind: "move",
      payload: { origin: "20", unit: u, capacity: u === "carrier" ? 4 : 0 },
    })),
    { id: "done_moving", label: "Finish Movement", kind: "decline" },
  ],
  details: { fleet_supply: details },
});

interface Scenario {
  name: string;
  board: BoardView;
  choice: ReturnType<typeof moves>;
  act: (page: import("@playwright/test").Page) => Promise<void>;
  check: (page: import("@playwright/test").Page) => Promise<void>;
}

const scenarios: Scenario[] = [
  {
    // Two Fighter II, no carrier: both move alone and fit the pool of 3.
    name: "alone-within",
    board: boardOf(unit("fighter2", 2)),
    choice: moves(["fighter2"], supply()),
    act: async (page) => {
      await page.getByTestId("rally-inc-20-fighter2").click();
      await page.getByTestId("rally-inc-20-fighter2").click();
    },
    check: async (page) => {
      await expect(page.getByTestId("rally-count-20-fighter2")).toHaveText("2");
      await expect(page.getByTestId("fleet-supply-gauge")).toContainText("2 / 3 Ships");
      await expect(page.getByTestId("fleet-supply-gauge")).not.toHaveAttribute("data-warning", "true");
    },
  },
  {
    // Two cruisers already stand in the target; three Fighter II would make five ships for three.
    name: "alone-over",
    board: boardOf(unit("fighter2", 3), unit("cruiser", 2)),
    choice: moves(["fighter2"], supply({ used: 2, ships: 2 })),
    act: async (page) => {
      await page.getByTestId("rally-all-20").click();
    },
    check: async (page) => {
      await expect(page.getByTestId("fleet-supply-gauge")).toContainText("5 / 3 Ships");
      await expect(page.getByTestId("fleet-supply-gauge")).toHaveAttribute("data-warning", "true");
      await expect(page.getByTestId("movement-uncarried-fighters")).toBeVisible();
    },
  },
  {
    // A carrier takes four of six; the other two go on their own and are the only fighters charged.
    name: "carried-and-alone",
    board: boardOf([...unit("carrier", 1), ...unit("fighter2", 6)]),
    choice: moves(["carrier", "fighter2"], supply()),
    act: async (page) => {
      await page.getByTestId("rally-all-20").click();
    },
    check: async (page) => {
      await expect(page.getByTestId("rally-count-cargo-20-fighter2-space")).toHaveText("4");
      await expect(page.getByTestId("rally-count-20-fighter2")).toHaveText("2");
      await expect(page.getByTestId("fleet-supply-gauge")).toContainText("3 / 3 Ships");
    },
  },
  {
    // Everything rides: the carrier is the only ship the pool sees.
    name: "all-carried",
    board: boardOf([...unit("carrier", 1), ...unit("fighter2", 3)]),
    choice: moves(["carrier", "fighter2"], supply()),
    act: async (page) => {
      await page.getByTestId("rally-all-20").click();
    },
    check: async (page) => {
      await expect(page.getByTestId("rally-count-cargo-20-fighter2-space")).toHaveText("3");
      await expect(page.getByTestId("rally-count-20-fighter2")).toHaveText("0");
      await expect(page.getByTestId("fleet-supply-gauge")).toContainText("1 / 3 Ships");
      await expect(page.getByTestId("movement-uncarried-fighters")).toHaveCount(0);
    },
  },
];

for (const scenario of scenarios) {
  for (const [name, size] of Object.entries(SIZES)) {
    test(`fighter2 ${scenario.name} ${name}`, async ({ page }, testInfo) => {
      await page.setViewportSize(size);
      await openMockedGame(page, { board: scenario.board, players: galleryPlayers, choice: scenario.choice });
      await page.getByTestId("tactical-movement-tray").waitFor();
      if (prefix === "before") page.setDefaultTimeout(1500);
      await scenario.act(page).catch((error) => {
        // Before the fix the + is disabled (no carrier) and "All from here" does not exist for it.
        if (prefix !== "before") throw error;
      });
      if (prefix !== "before") await scenario.check(page);
      await page.getByTestId("origin-group-20").scrollIntoViewIfNeeded();
      await shot(page, testInfo, `${prefix}-${scenario.name}-${name}`, {
        of: page.getByTestId("tactical-movement-tray"),
        pad: 8,
      });
    });
  }
}
