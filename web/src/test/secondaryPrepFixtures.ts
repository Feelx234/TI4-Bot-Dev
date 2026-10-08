import type {
  BoardView,
  ChoiceOptionDto,
  GameEvent,
  PendingChoiceDto,
  PlayerView,
} from "../protocol/types.ts";

/** Small builders for the strategy-card secondary preparation tests. */

export const player = (id: string, overrides: Partial<PlayerView> = {}): PlayerView => ({
  id,
  faction: "sol",
  victory_points: 0,
  trade_goods: 0,
  commodities: 0,
  tactic_tokens: 3,
  fleet_tokens: 3,
  strategic_tokens: 2,
  passed: false,
  strategy_cards: [],
  exhausted_strategy_cards: [],
  technologies: [],
  exhausted_technologies: [],
  relics: [],
  exhausted_relics: [],
  action_cards_count: 0,
  secret_objectives_count: 0,
  leaders: {},
  ...overrides,
});

let counter = 0;
/** A published decision event of an action; `detail` carries "<seat> played <Card>". */
export function actionEvent(
  actionId: string,
  actor: string,
  overrides: Partial<GameEvent> = {},
): GameEvent {
  counter += 1;
  return {
    id: `e${counter}`,
    timestamp: "2026-10-07T00:00:00Z",
    visibility: { visibility: "public" as const },
    event: { kind: "decision_resolved" },
    action_id: actionId,
    action_type: "strategic",
    action_actor: "a",
    actor,
    round: 2,
    phase: "action",
    ...overrides,
  } as GameEvent;
}

/** The log of seat `a` playing `card` (by printed name when `named`) as action_5. */
export const playedLog = (named: string | null = "Technology"): GameEvent[] => [
  actionEvent("action_5", "a", named ? { detail: `a played ${named}` } : { detail: "a began a component action" }),
];

export const option = (id: string, kind = "strategy", label = id): ChoiceOptionDto => ({ id, kind, label });

const yesNo = (): ChoiceOptionDto[] => [option("no", "strategy", "decline"), option("yes", "strategy", "go")];

/** The follower's own secondary question for `card`, as the engine sends it. */
export function secondaryChoice(
  card: string,
  overrides: Partial<PendingChoiceDto> = {},
  options: ChoiceOptionDto[] = yesNo(),
): PendingChoiceDto {
  return {
    actor: "b",
    nonce: "n-secondary",
    prompt: "spend a strategy token to research",
    options,
    context: { subtype: "strategy_secondary" } as PendingChoiceDto["context"],
    details: { kind: "strategy_secondary", card, played_by: "a", tokens_left: 2, costs_token: true },
    ...overrides,
  };
}

export const stepChoice = (
  subtype: string,
  options: ChoiceOptionDto[],
  nonce = `n-${subtype}`,
): PendingChoiceDto => ({
  actor: "b",
  nonce,
  prompt: subtype,
  options,
  context: { subtype } as PendingChoiceDto["context"],
});

export const boardWith = (
  planets: { system: string; planet: string; owner: string; exhausted?: boolean }[],
): BoardView => {
  const systems: BoardView["systems"] = {};
  for (const entry of planets) {
    systems[entry.system] ??= { system_id: entry.system, command_tokens: [], planets: {}, units: [] };
    systems[entry.system].planets[entry.planet] = {
      planet_id: entry.planet,
      controlled_by: entry.owner,
      exhausted: Boolean(entry.exhausted),
    };
  }
  return { systems };
};
