import React, { useState } from "react";
import type { BoardView, PendingChoiceDto } from "../protocol/types.ts";
import { describeRetreatPick, retreatChipLabel } from "../presentation/retreatPick.ts";
import { useParticipantText } from "../presentation/PlayerIdentity.tsx";

export interface RetreatPickBarProps {
  choice: PendingChoiceDto;
  boardView?: BoardView;
  selectedOptionId?: string;
  onSelectOption?: (optionId: string) => void;
  onSubmit: (optionId: string) => Promise<void>;
  lastError?: string | null;
}

/**
 * A retreat or Foresight destination, chosen on the map like a tactical move: the heading names
 * the ships and the system they leave (highlighted on the map), the legal destinations are
 * highlighted, a click picks one and Confirm sends it. The chips are the same choice as a list,
 * for a phone where the map is small.
 */
export const RetreatPickBar: React.FC<RetreatPickBarProps> = ({
  choice,
  boardView,
  selectedOptionId,
  onSelectOption,
  onSubmit,
  lastError,
}) => {
  const present = useParticipantText();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // The legality sentence is open on a wide screen and one tap away on a phone, where the map needs the room.
  const [showRule, setShowRule] = useState(
    () => !(typeof window !== "undefined" && window.matchMedia?.("(max-width: 720px)").matches),
  );
  const pick = describeRetreatPick(choice, boardView);
  if (!pick) return null;
  const selected = pick.destinations.find((d) => d.optionId === selectedOptionId) ?? null;

  const confirm = async (optionId: string) => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await onSubmit(optionId);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };
  const shownError = error ?? lastError;

  return (
    <aside
      className="choice-banner panel system-activation-bar retreat-pick-bar"
      data-testid="retreat-pick-bar"
      aria-label={present(pick.title)}
    >
      <div className="system-activation-bar__body retreat-pick-bar__body">
        <h2 className="retreat-pick-bar__title" data-testid="retreat-pick-title">
          {present(pick.title)}
        </h2>
        {pick.fleet && (
          <span className="retreat-pick-bar__fleet text-muted" data-testid="retreat-pick-fleet">
            Going: {pick.fleet}
          </span>
        )}
        <span className="retreat-pick-bar__hint text-muted">
          {selected
            ? "Tap another highlighted system to change, or confirm."
            : "Tap a highlighted system on the map, or pick from the list."}
        </span>
        <div className="retreat-pick-bar__legend" aria-hidden="true">
          <span className="retreat-pick-bar__swatch retreat-pick-bar__swatch--from" /> leaves from
          <span className="retreat-pick-bar__swatch retreat-pick-bar__swatch--to" /> can go to
        </div>
        <ul className="retreat-pick-bar__list" data-testid="retreat-pick-list">
          {pick.destinations.map((d) => (
            <li key={d.optionId}>
              <button
                type="button"
                className="retreat-pick-bar__chip"
                data-testid={`retreat-pick-${d.optionId}`}
                aria-pressed={selected?.optionId === d.optionId}
                onClick={() => onSelectOption?.(d.optionId)}
              >
                <strong>{retreatChipLabel(d.systemId, boardView)}</strong>
                {d.notes.length > 0 && <span className="retreat-pick-bar__notes">{d.notes.join(" · ")}</span>}
              </button>
            </li>
          ))}
        </ul>
        <button
          type="button"
          className="retreat-pick-bar__rule-toggle"
          aria-expanded={showRule}
          onClick={() => setShowRule(!showRule)}
        >
          {showRule ? "Hide" : "Why these systems?"}
        </button>
        {showRule && (
          <span className="retreat-pick-bar__rule text-muted" data-testid="retreat-pick-rule">
            {pick.rule}
          </span>
        )}
        <div className="system-activation-bar__confirm-row">
          {selected && (
            <span className="system-activation-bar__title" data-testid="retreat-pick-selected">
              To <strong>{selected.name}</strong>
            </span>
          )}
          <button
            type="button"
            className="button button--primary"
            data-testid="confirm-retreat-btn"
            disabled={!selected || busy}
            onClick={() => selected && void confirm(selected.optionId)}
          >
            {busy ? "Submitting..." : pick.kind === "foresight" ? "Confirm Foresight" : "Confirm"}
          </button>
          {pick.declineOptionId && (
            <button
              type="button"
              className="button button--secondary"
              disabled={busy}
              onClick={() => pick.declineOptionId && void confirm(pick.declineOptionId)}
            >
              Decline
            </button>
          )}
        </div>
        {shownError && (
          <div role="alert" className="system-activation-bar__error text-danger">
            {shownError}
          </div>
        )}
      </div>
    </aside>
  );
};
