import { expect, test } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { shot } from "../_shared/shot";
import { galleryPlayers } from "../_shared/fixtures";
import { actor } from "../_shared/players";
import type { BoardView } from "../../../src/protocol/types";

// What ti4-host::map::build_board_tiles sends: the ring tiles plus BOTH Nexus faces (82a, 82b) with
// special_area "nexus" at (0,0). Which face is in play is board.nexus_unlocked.
const prefix = process.env.TI4_SHOT_PREFIX ?? "after";
const SIZES = { desktop: { width: 1440, height: 900 }, "390x844": { width: 390, height: 844 } } as const;

const t = (system_id: string, label: string, q: number, r: number, extra: Record<string, unknown> = {}) => ({
  system_id, label, q, r, ...extra,
});
const nexusTiles = [
  t("82a", "Mallice - Nexus", 0, 0, {
    special_area: "nexus", wormholes: ["GAMMA"],
    planets: [{ id: "lockedmallice", label: "Mallice", resources: 0, influence: 3, traits: [], tech_specialties: [], legendary: false }],
  }),
  t("82b", "Mallice - Nexus", 0, 0, {
    special_area: "nexus", wormholes: ["ALPHA", "BETA", "GAMMA"],
    planets: [{ id: "mallice", label: "Mallice", resources: 0, influence: 3, traits: [], tech_specialties: [], legendary: false }],
  }),
];
const ring = [
  t("18", "Mecatol Rex", 0, 0), t("20", "#20", 1, 0), t("26", "#26", 0, 1), t("25", "#25", -1, 1),
  t("27", "#27", -1, 0), t("24", "#24", 0, -1), t("28", "#28", 1, -1),
];
const sys = (id: string, units: { owner: string; unit_type: string; damaged: boolean; planet?: string }[] = [], planets = {}) =>
  ({ system_id: id, command_tokens: [], planets, units });

function boardFor(nexusUnits: boolean): BoardView {
  // The engine only ever knows the Nexus as 82a (units, activation, control), open or not.
  const systems: BoardView["systems"] = {};
  for (const tile of ring) systems[tile.system_id] = sys(tile.system_id);
  systems["26"] = sys("26", [{ owner: actor, unit_type: "cruiser", damaged: false }]);
  systems["82a"] = sys("82a", nexusUnits ? [{ owner: actor, unit_type: "destroyer", damaged: false }] : []);
  return { active_system: null, map_tiles: [...ring, ...nexusTiles], systems } as unknown as BoardView;
}

const activate = (ids: string[]) => ({
  prompt: "Choose a system to activate",
  context: { subtype: "activate_system" },
  options: ids.map((id) => ({ id, label: `Activate ${id}`, kind: "activate", payload: { system: id } })),
});

const VARIANTS = [
  // name, nexus_unlocked, ships in the Nexus, gamma token system
  ["locked-gamma-token", false, false, "26"],
  ["open-with-ships", true, true, "26"],
] as const;

for (const [name, size] of Object.entries(SIZES)) {
  for (const [variant, unlocked, units, token] of VARIANTS) {
    test(`nexus ${variant} ${name}`, async ({ page }, testInfo) => {
      await page.setViewportSize(size);
      const sent: unknown[] = [];
      await openMockedGame(page, {
        board: boardFor(units),
        players: galleryPlayers,
        choice: activate(["20", "26", "82a"]),
        state: { nexus_unlocked: unlocked, wormhole_tokens: { GAMMA: token } },
      }).then(({ socket }) => {
        socket.onMessage((data) => {
          const message = JSON.parse(String(data)) as { type: string; option_id?: string };
          if (message.type === "submit_choice") sent.push(message);
        });
      });
      await page.getByTestId("ti4-board-svg").waitFor();
      const hex = page.getByTestId("system-hex-82a");
      await expect(hex).toHaveCount(1);
      if (prefix !== "before") {
        await expect(page.getByTestId("system-hex-82b")).toHaveCount(0);
        await expect(page.getByTestId("gamma-link-26")).toHaveCount(1);
        // Open face: alpha, beta and gamma; locked face: gamma only.
        await expect(page.getByTestId("system-hex-82a").locator("text", { hasText: /^[αβγ]$/ })).toHaveCount(unlocked ? 3 : 1);
      }
      await shot(page, testInfo, `${prefix}-${variant}-${name}`);
      const box = (await hex.boundingBox())!;
      const [cx, cy] = [box.x + box.width / 2, box.y + box.height / 2];
      const hit = await page.evaluate(([x, y]) => document.elementFromPoint(x, y)?.closest("[data-testid^='system-hex-']")?.getAttribute("data-testid"), [cx, cy] as const);
      if (prefix !== "before") {
        expect(hit).toBe("system-hex-82a");
        // A click (mouse) or tap (touch) on the Nexus answers the activation with 82a.
        await page.mouse.click(cx, cy);
        await expect(page.getByTestId("system-inspector")).toBeVisible();
        await shot(page, testInfo, `${prefix}-${variant}-${name}-picked`);
        await page.getByTestId("confirm-activation-btn").click();
        await expect.poll(() => sent.length).toBe(1);
        expect(sent[0]).toMatchObject({ option_id: "82a" });
      } else console.log(`hit at Nexus centre: ${hit}`);
    });
  }
}
