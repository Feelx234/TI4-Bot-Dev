import { pausedDecline } from "../presentation/pausedOffer.ts";
import { isPausedNow } from "./promptSettings.ts";
import { useOfferAutoDecline, type OfferAutoDeclineInput } from "./useOfferAutoDecline.ts";

/**
 * With an ability's "ask" switch off, declines its offer for the viewer's own seat (every
 * registered ability, see `PAUSABLE_PROMPTS`). The engine still asks and the decline is an ordinary
 * recorded `submit_choice`; the safety rules are those of `useOfferAutoDecline` (only after a
 * forward move, never after undo, redo, restore or load) and of `pausedDecline` (only when every
 * non-decline answer of the question is a paused ability).
 */
export function usePausedPromptAutoDecline(input: OfferAutoDeclineInput) {
  useOfferAutoDecline(input, {
    declineOption: (choice, viewerSeat) => pausedDecline(choice, viewerSeat, isPausedNow)?.optionId ?? null,
    readAsk: (choice) => pausedDecline(choice, input.viewerSeat, isPausedNow) === null,
    noticePrefix: (choice) => pausedDecline(choice, input.viewerSeat, isPausedNow)?.prompts[0]?.id ?? "prompt",
    noticeText: (choice) => {
      const labels = pausedDecline(choice, input.viewerSeat, isPausedNow)?.prompts.map((p) => p.label) ?? [];
      return `${labels.join(", ") || "Prompt"} skipped (switched off)`;
    },
  });
}
