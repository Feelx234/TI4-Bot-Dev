import type { GameEvent, PlayerView } from "../protocol/types.ts";
import { findStrategyCardMeta } from "../protocol/contentCatalog.ts";

/** The strategy cards whose secondary can be prepared. Printed names, lower-case. */
export type CardFamily =
  | "leadership"
  | "diplomacy"
  | "politics"
  | "construction"
  | "trade"
  | "warfare"
  | "technology"
  | "imperial";

const FAMILIES: readonly CardFamily[] = [
  "leadership",
  "diplomacy",
  "politics",
  "construction",
  "trade",
  "warfare",
  "technology",
  "imperial",
];

/** `pok7technology` and `te4construction` are both a known family; an unknown card is `null`. */
export function cardFamily(cardId: string): CardFamily | null {
  const name = findStrategyCardMeta(cardId)?.name?.toLowerCase();
  return FAMILIES.find((family) => family === name) ?? null;
}

export function cardName(cardId: string): string {
  return findStrategyCardMeta(cardId)?.name ?? cardId;
}

/** A strategic action the client can see is still being resolved. */
export interface StrategicAction {
  /** Identity of this action: round plus the decision-cursor id the server gives its events. */
  key: string;
  /** The server's action id (`action_<n>`), shared by every event of the action. */
  actionId: string;
  round: number;
  primary: string;
  card: string;
  cardName: string;
  family: CardFamily | null;
  /** True when the card could only be inferred (a one-card holding), not read from the log. */
  inferred: boolean;
}

export interface DetectInput {
  events: readonly GameEvent[] | undefined;
  players: readonly PlayerView[] | undefined;
  phase: string | undefined;
}

/** Coup d'Etat ends the primary's turn before anything resolves; the log says who played it. */
const COUP = /\bplayed Coup d.Etat\b/i;

/**
 * Which strategic action is in progress, from data the client already receives and nothing else.
 *
 * The public turn status does not name the card, so this reads the public event log: the action
 * selection is a published decision whose `action_type` is `strategic` and whose `detail` reads
 * "<seat> played <Card>" when the seat held several cards. A seat holding one card chooses the bare
 * `strategic` option, and the server's detail for it is generic, so the card is then the primary
 * seat's single unexhausted strategy card.
 *
 * "In progress" means: the action phase, the latest action in the log is a strategic action, its
 * primary's card is not exhausted yet (the engine exhausts it when the last follower is recorded),
 * and nobody cancelled it with Coup d'Etat (which leaves the card unexhausted).
 *
 * The primary comes from the log, never from `view.active_player`: that is the seat the engine is
 * asking right now, which is a follower, or the primary's own sub-question, during the action. Using
 * it made the action vanish whenever a follower was asked, which dropped every prepared plan just
 * before its window opened.
 */
export function detectStrategicAction({
  events,
  players,
  phase,
}: DetectInput): StrategicAction | null {
  if (phase !== "action" || !events?.length || !players?.length) return null;
  let last: GameEvent | undefined;
  for (let i = events.length - 1; i >= 0; i--) {
    if (events[i].action_id) {
      last = events[i];
      break;
    }
  }
  if (!last?.action_id || last.action_type !== "strategic") return null;
  const primarySeat = last.action_actor ?? last.actor;
  if (!primarySeat) return null;
  const actionId = last.action_id;
  if (events.some((event) => event.action_id === actionId && event.detail && COUP.test(event.detail))) {
    return null;
  }
  const primary = players.find((player) => player.id === primarySeat);
  if (!primary) return null;
  const unexhausted = primary.strategy_cards.filter(
    (card) => !primary.exhausted_strategy_cards.includes(card),
  );
  const prefix = `${primarySeat} played `;
  const named = events
    .filter((event) => event.action_id === actionId && event.detail?.startsWith(prefix))
    .map((event) => event.detail!.slice(prefix.length).trim())
    .at(0);
  let card: string | undefined;
  let inferred = false;
  if (named) {
    card = unexhausted.find((id) => findStrategyCardMeta(id)?.name === named);
  } else if (unexhausted.length === 1) {
    card = unexhausted[0];
    inferred = true;
  }
  if (!card) return null;
  const round = last.round ?? 0;
  return {
    key: `${round}:${actionId}:${primarySeat}:${card}`,
    actionId,
    round,
    primary: primarySeat,
    card,
    cardName: cardName(card),
    family: cardFamily(card),
    inferred,
  };
}

export interface Eligibility {
  canPrepare: boolean;
  /** Why not, for the UI and tests. */
  reason?: "no_seat" | "primary" | "asked" | "no_token" | "unknown_card";
}

/**
 * Whether the viewer may prepare a secondary: a seated follower who has not been asked yet and
 * could pay (a strategy token; Leadership needs none). A seat with no token but a faction waiver
 * is not detected, because the client cannot see waivers; it is simply not offered the panel.
 */
export function prepareEligibility(
  action: StrategicAction | null,
  viewerSeat: string | null | undefined,
  players: readonly PlayerView[] | undefined,
  events: readonly GameEvent[] | undefined,
): Eligibility {
  if (!action || !action.family) return { canPrepare: false, reason: "unknown_card" };
  if (!viewerSeat) return { canPrepare: false, reason: "no_seat" };
  if (viewerSeat === action.primary) return { canPrepare: false, reason: "primary" };
  const viewer = players?.find((player) => player.id === viewerSeat);
  if (!viewer) return { canPrepare: false, reason: "no_seat" };
  // Every decision of the action the viewer has answered is in the log; once there is one, the
  // viewer has been asked.
  if (events?.some((event) => event.action_id === action.actionId && event.actor === viewerSeat)) {
    return { canPrepare: false, reason: "asked" };
  }
  if (action.family !== "leadership" && viewer.strategic_tokens < 1) {
    return { canPrepare: false, reason: "no_token" };
  }
  return { canPrepare: true };
}
