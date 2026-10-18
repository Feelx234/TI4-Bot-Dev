import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
import type { HistoryStatus, PendingChoiceDto } from "../protocol/types.ts";
import { partialContext } from "../test/wire.ts";
import { AUTO_PLAY_DELAY_MS } from "./useSecondaryAutoPlay.ts";
import { SECONDARY_PREP_MODE_KEY } from "./useSecondaryPrepMode.ts";
import { loadStatusPlan, STATUS_PLAN_KEY_PREFIX, useStatusTokenPlan } from "./useStatusTokenPlan.ts";

const hist = (cursor: number): HistoryStatus => ({ cursor, redo_count: 0, generation: 0 });
const gain = (nonce = "g1", tokens = 2): PendingChoiceDto => ({
  prompt: "gain a command token into which pool",
  actor: "b",
  nonce,
  context: partialContext({ subtype: "gain_command_token", phase: "status", actor: "b" }),
  options: ["tactic", "fleet", "strategic"].map((p) => ({ id: `${p}_tokens`, kind: "pool", label: p, payload: {} })),
  details: {
    kind: "command_tokens",
    mode: "gain",
    pools: { tactic: 3, fleet: 3, strategic: 2 },
    reinforcements: 8,
    tokens_to_place: tokens,
  },
});

interface Props {
  round: number;
  choice: PendingChoiceDto | null;
  history: HistoryStatus;
}

function setup(initial: Props) {
  const submitChoice = vi.fn().mockResolvedValue(undefined);
  const submitBatch = vi.fn().mockResolvedValue(undefined);
  const hook = renderHook(
    (p: Props) =>
      useStatusTokenPlan({
        gameId: "g",
        viewerSeat: "b",
        round: p.round,
        ready: true,
        history: p.history,
        events: [],
        realChoice: p.choice,
        submitChoice,
        submitBatch,
      }),
    { initialProps: initial },
  );
  return { submitChoice, submitBatch, hook };
}

describe("useStatusTokenPlan", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.useFakeTimers();
  });
  afterEach(() => vi.useRealTimers());

  it("keeps the plan on this device for the round and drops it when a later round starts", () => {
    const { hook } = setup({ round: 2, choice: null, history: hist(1) });
    act(() => hook.result.current.save({ gain: { tactic: 2, fleet: 0, strategic: 0 } }));
    expect(loadStatusPlan("g", "b")?.round).toBe(2);
    expect(hook.result.current.hasPlan).toBe(true);
    hook.rerender({ round: 3, choice: null, history: hist(1) });
    expect(hook.result.current.hasPlan).toBe(false);
    expect(localStorage.getItem(`${STATUS_PLAN_KEY_PREFIX}:g:b`)).toBeNull();
  });

  it("clears the plan on request", () => {
    const { hook } = setup({ round: 2, choice: null, history: hist(1) });
    act(() => hook.result.current.save({ gain: { tactic: 2, fleet: 0, strategic: 0 } }));
    act(() => hook.result.current.clear());
    expect(hook.result.current.hasPlan).toBe(false);
  });

  it("in auto mode answers the gain from the plan once the game moved forward to it", async () => {
    localStorage.setItem(SECONDARY_PREP_MODE_KEY, "auto");
    const { hook, submitBatch } = setup({ round: 2, choice: null, history: hist(5) });
    act(() => hook.result.current.save({ gain: { tactic: 1, fleet: 1, strategic: 0 } }));
    hook.rerender({ round: 2, choice: null, history: hist(6) });
    hook.rerender({ round: 2, choice: gain(), history: hist(7) });
    expect(hook.result.current.pending?.text).toBe("Gain 2 command tokens: 1 tactic, 1 fleet");
    await act(async () => {
      vi.advanceTimersByTime(AUTO_PLAY_DELAY_MS + 10);
    });
    expect(submitBatch).toHaveBeenCalledWith({
      kind: "tokens",
      steps: [
        { kind: "pool", pool: "tactic_tokens" },
        { kind: "pool", pool: "fleet_tokens" },
      ],
    });
    expect(hook.result.current.played).toBe("Gain 2 command tokens: 1 tactic, 1 fleet");
    // The part is used now; an undo to before the answer makes it usable again.
    hook.rerender({ round: 2, choice: null, history: hist(8) });
    expect(hook.result.current.plan.gain).toBeUndefined();
    hook.rerender({ round: 2, choice: gain("g2"), history: hist(7) });
    expect(hook.result.current.plan.gain).toBeDefined();
  });

  it("never answers a gain that does not fit the plan, and never on a decision reached by undo", async () => {
    localStorage.setItem(SECONDARY_PREP_MODE_KEY, "auto");
    const { hook, submitBatch } = setup({ round: 2, choice: null, history: hist(5) });
    act(() => hook.result.current.save({ gain: { tactic: 2, fleet: 0, strategic: 0 } }));
    hook.rerender({ round: 2, choice: null, history: hist(6) });
    hook.rerender({ round: 2, choice: gain("g1", 3), history: hist(7) });
    expect(hook.result.current.resolution.kind).toBe("review");
    await act(async () => {
      vi.advanceTimersByTime(AUTO_PLAY_DELAY_MS + 10);
    });
    expect(submitBatch).not.toHaveBeenCalled();
    // Reached by going back in history: asked, not answered.
    hook.rerender({ round: 2, choice: null, history: hist(9) });
    hook.rerender({ round: 2, choice: gain("g3"), history: hist(4) });
    await act(async () => {
      vi.advanceTimersByTime(AUTO_PLAY_DELAY_MS + 10);
    });
    expect(submitBatch).not.toHaveBeenCalled();
    expect(hook.result.current.pending).toBeNull();
  });
});
