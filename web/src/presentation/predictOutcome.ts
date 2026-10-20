import type { ChoiceOption, PendingChoiceDto } from "../protocol/types.ts";
import { findActionCardByName, findActionCardMeta, findPlanetMeta, humanizeId } from "../protocol/contentCatalog.ts";
import { playerPickSeat, type DecisionTable } from "./politicsDecision.ts";

/** A rider asking which outcome of the agenda being voted on to predict. */
export const isPredictOutcome = (choice: Pick<PendingChoiceDto, "context">): boolean =>
  choice.context?.subtype === "predict_agenda_outcome";

/** The rider as printed: "Imperial Rider: predict the agenda outcome" names the card before the colon. */
export function describeRider(
  choice: Pick<PendingChoiceDto, "context" | "prompt">,
): { name: string; text: string; step: "predict" | "reward" } | null {
  if (isPredictOutcome(choice)) {
    const name = choice.prompt.split(":")[0].trim();
    if (!name) return null;
    const meta = findActionCardByName(name);
    return { name: meta?.name ?? name, text: meta?.description ?? "", step: "predict" };
  }
  // A correct rider's reward that asks (Diplomacy Rider: which system): the card is the source.
  const source = choice.context?.source as Record<string, unknown> | undefined;
  const alias = typeof source?.ActionCard === "string" ? source.ActionCard : "";
  if (!/rider/.test(alias)) return null;
  const meta = findActionCardMeta(alias);
  return { name: meta?.name ?? humanizeId(alias), text: meta?.description ?? "", step: "reward" };
}

/**
 * An outcome in words: For/Against, an elected planet or player, or the humanised id. `null` for
 * any other decision.
 */
export function describePrediction(
  choice: Pick<PendingChoiceDto, "context">,
  option: Pick<ChoiceOption, "id" | "label">,
  table: DecisionTable | null,
): string | null {
  if (!isPredictOutcome(choice)) return null;
  const id = option.id;
  const lower = id.toLowerCase();
  if (lower === "for" || lower === "against") return `${id[0].toUpperCase()}${lower.slice(1)}`;
  const planet = findPlanetMeta(id);
  if (planet) return `${planet.name} (${planet.resources}R/${planet.influence}I)`;
  const seat = table ? playerPickSeat(option, table) : undefined;
  if (seat) return `${humanizeId(seat.faction)} (${seat.victory_points} VP)`;
  return humanizeId(id);
}
