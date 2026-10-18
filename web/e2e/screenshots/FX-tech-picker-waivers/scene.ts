import type { Page } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { playerWithHand, opponent, actor } from "../_shared/players";
import { galleryBoard } from "../_shared/fixtures";
import type { BoardView } from "../../../src/protocol/types";

/** Which side of the change a run is: `before` is taken on the commit without the change. */
export const prefix = process.env.TI4_SHOT_PREFIX ?? "after";

export const SIZES = {
  desktop: { width: 1440, height: 900 },
  "390x844": { width: 390, height: 844 },
} as const;
export type SizeName = keyof typeof SIZES;

export interface Variant {
  key: string;
  /** Technologies the viewer (Sol) owns, and which of them are exhausted. */
  owned: string[];
  exhausted?: string[];
  /** Planets with a technology specialty the viewer controls: wellon / lazar are yellow, thibah is blue. */
  planets?: Array<{ id: string; exhausted: boolean }>;
  /** What the engine lists as researchable for that table (the legal option ids the picker is given). */
  engine: string[];
}

const board = (v: Variant): BoardView => ({
  ...galleryBoard,
  systems: Object.fromEntries(
    (v.planets ?? []).map((p, i) => [
      String(900 + i),
      {
        system_id: String(900 + i),
        command_tokens: [],
        units: [],
        planets: { [p.id]: { planet_id: p.id, controlled_by: actor, exhausted: p.exhausted } },
      },
    ]),
  ) as unknown as BoardView["systems"],
});

/** The Technology primary's research decision at rules version 15, the way the engine offers it. */
export async function openResearch(page: Page, size: SizeName, v: Variant) {
  await page.setViewportSize(SIZES[size]);
  await openMockedGame(page, {
    board: board(v),
    state: { rules_version: 15 },
    players: [
      playerWithHand({
        faction: "sol",
        technologies: v.owned,
        exhausted_technologies: v.exhausted ?? [],
        trade_goods: 8,
      }),
      opponent,
    ],
    choice: {
      prompt: "Research a technology",
      context: { subtype: "research_technology", source: { StrategyCard: { card: "Technology", secondary: false } } },
      options: v.engine.map((id) => ({ id, label: id, kind: "research", payload: {} })),
    },
  });
  await page.getByTestId("technology-modal").waitFor();
  await page.waitForTimeout(700); // the dialog fades in
}

export const VARIANTS: Variant[] = [
  {
    key: "aida-unlocks",
    owned: ["aida", "amd", "st"],
    // Dreadnought II (B B Y) and Cruiser II (G Y R) are each one prerequisite short; the ready aida ignores it.
    engine: ["nm", "ps", "gd", "gls", "cr2", "dn2"],
  },
  { key: "aida-exhausted", owned: ["aida", "amd", "st"], exhausted: ["aida"], engine: ["nm", "ps", "gd", "gls"] },
  {
    key: "pa-exhausted-planet",
    owned: ["pa"],
    planets: [{ id: "thibah", exhausted: true }],
    // The exhausted blue specialty (Thibah) still counts under Psychoarchaeology: Gravity Drive is on offer.
    engine: ["amd", "gd", "nm", "ps"],
  },
  { key: "plain", owned: ["amd", "st"], engine: ["nm", "ps", "gd", "gls"] },
];
