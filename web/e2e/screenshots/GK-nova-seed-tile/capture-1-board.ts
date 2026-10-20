import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { expect, test } from "@playwright/test";
import type { BoardView } from "../../../src/protocol/types";
import { galleryPlayers } from "../_shared/fixtures";
import { openMockedGame } from "../_shared/mockGame";
import { shot } from "../_shared/shot";

// The board view of a real engine state (data/before.json, data/after.json, written by
// `cargo run -p ti4-host --example nova_seed_board -- <dir>`): the Muaat hero's Nova Seed replaces a
// system tile with the Muaat supernova. The page is the real app; only the transport is mocked.
const SIZES = { desktop: { width: 1440, height: 900 }, "390x844": { width: 390, height: 844 } };
const dataDir = join(dirname(new URL(import.meta.url).pathname), "data");
const load = (name: string) => JSON.parse(readFileSync(join(dataDir, `${name}.json`), "utf8")) as BoardView;
const players = [{ ...galleryPlayers[0], faction: "muaat" }, galleryPlayers[1]];

for (const [size, viewport] of Object.entries(SIZES)) {
  for (const step of ["before", "after"] as const) {
    test(`nova seed ${step} ${size}`, async ({ page }, testInfo) => {
      await page.setViewportSize(viewport);
      const board = load(step);
      await openMockedGame(page, { board, players });
      await page.waitForTimeout(600);
      const nova = board.map_tiles.some((tile) => tile.system_id === "81");
      expect(nova).toBe(step === "after");
      if (step === "after") {
        expect(Object.keys(board.systems)).toContain("81");
        expect(board.map_tiles.filter((tile) => tile.special_area == null).length).toBe(
          load("before").map_tiles.filter((tile) => tile.special_area == null).length,
        );
      }
      await shot(page, testInfo, `${step}-${size}`);
    });
  }
}
