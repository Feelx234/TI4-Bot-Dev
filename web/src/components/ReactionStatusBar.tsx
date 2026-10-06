import React, { useState, useEffect, useMemo } from "react";
import type { BoardView, GameEvent, PendingChoiceDto } from "../protocol/types.ts";
import { ChoiceRendererModel } from "../presentation/choiceModel.ts";
import { usePlayerIdentity } from "../presentation/PlayerIdentity.tsx";
import { describeReaction, type ReactionCard } from "../presentation/reactionModel.ts";
import { humanizeId } from "../protocol/contentCatalog.ts";
import { DecisionHeader } from "./DecisionHeader.tsx";
import "./ReactionStatusBar.css";

export interface ReactionStatusBarProps {
  choice: PendingChoiceDto | null;
  model?: ChoiceRendererModel | null;
  viewerSeat?: string | null;
  onSubmit: (optionId: string) => Promise<void>;
  isOpen: boolean;
  onClose?: () => void;
  lastError?: string | null;
  autoPassTimeoutSeconds?: number;
  /** The public log: the fallback when the decision carries no trigger. */
  events?: readonly GameEvent[];
  boardView?: BoardView;
  activePlayerId?: string | null;
  activeSystemId?: string | null;
}

const CardBlock: React.FC<{ card: ReactionCard | null; note?: string | null; testId?: string }> = ({
  card,
  note,
  testId,
}) => {
  const text = card?.text || note || "";
  if (!card && !text) return null;
  return (
    <div className="reaction-card" data-testid={testId}>
      {card && <div className="reaction-card__name">{card.name}</div>}
      {text && <p className="reaction-card__text">{text}</p>}
    </div>
  );
};

