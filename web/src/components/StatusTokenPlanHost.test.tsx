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
  it("starts from what is held after the gain (two tokens in the tactic pool) and saves that arrangement", () => {
    const { onSave } = setup();
    expect(screen.queryByTestId("status-plan-gain-toggle")).not.toBeInTheDocument();
    expect(screen.getByTestId("status-plan-gain-note").textContent).toContain("5 tactic / 3 fleet / 2 strategy");
    fireEvent.click(screen.getByTestId("status-plan-save"));
    expect(onSave).toHaveBeenCalledWith({ redistribute: { tactic: 5, fleet: 3, strategic: 2 } });
  });

  it("blocks a wrong total", () => {
    const { onSave } = setup();
    // 8 held + the usual 2.
    expect(screen.getByTestId("status-plan-redis-tactic-count").textContent).toBe("5");
    expect(screen.getByTestId("status-plan-problem").textContent).toContain("all 10 tokens");
    fireEvent.click(screen.getByTestId("status-plan-redis-strategic-plus"));
    expect(screen.getByTestId("status-plan-problem").textContent).toContain("Remove 1");
    expect((screen.getByTestId("status-plan-save") as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByTestId("status-plan-redis-strategic-minus"));
    fireEvent.click(screen.getByTestId("status-plan-redis-tactic-minus"));
    fireEvent.click(screen.getByTestId("status-plan-redis-fleet-plus"));
    fireEvent.click(screen.getByTestId("status-plan-save"));
    expect(onSave).toHaveBeenCalledWith({ redistribute: { tactic: 4, fleet: 4, strategic: 2 } });
  });

  it("clears the plan", () => {
    const { onClear, onClose } = setup({ redistribute: { tactic: 4, fleet: 4, strategic: 2 } });
    fireEvent.click(screen.getByTestId("status-plan-clear"));
    expect(onClear).toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });
});
