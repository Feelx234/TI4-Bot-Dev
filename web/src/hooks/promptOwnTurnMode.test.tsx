import { describe, it, expect, beforeEach, vi } from "vitest";
import { renderHook, act, render, screen, fireEvent } from "@testing-library/react";
import type { PendingChoiceDto } from "../protocol/types.ts";
import { createAskSetting, modeOfStored } from "./askSetting.ts";
import { usePausedPromptAutoDecline } from "./usePausedPromptAutoDecline.ts";
import { OFFER_DECLINE_DELAY_MS } from "./useOfferAutoDecline.ts";
import { askSettingFor, isPausedNow, readModeFor, usePausedPrompts } from "./promptSettings.ts";
import { PAUSABLE_PROMPTS, offersOwnTurnMode } from "../presentation/pausablePrompts.ts";
import { isAnotherSeatsTurn, skipsNow } from "../presentation/promptTurn.ts";
import { PromptSheetControls } from "../components/PromptSheetControls.tsx";
import { PromptSwitches } from "../components/PromptSwitch.tsx";
import { PausedPromptsChip } from "../components/PausedPromptsChip.tsx";
import { PromptsPanel } from "../components/PromptsPanel.tsx";
import { openPromptsPanel } from "./promptsPanel.ts";

const AGENT = "leader:saar:saaragent:SYSTEM_ACTIVATED:after";
const offer = (nonce: string): PendingChoiceDto =>
  ({
    prompt: "after SYSTEM_ACTIVATED",
    actor: "p1",
    nonce,
    context: { subtype: "reaction_after_SYSTEM_ACTIVATED", optional: true },
    options: [
      { id: AGENT, label: "Captain Mendosa", kind: "ability" },
      { id: "decline", label: "Pass", kind: "decline" },
    ],
  }) as unknown as PendingChoiceDto;

const KEY = "player_ask_saar_agent";

describe("the stored value of an ask setting", () => {
  beforeEach(() => localStorage.clear());

  it("keeps the values of the first version: true asks, false skips, nothing asks", () => {
    expect(modeOfStored(null)).toBe("ask");
    expect(modeOfStored("true")).toBe("ask");
    expect(modeOfStored("false")).toBe("skip");
    expect(modeOfStored("ownturn")).toBe("ownturn");
    expect(modeOfStored("garbage")).toBe("ask");
    localStorage.setItem(KEY, "false");
    expect(askSettingFor("saar_agent").read()).toBe(false);
    expect(readModeFor("saar_agent")).toBe("skip");
  });

  it("writes the first version's values for ask and skip, and ownturn for the third", () => {
    const { result } = renderHook(() => createAskSetting("player_ask_test_mode").useSetting());
    act(() => result.current.setMode("skip"));
    expect(localStorage.getItem("player_ask_test_mode")).toBe("false");
    act(() => result.current.setMode("ownturn"));
    expect(localStorage.getItem("player_ask_test_mode")).toBe("ownturn");
    expect(result.current.mode).toBe("ownturn");
    // The boolean reader of the first version asks in the third mode.
    expect(createAskSetting("player_ask_test_mode").read()).toBe(true);
    act(() => result.current.setMode("ask"));
    expect(localStorage.getItem("player_ask_test_mode")).toBe("true");
  });

  it("toggle goes skipped and back, cycle adds the third mode only where asked to", () => {
    const { result } = renderHook(() => createAskSetting("player_ask_test_cycle").useSetting());
    act(() => result.current.toggle());
    expect(result.current.mode).toBe("skip");
    act(() => result.current.toggle());
    expect(result.current.mode).toBe("ask");
    const seen: string[] = [];
    for (let i = 0; i < 4; i++) {
      act(() => result.current.cycle(true));
      seen.push(result.current.mode);
    }
    expect(seen).toEqual(["skip", "ownturn", "ask", "skip"]);
    act(() => result.current.setMode("ask"));
    act(() => result.current.cycle(false));
    act(() => result.current.cycle(false));
    expect(result.current.mode).toBe("ask");
    // From "only in my turn" the two-state toggle skips outright.
    act(() => result.current.setMode("ownturn"));
    act(() => result.current.toggle());
    expect(result.current.mode).toBe("skip");
  });
});