export const ReactionStatusBar: React.FC<ReactionStatusBarProps> = ({
  choice,
  model,
  viewerSeat,
  onSubmit,
  isOpen,
  onClose,
  lastError,
  autoPassTimeoutSeconds,
  events,
  boardView,
  activePlayerId,
  activeSystemId,
}) => {
  const display = usePlayerIdentity();
  const isActor = Boolean(choice && viewerSeat && choice.actor === viewerSeat);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [secondsRemaining, setSecondsRemaining] = useState<number | null>(
    autoPassTimeoutSeconds ?? null,
  );
  const [isPinned, setIsPinned] = useState(false);
  const [submissionError, setSubmissionError] = useState<string | null>(null);

  const reaction = useMemo(() => {
    if (!choice) return null;
    return describeReaction({
      choice,
      events,
      activePlayerId,
      activeSystemId: activeSystemId ?? boardView?.active_system ?? null,
      viewerSeat,
      playerLabel: (id) => display(id).label,
      systemLabel: (id) => {
        const planets = Object.keys(boardView?.systems?.[id]?.planets ?? {});
        return planets.length
          ? `System ${id} (${planets.map(humanizeId).join(", ")})`
          : `System ${id}`;
      },
    });
  }, [choice, events, activePlayerId, activeSystemId, boardView, viewerSeat, display]);

  // Reset state on nonce change
  useEffect(() => {
    setIsSubmitting(false);
    setSecondsRemaining(autoPassTimeoutSeconds ?? null);
    setSubmissionError(null);
  }, [choice?.nonce, autoPassTimeoutSeconds]);

  // Use model.declineOption when available (centralized extraction), fallback to local search
  const declineOption = useMemo(() => {
    return (
      model?.declineOption ??
      choice?.options.find((o) => o.id === "decline" || o.kind === "decline") ??
      null
    );
  }, [choice, model]);

  const reactionOptions = useMemo(() => {
    if (!choice) return [];
    return choice.options.filter((o) => o.id !== "decline" && o.kind !== "decline");
  }, [choice]);

  const handleAction = async (optionId: string) => {
    if (isSubmitting) return;
    setIsSubmitting(true);
    try {
      await onSubmit(optionId);
    } catch (error) {
      setSubmissionError(error instanceof Error ? error.message : String(error));
    } finally {
      setIsSubmitting(false);
    }
  };

  // Keyboard navigation (Spacebar -> Pass, Enter -> Play Reaction)
  useEffect(() => {
    if (!isOpen || !choice || !isActor || isSubmitting) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      // Ignore if focused on an input element
      const activeTag = document.activeElement?.tagName?.toLowerCase();
      if (activeTag === "input" || activeTag === "textarea" || activeTag === "select") {
        return;
      }

      if (e.key === " " || e.code === "Space") {
        if (declineOption) {
          e.preventDefault();
          handleAction(declineOption.id);
        }
      } else if (e.key === "Enter") {
        if (reactionOptions.length > 0) {
          e.preventDefault();
          handleAction(reactionOptions[0].id);
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, choice, isActor, isSubmitting, declineOption, reactionOptions]);

  // Countdown timer for auto-pass (if configured and not pinned)
  useEffect(() => {
    if (!isOpen || !choice || !isActor || isPinned || secondsRemaining === null || isSubmitting) {
      return;
    }

    if (secondsRemaining <= 0) {
      if (declineOption) {
        // Clear the countdown immediately before calling handleAction so the
        // effect cannot re-fire if isSubmitting changes while the network
        // call is in flight.
        setSecondsRemaining(null);
        handleAction(declineOption.id);
      }
      return;
    }

    const timer = setTimeout(() => {
      setSecondsRemaining((prev) => (prev !== null ? prev - 1 : null));
    }, 1000);

    return () => clearTimeout(timer);
  }, [isOpen, choice, isActor, isPinned, secondsRemaining, isSubmitting, declineOption]);

  if (!isOpen || !choice || !reaction) return null;

  return (
    <div
      role="region"
      aria-label="Reaction Window"
      data-testid="reaction-status-bar"
      className="reaction-status-bar"
      title={choice.prompt}
    >
      <DecisionHeader
        actor={choice.actor}
        title={reaction.title}
        progress={
          secondsRemaining === null || isPinned
            ? undefined
            : `${secondsRemaining} seconds remaining`
        }
        onMinimize={() => onClose?.()}
      />

      <section
        className="reaction-status-bar__happened"
        data-testid="reaction-trigger-context"
        data-trigger-source={reaction.source}
        aria-label="What happened"
      >
        <h3 className="reaction-status-bar__heading">What happened</h3>
        <p data-testid="reaction-bar-prompt" className="reaction-status-bar__sentence">
          {reaction.sentence}
        </p>
        {reaction.facts.systemId && (
          <span
            className="reaction-status-bar__trigger-target"
            data-testid={`reaction-trigger-system-${reaction.facts.systemId}`}
          >
            System {reaction.facts.systemId}
          </span>
        )}
        <CardBlock card={reaction.facts.card} testId="reaction-trigger-card" />
      </section>

      {!isActor ? (
        <div data-testid="spectator-reaction-notice" className="reaction-status-bar__spectator">
          Waiting for {display(choice.actor).label}...
        </div>
      ) : (
        <>
          <section className="reaction-status-bar__can-now" aria-label="You can now">
            <h3 className="reaction-status-bar__heading">
              You can now
              {secondsRemaining !== null && !isPinned && (
                <span className="reaction-status-bar__countdown">({secondsRemaining}s)</span>
              )}
            </h3>
            <p className="reaction-status-bar__sentence" data-testid="reaction-can-now">
              {reaction.canNowSentence}
            </p>
            <ul className="reaction-status-bar__rows">
              {reaction.reactions.map((row) => (
                <li key={row.optionId} className="reaction-status-bar__row">
                  <CardBlock card={row.card} note={row.note} />
                  {!row.card && !row.note && (
                    <div className="reaction-card__name">{row.name}</div>
                  )}
                </li>
              ))}
            </ul>
          </section>
          <div className="reaction-status-bar__actions">
            {reaction.reactions.map((row) => (
              <button
                key={row.optionId}
                type="button"
                data-testid={`play-reaction-btn-${row.optionId}`}
                onClick={() => handleAction(row.optionId)}
                disabled={isSubmitting}
                className="button button--primary reaction-status-bar__play"
              >
                {row.buttonLabel}
              </button>
            ))}

            {declineOption && (
              <button
                type="button"
                data-testid="pass-reaction-btn"
                onClick={() => handleAction(declineOption.id)}
                disabled={isSubmitting}
                className="button button--secondary reaction-status-bar__pass"
              >
                Pass (Spacebar)
              </button>
            )}

            <label className="reaction-status-bar__pin">
              <input
                type="checkbox"
                data-testid="pin-reaction-toggle"
                checked={isPinned}
                onChange={(e) => setIsPinned(e.target.checked)}
              />
              Pin
            </label>
          </div>
        </>
      )}

      {(lastError || submissionError) && (
        <div data-testid="reaction-error-badge" role="alert" className="reaction-status-bar__error">
          {submissionError || lastError}
        </div>
      )}
    </div>
  );
};
