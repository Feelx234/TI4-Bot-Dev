import { expect, test } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { shot } from "../_shared/shot";
import { galleryPlayers } from "../_shared/fixtures";
import { actor } from "../_shared/players";
import type { BoardView } from "../../../src/protocol/types";

// The host sends the seven Fracture tiles (Thunder's Edge) in every map_tiles list. Before the
// change the board drew them in a row under the galaxy for every table: four unused planets
// (Cocytus, Styx, Lethe, Phlegethon) with Thunder's Edge off. Now they are drawn only once the
// Fracture is in play (state.fracture_in_play).
const prefix = process.env.TI4_SHOT_PREFIX ?? "after";
const SIZES = { desktop: { width: 1440, height: 900 }, "390x1300": { width: 390, height: 1300 } } as const;

const t = (system_id: string, label: string, q: number, r: number, extra: Record<string, unknown> = {}) => ({
  system_id, label, q, r, ...extra,
});
const planet = (id: string, label: string, resources: number, influence: number, legendary = false) =>
  ({ id, label, resources, influence, traits: [], tech_specialties: [], legendary });
const ring = [
  t("18", "Mecatol Rex", 0, 0, { planets: [planet("mecatol", "Mecatol Rex", 1, 6)] }),
  t("20", "#20", 1, 0), t("26", "#26", 0, 1), t("25", "#25", -1, 1),
  t("27", "#27", -1, 0), t("24", "#24", 0, -1), t("28", "#28", 1, -1),
];
const fracture = [
  t("fracture1", "Cocytus", 0, 0, { special_area: "fracture", planets: [planet("cocytus", "Cocytus", 3, 0)] }),
  t("fracture2", "Fracture Egress Left", 1, 0, { special_area: "fracture", egress: true }),
  t("fracture3", "Fracture Void Left", 2, 0, { special_area: "fracture" }),
  t("fracture4", "Styx", 3, 0, { special_area: "fracture", planets: [planet("styx", "Styx", 4, 0, true)] }),
  t("fracture5", "Fracture Void Right", 4, 0, { special_area: "fracture" }),
  t("fracture6", "Fracture Egress Right", 5, 0, { special_area: "fracture", egress: true }),
  t("fracture7", "Lethe/Phlegethon", 6, 0, {
    special_area: "fracture",
    planets: [planet("lethe", "Lethe", 0, 2), planet("phlegethon", "Phlegethon", 1, 2)],
  }),
];
const sys = (id: string, planets = {}) => ({ system_id: id, command_tokens: [], planets, units: [] });

function board(inPlay: boolean): BoardView {
  const systems: BoardView["systems"] = {};
  for (const tile of ring) systems[tile.system_id] = sys(tile.system_id);
  // The engine holds the Fracture systems only once the Fracture entered play.
  if (inPlay) for (const tile of fracture) systems[tile.system_id] = sys(tile.system_id);
  return { active_system: null, map_tiles: [...ring, ...fracture], systems } as unknown as BoardView;
}

const VARIANTS = [
  ["1-te-off", "Thunder's Edge off", {}, false, 0],
  ["2-te-on-fracture-not-yet", "Thunder's Edge on, Fracture not in play yet", { thunders_edge: true }, false, 0],
  ["3-te-on-fracture-in-play", "Thunder's Edge on, Fracture in play", { thunders_edge: true, fracture_in_play: true }, true, 7],
] as const;

for (const [name, size] of Object.entries(SIZES)) {
  for (const [variant, , state, inPlay, shown] of VARIANTS) {
    test(`fracture ${variant} ${name}`, async ({ page }, testInfo) => {
      await page.setViewportSize(size);
      await openMockedGame(page, { board: board(inPlay), players: galleryPlayers, state, choice: null });
      await page.getByTestId("ti4-board-svg").waitFor();
      await expect(page.getByTestId("system-hex-18")).toHaveCount(1);
      if (prefix !== "before") {
        await expect(page.locator("[data-testid^='system-hex-fracture']")).toHaveCount(shown);
      } else {
        console.log(`fracture tiles drawn: ${await page.locator("[data-testid^='system-hex-fracture']").count()}`);
      }
      void actor;
      await shot(page, testInfo, `${prefix}-${variant}-${name}`);
    });
  }
}
