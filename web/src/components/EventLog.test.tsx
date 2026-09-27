import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { GameLogEntry } from "../protocol/client.ts";
import { EventLog, foldLogEvents } from "./EventLog.tsx";
import { PlayerIdentityProvider } from "../presentation/PlayerIdentity.tsx";

const decision = (cursor: number, batch = "batch"): GameLogEntry => ({
  id: `event-${cursor}`,
  timestamp: "12:00",
  visibility: "public",
  decision_count: cursor,
  batch_id: batch,
  batch_start_cursor: 10,
  batch_end_cursor: 14,
  action_id: "action_11",
  action_start_cursor: 10,
  detail: `Choice ${cursor}`,
  event: { kind: "decision_resolved" },
});

describe("event log grouping", () => {
  it("counts same-cursor details once and folds over private gaps and phase events", () => {
    const entries: GameLogEntry[] = [
      decision(11),
      {
        ...decision(11),
        id: "private-11",
        visibility: "seat",
        seat: "p1",
        private_detail: "Only P1",
      },
      {
        ...decision(11),
        id: "phase-11",
        batch_id: undefined,
        event: { kind: "phase_transition", phase: "action", round: 1 },
      },
      decision(13),
      decision(14),
    ];
    const rows = foldLogEvents(entries);
    expect(rows).toHaveLength(2);
    expect(rows[0].map((event) => event.decision_count)).toEqual([11, 11, 13, 14]);
    const restore = vi.fn();
    render(
      <PlayerIdentityProvider lobby={null} seatingOrder={[]}>
        <EventLog events={entries} isOpen onToggle={vi.fn()} cursor={14} onRestore={restore} />
      </PlayerIdentityProvider>,
    );
    expect(screen.getAllByTestId("event-log-entry")).toHaveLength(2);
    expect(screen.getAllByRole("button", { name: /Undo from decision/ })).toHaveLength(3);
    expect(screen.queryByText("⋯")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Undo from decision 11" }));
    expect(restore).toHaveBeenCalledWith(10);
    fireEvent.click(screen.getByRole("button", { name: "Undo from decision 13" }));
    expect(restore).toHaveBeenCalledWith(12);
    expect(screen.queryByRole("button", { name: /action start|batch start/i })).toBeNull();
  });

  it("shows one inline undo for an individual decision without hiding its event metadata", () => {
    const restore = vi.fn();
    render(
      <PlayerIdentityProvider lobby={null} seatingOrder={[]}>
        <EventLog
          events={[{ ...decision(5), batch_id: undefined, version: 3 }]}
          isOpen
          onToggle={vi.fn()}
          cursor={5}
          onRestore={restore}
        />
      </PlayerIdentityProvider>,
    );
    expect(screen.getByTestId("event-log-entry")).toHaveTextContent("12:00");
    expect(screen.getByTestId("event-log-entry")).toHaveTextContent("v3");
    expect(screen.getByTestId("event-log-entry")).toHaveTextContent("Choice 5");
    const undo = screen.getByRole("button", { name: "Undo from decision 5" });
    expect(undo).toHaveTextContent("Undo");
    fireEvent.click(undo);
    expect(restore).toHaveBeenCalledWith(4);
  });
});
