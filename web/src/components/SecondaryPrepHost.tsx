import React, { useState } from "react";
import { usePlayerIdentity } from "../presentation/PlayerIdentity.tsx";
import { describePlan } from "../presentation/secondaryPlan.ts";
import type { SecondaryPrepare } from "../hooks/useSecondaryPrepare.ts";
import { ValueText } from "./PlanetValueIcons.tsx";
import "./SecondaryPrep.css";

export interface SecondaryPrepHostProps {
  prep: SecondaryPrepare;
  /** Sends the prepared answer for the viewer's real pending decision (the click-equivalent path). */
  onConfirm: (resolution: SecondaryPrepare["resolution"]) => Promise<void>;
  busy?: boolean;
}

/**
 * The chrome around preparing a strategy-card secondary with the usual UI:
 *  - the chip that opens preparation mode, and the "Preparing" banner while it is open;
 *  - the prepared answer for the viewer's real question: a one-click bar (Review), "Needs review",
 *    and the auto-play toasts. The question itself is rendered by the real components.
 * Spectators and non-followers see nothing.
 */
export const SecondaryPrepHost: React.FC<SecondaryPrepHostProps> = ({ prep, onConfirm, busy }) => {
  const display = usePlayerIdentity();
  const { action, plan, resolution, realChoice, mine, pending, played, holding } = prep;
  const [dismissed, setDismissed] = useState<string | null>(null);
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const nonce = mine && realChoice ? realChoice.nonce : null;

  const confirm = async () => {
    setSending(true);
    setError(null);
    try {
      await onConfirm(resolution);
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : String(failure));
    } finally {
      setSending(false);
    }
  };

  const showChip = prep.canPrepare && !prep.preparing && action !== null;
  const actionable = resolution.kind === "option" || resolution.kind === "tokens";
  const showBar =
    nonce !== null &&
    dismissed !== nonce &&
    !pending &&
    !holding &&
    (actionable || resolution.kind === "review");
  const showBanner = prep.preparing && action !== null;
  if (!showChip && !showBanner && !showBar && !pending && !played) return null;

  return (
    <div className={`secondary-prep${showBanner ? " secondary-prep--preparing" : ""}`} data-testid="secondary-prep">
      {showChip && action && (
        <button type="button" className="secondary-prep__chip" data-testid="secondary-prep-chip" onClick={prep.open}>
          {plan ? "Prepared" : "Prepare your secondary"}
          <span className="secondary-prep__note" style={{ margin: 0 }}>
            {plan ? describePlan(plan) : `${action.cardName}, played by ${display(action.primary).label}`}
          </span>
        </button>
      )}
      {showChip && plan && (
        <button type="button" className="button button--secondary button--sm" data-testid="prep-clear-chip" onClick={prep.clear}>
          Clear prepared
        </button>
      )}
      {showBanner && action && (
        <section className="secondary-prep__banner" data-testid="prepare-banner" aria-label="Preparing your secondary">
          <div className="secondary-prep__head">
            <div>
              <span className="secondary-prep__eyebrow">Preparing &mdash; nothing is sent or spent</span>
              <span className="secondary-prep__title">
                {action.cardName} secondary
                <span className="secondary-prep__note" style={{ display: "inline", marginLeft: 8 }}>
                  played by {display(action.primary).label}
                </span>
              </span>
            </div>
            <div className="secondary-prep__row">
              {plan && (
                <span className="secondary-prep__badge" data-testid="secondary-prepared-badge">
                  Prepared
                </span>
              )}
              <button type="button" className="button button--primary button--sm" data-testid="prep-save" onClick={prep.close}>
                Save plan
              </button>
              <button
                type="button"
                className="button button--secondary button--sm"
                data-testid="prep-clear"
                disabled={!plan}
                onClick={() => {
                  prep.clear();
                  prep.close();
                }}
              >
                Clear plan
              </button>
            </div>
          </div>
          <p className="secondary-prep__note" data-testid="prepare-plan-summary">
            {plan ? `Your plan: ${describePlan(plan)}. ` : "Answer the question below as you would for real. "}
            <span data-testid="prep-private-note">Private to this device; you can still change your mind when the real question comes.</span>
          </p>
          {prep.dry?.approximate && (
            <details className="secondary-prep__warning" data-testid="prepare-approximate" role="note">
              <summary>Approximate until the real question opens</summary>
              {prep.dry.approximate}
            </details>
          )}
        </section>
      )}
      {pending && (
        <div className="secondary-prep__bar" role="status" data-testid="secondary-autoplay-toast">
          <span className="secondary-prep__bar-text">
            Auto-playing your prepared secondary: <strong><ValueText text={pending.text} /></strong>
          </span>
          <span className="secondary-prep__bar-actions">
            <button type="button" className="button button--secondary button--sm" data-testid="secondary-autoplay-cancel" onClick={prep.cancel}>
              Cancel
            </button>
          </span>
        </div>
      )}
      {!pending && played && (
        <div className="secondary-prep__bar" role="status" data-testid="secondary-autoplayed-toast">
          <span className="secondary-prep__bar-text">
            Auto-played your prepared secondary: <strong><ValueText text={played} /></strong>
          </span>
        </div>
      )}
      {showBar && actionable && plan && (
        <div className="secondary-prep__bar" data-testid="secondary-prepared-bar">
          <span className="secondary-prep__bar-text">
            <span className="secondary-prep__badge" style={{ marginRight: 8 }}>Prepared</span>
            <strong>
              <ValueText text={resolution.kind === "option" || resolution.kind === "tokens" ? resolution.text : ""} />
            </strong>
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
            <button type="button" className="button button--secondary button--sm" data-testid="secondary-prepared-clear" onClick={prep.clear}>
              Clear prepared
            </button>
          </span>
        </div>
      )}
    </div>
  );
};
