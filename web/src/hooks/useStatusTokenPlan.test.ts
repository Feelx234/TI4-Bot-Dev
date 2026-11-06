import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
import type { HistoryStatus, PendingChoiceDto } from "../protocol/types.ts";
import { partialContext } from "../test/wire.ts";
import { AUTO_PLAY_DELAY_MS } from "./useSecondaryAutoPlay.ts";
import { SECONDARY_PREP_MODE_KEY } from "./useSecondaryPrepMode.ts";
import { loadStatusPlan, STATUS_PLAN_KEY_PREFIX, useStatusTokenPlan } from "./useStatusTokenPlan.ts";

const hist = (cursor: number): HistoryStatus => ({ cursor, redo_count: 0, generation: 0 });
/** The status phase's redistribution of `total` tokens (fleet floor 2). */
const redis = (nonce = "r1", held = { tactic: 5, fleet: 3, strategic: 2 }): PendingChoiceDto => {
  const total = held.tactic + held.fleet + held.strategic;
  const options = [];
  for (let tactic = 0; tactic <= total; tactic++)
    for (let fleet = 2; fleet <= total - tactic; fleet++) {
      const id = `${tactic}|${fleet}|${total - tactic - fleet}`;
      options.push({ id, kind: "redistribute", label: id, payload: {} });
    }
  return {
    prompt: "redistribute your command tokens",
    actor: "b",
    nonce,
    context: partialContext({ subtype: "status_redistribute_tokens", phase: "status", actor: "b" }),
    options,
    details: { kind: "command_tokens", mode: "redistribute", pools: held, reinforcements: 6, total },
  };
};
const PLAN = { redistribute: { tactic: 4, fleet: 4, strategic: 2 } };

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
    act(() => hook.result.current.save(PLAN));
    expect(loadStatusPlan("g", "b")?.round).toBe(2);
    expect(hook.result.current.hasPlan).toBe(true);
    hook.rerender({ round: 3, choice: null, history: hist(1) });
    expect(hook.result.current.hasPlan).toBe(false);
    expect(localStorage.getItem(`${STATUS_PLAN_KEY_PREFIX}:g:b`)).toBeNull();
  });

  it("clears the plan on request", () => {
    const { hook } = setup({ round: 2, choice: null, history: hist(1) });
    act(() => hook.result.current.save(PLAN));
    act(() => hook.result.current.clear());
    expect(hook.result.current.hasPlan).toBe(false);
  });

  it("in auto mode answers the redistribution from the plan once the game moved forward to it", async () => {
    localStorage.setItem(SECONDARY_PREP_MODE_KEY, "auto");
    const { hook, submitChoice } = setup({ round: 2, choice: null, history: hist(5) });
    act(() => hook.result.current.save(PLAN));
    hook.rerender({ round: 2, choice: null, history: hist(6) });
    hook.rerender({ round: 2, choice: redis(), history: hist(7) });
    expect(hook.result.current.pending?.text).toBe("Redistribute to tactic 4 / fleet 4 / strategy 2");
    await act(async () => {
      vi.advanceTimersByTime(AUTO_PLAY_DELAY_MS + 10);
    });
    expect(submitChoice).toHaveBeenCalledWith("4|4|2");
    expect(hook.result.current.played).toBe("Redistribute to tactic 4 / fleet 4 / strategy 2");
    // The part is used now; an undo to before the answer makes it usable again.
    hook.rerender({ round: 2, choice: null, history: hist(8) });
    expect(hook.result.current.plan.redistribute).toBeUndefined();
    hook.rerender({ round: 2, choice: redis("r2"), history: hist(7) });
    expect(hook.result.current.plan.redistribute).toBeDefined();
  });

  it("never answers a redistribution that does not fit the plan, and never on a decision reached by undo", async () => {
    localStorage.setItem(SECONDARY_PREP_MODE_KEY, "auto");
    const { hook, submitChoice } = setup({ round: 2, choice: null, history: hist(5) });
    act(() => hook.result.current.save(PLAN));
    hook.rerender({ round: 2, choice: null, history: hist(6) });
    hook.rerender({ round: 2, choice: redis("r1", { tactic: 5, fleet: 3, strategic: 3 }), history: hist(7) });
    expect(hook.result.current.resolution.kind).toBe("review");
    await act(async () => {
      vi.advanceTimersByTime(AUTO_PLAY_DELAY_MS + 10);
    });
    expect(submitChoice).not.toHaveBeenCalled();
    // Reached by going back in history: asked, not answered.
    hook.rerender({ round: 2, choice: null, history: hist(9) });
    hook.rerender({ round: 2, choice: redis("r3"), history: hist(4) });
    await act(async () => {
      vi.advanceTimersByTime(AUTO_PLAY_DELAY_MS + 10);
    });
    expect(submitChoice).not.toHaveBeenCalled();
    expect(hook.result.current.pending).toBeNull();
  });
});
