import { useRef } from "react";
import { pausedDecline } from "../presentation/pausedOffer.ts";
import type { TurnContext } from "../presentation/promptTurn.ts";
import { isPausedNow, readModeFor } from "./promptSettings.ts";
import { useOfferAutoDecline, type OfferAutoDeclineInput } from "./useOfferAutoDecline.ts";

export interface PausedPromptAutoDeclineInput extends OfferAutoDeclineInput {
  /** Whose turn it is, for the abilities set to "Only in my turn"; absent means nobody's. */
  turn?: { phase?: string | null; activePlayer?: string | null } | null;
}

/**
 * With an ability's "ask" switch off, declines its offer for the viewer's own seat (every
 * registered ability, see `PAUSABLE_PROMPTS`). The engine still asks and the decline is an ordinary
 * recorded `submit_choice`; the safety rules are those of `useOfferAutoDecline` (only after a
 * forward move, never after undo, redo, restore or load) and of `pausedDecline` (only when every
 * non-decline answer of the question is a paused ability).
 *
 * An ability set to "Only in my turn" counts as paused while another seat is the active player of
 * the action phase, and as asked in every other situation (see `skipsNow`).
 */
export function usePausedPromptAutoDecline(input: PausedPromptAutoDeclineInput) {
  const turnRef = useRef(input.turn);
  turnRef.current = input.turn;
  const isPaused = (id: string) => {
    const turn: TurnContext = { viewerSeat: input.viewerSeat, ...turnRef.current };
    return isPausedNow(id, turn);
  };
  useOfferAutoDecline(input, {
    declineOption: (choice, viewerSeat) => pausedDecline(choice, viewerSeat, isPaused)?.optionId ?? null,
    readAsk: (choice) => pausedDecline(choice, input.viewerSeat, isPaused) === null,
    noticePrefix: (choice) => pausedDecline(choice, input.viewerSeat, isPaused)?.prompts[0]?.id ?? "prompt",
    noticeText: (choice) => {
      const prompts = pausedDecline(choice, input.viewerSeat, isPaused)?.prompts ?? [];
      const labels = prompts.map((p) => p.label);
      const why = prompts.length > 0 && prompts.every((p) => readModeFor(p.id) === "ownturn") ? "not your turn" : "switched off";
      return `${labels.join(", ") || "Prompt"} skipped (${why})`;
    },
  });
}
