import type { PendingChoiceDto } from "../protocol/types.ts";

export type TokenPool = "tactic" | "fleet" | "strategic";
export const TOKEN_POOLS: readonly TokenPool[] = ["tactic", "fleet", "strategic"];
export type Pools = Record<TokenPool, number>;

export const POOL_LABEL: Record<TokenPool, string> = {
  tactic: "Tactic",
  fleet: "Fleet",
  strategic: "Strategy",
};

/** What each pool is for, shown under its name. */
export const POOL_PURPOSE: Record<TokenPool, string> = {
  tactic: "activate systems",
  fleet: "fleet supply, moves ships",
  strategic: "strategy secondaries and abilities",
};

/** A fleet pool of 0 or 1 leaves a faction unable to move; the engine never offers it. */
export const MIN_FLEET_POOL = 2;

const gainOptionId = (pool: TokenPool) => `${pool}_tokens`;

export interface CommandTokenView {
  /** "gain": add new tokens to the pools; "redistribute": rearrange the tokens already held. */
  mode: "gain" | "redistribute";
  /** The pools as the engine has them now. */
  current: Pools;
  /** Tokens left in reinforcements. */
  reinforcements: number | null;
  /** Gain: tokens to place in this window. Redistribute: every token held. */
  total: number;
  /** Redistribute: the arrangement option ids the engine offers. */
  arrangements: ReadonlySet<string>;
}

/** Staged tokens per pool: added ones for a gain, the whole arrangement for a redistribute. */
export type TokenStaging = Pools;

const asCount = (value: unknown): number | null =>
  typeof value === "number" && Number.isFinite(value) && value >= 0 ? Math.floor(value) : null;

/**
 * A command-token decision as pools to fill, from the server's display-only details. `null` for
 * every other decision. A gain panel needs a way to send many tokens at once, so it is offered
 * only when `canBatch`; a redistribution is a single decision and always is.
 */
export function describeCommandTokens(
  choice: Pick<PendingChoiceDto, "options" | "details">,
  canBatch: boolean,
): CommandTokenView | null {
  const details = choice.details;
  if (!details || details.kind !== "command_tokens") return null;
  const pools = details.pools as Partial<Record<TokenPool, unknown>> | undefined;
  const current = {
    tactic: asCount(pools?.tactic),
    fleet: asCount(pools?.fleet),
    strategic: asCount(pools?.strategic),
  };
  if (current.tactic === null || current.fleet === null || current.strategic === null) return null;
  const held: Pools = { tactic: current.tactic, fleet: current.fleet, strategic: current.strategic };
  const reinforcements = asCount(details.reinforcements);
  if (details.mode === "gain") {
    const total = asCount(details.tokens_to_place);
    const ids = new Set(choice.options.map((option) => option.id));
    if (!canBatch || !total || !TOKEN_POOLS.every((pool) => ids.has(gainOptionId(pool)))) {
      return null;
    }
    return { mode: "gain", current: held, reinforcements, total, arrangements: new Set() };
  }
  if (details.mode === "redistribute") {
    const total = asCount(details.total);
    const arrangements = new Set(
      choice.options.filter((option) => option.kind === "redistribute").map((option) => option.id),
    );
    if (total === null || arrangements.size === 0) return null;
    return { mode: "redistribute", current: held, reinforcements, total, arrangements };
  }
  return null;
}

export const stagedTotal = (staging: TokenStaging): number =>
  TOKEN_POOLS.reduce((sum, pool) => sum + staging[pool], 0);

/** The staging before the player touches anything. */
export function initialStaging(view: CommandTokenView): TokenStaging {
  return view.mode === "gain"
    ? { tactic: 0, fleet: 0, strategic: 0 }
    : { ...view.current };
}

/** Tokens still to assign. */
export const tokensRemaining = (view: CommandTokenView, staging: TokenStaging): number =>
  Math.max(0, view.total - stagedTotal(staging));

export const canAddToken = (view: CommandTokenView, staging: TokenStaging): boolean =>
  tokensRemaining(view, staging) > 0;

export const canRemoveToken = (staging: TokenStaging, pool: TokenPool): boolean =>
  staging[pool] > 0;

export function addToken(view: CommandTokenView, staging: TokenStaging, pool: TokenPool): TokenStaging {
  return canAddToken(view, staging) ? { ...staging, [pool]: staging[pool] + 1 } : staging;
}

export function removeToken(staging: TokenStaging, pool: TokenPool): TokenStaging {
  return canRemoveToken(staging, pool) ? { ...staging, [pool]: staging[pool] - 1 } : staging;
}

/** What the pool will hold once the staging is confirmed. */
export const resultingCount = (
  view: CommandTokenView,
  staging: TokenStaging,
  pool: TokenPool,
): number => (view.mode === "gain" ? view.current[pool] + staging[pool] : staging[pool]);

export interface PoolPips {
  /** Tokens that stay as they are. */
  kept: number;
  /** Newly assigned tokens. */
  added: number;
  /** Tokens that were here and are taken out (redistribute only). */
  removed: number;
}

export function poolPips(view: CommandTokenView, staging: TokenStaging, pool: TokenPool): PoolPips {
  if (view.mode === "gain") {
    return { kept: view.current[pool], added: staging[pool], removed: 0 };
  }
  const now = view.current[pool];
  const next = staging[pool];
  return {
    kept: Math.min(now, next),
    added: Math.max(0, next - now),
    removed: Math.max(0, now - next),
  };
}

/** The option id of the redistribution that matches the staging, when the engine offers it. */
export function arrangementId(view: CommandTokenView, staging: TokenStaging): string | null {
  const id = `${staging.tactic}|${staging.fleet}|${staging.strategic}`;
  return view.mode === "redistribute" && view.arrangements.has(id) ? id : null;
}

/** Why a fully assigned staging still cannot be confirmed, or `null` when it can. */
export function confirmBlocker(view: CommandTokenView, staging: TokenStaging): string | null {
  const remaining = tokensRemaining(view, staging);
  if (remaining > 0) {
    return `Assign ${remaining} more token${remaining === 1 ? "" : "s"} to confirm.`;
  }
  if (view.mode === "redistribute" && arrangementId(view, staging) === null) {
    return view.total >= MIN_FLEET_POOL
      ? `Keep at least ${MIN_FLEET_POOL} tokens in the fleet pool.`
      : "That arrangement is not allowed.";
  }
  return null;
}

export const canConfirmTokens = (view: CommandTokenView, staging: TokenStaging): boolean =>
  view.total > 0 && confirmBlocker(view, staging) === null;

/** A step of a token batch plan, as the server's `tokens` batch kind takes it. */
export interface TokenStep {
  kind: "pool";
  pool: string;
}

/** The gain as one plan: a pool step per token, tactic first, then fleet, then strategy. */
export function tokenPlan(staging: TokenStaging): TokenStep[] {
  return TOKEN_POOLS.flatMap((pool) =>
    Array.from({ length: staging[pool] }, () => ({ kind: "pool" as const, pool: gainOptionId(pool) })),
  );
}

export type TokenOutcome =
  | { kind: "plan"; steps: TokenStep[] }
  | { kind: "option"; optionId: string };

/** What confirming sends: a batch plan for a gain, the arrangement's option for a redistribute. */
export function tokenOutcome(view: CommandTokenView, staging: TokenStaging): TokenOutcome | null {
  if (!canConfirmTokens(view, staging)) return null;
  if (view.mode === "gain") return { kind: "plan", steps: tokenPlan(staging) };
  const optionId = arrangementId(view, staging);
  return optionId === null ? null : { kind: "option", optionId };
}
