import { describe, it, expect, beforeEach, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { ASK_AETHERPASSAGE_KEY, readAskAetherpassage, useAetherpassageSetting } from "./useAetherpassageSetting.ts";
import { aetherpassageDeclineOption } from "../presentation/aetherpassageOffer.ts";
import { useAetherpassageAutoDecline, AETHERPASSAGE_DECLINE_DELAY_MS } from "./useAetherpassageAutoDecline.ts";
import { ASK_I48S_KEY, readAskI48s } from "./useI48sSetting.ts";
import type { PendingChoiceDto } from "../protocol/types.ts";

const AP = "ability:empyrean:aetherpassage:SYSTEM_ACTIVATED:after";
const offer = (actor = "p1", nonce = "n1", extra: unknown[] = []): PendingChoiceDto =>
  ({
    prompt: "after SYSTEM_ACTIVATED",
    actor,
    nonce,
    context: { subtype: "reaction_after_SYSTEM_ACTIVATED", optional: true },
    options: [
      { id: AP, label: "Aetherpassage (after system activated)", kind: "ability" },
      ...extra,
      { id: "decline", label: "Pass", kind: "decline" },
    ],
  }) as unknown as PendingChoiceDto;

describe("Aetherpassage setting", () => {
  beforeEach(() => localStorage.clear());

  it("asks by default, toggles and persists, independent of I48S", () => {
    expect(readAskAetherpassage()).toBe(true);
    const a = renderHook(() => useAetherpassageSetting());
    const b = renderHook(() => useAetherpassageSetting());
    act(() => a.result.current.toggle());
    expect(localStorage.getItem(ASK_AETHERPASSAGE_KEY)).toBe("false");
    expect(b.result.current.ask).toBe(false);
    expect(readAskI48s()).toBe(true);
    expect(localStorage.getItem(ASK_I48S_KEY)).toBeNull();
    act(() => b.result.current.toggle());
    expect(a.result.current.ask).toBe(true);
  });

  it("finds the decline only for the owner's own, sole Aetherpassage offer", () => {
    expect(aetherpassageDeclineOption(offer(), "p1")).toBe("decline");
    expect(aetherpassageDeclineOption(offer(), "p2")).toBeNull();
    expect(aetherpassageDeclineOption(offer(), null)).toBeNull();
    const shared = offer("p1", "n", [{ id: "leader:l1z1x:l1z1xagent:SYSTEM_ACTIVATED:after", label: "I48S", kind: "ability" }]);
    expect(aetherpassageDeclineOption(shared, "p1")).toBeNull();
  });

  it("declines once when off and the game moved forward; asks after a load or when on", () => {
    vi.useFakeTimers();
    const submit = vi.fn().mockResolvedValue(undefined);
    const h = (cursor: number, choice: PendingChoiceDto | null) =>
      ({ choice, viewerSeat: "p1", history: { cursor, redo_count: 0, generation: 0 }, submit }) as never;
    let r = renderHook((p) => useAetherpassageAutoDecline(p), { initialProps: h(1, null) });
    r.rerender(h(2, offer("p1", "a")));
    act(() => void vi.advanceTimersByTime(AETHERPASSAGE_DECLINE_DELAY_MS + 10));
    expect(submit).not.toHaveBeenCalled();
    localStorage.setItem(ASK_AETHERPASSAGE_KEY, "false");
    r = renderHook((p) => useAetherpassageAutoDecline(p), { initialProps: h(2, null) });
    r.rerender(h(3, offer("p1", "b")));
    act(() => void vi.advanceTimersByTime(AETHERPASSAGE_DECLINE_DELAY_MS + 10));
    expect(submit).toHaveBeenCalledWith("decline", "b");
    submit.mockClear();
    renderHook((p) => useAetherpassageAutoDecline(p), { initialProps: h(3, offer("p1", "c")) });
    act(() => void vi.advanceTimersByTime(AETHERPASSAGE_DECLINE_DELAY_MS + 10));
    expect(submit).not.toHaveBeenCalled();
    vi.useRealTimers();
  });
});
