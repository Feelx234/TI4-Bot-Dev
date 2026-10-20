import type { Browser, Page } from "@playwright/test";
import { actor, galleryBoard } from "../_shared/fixtures";
import { openMockedGame, type MockGameOptions } from "../_shared/mockGame";
import { me, opponent, withPlayer } from "../_shared/players";
import type { PlayerView } from "../../../src/protocol/types";

/** "before" is taken on the base commit (TI4_SHOT_PREFIX=before) and committed under before/. */
export const prefix = process.env.TI4_SHOT_PREFIX ?? "after";

export const SIZES = {
  desktop: { width: 1440, height: 900 },
  "390x844": { width: 390, height: 844 },
} as const;
export type SizeName = keyof typeof SIZES;

export const A = actor; // Sol, the viewer
export const B = opponent.id; // Hacan, the leader
export const C = "seat_c"; // Arborec
export const D = "seat_d"; // Letnev

const base = (id: string, faction: string, overrides: Partial<PlayerView>): PlayerView =>
  withPlayer(me, { id, faction, strategy_cards: [], ...overrides });

export const players = (passed: readonly string[] = [D], thunders = false): PlayerView[] => [
  base(A, "sol", { victory_points: 7, scored_secret_objectives: ["faa"], passed: passed.includes(A) }),
  base(B, "hacan", { victory_points: 8, scored_secret_objectives: ["btgk", "ctr"], passed: passed.includes(B) }),
  base(C, "arborec", { victory_points: 3, scored_secret_objectives: ["ans"], passed: passed.includes(C) }),
  base(D, "letnev", { victory_points: 3, scored_secret_objectives: [], passed: passed.includes(D) }),
].map((p) => (thunders ? p : p));

export const REVEALED = ["expand_borders", "lead", "research_outposts", "conquer", "brain_trust", "golden_age"];
const SCORED: Record<string, string[]> = {
  [A]: ["expand_borders", "lead", "conquer"],
  [B]: ["expand_borders", "research_outposts", "brain_trust"],
  [C]: ["lead"],
  [D]: ["golden_age"],
};

/** The engine's `ScoreFacts` for the game above (same shape `project_score_facts` produces). */
export const scoreFacts = {
  target: 10,
  public: [
    ["expand_borders", "Expand Borders", 1, [A, B]],
    ["lead", "Lead From the Front", 1, [A, C]],
    ["research_outposts", "Found Research Outposts", 1, [B]],
    ["conquer", "Conquer the Weak", 2, [A]],
    ["brain_trust", "Form Galactic Brain Trust", 2, [B]],
    ["golden_age", "Found a Golden Age", 2, [D]],
  ].map(([id, name, points, scored]) => ({ id, name, points, text: "", scored })),
  sources: [
    { kind: "secrets", name: "", points: { [A]: 1, [B]: 2, [C]: 1 } },
    { kind: "custodians", name: "", points: { [A]: 1 } },
    { kind: "imperial", name: "", points: { [B]: 2 } },
    { kind: "support", name: "", points: { [A]: 1 }, from: { [A]: [B] } },
    { kind: "agenda", name: "Mutiny", points: { [C]: 1 }, text: "For: each player who voted for gains 1 VP." },
    { kind: "relic", name: "Shard of the Throne", points: { [D]: 1 }, text: "When you gain this card, gain 1 VP." },
  ],
};

export const baseState = (thunders: boolean) => ({
  ...(thunders ? { thunders_edge: true } : {}),
  vp_ledger: [],
});

export interface SceneOptions {
  passed?: readonly string[];
  thunders?: boolean;
  facts?: boolean;
  extra?: Partial<MockGameOptions>;
}

export async function openScene(page: Page, size: SizeName, options: SceneOptions = {}) {
  await page.setViewportSize(SIZES[size]);
  const game = await openMockedGame(page, {
    phase: "action",
    round: 3,
    board: galleryBoard,
    players: players(options.passed ?? [D]),
    table: { revealed_objectives: REVEALED, scored_objectives: SCORED },
    state: baseState(options.thunders ?? false),
    facts: options.facts === false ? undefined : { score: scoreFacts },
    choice: { prompt: "action phase", player: B, options: [{ id: "pass", label: "pass", kind: "action" }] },
    nicknames: { [A]: "You", [B]: "Blair", [C]: "Casey", [D]: "Drew" },
    ...options.extra,
  });
  await page.getByTestId("ti4-board-svg").waitFor();
  await page.waitForTimeout(400);
  return game;
}

export async function openPlayers(page: Page, size: SizeName) {
  if (size !== "desktop") {
    await page.getByTestId("player-sheet-toggle").click();
    await page.waitForTimeout(400);
  }
}

export const noHorizontalScroll = (page: Page) =>
  page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);

export type { Browser };
