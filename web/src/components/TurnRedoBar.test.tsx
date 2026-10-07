import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { TurnRedoBar } from "./TurnRedoBar.tsx";
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

const handlers = { onAutoplay: vi.fn(), onRestore: vi.fn(), onKeep: vi.fn() };
const renderBar = (status: TurnRedoStatus | null, busy = null as null | "autoplay", error = null as string | null) =>
  render(<TurnRedoBar status={status} busy={busy} error={error} {...handlers} />);

describe("TurnRedoBar", () => {
  it("renders nothing when no redo is in flight", () => {
    const { container } = renderBar(null);
    expect(container).toBeEmptyDOMElement();
  });

  it("waits for the new turn and offers the way back", () => {
    renderBar(base);
    const bar = screen.getByTestId("turn-redo-bar");
    expect(bar).toHaveAttribute("data-state", "new-turn");
    expect(bar).toHaveAttribute("role", "status");
    expect(bar.textContent).toMatch(/Redoing p2's last turn/);
    expect(screen.queryByTestId("turn-redo-keep")).toBeNull();
    fireEvent.click(screen.getByTestId("turn-redo-restore"));
    expect(handlers.onRestore).toHaveBeenCalledOnce();
  });

  it("names two turns", () => {
    renderBar({ ...base, turns_back: 2 });
    expect(screen.getByTestId("turn-redo-bar").textContent).toMatch(/last two turns/);
  });

  it("shows a status while the round replays, busy with no buttons", () => {
    renderBar({ ...base, turn_complete: true }, "autoplay");
    const bar = screen.getByTestId("turn-redo-bar");
    expect(bar).toHaveAttribute("data-state", "replaying");
    expect(bar).toHaveAttribute("aria-busy", "true");
    expect(bar.textContent).toMatch(/Replaying the round/);
    expect(screen.queryByTestId("turn-redo-replay")).toBeNull();
  });

  it("offers a manual replay when the new turn is done and nothing is running", () => {
    renderBar({ ...base, turn_complete: true });
    fireEvent.click(screen.getByTestId("turn-redo-replay"));
    expect(handlers.onAutoplay).toHaveBeenCalledOnce();
  });

  it("reports a hand-off back to the redoing seat with keep and restore", () => {
    renderBar({
      ...base,
      stage: "auto_played",
      handoff_len: 112,
      outcome: {
        kept: 9,
        tail_total: 30,
        stop: { kind: "handoff", seat: "p2" },
        asking_seat: "p2",
        deck_offsets: [{ deck: "action_card", delta: -1 }],
      },
    });
    const bar = screen.getByTestId("turn-redo-bar");
    expect(bar).toHaveAttribute("data-state", "handoff");
    expect(bar.textContent).toMatch(/Back to p2/);
    expect(bar.textContent).toMatch(/9 decisions of the other seats were replayed/);
    expect(screen.getByTestId("turn-redo-deck-offsets").textContent).toMatch(/action card -1/);
    fireEvent.click(screen.getByTestId("turn-redo-keep"));
    expect(handlers.onKeep).toHaveBeenCalledOnce();
    expect(screen.getByTestId("turn-redo-keep").textContent).toBe("Keep this timeline");
  });

  it("explains a conflict without naming the recorded choice and offers continue or restore", () => {
    renderBar({
      ...base,
      stage: "auto_played",
      handoff_len: 106,
      outcome: {
        kept: 1,
        tail_total: 9,
        stop: {
          kind: "conflict",
          conflict: {
            original_cursor: 108,
            kind: "deck_cursor",
            seat: "p3",
            prompt: "action phase",
            detail: "…",
            deck_deltas: [{ deck: "action_card", delta: 1 }],
          },
        },
        asking_seat: "p3",
        deck_offsets: [],
      },
    });
    const bar = screen.getByTestId("turn-redo-bar");
    expect(bar).toHaveAttribute("data-state", "conflict");
    expect(bar.textContent).toMatch(/1 recorded decision was kept/);
    expect(bar.textContent).toMatch(/drew a different number of cards/);
    expect(bar.textContent).toMatch(/p3 decides/);
    expect(screen.getByTestId("turn-redo-keep").textContent).toBe("Continue from here");
    fireEvent.click(screen.getByTestId("turn-redo-restore"));
    expect(handlers.onRestore).toHaveBeenCalled();
  });

  it("reports an exhausted tail", () => {
    renderBar({
      ...base,
      stage: "auto_played",
      handoff_len: 120,
      outcome: { kept: 4, tail_total: 4, stop: { kind: "tail_exhausted" }, asking_seat: null, deck_offsets: [] },
    });
    expect(screen.getByTestId("turn-redo-bar")).toHaveAttribute("data-state", "complete");
  });

  it("lets other players watch but not steer", () => {
    renderBar({ ...base, can_control: false, stage: "auto_played", handoff_len: 1, outcome: {
      kept: 0, tail_total: 0, stop: { kind: "tail_exhausted" }, asking_seat: null, deck_offsets: [] } });
    expect(screen.getByTestId("turn-redo-bar")).toBeVisible();
    expect(screen.queryByTestId("turn-redo-restore")).toBeNull();
    expect(screen.queryByTestId("turn-redo-keep")).toBeNull();
  });

  it("shows the server's refusal", () => {
    renderBar(null, null, "Turn redo failed (403): only the host may redo another seat's turn");
    expect(screen.getByTestId("turn-redo-error").textContent).toMatch(/only the host/);
  });
});
