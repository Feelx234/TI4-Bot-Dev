import type { PendingChoiceDto } from "../protocol/types.ts";
import { contentSourceOf, promptForOption, promptsForSubtype, type PausablePrompt } from "./pausablePrompts.ts";

export interface PausedDecline {
  /** The option that declines the question. */
  optionId: string;
  /** The paused abilities this decline passes on (for the toast). */
  prompts: PausablePrompt[];
}

/**
 * Whether the client may answer `choice` with a decline because the viewer switched the asked
 * ability(ies) off. Only for the viewer's own question that carries a decline answer, and only when
 * everything the decline would pass on is paused:
 *
 * - a reaction window lists abilities as options: every option except the decline must be a
 *   registered, paused ability (any other ability or action card keeps the question open for the
 *   player);
 * - an ability that asks its own question (decision subtype) is declined when it is paused.
 *
 * `isPaused` reads the viewer's switch for an ability id.
 */
export function pausedDecline(
  choice: PendingChoiceDto | null | undefined,
  viewerSeat: string | null | undefined,
  isPaused: (id: string) => boolean,
): PausedDecline | null {
  if (!choice || !viewerSeat || choice.actor !== viewerSeat) return null;
  const decline = choice.options.find((o) => o.kind === "decline");
  if (!decline) return null;

  const offered = choice.options.filter((o) => o.kind !== "decline");
  if (offered.length > 0) {
    const matched = offered.map((o) => promptForOption(o.id));
    if (matched.every((m): m is PausablePrompt => Boolean(m) && isPaused(m!.id))) {
      const prompts = matched.filter((m, i): m is PausablePrompt => Boolean(m) && matched.indexOf(m) === i);
      return { optionId: decline.id, prompts };
    }
  }

  const bySubtype = promptsForSubtype(choice.context?.subtype, contentSourceOf(choice)).filter((p) => isPaused(p.id));
  if (bySubtype.length > 0) return { optionId: decline.id, prompts: bySubtype };
  return null;
}