describe("whose turn skips", () => {
  const turn = (activePlayer: string | null, phase = "action", viewerSeat = "p1") => ({ viewerSeat, phase, activePlayer });

  it("only the action phase with another seat active is another seat's turn", () => {
    expect(isAnotherSeatsTurn(turn("p2"))).toBe(true);
    expect(isAnotherSeatsTurn(turn("p1"))).toBe(false);
    expect(isAnotherSeatsTurn(turn("p2", "status"))).toBe(false);
    expect(isAnotherSeatsTurn(turn("p2", "agenda"))).toBe(false);
    expect(isAnotherSeatsTurn(turn(null))).toBe(false);
    expect(isAnotherSeatsTurn(undefined)).toBe(false);
    expect(isAnotherSeatsTurn({ phase: "action", activePlayer: "p2" })).toBe(false);
  });

  it("skip always skips, ask never, ownturn only in another seat's turn and only where it exists", () => {
    expect(skipsNow("saar_agent", "skip", turn("p1"))).toBe(true);
    expect(skipsNow("saar_agent", "ask", turn("p2"))).toBe(false);
    expect(skipsNow("saar_agent", "ownturn", turn("p2"))).toBe(true);
    expect(skipsNow("saar_agent", "ownturn", turn("p1"))).toBe(false);
    expect(skipsNow("saar_agent", "ownturn", null)).toBe(false);
    // Bio-Stims only comes up at the end of the owner's own turn: a stale "ownturn" asks.
    expect(skipsNow("bio_stims", "ownturn", turn("p2"))).toBe(false);
  });

  it("a stored ownturn on an ability without the choice reads as ask", () => {
    localStorage.setItem("player_ask_bio_stims", "ownturn");
    expect(readModeFor("bio_stims")).toBe("ask");
    expect(isPausedNow("bio_stims", turn("p2"))).toBe(false);
    localStorage.clear();
  });
});

describe("which abilities have the choice", () => {
  const without = (turns: "own" | "others") => PAUSABLE_PROMPTS.filter((p) => p.turns === turns).map((p) => p.id);

  it("leaves it out where the ability comes up in one context only", () => {
    expect(without("own").sort()).toEqual(
      [
        "scanlink_drone_network",
        "spatial_conduit_cylinders",
        "bio_stims",
        "predictive_intelligence",
        "self_assembly_routines",
        "ai_development_algorithm",
        "agency_supply_network",
        "titans_commander",
        "hegemonic_trade_policy",
        "yin_spinner",
      ].sort(),
    );
    expect(without("others").sort()).toEqual(
      ["nullification_field", "arborec_commander", "empyrean_commander", "void_tether", "foresight", "yssaril_commander"].sort(),
    );
  });

  it("offers it for abilities that come up in your own and in others' turns", () => {
    for (const id of ["i48s", "saar_agent", "phoenix_standard", "exotrireme", "crown_of_thalnos", "laws_order", "winnu_agent"]) {
      expect(offersOwnTurnMode(PAUSABLE_PROMPTS.find((p) => p.id === id)!), id).toBe(true);
    }
    expect(PAUSABLE_PROMPTS.filter(offersOwnTurnMode).length).toBe(PAUSABLE_PROMPTS.length - 16);
  });
});

const props = (
  cursor: number,
  choice: PendingChoiceDto | null,
  submit: () => Promise<void>,
  turn: { phase: string; activePlayer: string | null },
  onNotice?: (n: { id: string; text: string }) => void,
) => ({ choice, viewerSeat: "p1", turn, history: { cursor, redo_count: 0, generation: 0 }, submit, onNotice }) as never;

describe("declining in other players' turns", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.useFakeTimers();
  });

  const run = (turn: { phase: string; activePlayer: string | null }) => {
    const submit = vi.fn().mockResolvedValue(undefined);
    const notes: { id: string; text: string }[] = [];
    const onNotice = (n: { id: string; text: string }) => notes.push(n);
    const r = renderHook((p) => usePausedPromptAutoDecline(p), { initialProps: props(1, null, submit, turn, onNotice) });
    r.rerender(props(2, offer("a"), submit, turn, onNotice));
    act(() => void vi.advanceTimersByTime(OFFER_DECLINE_DELAY_MS + 10));
    return { submit, notes };
  };

  it("declines while another seat has the turn, with its own toast", () => {
    localStorage.setItem(KEY, "ownturn");
    const { submit, notes } = run({ phase: "action", activePlayer: "p2" });
    expect(submit).toHaveBeenCalledWith("decline", "a");
    expect(notes).toEqual([{ id: "saar_agent-a", text: "Captain Mendosa skipped (not your turn)" }]);
  });

  it("asks in the viewer's own turn and outside the action phase", () => {
    localStorage.setItem(KEY, "ownturn");
    expect(run({ phase: "action", activePlayer: "p1" }).submit).not.toHaveBeenCalled();
    expect(run({ phase: "status", activePlayer: "p2" }).submit).not.toHaveBeenCalled();
  });

  it("a plain skip still declines in the viewer's own turn, with the old toast", () => {
    localStorage.setItem(KEY, "false");
    const { submit, notes } = run({ phase: "action", activePlayer: "p1" });
    expect(submit).toHaveBeenCalledWith("decline", "a");
    expect(notes[0].text).toBe("Captain Mendosa skipped (switched off)");
  });
});

