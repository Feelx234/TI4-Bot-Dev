import { describe, it, expect, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import type { AutoResolvedNote, GameEvent } from "../protocol/types.ts";
import { useCornerToasts } from "./useCornerToasts.ts";
import type { AgendaRecordDto } from "../presentation/agendaToasts.ts";
import { TOAST_MUTE_KEY } from "./useToastMute.ts";

const ev = (id: string, actor: string, detail: string): GameEvent =>
  ({
    id,
    timestamp: "1",
    visibility: { visibility: "public" as const },
    event: { kind: "decision_resolved" },
    actor,
    detail: `${actor} ${detail}`,
  }) as GameEvent;

type Props = Parameters<typeof useCornerToasts>[0];
const base: Props = { events: [], viewerSeat: "p1", ready: true };

describe("useCornerToasts", () => {
  beforeEach(() => localStorage.clear());

  it("does not toast what was already in the log when the game loaded", () => {
    const { result } = renderHook((p: Props) => useCornerToasts(p), {
      initialProps: { ...base, events: [ev("a", "p2", "played Sabotage")] },
    });
    expect(result.current.notifications).toHaveLength(0);
  });

  it("toasts a finished exploration for the viewer too, but not the ones present at load", () => {
    const done = { player: "p1", deck: "HAZARDOUS", card: "cm1", planet: "mehar_xull", outcome: "resolved", result: "gained 1 trade good" };
    const { result, rerender } = renderHook((p: Props) => useCornerToasts(p), {
      initialProps: { ...base, explorations: [done] },
    });
    expect(result.current.notifications).toHaveLength(0);
    rerender({ ...base, explorations: [done, { ...done, player: "p2", card: "cm2" }] });
    expect(result.current.notifications).toHaveLength(1);
    expect(result.current.notifications[0]).toMatchObject({ kind: "action", actor: "p2" });
    expect(result.current.notifications[0].text).toContain("gained 1 trade good");
  });

  it("toasts a finished agenda vote once, for every seat including the voters", () => {
    const first: AgendaRecordDto = { round: 3, agenda: "mutiny", name: "Mutiny", outcome: "for", votes: { for: 7, against: 3 } };
    const second: AgendaRecordDto = { round: 3, agenda: "minister_peace", name: "Minister of Peace", outcome: "p1", votes: { p1: 5, p2: 2 } };
    // The viewer p1 voted on both and is the one elected: the toast is still news to them.
    const { result, rerender } = renderHook((p: Props) => useCornerToasts(p), {
      initialProps: { ...base, agendas: [first] },
    });
    expect(result.current.notifications).toHaveLength(0);
    rerender({ ...base, agendas: [first, second] });
    expect(result.current.notifications).toHaveLength(1);
    expect(result.current.notifications[0]).toMatchObject({ kind: "agenda", tag: "Agenda" });
    expect(result.current.notifications[0].text).toContain("Minister of Peace:");
    expect(result.current.notifications[0].text).toContain("elected");
    // A new state with the same log does not toast again.
    rerender({ ...base, agendas: [first, second] });
    expect(result.current.notifications).toHaveLength(1);
    // Undo/redo gives a new history generation: what reappears is history.
    rerender({ ...base, agendas: [first], historyGeneration: 1 });
    rerender({ ...base, agendas: [first, second], historyGeneration: 2 });
    expect(result.current.notifications).toHaveLength(1);
    // A spectator sees it too.
    const spectator = renderHook((p: Props) => useCornerToasts(p), {
      initialProps: { ...base, viewerSeat: null, agendas: [] as AgendaRecordDto[] },
    });
    spectator.rerender({ ...base, viewerSeat: null, agendas: [first] });
    expect(spectator.result.current.notifications[0].text).toContain("Mutiny: For wins");
  });

  it("waits for the first snapshot before treating the log as history", () => {
    const { result, rerender } = renderHook((p: Props) => useCornerToasts(p), {
      initialProps: { ...base, ready: false },
    });
    rerender({ ...base, ready: true, events: [ev("a", "p2", "played Sabotage")] });
    expect(result.current.notifications).toHaveLength(0);
    rerender({ ...base, ready: true, events: [ev("a", "p2", "played Sabotage"), ev("b", "p3", "activated #22")] });
    expect(result.current.notifications).toHaveLength(1);
    expect(result.current.notifications[0]).toMatchObject({ kind: "action", actor: "p3", text: "activated system #22" });
  });

  it("toasts other players only, merges a burst and bounds the stack", () => {
    const { result, rerender } = renderHook((p: Props) => useCornerToasts(p), { initialProps: base });
    const events = [
      ev("1", "p1", "played Sabotage"),
      ev("2", "p2", "moved cruiser from #1 to #22"),
      ev("3", "p2", "moved carrier from #3 to #22"),
      ev("4", "p3", "activated #5"),
      ev("5", "p4", "played Direct Hit"),
      ev("6", "p5", "played Shields Holding"),
    ];
    rerender({ ...base, events });
    const toasts = result.current.notifications;
    expect(toasts.map((t) => t.actor)).toEqual(["p3", "p4", "p5"]);
    rerender({ ...base, events: events.slice(0, 3) });
    expect(result.current.notifications.map((t) => t.actor)).toEqual(["p3", "p4", "p5"]);
  });

  it("merges two moves by one player into one toast", () => {
    const { result, rerender } = renderHook((p: Props) => useCornerToasts(p), { initialProps: base });
    rerender({
      ...base,
      events: [ev("2", "p2", "moved cruiser from #1 to #22"), ev("3", "p2", "moved carrier from #3 to #22")],
    });
    expect(result.current.notifications).toHaveLength(1);
    expect(result.current.notifications[0].text).toBe("moved 2 units");
  });

  it("spectators get toasts for every player", () => {
    const { result, rerender } = renderHook((p: Props) => useCornerToasts(p), {
      initialProps: { ...base, viewerSeat: undefined },
    });
    rerender({ ...base, viewerSeat: undefined, events: [ev("1", "p1", "played Sabotage")] });
    expect(result.current.notifications).toHaveLength(1);
  });

  it("shows nothing while muted, and clears what is showing when muted", () => {
    const { result, rerender } = renderHook((p: Props) => useCornerToasts(p), { initialProps: base });
    rerender({ ...base, events: [ev("1", "p2", "played Sabotage")] });
    expect(result.current.notifications).toHaveLength(1);
    act(() => {
      localStorage.setItem(TOAST_MUTE_KEY, "true");
      window.dispatchEvent(new Event("storage"));
    });
    expect(result.current.notifications).toHaveLength(0);
    rerender({ ...base, events: [ev("1", "p2", "played Sabotage"), ev("2", "p3", "activated #5")] });
    expect(result.current.notifications).toHaveLength(0);
  });

  it("reports a gain in another player's victory points", () => {
    const { result, rerender } = renderHook((p: Props) => useCornerToasts(p), {
      initialProps: { ...base, players: [{ id: "p2", victory_points: 1 }] },
    });
    rerender({ ...base, players: [{ id: "p2", victory_points: 2 }] });
    expect(result.current.notifications[0]).toMatchObject({ actor: "p2", text: "gained 1 VP (now 2)" });
  });

  it("keeps the auto-resolved single-choice toast, once per decision", () => {
    const choice = {
      prompt: "Strategy Card",
      actor: "p1",
      nonce: "n1",
      options: [{ id: "tech", label: "Technology", kind: "strategy_card", auto_resolved: true }],
    };
    const { result, rerender } = renderHook((p: Props) => useCornerToasts(p), {
      initialProps: { ...base, pendingChoice: choice },
    });
    expect(result.current.notifications).toHaveLength(1);
    expect(result.current.notifications[0]).toMatchObject({
      decisionType: "Strategy Card",
      selectedValue: "Technology",
    });
    rerender({ ...base, pendingChoice: { ...choice } });
    expect(result.current.notifications).toHaveLength(1);
  });

  it("toasts the server's auto-resolved notes once each, with the reason and a repeat count", () => {
    const note: AutoResolvedNote = { id: "auto-1", prompt: "pay 1 more resources", selected: "trade goods", reason: "only way", count: 3 };
    const { result, rerender } = renderHook((p: Props) => useCornerToasts(p), {
      initialProps: { ...base, autoResolved: [note] },
    });
    expect(result.current.notifications).toHaveLength(1);
    expect(result.current.notifications[0]).toMatchObject({
      decisionType: "pay 1 more resources",
      selectedValue: "trade goods \u00d73",
      reason: "only way",
    });
    // The same note on a later render, or a fresh array holding it, does not toast again.
    rerender({ ...base, autoResolved: [{ ...note }] });
    expect(result.current.notifications).toHaveLength(1);
    rerender({ ...base, autoResolved: [{ ...note }, { id: "auto-2", prompt: "q", selected: "x", reason: "" }] });
    expect(result.current.notifications).toHaveLength(2);
    expect(result.current.notifications[1].reason).toBeUndefined();
  });
});
