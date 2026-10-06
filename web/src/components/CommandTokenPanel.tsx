import React, { useMemo, useState } from "react";
import {
  POOL_LABEL,
  POOL_PURPOSE,
  TOKEN_POOLS,
  addToken,
  canAddToken,
  canConfirmTokens,
  canRemoveToken,
  confirmBlocker,
  initialStaging,
  poolPips,
  removeToken,
  resultingCount,
  tokenOutcome,
  tokensRemaining,
  type CommandTokenView,
  type TokenOutcome,
  type TokenStaging,
} from "../presentation/commandTokens.ts";
import "./DecisionContext.css";

export interface CommandTokenPanelProps {
  view: CommandTokenView;
  disabled?: boolean;
  /** Send the staged result: a batch plan for a gain, an option id for a redistribution. */
  onConfirm: (outcome: TokenOutcome) => Promise<void>;
}

const Pips: React.FC<{ kept: number; added: number; removed: number }> = ({
  kept,
  added,
  removed,
}) => (
  <span className="token-panel__pips" aria-hidden="true">
    {Array.from({ length: kept }, (_, i) => (
      <i key={`k${i}`} className="token-panel__pip token-panel__pip--kept" />
    ))}
    {Array.from({ length: added }, (_, i) => (
      <i key={`a${i}`} className="token-panel__pip token-panel__pip--added" />
    ))}
    {Array.from({ length: removed }, (_, i) => (
      <i key={`r${i}`} className="token-panel__pip token-panel__pip--removed" />
    ))}
  </span>
);

/**
 * Command tokens as three pools to fill. Nothing is sent until every token is assigned and the
 * player confirms; a gain goes out as one batch, a redistribution as its single decision.
 */
export const CommandTokenPanel: React.FC<CommandTokenPanelProps> = ({
  view,
  disabled,
  onConfirm,
}) => {
  const start = useMemo(() => initialStaging(view), [view]);
  const [staging, setStaging] = useState<TokenStaging>(start);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const locked = disabled || submitting;
  const remaining = tokensRemaining(view, staging);
  const blocker = confirmBlocker(view, staging);
  const changed = TOKEN_POOLS.some((pool) => staging[pool] !== start[pool]);
  const gain = view.mode === "gain";

  const confirm = async () => {
    const outcome = tokenOutcome(view, staging);
    if (!outcome) return;
    setSubmitting(true);
    setError(null);
    try {
      await onConfirm(outcome);
    } catch (cause) {
      // The engine moved on or rejected the plan: start again from what it offers now.
      setError(cause instanceof Error ? cause.message : String(cause));
      setStaging(start);
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="token-panel" data-testid="command-token-panel" data-mode={view.mode}>
      <div className="token-panel__summary">
        <span data-testid="token-total">
          {gain ? "Tokens to assign" : "Tokens to arrange"} <strong>{view.total}</strong>
        </span>
        <span
          className="token-panel__remaining"
          data-testid="token-remaining"
          data-done={remaining === 0}
        >
          Remaining <strong>{remaining}</strong>
        </span>
        {view.reinforcements !== null && (
          <span data-testid="token-reinforcements">
            In reinforcements <strong>{view.reinforcements}</strong>
          </span>
        )}
      </div>
      <div className="token-panel__pools">
        {TOKEN_POOLS.map((pool) => {
          const pips = poolPips(view, staging, pool);
          const label = POOL_LABEL[pool];
          return (
            <div
              key={pool}
              className="token-panel__pool"
              data-testid={`token-pool-${pool}`}
              data-staged={staging[pool]}
            >
              <span className="token-panel__name">
                {label}
                <span className="token-panel__purpose">{POOL_PURPOSE[pool]}</span>
              </span>
              <span className="token-panel__stepper">
                <button
                  type="button"
                  className="button button--secondary button--sm"
                  data-testid={`token-minus-${pool}`}
                  aria-label={`Remove a token from ${label}`}
                  disabled={locked || !canRemoveToken(staging, pool)}
                  onClick={() => setStaging((s) => removeToken(s, pool))}
                >
                  −
                </button>
                <span
                  className="token-panel__count"
                  data-testid={`token-count-${pool}`}
                  aria-label={`${label} tokens`}
                >
                  {resultingCount(view, staging, pool)}
                </span>
                <button
                  type="button"
                  className="button button--secondary button--sm"
                  data-testid={`token-plus-${pool}`}
                  aria-label={`Add a token to ${label}`}
                  disabled={locked || !canAddToken(view, staging)}
                  onClick={() => setStaging((s) => addToken(view, s, pool))}
                >
                  +
                </button>
              </span>
              <span data-testid={`token-pips-${pool}`} data-kept={pips.kept} data-added={pips.added} data-removed={pips.removed}>
                <Pips {...pips} />
              </span>
            </div>
          );
        })}
      </div>
      <div className="token-panel__legend" aria-hidden="true">
        <span>
          <i className="token-panel__pip token-panel__pip--kept" /> already there
        </span>
        <span>
          <i className="token-panel__pip token-panel__pip--added" />{" "}
          {gain ? "newly assigned" : "moved in"}
        </span>
        {!gain && (
          <span>
            <i className="token-panel__pip token-panel__pip--removed" /> moved out
          </span>
        )}
      </div>
      {blocker && (
        <p className="token-panel__note" data-testid="token-blocker">
          {blocker}
        </p>
      )}
      <div className="token-panel__actions">
        <button
          type="button"
          className="button button--secondary"
          data-testid="token-reset"
          disabled={locked || !changed}
          onClick={() => setStaging(start)}
        >
          Reset
        </button>
        <button
          type="button"
          className="button button--primary"
          data-testid="token-confirm"
          disabled={locked || !canConfirmTokens(view, staging)}
          onClick={() => void confirm()}
        >
          {submitting ? "Submitting..." : gain ? "Confirm tokens" : "Confirm arrangement"}
        </button>
      </div>
      {error && (
        <div className="hit-assignment__error" role="alert" data-testid="token-error">
          {error}
        </div>
      )}
    </div>
  );
};
