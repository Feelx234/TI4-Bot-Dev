import React from "react";
import type { TurnRedoBusy } from "../hooks/useTurnRedo.ts";
import type { TurnRedoConflictKind, TurnRedoStatus } from "../protocol/turnRedo.ts";
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
  deck_cursor: "the new turn drew a different number of cards, so later draws would land differently",
};

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

const deckText = (deltas: readonly { deck: string; delta: number }[]) =>
  deltas
    .map(
      ({ deck, delta }) =>
        `${deck.replace(/_/g, " ").replace(":", " ")} ${delta > 0 ? "+" : ""}${delta}`,
    )
    .join(", ");

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
    detail = `${name(status.seat)}'s new turn is done. The other seats' recorded decisions are being replayed with the original dice.`;
    // Restore stays while the replay request runs (disabled): taking it away and putting it back made
    // it flicker, and a click aimed at it could land on a detached element.
    actions = "replay";
  } else if (status.stage === "new_turn") {
    state = "new-turn";
    title = `Redoing ${name(status.seat)}'s ${status.turns_back === 2 ? "last two turns" : "last turn"}`;
    detail =
      "Play the new turn. When it ends, the other seats' recorded decisions replay on top of it, with the original dice.";
    actions = "restore";
  } else if (outcome?.stop.kind === "conflict") {
    const { conflict } = outcome.stop;
    state = "conflict";
    title = "The replay stopped";
    detail = `${plural(outcome.kept, "recorded decision was", "recorded decisions were")} kept, then ${REASON[conflict.kind]}. ${name(conflict.seat)} decides "${conflict.prompt}" now.`;
    actions = "decide";
  } else if (outcome?.stop.kind === "handoff") {
    state = "handoff";
    title = `Back to ${name(outcome.stop.seat)}`;
    detail = `${plural(outcome.kept, "decision", "decisions")} of the other seats ${outcome.kept === 1 ? "was" : "were"} replayed and kept.`;
    actions = "decide";
  } else {
    state = "complete";
    title = "The recorded round was replayed";
    detail = `All ${plural(outcome?.kept ?? 0, "decision", "decisions")} that followed the redone turn were kept.`;
    actions = "decide";
  }

  const offsets = outcome?.deck_offsets ?? [];
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
        {offsets.length > 0 && (
          <span className="turn-redo__note" data-testid="turn-redo-deck-offsets">
            Decks now sit a different number of cards from the original: {deckText(offsets)}.
          </span>
        )}
        {error && (
          <span className="turn-redo__error text-danger" data-testid="turn-redo-error">
            {error}
          </span>
        )}
      </div>
      {control && (
        <div className="turn-redo__actions">
          {actions === "replay" && busy === null && (
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
