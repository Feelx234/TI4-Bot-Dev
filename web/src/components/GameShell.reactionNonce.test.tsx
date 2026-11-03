import { act } from "react";
import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { ChoiceRendererDispatcher } from "./GameShell.tsx";
import { StaleChoiceError } from "../protocol/client.ts";
import type { PendingChoiceDto } from "../protocol/types.ts";
import { partialContext } from "../test/wire.ts";

// Nightly 2026-10-20 (o3-176-0551): the Play click of a reaction window went out without the
// question's nonce, so it landed on the next question ("Unknown option ...aetherpassage...").
const choice = (nonce: string): PendingChoiceDto => ({
  actor: "seat_1",
  nonce,
  prompt: "reaction_after_SYSTEM_ACTIVATED",
  context: partialContext({ subtype: "reaction_after_SYSTEM_ACTIVATED", optional: true, source: { Reaction: "SYSTEM_ACTIVATED" } }),
  options: [
    { id: "ability:empyrean:aetherpassage:SYSTEM_ACTIVATED:after", label: "Aetherpassage", kind: "reaction", payload: {} },
    { id: "decline", label: "Pass", kind: "decline", payload: {} },
  ],
});

describe("action_card_reaction answers carry the question nonce", () => {
  it("a Play click on a question that was replaced sends nothing and shows no error", async () => {
    let pending = "50";
    const sent: string[] = [];
    const onSubmit = vi.fn(async (id: string, nonce?: string) => {
      if (nonce !== pending) throw new StaleChoiceError(nonce ?? "", pending);
      sent.push(id);
    });
    const view = (c: PendingChoiceDto) => (
      <ChoiceRendererDispatcher choice={c} viewerSeat="seat_1" onSubmit={onSubmit} isMinimized={false} onMinimizedChange={vi.fn()} />
    );
    const { rerender } = render(view(choice("50")));
    const play = screen.getByTestId("play-reaction-btn-ability:empyrean:aetherpassage:SYSTEM_ACTIVATED:after");
    // The engine moved on, but the click handler of the old render is still the one that fires.
    pending = "51";
    await act(async () => {
      fireEvent.click(play);
    });
    expect(onSubmit).toHaveBeenCalledWith("ability:empyrean:aetherpassage:SYSTEM_ACTIVATED:after", "50");
    expect(sent).toEqual([]);
    expect(screen.queryByText(/already been answered|Rejected/i)).toBeNull();
    rerender(view(choice("51")));
  });
});

// Nightly 2026-10-25 (o1-29-0036, o2-133-0508, o3-34-0053, o3-57-0202, o4-26-0034, o4-135-0519,
// o1-78-0243): an ability set to "Don't ask" or "Only in my turn" is declined by the client, but its
// offer stayed clickable while that decline was being sent; a "Use" click then failed with
// "Another choice submission is still pending".
describe("a reaction window the client is declining", () => {
  it("keeps its buttons off while the decline is being sent", () => {
    const onSubmit = vi.fn(async () => undefined);
    const view = (autoDeclineNonce: string | null) => (
      <ChoiceRendererDispatcher
        choice={choice("50")}
        viewerSeat="seat_1"
        onSubmit={onSubmit}
        isMinimized={false}
        onMinimizedChange={vi.fn()}
        autoDeclineNonce={autoDeclineNonce}
      />
    );
    const { rerender } = render(view("50"));
    const play = screen.getByTestId("play-reaction-btn-ability:empyrean:aetherpassage:SYSTEM_ACTIVATED:after");
    expect(play).toBeDisabled();
    expect(screen.getByTestId("pass-reaction-btn")).toBeDisabled();
    fireEvent.click(play);
    expect(onSubmit).not.toHaveBeenCalled();
    // A decline of an older question leaves this one to the player.
    rerender(view("49"));
    expect(screen.getByTestId("play-reaction-btn-ability:empyrean:aetherpassage:SYSTEM_ACTIVATED:after")).toBeEnabled();
  });
});