describe("the controls with the third choice", () => {
  beforeEach(() => localStorage.clear());

  it("the offer shows a second checkbox that excludes the first", () => {
    render(<PromptSwitches choice={offer("a")} />);
    const skip = screen.getByTestId("saar_agent-switch-input") as HTMLInputElement;
    const own = screen.getByTestId("saar_agent-ownturn-input") as HTMLInputElement;
    fireEvent.click(own);
    expect(localStorage.getItem(KEY)).toBe("ownturn");
    expect(own.checked).toBe(true);
    expect(skip.checked).toBe(false);
    fireEvent.click(skip);
    expect(localStorage.getItem(KEY)).toBe("false");
    expect(own.checked).toBe(false);
    fireEvent.click(skip);
    expect(localStorage.getItem(KEY)).toBe("true");
  });

  it("has no second checkbox for an ability that only comes up in one context", () => {
    const bioStims = { ...offer("b"), context: { subtype: "bio_stims_ready" }, options: [{ id: "decline", label: "No", kind: "decline" }, { id: "x", label: "Ready", kind: "choice" }] };
    render(<PromptSwitches choice={bioStims as unknown as PendingChoiceDto} />);
    expect(screen.getByTestId("bio_stims-switch-input")).not.toBeNull();
    expect(screen.queryByTestId("bio_stims-ownturn-input")).toBeNull();
  });

  it("the sheet button cycles ask, skipped, my turn only, ask", () => {
    render(<PromptSheetControls holder={{ faction: "l1z1x" }} />);
    const button = screen.getByTestId("i48s-ask-btn");
    fireEvent.click(button);
    expect(button.textContent).toBe("I48S skipped");
    fireEvent.click(button);
    expect(button.textContent).toBe("I48S my turn only");
    expect(localStorage.getItem("player_ask_i48s")).toBe("ownturn");
    fireEvent.click(button);
    expect(button.textContent).toBe("I48S ask");
  });

  it("the sheet button of a one-context ability stays two-state", () => {
    render(<PromptSheetControls holder={{ technologies: ["bs"] }} />);
    const button = screen.getByTestId("bio_stims-ask-btn");
    fireEvent.click(button);
    expect(button.textContent).toBe("Bio-Stims skipped");
    fireEvent.click(button);
    expect(button.textContent).toBe("Bio-Stims ask");
  });

  it("the chip tells what is skipped in other players' turns", () => {
    const Both = () => (
      <>
        <PausedPromptsChip />
        <span data-testid="count">{usePausedPrompts().length}</span>
      </>
    );
    render(<Both />);
    act(() => localStorage.setItem("player_ask_i48s", "ownturn"));
    act(() => window.dispatchEvent(new StorageEvent("storage")));
    expect(screen.getByTestId("paused-prompts-chip").textContent).toBe("Prompt skipped in others' turns: I48S");
    act(() => localStorage.setItem("player_ask_i48s", "false"));
    act(() => window.dispatchEvent(new StorageEvent("storage")));
    expect(screen.getByTestId("paused-prompts-chip").textContent).toBe("Prompt skipped: I48S");
  });

  it("the Prompts list has three buttons where the choice exists and the old switch where not", () => {
    render(<PromptsPanel holder={{ faction: "l1z1x", technologies: ["bs"] }} />);
    act(() => openPromptsPanel());
    expect(screen.queryByTestId("prompt-toggle-i48s")).toBeNull();
    fireEvent.click(screen.getByTestId("prompt-mode-i48s-ownturn"));
    expect(localStorage.getItem("player_ask_i48s")).toBe("ownturn");
    expect(screen.getByTestId("prompt-mode-i48s-ownturn")).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByTestId("prompt-row-i48s")).toHaveAttribute("data-paused", "true");
    fireEvent.click(screen.getByTestId("prompt-mode-i48s-skip"));
    expect(localStorage.getItem("player_ask_i48s")).toBe("false");
    fireEvent.click(screen.getByTestId("prompt-mode-i48s-ask"));
    expect(localStorage.getItem("player_ask_i48s")).toBe("true");
    expect(screen.queryByTestId("prompt-mode-bio_stims-ownturn")).toBeNull();
    fireEvent.click(screen.getByTestId("prompt-toggle-bio_stims"));
    expect(localStorage.getItem("player_ask_bio_stims")).toBe("false");
  });
});
