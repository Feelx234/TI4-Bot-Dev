import React, { useEffect, useMemo, useState } from "react";
import type {
  BoardView,
  GameEvent,
  HistoryStatus,
  PendingChoiceDto,
  PlayerView,
} from "../protocol/types.ts";
import type { BasketPlan } from "../protocol/client.ts";
import type { TokenStep } from "../presentation/commandTokens.ts";
import {
  detectStrategicAction,
  prepareEligibility,
} from "../presentation/strategicAction.ts";
import { isSecondaryQuestion, resolveStep, describePlan } from "../presentation/secondaryPlan.ts";
import { usePreparedPlan } from "../hooks/useSecondaryPlan.ts";
import { useSecondaryAutoPlay } from "../hooks/useSecondaryAutoPlay.ts";
import { SecondaryPrepPanel } from "./SecondaryPrepPanel.tsx";
import "./SecondaryPrep.css";

export interface SecondaryPrepHostProps {
  gameId?: string;
  viewerSeat?: string | null;
  players: readonly PlayerView[];
  events: readonly GameEvent[];
  board?: BoardView;
  phase?: string;
  activePlayer?: string | null;
  history?: HistoryStatus;
  choice: PendingChoiceDto | null;
  /** A pipeline or a history change is busy: nothing is sent on the player's behalf. */
  busy?: boolean;
  onSubmitChoice: (optionId: string) => Promise<void>;
  onSubmitBasketBatch?: (plan: BasketPlan) => Promise<void>;
  /** Open the panel at once (screenshots). */
  defaultOpen?: boolean;
}

/**
 * Everything about prepared strategy-card secondaries, on the viewer's side only:
 *  - while another seat resolves a card and the viewer has not been asked: the "Prepare your
 *    secondary" panel (no public "ready" signal; nothing leaves the device);
 *  - when the viewer's real question arrives: the prepared answer, validated against the options
 *    the engine really offers, shown for a one-click confirm ("Review", the default) or, in auto
 *    mode, sent after a short cancellable delay;
 *  - "Needs review" instead of either, when the prepared answer no longer validates.
 * Spectators see nothing.
 */
export const SecondaryPrepHost: React.FC<SecondaryPrepHostProps> = ({
  gameId,
  viewerSeat,
  players,
  events,
  board,
  phase,
  activePlayer,
  history,
  choice,
  busy,
  onSubmitChoice,
  onSubmitBasketBatch,
  defaultOpen,
}) => {
  const generation = history?.generation ?? 0;
  const action = useMemo(
    () => detectStrategicAction({ events, players, activePlayer, phase }),
    [events, players, activePlayer, phase],
  );
  const eligibility = prepareEligibility(action, viewerSeat, players, events);
  const ready = events.length > 0 && players.length > 0;
  const { plan, set, clear } = usePreparedPlan({
    gameId,
    viewerSeat,
    actionKey: action?.key ?? null,
    generation,
    ready,
  });

  // The follow-up prompts (technology, planets, site) belong to the plan only once its own window
  // opened in this action: a decision seen after a page load, or any other research prompt, is not
  // answered by it.
  const [openedFor, setOpenedFor] = useState<string | null>(null);
  const mine = Boolean(choice && viewerSeat && choice.actor === viewerSeat);
  const secondaryOpen = Boolean(
    mine && choice && action && isSecondaryQuestion(choice) && choice.details?.card === action.card,
  );
  useEffect(() => {
    if (secondaryOpen && action) setOpenedFor(action.key);
  }, [secondaryOpen, action]);

  const applicable = secondaryOpen || (action !== null && openedFor === action.key);
  const resolution = useMemo(
    () => (plan && applicable ? resolveStep(plan, choice, viewerSeat) : { kind: "none" as const }),
    [plan, applicable, choice, viewerSeat],
  );

  const submitTokens = onSubmitBasketBatch
    ? (steps: TokenStep[]) => onSubmitBasketBatch({ kind: "tokens", steps })
    : undefined;
  const { pending, cancel } = useSecondaryAutoPlay({
    choice,
    viewerSeat,
    history,
    busy,
    resolution,
    submitOption: onSubmitChoice,
    submitTokens,
  });

  const [dismissed, setDismissed] = useState<string | null>(null);
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const nonce = mine && choice ? choice.nonce : null;

  const confirm = async () => {
    if (resolution.kind !== "option" && resolution.kind !== "tokens") return;
    setSending(true);
    setError(null);
    try {
      if (resolution.kind === "option") await onSubmitChoice(resolution.optionId);
      else if (submitTokens) await submitTokens(resolution.steps);
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : String(failure));
    } finally {
      setSending(false);
    }
  };

  if (!viewerSeat) return null;

  const showPanel = action !== null && eligibility.canPrepare && !mine;
  const actionable = resolution.kind === "option" || resolution.kind === "tokens";
  const showBar = nonce !== null && dismissed !== nonce && !pending && (actionable || resolution.kind === "review");

  if (!showPanel && !showBar && !pending) return null;
  return (
    <div className="secondary-prep" data-testid="secondary-prep">
      {showPanel && action && (
        <SecondaryPrepPanel
          action={action}
          viewer={players.find((player) => player.id === viewerSeat)}
          board={board}
          plan={plan}
          onChange={set}
          onClear={clear}
          defaultOpen={defaultOpen}
        />
      )}
      {pending && (
        <div className="secondary-prep__bar" role="status" data-testid="secondary-autoplay-toast">
          <span className="secondary-prep__bar-text">
            Auto-playing your prepared secondary: <strong>{pending.text}</strong>
          </span>
          <span className="secondary-prep__bar-actions">
            <button type="button" className="button button--secondary button--sm" data-testid="secondary-autoplay-cancel" onClick={cancel}>
              Cancel
            </button>
          </span>
        </div>
      )}
      {showBar && actionable && plan && (
        <div className="secondary-prep__bar" data-testid="secondary-prepared-bar">
          <span className="secondary-prep__bar-text">
            <span className="secondary-prep__badge" style={{ marginRight: 8 }}>Prepared</span>
            <strong>{resolution.kind === "option" || resolution.kind === "tokens" ? resolution.text : ""}</strong>
            {error && <span role="alert"> {error}</span>}
          </span>
          <span className="secondary-prep__bar-actions">
            <button
              type="button"
              className="button button--primary button--sm"
              data-testid="secondary-prepared-confirm"
              disabled={sending || busy}
              onClick={() => void confirm()}
            >
              Confirm
            </button>
            <button
              type="button"
              className="button button--secondary button--sm"
              data-testid="secondary-prepared-dismiss"
              onClick={() => setDismissed(nonce)}
            >
              Choose myself
            </button>
          </span>
        </div>
      )}
      {showBar && resolution.kind === "review" && (
        <div className="secondary-prep__bar secondary-prep__bar--review" data-testid="secondary-needs-review">
          <span className="secondary-prep__bar-text">
            <span className="secondary-prep__badge secondary-prep__badge--review" style={{ marginRight: 8 }}>
              Needs review
            </span>
            {resolution.reason}
            {plan && <span className="secondary-prep__note"> (You prepared: {describePlan(plan)}.)</span>}
          </span>
          <span className="secondary-prep__bar-actions">
            <button type="button" className="button button--secondary button--sm" data-testid="secondary-prepared-clear" onClick={clear}>
              Clear prepared
            </button>
          </span>
        </div>
      )}
    </div>
  );
};
