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
