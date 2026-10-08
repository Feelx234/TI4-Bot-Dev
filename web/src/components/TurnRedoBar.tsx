import React from "react";
import type { TurnRedoBusy } from "../hooks/useTurnRedo.ts";
import {
  findActionCardMeta,
  findExplorationCardMeta,
  findPublicObjectiveMeta,
  findSecretObjectiveMeta,
  humanizeId,
} from "../protocol/contentCatalog.ts";
import type { TurnRedoConflict, TurnRedoConflictKind, TurnRedoStatus } from "../protocol/turnRedo.ts";
import { usePlayerIdentity } from "../presentation/PlayerIdentity.tsx";

export interface TurnRedoBarProps {
  status: TurnRedoStatus | null;
  busy: TurnRedoBusy;
  error: string | null;
  onAutoplay: () => void;
  onRestore: () => void;
  onKeep: () => void;
}

/** What stopped the replay, in words that name no hidden card and no recorded choice. */
const REASON: Record<TurnRedoConflictKind, string> = {
  actor: "the game now asks a different player",
  prompt: "the game now asks a different question",
  context: "the situation is different now",
  chosen_not_offered: "the recorded answer is not available any more",
  options_changed: "the options on offer are different now",
  quantity_changed: "the amounts owed or available are different now",
  engine_ended: "the game ended before the recorded decisions did",
  engine_error: "the game could not continue with the recorded decisions",
  reserved_card: "a card that was drawn for a recorded decision is no longer in the deck",
};

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

const DECK_NOUN: Record<string, string> = {
  action_card: "action card",
  secret: "secret objective",
  objective: "public objective",
  relic: "relic",
  agenda: "agenda",
};

/** "the action card Sabotage" / "an action card": the card is absent for a hidden deck's draw. */
function reservedCardText(deck: string | null, card: string | null): string {
  const kind = deck?.startsWith("exploration:")
    ? `${deck.slice("exploration:".length)} exploration card`
    : ((deck && DECK_NOUN[deck]) ?? "card");
  if (!card) return `an ${kind}`.replace(/^an ([^aeiou])/, "a $1");
  const meta =
    deck === "action_card"
      ? findActionCardMeta(card)
      : deck === "secret"
        ? findSecretObjectiveMeta(card)
        : deck === "objective"
          ? findPublicObjectiveMeta(card)
          : deck?.startsWith("exploration:")
            ? findExplorationCardMeta(card)
            : undefined;
  return `the ${kind} ${meta?.name ?? humanizeId(card)}`;
}

/** Why the replay stopped at a conflict, as one clause. */
function conflictReason(
  conflict: TurnRedoConflict,
  name: (seat: string) => string,
): string {
  if (conflict.kind === "reserved_card") {
    const who = name(conflict.recipient ?? conflict.seat);
    return `${reservedCardText(conflict.deck, conflict.card)} reserved for ${who} is no longer in the deck`;
  }
  return REASON[conflict.kind];
}

/**
 * The strip for a turn redo in flight: waiting for the new turn, replaying the round, and where
 * the replay stopped. Restore and keep are for the host and the redoing seat; everyone else
 * only sees what is happening.
 */
export const TurnRedoBar: React.FC<TurnRedoBarProps> = ({
  status,
  busy,
  error,
  onAutoplay,
  onRestore,
  onKeep,
}) => {
  const display = usePlayerIdentity();
  const name = (seat: string) => display(seat).label;
  if (!status && !busy && !error) return null;

  let state = "idle";
  let title = "";
  let detail: React.ReactNode = null;
  let actions: "none" | "restore" | "decide" | "replay" = "none";
  const outcome = status?.outcome ?? null;

  if (!status) {
    state = busy === "restore" ? "restoring" : busy ? "rewinding" : "error";
    title =
      busy === "restore"
        ? "Restoring the original timeline…"
        : busy
          ? "Going back to the start of the turn…"
          : "The turn redo did not happen";
  } else if (busy === "autoplay" || (status.stage === "new_turn" && status.turn_complete)) {
    state = "replaying";
    title = "Replaying the round…";
    detail = `${name(status.seat)}'s new turn is done. The recorded decisions that followed are being replayed, with the original dice and the cards they drew, up to ${name(status.seat)}'s next turn.`;
    actions = busy ? "none" : "replay";
  } else if (status.stage === "new_turn") {
    state = "new-turn";
    title = `Redoing ${name(status.seat)}'s ${status.turns_back === 2 ? "last two turns" : "last turn"}`;
    detail =
      "Play the new turn. When it ends, the recorded decisions that followed replay on top of it, with the original dice and the cards they drew, and stop when your next turn begins.";
    actions = "restore";
  } else if (outcome?.stop.kind === "conflict") {
    const { conflict } = outcome.stop;
    state = "conflict";
    title = "The replay stopped";
    detail = `${plural(outcome.kept, "recorded decision was", "recorded decisions were")} kept, then ${conflictReason(conflict, name)}. ${name(conflict.seat)} decides "${conflict.prompt}" now.`;
    actions = "decide";
  } else if (outcome?.stop.kind === "handoff") {
    state = "handoff";
    title = `Back to ${name(outcome.stop.seat)}: the next turn begins`;
    detail = `${plural(outcome.kept, "recorded decision", "recorded decisions")} ${outcome.kept === 1 ? "was" : "were"} replayed and kept, including that seat's own reactions and votes. Nothing more is replayed; the rest of the original stays in the saved timeline.`;
    actions = "decide";
  } else {
    state = "complete";
    title = "The recorded round was replayed";
    detail = `All ${plural(outcome?.kept ?? 0, "recorded decision", "recorded decisions")} that followed the redone turn were kept.`;
    actions = "decide";
  }

  const control = status?.can_control ?? false;

  return (
    <div
      className="turn-redo"
      role="status"
      aria-live="polite"
      aria-busy={busy !== null || state === "replaying"}
      data-testid="turn-redo-bar"
      data-state={state}
    >
      <div className="turn-redo__text">
        <strong className="turn-redo__title">{title}</strong>
        {detail && <span className="turn-redo__detail">{detail}</span>}
        {error && (
          <span className="turn-redo__error text-danger" data-testid="turn-redo-error">
            {error}
          </span>
        )}
      </div>
      {control && (
        <div className="turn-redo__actions">
          {actions === "replay" && (
            <button
              type="button"
              className="button button--secondary button--sm"
              data-testid="turn-redo-replay"
              onClick={onAutoplay}
            >
              Replay the round now
            </button>
          )}
          {(actions === "restore" || actions === "decide" || actions === "replay") && (
            <button
              type="button"
              className="button button--secondary button--sm"
              data-testid="turn-redo-restore"
              disabled={busy !== null}
              onClick={onRestore}
            >
              Restore original timeline
            </button>
          )}
          {actions === "decide" && (
            <button
              type="button"
              className="button button--primary button--sm"
              data-testid="turn-redo-keep"
              disabled={busy !== null}
              onClick={onKeep}
            >
              {state === "conflict" ? "Continue from here" : "Keep this timeline"}
            </button>
          )}
        </div>
      )}
    </div>
  );
};
