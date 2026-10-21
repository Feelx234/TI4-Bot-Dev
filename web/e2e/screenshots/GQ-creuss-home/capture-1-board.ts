import { expect, test } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { shot } from "../_shared/shot";
import { galleryPlayers } from "../_shared/fixtures";
import { actor } from "../_shared/players";
import type { BoardView } from "../../../src/protocol/types";

// What ti4-host::map::with_recorded_edits sends for a Creuss table: the ring tiles, the Creuss Gate
// (17) on the map, and the Creuss home system (51) as an "off-map" tile on the free hex beside the
// Gate, facing away from the centre. A Muaat supernova (Nova Seed) replaces a ring system.
const prefix = process.env.TI4_SHOT_PREFIX ?? "after";
const SIZES = { desktop: { width: 1440, height: 900 }, "390x844": { width: 390, height: 844 } } as const;

const t = (system_id: string, label: string, q: number, r: number, extra: Record<string, unknown> = {}) => ({
  system_id, label, q, r, ...extra,
});
const ring = [
  t("18", "Mecatol Rex", 0, 0), t("20", "#20", 1, 0), t("26", "#26", 0, 1), t("25", "#25", -1, 1),
  t("27", "#27", -1, 0), t("24", "#24", 0, -1),
  t("17", "Creuss Gate - Creuss", -2, 2, { wormholes: ["DELTA"] }),
];
const nova = t("novaseed", "Nova Seed", 1, -1, { anomalies: ["supernova"] });
const home = t("51", "Creuss - Creuss", -3, 3, {
  special_area: "off-map", wormholes: ["DELTA"],
  planets: [{ id: "creuss", label: "Creuss", resources: 4, influence: 2, traits: [], tech_specialties: [], legendary: false }],
});

const sys = (id: string, units: { owner: string; unit_type: string; damaged: boolean; planet?: string }[] = [], planets = {}) =>
  ({ system_id: id, command_tokens: [], planets, units });

function boardFor(withHome: boolean): BoardView {
  const tiles = [...ring, nova, ...(withHome ? [home] : [])];
  const systems: BoardView["systems"] = {};
  for (const tile of tiles) systems[tile.system_id] = sys(tile.system_id);
  systems["51"] = sys("51", [
    { owner: actor, unit_type: "carrier", damaged: false, galvanized: false, planet: null },
    { owner: actor, unit_type: "infantry", damaged: false, planet: "creuss" },
  ], { creuss: { planet_id: "creuss", controlled_by: actor, exhausted: false } });
  return { active_system: null, map_tiles: tiles, systems } as unknown as BoardView;
}

const activate = (ids: string[]) => ({
  prompt: "Choose a system to activate",
  context: { subtype: "activate_system" },
  options: ids.map((id) => ({ id, label: `Activate ${id}`, kind: "activate", payload: { system: id } })),
});

for (const [name, size] of Object.entries(SIZES)) {
  test(`creuss home ${name}`, async ({ page }, testInfo) => {
    await page.setViewportSize(size);
    page.on("pageerror", (e) => console.log("PAGEERROR", e.message, (e.stack ?? "").split("\n").slice(0, 4).join("|")));
    const sent: unknown[] = [];
    const withHome = prefix !== "before";
    await openMockedGame(page, {
      board: boardFor(withHome),
      players: galleryPlayers,
      choice: activate(["51", "17", "20"]),
    }).then(({ socket }) => {
      socket.onMessage((data) => {
        const message = JSON.parse(String(data)) as { type: string; option_id?: string };
        if (message.type === "submit_choice") sent.push(message);
      });
    });
    await page.getByTestId("ti4-board-svg").waitFor();
    const hex = page.getByTestId("system-hex-51");
    if (!withHome) {
      await expect(hex).toHaveCount(0);
      await shot(page, testInfo, `${prefix}-${name}`);
      return;
    }
    await expect(hex).toHaveCount(1);
    await expect(page.getByTestId("system-hex-novaseed")).toHaveCount(1);
    await shot(page, testInfo, `${prefix}-${name}`);
    const box = (await hex.boundingBox())!;
    const [cx, cy] = [box.x + box.width / 2, box.y + box.height / 2];
    const inView = await page.evaluate(
      ([x, y]) => x >= 0 && y >= 0 && x <= window.innerWidth && y <= window.innerHeight,
      [cx, cy] as const,
    );
    expect(inView).toBe(true);
    const hit = await page.evaluate(
      ([x, y]) => document.elementFromPoint(x, y)?.closest("[data-testid^='system-hex-']")?.getAttribute("data-testid"),
      [cx, cy] as const,
    );
    expect(hit).toBe("system-hex-51");
    await page.mouse.click(cx, cy);
    await expect(page.getByTestId("system-inspector")).toBeVisible();
    await shot(page, testInfo, `${prefix}-${name}-picked`);
    await page.getByTestId("confirm-activation-btn").click();
    await expect.poll(() => sent.length).toBe(1);
    expect(sent[0]).toMatchObject({ option_id: "51" });
  });
}
