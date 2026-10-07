import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useTurnRedo } from "./useTurnRedo.ts";
import type { TurnRedoStatus } from "../protocol/turnRedo.ts";

const base: TurnRedoStatus = {
  seat: "p2",
  requested_by: "p2",
  turns_back: 1,
  redo_count: 1,
  stage: "new_turn",
  original_decisions: 120,
  rewound_to: 101,
  turn_complete: false,
  handoff_len: null,
  outcome: null,
  can_control: true,
};

function setup(statuses: Array<TurnRedoStatus | null>, command = vi.fn().mockResolvedValue(undefined)) {
  let i = 0;
  const fetchStatus = vi.fn(async () => statuses[Math.min(i++, statuses.length - 1)]);
  const hook = renderHook(
    (props: { gameVersion: number; generation: number }) =>
      useTurnRedo({ enabled: true, fetchStatus, command, ...props }),
    { initialProps: { gameVersion: 5, generation: 0 } },
  );
  return { hook, fetchStatus, command };
}

describe("useTurnRedo", () => {
  it("reads the status on mount and does nothing for a spectator", async () => {
    const { hook } = setup([base]);
    await waitFor(() => expect(hook.result.current.status).toEqual(base));
    const fetchStatus = vi.fn();
    renderHook(() =>
      useTurnRedo({ enabled: false, fetchStatus, command: vi.fn(), gameVersion: 1, generation: 0 }),
    );
    expect(fetchStatus).not.toHaveBeenCalled();
  });

  it("asks for the round to replay once, as soon as the new turn is complete", async () => {
    const complete = { ...base, turn_complete: true };
    const { hook, command } = setup([base, complete]);
    await waitFor(() => expect(hook.result.current.status?.turn_complete).toBe(false));
    // The game moves on: the status is re-read, the turn is complete, autoplay fires.
    hook.rerender({ gameVersion: 6, generation: 0 });
    await waitFor(() => expect(command).toHaveBeenCalledWith({ action: "autoplay" }));
    expect(command).toHaveBeenCalledTimes(1);
  });

  it("does not auto-play for a viewer who may not steer the redo", async () => {
    const { hook, command } = setup([{ ...base, turn_complete: true, can_control: false }]);
    await waitFor(() => expect(hook.result.current.status).not.toBeNull());
    expect(command).not.toHaveBeenCalled();
  });

  it("runs commands, reports the server's refusal and clears it on the next try", async () => {
    const command = vi
      .fn()
      .mockRejectedValueOnce(new Error("Turn redo failed (403): only the host may redo another seat's turn"))
      .mockResolvedValue(undefined);
    const onTimelineChanged = vi.fn();
    let i = 0;
    const fetchStatus = vi.fn(async () => (i++ === 0 ? null : base));
    const { result } = renderHook(() =>
      useTurnRedo({ enabled: true, fetchStatus, command, gameVersion: 1, generation: 0, onTimelineChanged }),
    );
    await act(async () => result.current.request({ turns: 2, seat: "p3" }));
    expect(command).toHaveBeenCalledWith({ action: "request", turns: 2, seat: "p3" });
    await waitFor(() => expect(result.current.error).toMatch(/only the host/));
    expect(onTimelineChanged).not.toHaveBeenCalled();
    await act(async () => result.current.request());
    await waitFor(() => expect(result.current.error).toBeNull());
    expect(command).toHaveBeenLastCalledWith({ action: "request", turns: 1, seat: undefined });
    expect(onTimelineChanged).toHaveBeenCalledOnce();
    await waitFor(() => expect(result.current.status).toEqual(base));
  });

  it("restores and keeps through the same command channel", async () => {
    const { hook, command } = setup([base]);
    await waitFor(() => expect(hook.result.current.status).not.toBeNull());
    await act(async () => hook.result.current.restore());
    await waitFor(() => expect(command).toHaveBeenCalledWith({ action: "restore" }));
    await act(async () => hook.result.current.keep());
    await waitFor(() => expect(command).toHaveBeenCalledWith({ action: "keep" }));
  });
});
