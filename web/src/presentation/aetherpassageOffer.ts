import type { PendingChoiceDto } from "../protocol/types.ts";

/** Ability ids look like `ability:empyrean:aetherpassage:SYSTEM_ACTIVATED:after`. */
export function isAetherpassageOptionId(id: string): boolean {
  return /^ability:[^:]+:aetherpassage:/.test(id);
}

/** Whether the question offers Aetherpassage as one of its answers. */
export function isAetherpassageOffer(choice: PendingChoiceDto | null | undefined): boolean {
  return Boolean(choice?.options.some((o) => isAetherpassageOptionId(o.id)));
}

/**
 * The option that declines Aetherpassage when the client may answer for `viewerSeat`: the offer is
 * the viewer's own, and Aetherpassage is the only thing the decline would pass on (any other
 * ability in the same window keeps the question open for the player).
 */
export function aetherpassageDeclineOption(
  choice: PendingChoiceDto | null | undefined,
  viewerSeat: string | null | undefined,
): string | null {
  if (!choice || !viewerSeat || choice.actor !== viewerSeat || !isAetherpassageOffer(choice)) return null;
  const others = choice.options.filter((o) => o.kind !== "decline" && !isAetherpassageOptionId(o.id));
  if (others.length > 0) return null;
  return choice.options.find((o) => o.kind === "decline")?.id ?? null;
}
