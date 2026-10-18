import type { PendingChoiceDto } from "../protocol/types.ts";
import {
  TOKEN_POOLS,
  describeCommandTokens,
  hasArmada,
  minFleetOffered,
  stagedTotal,
  tokenOutcome,
  type Pools,
} from "./commandTokens.ts";
import { describePools, type StepResolution, type TokenPrefill } from "./secondaryPlan.ts";

/**
 * A prepared status phase: the follower's private, revocable suggestion for the two token
 * decisions of rule 81.5. Like a prepared secondary it is stored by meaning (a distribution of
 * pools), never by option id, lives on this device only and is checked against the real question
 * when the engine asks; nothing is sent or spent before then.
 */
export interface StatusTokenPlan {
  /** Where the tokens gained in step 81.5 go, e.g. `{ tactic: 2, fleet: 0, strategic: 0 }`. */
  gain?: Pools;
  /** The arrangement wanted after the gain: the final pools of the redistribution. */
  redistribute?: Pools;
}

export type StatusTokenPart = "gain" | "redistribute";

/** A plan as kept on the device, with the round it is for and which parts were already answered. */
export interface StoredStatusPlan {
  v: 1;
  /** The round whose status phase this plan is for (it is dropped once a later round starts). */
  round: number;
  plan: StatusTokenPlan;
  /**
   * History cursor at which a part was answered from the plan. A part counts as used only while
   * the cursor is still past that point, so an undo to before the answer makes the plan usable
   * again.
   */
  applied?: Partial<Record<StatusTokenPart, number>>;
}

/** Most tokens a plan may name per part (a faction owns 16 in all, LRR 20.4). */
export const MAX_PLANNED_TOKENS = 16;

const asCount = (value: unknown): number | null =>
  typeof value === "number" && Number.isInteger(value) && value >= 0 && value <= MAX_PLANNED_TOKENS
    ? value
    : null;

function parsePools(raw: unknown, minTotal: number): Pools | null {
  if (!raw || typeof raw !== "object") return null;
  const source = raw as Record<string, unknown>;
  const counts = TOKEN_POOLS.map((pool) => asCount(source[pool]));
  if (counts.some((count) => count === null)) return null;
  const [tactic, fleet, strategic] = counts as number[];
  const total = tactic + fleet + strategic;
  return total >= minTotal && total <= MAX_PLANNED_TOKENS ? { tactic, fleet, strategic } : null;
}

/** Parses a stored plan strictly; anything unexpected is `null` (the plan is then simply gone). */
export function parseStoredStatusPlan(raw: string | null | undefined): StoredStatusPlan | null {
  if (!raw) return null;
  let data: unknown;
  try {
    data = JSON.parse(raw);
  } catch {
    return null;
  }
  if (!data || typeof data !== "object") return null;
  const record = data as Record<string, unknown>;
  if (record.v !== 1 || typeof record.round !== "number" || !Number.isInteger(record.round)) return null;
  const source = (record.plan ?? {}) as Record<string, unknown>;
  const plan: StatusTokenPlan = {};
  const gain = parsePools(source.gain, 1);
  if (gain) plan.gain = gain;
  const redistribute = parsePools(source.redistribute, 1);
  if (redistribute) plan.redistribute = redistribute;
  if (!plan.gain && !plan.redistribute) return null;
  const stored: StoredStatusPlan = { v: 1, round: record.round, plan };
  const applied = record.applied as Record<string, unknown> | undefined;
  if (applied && typeof applied === "object") {
    stored.applied = {};
    for (const part of ["gain", "redistribute"] as const) {
      const cursor = applied[part];
      if (typeof cursor === "number" && Number.isInteger(cursor)) stored.applied[part] = cursor;
    }
  }
  return stored;
}

export const serializeStoredStatusPlan = (stored: StoredStatusPlan): string => JSON.stringify(stored);

export const planTotal = (pools: Pools): number => stagedTotal(pools);

/** A part is used (answered from the plan) while the game is still past the answer. */
export function partUsed(stored: StoredStatusPlan, part: StatusTokenPart, cursor: number | undefined): boolean {
  const at = stored.applied?.[part];
  return at !== undefined && (cursor ?? 0) > at;
}

/** The plan as the answers still to give: parts already answered from it are left out. */
export function openParts(stored: StoredStatusPlan | null, cursor: number | undefined): StatusTokenPlan {
  if (!stored) return {};
  const open: StatusTokenPlan = {};
  if (stored.plan.gain && !partUsed(stored, "gain", cursor)) open.gain = stored.plan.gain;
  if (stored.plan.redistribute && !partUsed(stored, "redistribute", cursor)) {
    open.redistribute = stored.plan.redistribute;
  }
  return open;
}

/** Which of the two plannable decisions a pending choice is, or `null` for any other question. */
export function statusTokenPart(
  choice: Pick<PendingChoiceDto, "context"> | null | undefined,
): StatusTokenPart | null {
  const context = choice?.context;
  if (!context) return null;
  const phase = String(context.phase ?? "").toLowerCase();
  if (context.subtype === "status_redistribute_tokens") return "redistribute";
  // Rule 52.4's gain is asked by Leadership, action cards and exploration too; only the status
  // phase's own (81.5) is planned.
  if (context.subtype === "gain_command_token" && phase === "status") return "gain";
  return null;
}

