import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import type { PlayerView } from "../protocol/types.ts";
import { StatusTokenPlanDialog } from "./StatusTokenPlanHost.tsx";

const viewer = { id: "b", faction: "hacan", tactic_tokens: 3, fleet_tokens: 3, strategic_tokens: 2 } as unknown as PlayerView;

const setup = (initial = {}) => {
  const onSave = vi.fn();
  const onClear = vi.fn();
  const onClose = vi.fn();
  render(<StatusTokenPlanDialog viewer={viewer} initial={initial} onSave={onSave} onClear={onClear} onClose={onClose} />);
  return { onSave, onClear, onClose };
};

describe("StatusTokenPlanDialog", () => {
  it("cannot be saved empty", () => {
    setup();
    expect((screen.getByTestId("status-plan-save") as HTMLButtonElement).disabled).toBe(true);
  });

  it("saves a gain plan of two tactic tokens by default", () => {
    const { onSave } = setup();
    fireEvent.click(screen.getByTestId("status-plan-gain-toggle"));
    fireEvent.click(screen.getByTestId("status-plan-save"));
    expect(onSave).toHaveBeenCalledWith({ gain: { tactic: 2, fleet: 0, strategic: 0 } });
  });

  it("starts a redistribution from what is held after the gain and blocks a wrong total", () => {
    const { onSave } = setup();
    fireEvent.click(screen.getByTestId("status-plan-redistribute-toggle"));
    // 8 held + the usual 2.
    expect(screen.getByTestId("status-plan-redis-tactic-count").textContent).toBe("5");
    expect(screen.getByTestId("status-plan-problem").textContent).toContain("all 10 tokens");
    fireEvent.click(screen.getByTestId("status-plan-redis-strategic-plus"));
    expect(screen.getByTestId("status-plan-problem").textContent).toContain("Remove 1");
    expect((screen.getByTestId("status-plan-save") as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByTestId("status-plan-redis-strategic-minus"));
    fireEvent.click(screen.getByTestId("status-plan-save"));
    expect(onSave).toHaveBeenCalledWith({ redistribute: { tactic: 5, fleet: 3, strategic: 2 } });
  });

  it("clears the plan", () => {
    const { onClear, onClose } = setup({ gain: { tactic: 1, fleet: 1, strategic: 0 } });
    fireEvent.click(screen.getByTestId("status-plan-clear"));
    expect(onClear).toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });
});
