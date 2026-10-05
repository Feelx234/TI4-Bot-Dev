import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { GameLogEntry } from "../protocol/client.ts";
import { EventLog, buildEventTree } from "./EventLog.tsx";
import { PlayerIdentityProvider } from "../presentation/PlayerIdentity.tsx";
import type { LobbyDto } from "../protocol/types.ts";

const decision = (cursor: number, stage = "movement", actor = "p1"): GameLogEntry => ({
  id: `event-${cursor}`,
  timestamp: "12:00",
  visibility: "public",
  decision_count: cursor,
  round: 1,
  phase: "action",
  action_id: "action_1",
  action_type: "tactical",
  actor,
  stage,
  detail: `Choice ${cursor}`,
  version: 3,
  event: { kind: "decision_resolved" },
});
const wrap = (
  events: GameLogEntry[],
  props: Partial<React.ComponentProps<typeof EventLog>> = {},
) => (
  <PlayerIdentityProvider lobby={null} seatingOrder={["p1", "p2"]}>
    <EventLog events={events} isOpen onToggle={vi.fn()} cursor={100} {...props} />
  </PlayerIdentityProvider>
);

describe("hierarchical event log", () => {
  it("preserves repeated stage order and merges same-cursor private facts", () => {
    const entries = [
      decision(1),
      {
        ...decision(1),
        id: "private-1",
        visibility: "seat" as const,
        seat: "p1",
        private_detail: "Only P1",
      },
      decision(2, "combat", "p2"),
      decision(3),
    ];
    const tree = buildEventTree(entries);
    const phase = tree[0].children[0];
    const phaseEvents = phase.children;
    // With flattened structure: [action marker, stage1 marker, decision1, stage2 marker, decision2, stage1 marker, decision3]
    const stageMarkers = phaseEvents.filter((e) => e.kind === "marker" && e.stage);
    const decisions = phaseEvents.filter((e) => e.kind === "decision");
    expect(stageMarkers.map((stage) => stage.label)).toEqual(["Movement", "Combat", "Movement"]);
    expect(tree[0].count).toBe(3);
    expect(decisions[0].entry?.private_detail).toBe("Only P1");
  });

  it("mounts only the current path; lets readers expand, collapse and undo real cursors", () => {
    const restore = vi.fn();
    const entries = [
      {
        ...decision(0),
        id: "start",
        event: { kind: "game_initialized" as const, round: 1, phase: "action", speaker: "p1" },
      },
      decision(1),
      decision(2, "combat", "p2"),
      {
        ...decision(2),
        round: undefined,
        phase: undefined,
        id: "round2",
        event: { kind: "phase_transition" as const, round: 2, phase: "strategy" },
      },
      { ...decision(3), round: 2, phase: "strategy", action_id: undefined, stage: undefined },
    ];
    render(
      wrap(entries, {
        currentPath: { round: 1, phase: "action", action_id: "action_1", stage: "combat" },
        onRestore: restore,
      }),
    );
    expect(screen.getAllByText(/Round [12]/)).toHaveLength(2);
    // With flattened structure, the phase is opened automatically by currentPath
    // All events in the phase are visible: game_initialized marker, action marker, stage markers, decisions
    const entries1 = screen.getAllByTestId("event-log-entry");
    expect(entries1.length).toBeGreaterThan(1);
    expect(screen.getByText("Choice 1")).toBeInTheDocument();
    expect(screen.getByText("Choice 2")).toBeInTheDocument();
    const actor = screen.getByLabelText("Participant at position 1");
    expect(actor).toHaveAttribute("title", "Participant at position 1");
    fireEvent.click(screen.getByRole("button", { name: "Undo from decision 1" }));
    expect(restore).toHaveBeenCalledWith(0);
    fireEvent.click(screen.getByRole("button", { name: /Round 1/ }));
    // Collapsing the round should hide all its events
    expect(screen.queryByText("Choice 1")).toBeNull();
  });

  it("colors participant names without repeating seat labels in decisions or symbol tooltips", () => {
    const a = `player_${"a".repeat(64)}`;
    const b = `player_${"b".repeat(64)}`;
    const lobby: LobbyDto = {
      game_id: "game",
      phase: "running",
      lobby_version: 1,
      host_player_id: a,
      slots: [a, b].map((occupant, index) => ({
        slot_id: `slot_${index + 1}`,
        position: index + 1,
        occupant,
        nickname: "az2",
        ready: true,
        connected: true,
        can_take_over: false,
      })),
    };
    const entries = [
      {
        ...decision(0),
        id: "start",
        event: { kind: "game_initialized" as const, round: 1, phase: "action", speaker: a },
      },
      { ...decision(1, "movement", a), detail: `${a} landed sol_infantry on kraag` },
      { ...decision(2, "movement", b), detail: `${b} supported ${a}` },
    ];
    render(
      <PlayerIdentityProvider lobby={lobby} seatingOrder={[a, b]}>
        <EventLog
          events={entries}
          isOpen
          onToggle={vi.fn()}
          currentPath={{ round: 1, phase: "action", action_id: "action_1", stage: "movement" }}
        />
      </PlayerIdentityProvider>,
    );
    const rows = screen.getAllByTestId("event-log-entry");
    expect(rows[1].querySelector(".event-log__body")).toHaveTextContent(
      "az2 landed sol_infantry on kraag",
    );
    expect(rows[1].querySelector(".event-log__body")).not.toHaveTextContent("Position 1");
    expect(rows[1].querySelector(".event-log__participant")).toHaveStyle({ color: "#E69F00" });
    expect(rows[2].querySelectorAll(".event-log__participant")).toHaveLength(2);
    expect(rows[2].querySelectorAll(".event-log__participant")[0]).toHaveStyle({
      color: "#56B4E9",
    });
    expect(rows[2].querySelectorAll(".event-log__participant")[1]).toHaveStyle({
      color: "#E69F00",
    });
    const symbol = rows[1].querySelector(".event-log__actor");
    expect(symbol).toHaveAttribute("title", "az2 (● Position 1)");
    expect(symbol).toHaveAttribute("aria-label", "az2 (● Position 1)");
  });

  it("retains unknown historical decisions and more than 500 events", () => {
    const entries = Array.from({ length: 520 }, (_, i) => ({
      ...decision(i + 1),
      round: undefined,
      phase: undefined,
      action_id: undefined,
      stage: undefined,
    }));
    const tree = buildEventTree(entries);
    expect(tree).toHaveLength(1);
    expect(tree[0].count).toBe(520);
    const phaseEvents = tree[0].children[0].children;
    // With flattened structure, we should have 520 decision events
    const decisionEvents = phaseEvents.filter((e) => e.kind === "decision");
    expect(decisionEvents).toHaveLength(520);
  });

  it("keeps legacy action decisions under their phase without inventing actions", () => {
    const entries = [
      decision(1),
      {
        ...decision(2),
        action_id: undefined,
        stage: undefined,
        detail: "A historical action choice",
      },
    ];
    const phase = buildEventTree(entries)[0].children[0];
    const phaseEvents = phase.children;
    // With flattened structure: [action marker, stage marker, decision1, decision2 (no stage)]
    const actionMarker = phaseEvents.find((e) => e.actionId);
    const decision2 = phaseEvents.find((e) => e.entry?.detail === "A historical action choice");
    expect(actionMarker?.label).toBe("Tactical action");
    expect(decision2?.entry?.detail).toBe("A historical action choice");
    const stageMarker = phaseEvents.find((e) => e.stage === "movement");
    expect(stageMarker?.label).toBe("Movement");
  });

  it("renders action card names with tooltips when mentioned in detail text", () => {
    const entries = [
      {
        ...decision(1),
        detail: "Player played Bribery to increase votes",
      },
      {
        ...decision(2),
        detail: "Discarded Ancient Burial Sites",
      },
    ];
    render(
      wrap(entries, {
        currentPath: { round: 1, phase: "action", action_id: "action_1", stage: "movement" },
      }),
    );

    // Check that action card names are in the document
    const entries_rendered = screen.getAllByTestId("event-log-entry");
    expect(entries_rendered.length).toBeGreaterThan(0);
    const body1 = entries_rendered[0]?.querySelector(".event-log__body");
    const body2 = entries_rendered[1]?.querySelector(".event-log__body");

    expect(body1).toHaveTextContent("Bribery");
    expect(body2).toHaveTextContent("Ancient Burial Sites");

    // Check for tooltip elements with data-testid or class
    const tooltips = document.querySelectorAll(".event-log__action-card");
    expect(tooltips.length).toBeGreaterThanOrEqual(2);

    // Verify tooltips have title attributes
    tooltips.forEach((tooltip) => {
      expect(tooltip).toHaveAttribute("title");
      const title = tooltip.getAttribute("title");
      expect(title).toBeTruthy();
      expect(title?.length).toBeGreaterThan(0);
    });
  });
});