/** "Gain 2 command tokens: 2 tactic" for the one-line summaries. */
export function describeStatusPart(part: StatusTokenPart, pools: Pools): string {
  const total = planTotal(pools);
  return part === "gain"
    ? `Gain ${total} command token${total === 1 ? "" : "s"}: ${describePools(pools)}`
    : `Redistribute to tactic ${pools.tactic} / fleet ${pools.fleet} / strategy ${pools.strategic}`;
}

export function describeStatusPlan(plan: StatusTokenPlan): string {
  return [
    plan.gain ? describeStatusPart("gain", plan.gain) : null,
    plan.redistribute ? describeStatusPart("redistribute", plan.redistribute) : null,
  ]
    .filter(Boolean)
    .join("; ");
}

/** What a plan says about one pending decision. */
export interface StatusResolution {
  part: StatusTokenPart | null;
  resolution: StepResolution;
  /** The planned pools, to open the usual panel with (shown as a suggestion, never sent). */
  prefill?: TokenPrefill;
}

const NONE: StatusResolution = { part: null, resolution: { kind: "none" } };

/**
 * The prepared answer for one status-phase token decision of the viewer's, validated against the
 * options the engine really offers. Pure; the caller decides whether to show, prefill or (auto
 * mode) submit it. Never a half plan: anything that does not fit exactly is `review`.
 */
export function resolveStatusTokens(
  plan: StatusTokenPlan | null | undefined,
  choice: PendingChoiceDto | null | undefined,
  viewerSeat: string | null | undefined,
): StatusResolution {
  if (!plan || !choice || !viewerSeat || choice.actor !== viewerSeat) return NONE;
  const part = statusTokenPart(choice);
  if (!part) return NONE;
  const wanted = plan[part];
  if (!wanted) return NONE;
  const planned = planTotal(wanted);
  const prefill: TokenPrefill = { bought: 0, pools: wanted, override: null };
  const review = (reason: string, withPrefill = false): StatusResolution => ({
    part,
    resolution: { kind: "review", reason },
    ...(withPrefill ? { prefill } : {}),
  });
  const view = describeCommandTokens(choice, true);
  if (!view || view.mode !== (part === "gain" ? "gain" : "redistribute")) {
    return review("The token question is not in a form the plan can answer; choose by hand.");
  }
  if (part === "gain") {
    if (view.purchase) return review("A purchase is offered with this gain; choose by hand.");
    if (view.total !== planned) {
      return review(
        `You gain ${view.total} command token${view.total === 1 ? "" : "s"} now, not the ${planned} you planned.`,
      );
    }
    if (view.reinforcements !== null && planned > view.reinforcements) {
      return review(`Only ${view.reinforcements} tokens are left in your reinforcements.`);
    }
    const outcome = tokenOutcome(view, wanted);
    if (!outcome || outcome.kind !== "plan") {
      return review("The planned pools can no longer be placed.", true);
    }
    return {
      part,
      resolution: { kind: "tokens", steps: outcome.steps, text: describeStatusPart(part, wanted), prefill },
      prefill,
    };
  }
  if (view.total !== planned) {
    return review(
      `You hold ${view.total} command token${view.total === 1 ? "" : "s"} now, not the ${planned} your plan arranges.`,
    );
  }
  const outcome = tokenOutcome(view, wanted);
  if (!outcome || outcome.kind !== "option") {
    const floor = minFleetOffered(view);
    return review(
      floor > 0 && wanted.fleet < floor
        ? `The game keeps at least ${floor} tokens in the fleet pool; your plan has ${wanted.fleet}.`
        : "That arrangement is not offered any more.",
      true,
    );
  }
  return {
    part,
    resolution: { kind: "option", optionId: outcome.optionId, text: describeStatusPart(part, wanted) },
    prefill,
  };
}

/** The pools a player holds now after gaining `gain` into `pools` (what a redistribution starts from). */
export function poolsAfterGain(current: Pools, gain: Pools | undefined): Pools {
  return {
    tactic: current.tactic + (gain?.tactic ?? 0),
    fleet: current.fleet + (gain?.fleet ?? 0),
    strategic: current.strategic + (gain?.strategic ?? 0),
  };
}

/** The fewest fleet tokens a redistribution may keep for a faction (Letnev's Armada may empty it). */
export const plannedFleetFloor = (faction: string | null | undefined): number => (hasArmada(faction) ? 0 : 2);

/** Why a redistribution target cannot be saved as a plan, or `null`. */
export function redistributionProblem(
  target: Pools,
  expectedTotal: number,
  faction: string | null | undefined,
): string | null {
  const total = planTotal(target);
  if (total !== expectedTotal) {
    return total < expectedTotal
      ? `Assign ${expectedTotal - total} more token${expectedTotal - total === 1 ? "" : "s"} (you will hold ${expectedTotal}).`
      : `Remove ${total - expectedTotal} token${total - expectedTotal === 1 ? "" : "s"} (you will hold ${expectedTotal}).`;
  }
  const floor = plannedFleetFloor(faction);
  if (expectedTotal >= floor && target.fleet < floor) return `Keep at least ${floor} tokens in the fleet pool.`;
  return null;
}
