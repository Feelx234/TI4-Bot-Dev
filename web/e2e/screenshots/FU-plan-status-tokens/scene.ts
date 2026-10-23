import type { Page } from "@playwright/test";
import { openMockedGame, type MockedGame } from "../_shared/mockGame";
import { playerWithHand, opponent, actor } from "../_shared/players";
import { playedEvent, PRIMARY } from "../AH-secondary-prep-real-ui/prep";

export const SIZES = [
  { name: "desktop", viewport: { width: 1440, height: 900 } },
  { name: "phone", viewport: { width: 390, height: 844 } },
] as const;
export type Size = (typeof SIZES)[number];

export const HELD = { tactic: 3, fleet: 3, strategic: 2 };
const ROUND = 3;

/** The status phase of round 3: another seat is deciding, the viewer waits and holds 8 tokens. */
export async function openStatusWaiting(page: Page, size: Size, mode?: "auto" | "review", bots?: string[]): Promise<MockedGame> {
  await page.setViewportSize(size.viewport);
  if (mode) {
    await page.addInitScript((value) => localStorage.setItem("player_secondary_prep_mode", value), mode);
  }
  const game = await openMockedGame(page, {
    round: ROUND,
    phase: "status",
    bots,
    players: [
      playerWithHand({ tactic_tokens: HELD.tactic, fleet_tokens: HELD.fleet, strategic_tokens: HELD.strategic, strategy_cards: ["pok8imperial"] }),
      { ...opponent, strategy_cards: ["pok7technology"] },
    ],
    events: [playedEvent("Technology")],
    view: { active_player: PRIMARY },
    turnStatus: { kind: "waiting_for_decision", seat: PRIMARY, phase: "status", round: ROUND, stage: "Waiting for player" },
  });
  push(game, { version: 41, history: { cursor: 40, redo_count: 0, generation: 0 } });
  await page.waitForTimeout(150);
  return game;
}

const pools = ["tactic", "fleet", "strategic"];

/** The engine's status-phase gain question (81.5 / 52.4) for `tokens` tokens. */
export const gainQuestion = (tokens: number) => ({
  prompt: "gain a command token into which pool",
  subtype: "gain_command_token",
  options: pools.map((pool) => ({ id: `${pool}_tokens`, kind: "pool", label: `${pool} pool` })),
  details: { kind: "command_tokens", mode: "gain", pools: HELD, reinforcements: 8, tokens_to_place: tokens },
});

/** The engine's status-phase redistribution (81.5) over every arrangement keeping 2 fleet tokens. */
export function redistributeQuestion(held = { tactic: 5, fleet: 3, strategic: 2 }) {
  const total = held.tactic + held.fleet + held.strategic;
  const options = [];
  for (let tactic = 0; tactic <= total; tactic++)
    for (let fleet = 2; fleet <= total - tactic; fleet++) {
      const id = `${tactic}|${fleet}|${total - tactic - fleet}`;
      options.push({ id, kind: "redistribute", label: `tactic ${tactic} / fleet ${fleet} / strategy ${total - tactic - fleet}` });
    }
  return {
    prompt: "redistribute your command tokens",
    subtype: "status_redistribute_tokens",
    options,
    details: { kind: "command_tokens", mode: "redistribute", pools: held, reinforcements: 6, total },
  };
}

type Question = ReturnType<typeof gainQuestion> | ReturnType<typeof redistributeQuestion>;

/** A state update that moves the game forward; with `choice` it is the viewer's own question. */
export function push(
  game: MockedGame,
  update: { version: number; history: { cursor: number; redo_count: number; generation: number }; choice?: Question | null },
) {
  const { events: _events, ...base } = game.snapshot;
  game.send({
    ...base,
    type: "state_update",
    game_version: update.version,
    history: update.history,
    turn_status: update.choice
      ? { kind: "waiting_for_decision", seat: actor, phase: "status", round: ROUND, stage: update.choice.subtype }
      : base.turn_status,
    pending_choice: update.choice
      ? {
          nonce: `status-nonce-${update.version}`,
          choice: {
            player: actor,
            prompt: update.choice.prompt,
            context: { subtype: update.choice.subtype, phase: "status", actor, round: ROUND },
            options: update.choice.options,
            details: update.choice.details,
          },
        }
      : null,
  });
}

/** Answers a gain batch: the question is gone and the log moved on (what the server returns). */
export async function acceptBatches(page: Page, game: MockedGame) {
  await page.route("**/api/games/screenshot-game/batches", (route) => {
    const body = route.request().postDataJSON() as { request_id: string; plan: { steps: unknown[] } };
    return route.fulfill({
      json: {
        request_id: body.request_id,
        batch_id: "batch-1",
        start_cursor: 41,
        end_cursor: 42,
        active: true,
        snapshot: {
          ...game.snapshot,
          game_version: 43,
          history: { cursor: 42, redo_count: 0, generation: 0 },
          pending_choice: null,
          turn_status: { kind: "waiting_for_decision", seat: PRIMARY, phase: "status", round: ROUND, stage: "Waiting for player" },
        },
      },
    });
  });
}

/** Opens the planner from its chip. */
export async function openPlanner(page: Page) {
  await page.getByTestId("status-plan-chip").click();
  await page.getByTestId("status-plan-dialog").waitFor();
}
