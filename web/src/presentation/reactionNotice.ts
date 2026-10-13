import type { BoardView, PendingChoiceDto } from "../protocol/types.ts";
import { decodeDecisionTrigger } from "../protocol/decode.ts";
import { factsFromTrigger, triggerSentence } from "./reactionModel.ts";
import { humanizeId } from "../protocol/contentCatalog.ts";
import { systemTag } from "./systemFacts.ts";

export interface ReactionCardNotice {
  /** Stable per trigger event, so re-sent snapshots of one window toast once. */
  id: string;
  text: string;
}

/**
 * The toast for a reaction window opened by another player's action card (Sabotage, Reverse
 * Engineer, ...): which card, who played or discarded it, its text, and every target the engine's
 * public trigger names (system, planet, player). The window's dialog is about the viewer's own
 * card; this keeps what it answers visible afterwards.
 */
export function reactionCardNotice(
  choice: PendingChoiceDto | null | undefined,
  viewerSeat: string | null | undefined,
  labels: { actor: (id: string) => string },
  board?: BoardView | null,
): ReactionCardNotice | null {
  if (!choice || !viewerSeat || choice.actor !== viewerSeat) return null;
  const trigger = decodeDecisionTrigger(choice.context?.trigger);
  if (!trigger || (trigger.kind !== "action_card_played" && trigger.kind !== "action_card_discarded")) return null;
  if (trigger.actor === viewerSeat) return null;
  const facts = factsFromTrigger(trigger);
  const system = (id: string) => systemTag(id, board);
  const parts = [triggerSentence(facts, trigger.relation, { actor: labels.actor, system, viewerSeat })];
  if (facts.card?.text) parts.push(facts.card.text);
  const targets: string[] = [];
  if (facts.systemId) targets.push(`system ${system(facts.systemId)}`);
  if (facts.planetId) targets.push(`planet ${humanizeId(facts.planetId)}`);
  if (facts.subjectId) targets.push(`player ${labels.actor(facts.subjectId)}`);
  if (targets.length) parts.push(`Target: ${targets.join(", ")}.`);
  return { id: `reactcard:${trigger.event_id}:${trigger.kind}`, text: parts.join(" ") };
}
