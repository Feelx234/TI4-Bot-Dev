import { describe, it, expect, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import type { GameEvent } from "../protocol/types.ts";
import { useCornerToasts } from "./useCornerToasts.ts";
import { TOAST_MUTE_KEY } from "./useToastMute.ts";

const ev = (id: string, actor: string, detail: string): GameEvent =>
  ({
    id,
    timestamp: "1",
    visibility: "public",
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

  it("waits for the first snapshot before treating the log as history", () => {
    const { result, rerender } = renderHook((p: Props) => useCornerToasts(p), {
      initialProps: { ...base, ready: false },
    });
    rerender({ ...base, ready: true, events: [ev("a", "p2", "played Sabotage")] });
    expect(result.current.notifications).toHaveLength(0);
    rerender({ ...base, ready: true, events: [ev("a", "p2", "played Sabotage"), ev("b", "p3", "activated #22")] });
    expect(result.current.notifications).toHaveLength(1);
    expect(result.current.notifications[0]).toMatchObject({ kind: "action", actor: "p3", text: "activated system 22" });
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
});
