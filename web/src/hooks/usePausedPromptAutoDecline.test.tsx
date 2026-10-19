import { describe, it, expect, beforeEach, vi } from "vitest";
import { renderHook, act, render, screen, fireEvent } from "@testing-library/react";
import type { PendingChoiceDto } from "../protocol/types.ts";
import { usePausedPromptAutoDecline } from "./usePausedPromptAutoDecline.ts";
import { OFFER_DECLINE_DELAY_MS } from "./useOfferAutoDecline.ts";
import { askSettingFor, usePausedPrompts } from "./promptSettings.ts";
import { PromptSheetControls, MAX_SHEET_BUTTONS } from "../components/PromptSheetControls.tsx";
import { PromptSwitches } from "../components/PromptSwitch.tsx";
import { PausedPromptsChip } from "../components/PausedPromptsChip.tsx";

const AGENT = "leader:saar:saaragent:SYSTEM_ACTIVATED:after";
const offer = (nonce: string, actor = "p1", extra: unknown[] = []): PendingChoiceDto =>
  ({
    prompt: "after SYSTEM_ACTIVATED",
    actor,
    nonce,
    context: { subtype: "reaction_after_SYSTEM_ACTIVATED", optional: true },
    options: [{ id: AGENT, label: "Captain Mendosa", kind: "ability" }, ...extra, { id: "decline", label: "Pass", kind: "decline" }],
  }) as unknown as PendingChoiceDto;

const props = (cursor: number, choice: PendingChoiceDto | null, submit: () => Promise<void>, onNotice?: (n: { id: string; text: string }) => void) =>
  ({ choice, viewerSeat: "p1", history: { cursor, redo_count: 0, generation: 0 }, submit, onNotice }) as never;

describe("usePausedPromptAutoDecline", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.useRealTimers();
  });

  it("declines a newly registered ability once it is paused, with its toast, and never after a load or undo", () => {
    vi.useFakeTimers();
    const submit = vi.fn().mockResolvedValue(undefined);
    const notes: { id: string; text: string }[] = [];
    const onNotice = (n: { id: string; text: string }) => notes.push(n);

    let r = renderHook((p) => usePausedPromptAutoDecline(p), { initialProps: props(1, null, submit, onNotice) });
    r.rerender(props(2, offer("a"), submit, onNotice));
    act(() => void vi.advanceTimersByTime(OFFER_DECLINE_DELAY_MS + 10));
    expect(submit).not.toHaveBeenCalled();

    localStorage.setItem("player_ask_saar_agent", "false");
    r = renderHook((p) => usePausedPromptAutoDecline(p), { initialProps: props(2, null, submit, onNotice) });
    r.rerender(props(3, offer("b"), submit, onNotice));
    act(() => void vi.advanceTimersByTime(OFFER_DECLINE_DELAY_MS + 10));
    expect(submit).toHaveBeenCalledWith("decline", "b");
    expect(notes).toEqual([{ id: "saar_agent-b", text: "Captain Mendosa skipped (switched off)" }]);

    submit.mockClear();
    // A page load shows the offer at once: asked.
    renderHook((p) => usePausedPromptAutoDecline(p), { initialProps: props(3, offer("c"), submit) });
    act(() => void vi.advanceTimersByTime(OFFER_DECLINE_DELAY_MS + 10));
    expect(submit).not.toHaveBeenCalled();

    // An undo that lands on the offer: asked again, not declined.
    r = renderHook((p) => usePausedPromptAutoDecline(p), { initialProps: props(4, null, submit) });
    r.rerender(props(3, offer("d"), submit));
    act(() => void vi.advanceTimersByTime(OFFER_DECLINE_DELAY_MS + 10));
    expect(submit).not.toHaveBeenCalled();
  });

  it("leaves a window with another ability open and does not answer for another seat", () => {
    vi.useFakeTimers();
    localStorage.setItem("player_ask_saar_agent", "false");
    const submit = vi.fn().mockResolvedValue(undefined);
    const other = { id: "ability:empyrean:aetherpassage:SYSTEM_ACTIVATED:after", label: "Aether", kind: "ability" };
    let r = renderHook((p) => usePausedPromptAutoDecline(p), { initialProps: props(1, null, submit) });
    r.rerender(props(2, offer("a", "p1", [other]), submit));
    act(() => void vi.advanceTimersByTime(OFFER_DECLINE_DELAY_MS + 10));
    expect(submit).not.toHaveBeenCalled();
    r = renderHook((p) => usePausedPromptAutoDecline(p), { initialProps: props(2, null, submit) });
    r.rerender(props(3, offer("b", "p2"), submit));
    act(() => void vi.advanceTimersByTime(OFFER_DECLINE_DELAY_MS + 10));
    expect(submit).not.toHaveBeenCalled();
  });
});

describe("the prompt controls", () => {
  beforeEach(() => localStorage.clear());

  it("shows the offer's checkbox for a registered ability and switches it", () => {
    render(<PromptSwitches choice={offer("a")} />);
    const input = screen.getByTestId("saar_agent-switch-input") as HTMLInputElement;
    expect(input.checked).toBe(false);
    fireEvent.click(input);
    expect(localStorage.getItem("player_ask_saar_agent")).toBe("false");
    expect(askSettingFor("saar_agent").read()).toBe(false);
  });

  it("gives a seat with few abilities a button each, and keeps the I48S and Aetherpassage ids", () => {
    render(<PromptSheetControls holder={{ faction: "l1z1x" }} />);
    const button = screen.getByTestId("i48s-ask-btn");
    expect(button.textContent).toBe("I48S ask");
    fireEvent.click(button);
    expect(localStorage.getItem("player_ask_i48s")).toBe("false");
    expect(button.textContent).toBe("I48S skipped");
  });

  it("gives a seat with many abilities one Prompts button, and shows a paused foreign ability", () => {
    const { rerender } = render(<PromptSheetControls holder={{ faction: "empyrean" }} />);
    expect(screen.getByTestId("prompts-btn").textContent).toMatch(/^Prompts \(\d+\)$/);
    expect(screen.queryByTestId("aetherpassage-ask-btn")).toBeNull();
    // A seat with exactly the limit shows buttons; another seat's paused ability is listed too.
    rerender(<PromptSheetControls holder={{ faction: "hacan" }} />);
    expect(screen.queryByTestId("wrath_of_kenara-ask-btn")).not.toBeNull();
    localStorage.setItem("player_ask_saar_agent", "false");
    rerender(<PromptSheetControls holder={{ faction: "hacan" }} />);
    expect(screen.queryByTestId("saar_agent-ask-btn")).not.toBeNull();
    expect(MAX_SHEET_BUTTONS).toBe(3);
  });

  it("shows the chip only while something is skipped", () => {
    const Both = () => (
      <>
        <PausedPromptsChip />
        <span data-testid="count">{usePausedPrompts().length}</span>
      </>
    );
    render(<Both />);
    expect(screen.queryByTestId("paused-prompts-chip")).toBeNull();
    localStorage.setItem("player_ask_i48s", "false");
    act(() => window.dispatchEvent(new StorageEvent("storage")));
    expect(screen.getByTestId("paused-prompts-chip").textContent).toBe("Prompt skipped: I48S");
  });
});
