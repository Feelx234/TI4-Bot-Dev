import type { Page } from "@playwright/test";
import { openMockedGame, type MockChoice } from "../_shared/mockGame";
import { actor, galleryBoard } from "../_shared/fixtures";
import { me, opponent, withPlayer } from "../_shared/players";
import type { BoardView, PlayerView } from "../../../src/protocol/types";

/** Which side of the change a run is: `before` is taken on the commit without the change. */
export const prefix = process.env.TI4_SHOT_PREFIX ?? "after";

export const SIZES = {
  desktop: { width: 1440, height: 900 },
  "390x844": { width: 390, height: 844 },
} as const;
export type SizeName = keyof typeof SIZES;

/**
 * The gallery board with attachments on the viewer's planets: Jord (no specialty) has a Warfare
 * Research Facility, Tarmann (exhausted) a Cybernetic one, Wellon (printed yellow specialty) the
 * +1/+1 side of a Warfare one, Centauri the Demilitarized Zone.
 */
export function attachedBoard(): BoardView {
  const systems = structuredClone(galleryBoard.systems);
  const patch: Record<string, Record<string, unknown>> = {
    jord: { attachments: ["warfare"] },
    tarmann: { attachments: ["cybernetic"], exhausted: true },
    wellon: { attachments: ["warfarestat"] },
    centauri: { attachments: ["dmz"], controlled_by: actor },
  };
  for (const system of Object.values(systems)) {
    for (const [id, planet] of Object.entries(system.planets)) {
      if (patch[id]) system.planets[id] = { ...planet, controlled_by: actor, ...patch[id] } as typeof planet;
    }
  }
  return { ...galleryBoard, systems };
}

export const sol: PlayerView = withPlayer(me, { faction: "sol", technologies: ["st"], trade_goods: 8 });

export async function openResearch(page: Page, size: SizeName, options: { owned?: string[]; engine: string[] }) {
  await page.setViewportSize(SIZES[size]);
  await openMockedGame(page, {
    board: attachedBoard(),
    state: { rules_version: 15 },
    players: [withPlayer(sol, { technologies: options.owned ?? ["st"] }), opponent],
    choice: {
      prompt: "Research a technology",
      context: { subtype: "research_technology", source: { StrategyCard: { card: "Technology", secondary: false } } },
      options: options.engine.map((id) => ({ id, label: id, kind: "research", payload: {} })),
    } as MockChoice,
  });
  await page.getByTestId("technology-modal").waitFor();
  await page.waitForTimeout(700);
}

export async function openBoard(page: Page, size: SizeName) {
  await page.setViewportSize(SIZES[size]);
  await openMockedGame(page, {
    phase: "action",
    board: attachedBoard(),
    players: [sol, opponent],
    choice: { prompt: "action phase", player: opponent.id, options: [{ id: "pass", label: "pass", kind: "action" }] } as MockChoice,
  });
  await page.getByTestId("ti4-board-svg").waitFor();
  await page.waitForTimeout(400);
}

export async function openHeldItems(page: Page, size: SizeName) {
  if (size !== "desktop") {
    await page.getByTestId("player-sheet-toggle").click();
    await page.waitForTimeout(400);
  }
  await page.getByTestId("held-items-button-0").scrollIntoViewIfNeeded();
  await page.getByTestId("held-items-button-0").click();
  await page.getByTestId("held-items-panel").waitFor();
  await page.waitForTimeout(250);
}
