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
  fitStaging,
  initialStaging,
  maxPurchases,
  overrideFromPlan,
  overrideIsValid,
  paymentCheck,
  planPayment,
  poolPips,
  removeToken,
  resultingCount,
  tokenOutcome,
  tokensRemaining,
  tokensToAssign,
  type CommandTokenView,
  type PaymentOverride,
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
  const [bought, setBought] = useState(0);
  // null: Auto-pay plans the payment. Set: the player's own choice of planets and trade goods.
  const [override, setOverride] = useState<PaymentOverride | null>(null);
  const [editing, setEditing] = useState(false);
  const [fallbackNote, setFallbackNote] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const locked = disabled || submitting;
  const remaining = tokensRemaining(view, staging, bought);
  const blocker = confirmBlocker(view, staging, bought);
  const changed = bought > 0 || override !== null || TOKEN_POOLS.some((pool) => staging[pool] !== start[pool]);
  const gain = view.mode === "gain";
  const purchase = view.purchase;
  const buyLimit = maxPurchases(view);
  const check = paymentCheck(view, bought, override);
  const payment = purchase ? check.plan : null;
  const total = tokensToAssign(view, bought);
  const useAutoPay = () => {
    setOverride(null);
    setEditing(false);
    setFallbackNote(null);
  };
  const changeBought = (next: number) => {
    const clamped = Math.max(0, Math.min(buyLimit, next));
    setBought(clamped);
    setStaging((current) => fitStaging(view, current, clamped));
    if (override) {
      // Keep the player's payment while it still is a legal one for the new count.
      if (clamped === 0) {
        useAutoPay();
      } else if (!overrideIsValid(view, clamped, override)) {
        setOverride(null);
        setEditing(false);
        setFallbackNote(
          `Your payment no longer fits ${clamped} token${clamped === 1 ? "" : "s"}; back to Auto-pay.`,
        );
      }
    } else {
      setFallbackNote(null);
    }
  };
  const startEditing = () => {
    const auto = planPayment(view, bought);
    if (!override && auto) setOverride(overrideFromPlan(auto));
    setEditing(true);
    setFallbackNote(null);
  };
  const togglePlanet = (id: string) =>
    setOverride((current) => {
      if (!current) return current;
      const has = current.planetIds.includes(id);
      return { ...current, planetIds: has ? current.planetIds.filter((x) => x !== id) : [...current.planetIds, id] };
    });
  const stepGoods = (delta: number) =>
    setOverride((current) =>
      current && purchase
        ? { ...current, tradeGoods: Math.max(0, Math.min(purchase.tradeGoods, current.tradeGoods + delta)) }
        : current,
    );

  const confirm = async () => {
    const outcome = tokenOutcome(view, staging, bought, override);
    if (!outcome) return;
    setSubmitting(true);
    setError(null);
    try {
      await onConfirm(outcome);
    } catch (cause) {
      // The engine moved on or rejected the plan: start again from what it offers now.
      setError(cause instanceof Error ? cause.message : String(cause));
      setStaging(start);
      setBought(0);
      useAutoPay();
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="token-panel" data-testid="command-token-panel" data-mode={view.mode}>
      <div className="token-panel__summary">
        <span data-testid="token-total">
          {gain ? "Tokens to assign" : "Tokens to arrange"} <strong>{total}</strong>
          {purchase && (
            <span className="token-panel__split" data-testid="token-split">
              {" "}
              ({view.total} free + {bought} bought)
            </span>
          )}
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
      {purchase && (
        <div className="token-panel__buy" data-testid="token-buy">
          <span className="token-panel__name">
            Buy tokens
            <span className="token-panel__purpose">
              {purchase.cost} influence each, up to {buyLimit}
            </span>
          </span>
          <span className="token-panel__stepper">
            <button
              type="button"
              className="button button--secondary button--sm"
              data-testid="token-buy-minus"
              aria-label="Buy one token less"
              disabled={locked || bought === 0}
              onClick={() => changeBought(bought - 1)}
            >
              −
            </button>
            <span className="token-panel__count" data-testid="token-buy-count" aria-label="Tokens bought">
              {bought}
            </span>
            <button
              type="button"
              className="button button--secondary button--sm"
              data-testid="token-buy-plus"
              aria-label="Buy one token more"
              disabled={locked || bought >= buyLimit}
              onClick={() => changeBought(bought + 1)}
            >
              +
            </button>
          </span>
          <div className="token-panel__influence" data-testid="token-influence">
            Influence available <strong>{purchase.influence}</strong> · spent{" "}
            <strong data-testid="token-influence-spent">{payment?.spent ?? 0}</strong>
            {payment && payment.extra > 0 && (
              <span data-testid="token-influence-extra">
                {" "}
                ({payment.extra} more than the bill
                {bought < buyLimit ? ", carried to the next token" : ", lost"})
              </span>
            )}
          </div>
          {payment && bought > 0 && (
            <p className="token-panel__note" data-testid="token-payment">
              Pays with{" "}
              {[
                ...payment.planets.map((planet) => `${planet.id} (${planet.worth})`),
                ...(payment.tradeGoods > 0
                  ? [`${payment.tradeGoods} trade good${payment.tradeGoods === 1 ? "" : "s"}`]
                  : []),
              ].join(" + ") || "nothing yet"}
              .{" "}
              {override
                ? "Your own choice."
                : "Chosen like Auto-pay: the planets covering the bill with the least waste, trade goods only when planets fall short."}
            </p>
          )}
          {bought > 0 && fallbackNote && (
            <p className="token-panel__note" data-testid="token-payment-fallback" role="status">
              {fallbackNote}
            </p>
          )}
          {bought > 0 && !editing && (
            <button
              type="button"
              className="button button--secondary button--sm"
              data-testid="token-payment-change"
              disabled={locked}
              onClick={startEditing}
            >
              Change payment
            </button>
          )}
          {bought > 0 && override && (
            <div className="token-panel__payment" data-testid="token-payment-editor">
              {editing && purchase && (
                <>
                  <ul className="token-panel__planets">
                    {purchase.planets.map((planet) => {
                      const chosen = override.planetIds.includes(planet.id);
                      return (
                        <li key={planet.id}>
                          <button
                            type="button"
                            className="button button--secondary button--sm"
                            data-testid={`token-payment-planet-${planet.id}`}
                            aria-pressed={chosen}
                            disabled={locked}
                            onClick={() => togglePlanet(planet.id)}
                          >
                            {planet.id} · {planet.worth} influence · ready{chosen ? " → exhaust" : ""}
                          </button>
                        </li>
                      );
                    })}
                  </ul>
                  {purchase.tradeGoods > 0 && (
                    <span className="token-panel__stepper" data-testid="token-payment-goods">
                      Trade goods ({purchase.tradeGoodWorth} influence each, {purchase.tradeGoods} held)
                      <button
                        type="button"
                        className="button button--secondary button--sm"
                        data-testid="token-payment-goods-minus"
                        aria-label="Spend one trade good less"
                        disabled={locked || override.tradeGoods === 0}
                        onClick={() => stepGoods(-1)}
                      >
                        −
                      </button>
                      <span className="token-panel__count" data-testid="token-payment-goods-count">
                        {override.tradeGoods}
                      </span>
                      <button
                        type="button"
                        className="button button--secondary button--sm"
                        data-testid="token-payment-goods-plus"
                        aria-label="Spend one trade good more"
                        disabled={locked || override.tradeGoods >= purchase.tradeGoods}
                        onClick={() => stepGoods(1)}
                      >
                        +
                      </button>
                    </span>
                  )}
                </>
              )}
              <p className="token-panel__note" data-testid="token-payment-account" data-ok={check.problem === null}>
                Paid <strong>{check.paid}</strong> · owed <strong>{check.bill}</strong> · remainder{" "}
                <strong>{check.remainder}</strong> · waste <strong>{check.waste}</strong>
              </p>
              {check.problem && (
                <p className="token-panel__note" role="alert" data-testid="token-payment-problem">
                  {check.problem}
                </p>
              )}
              <button
                type="button"
                className="button button--secondary button--sm"
                data-testid="token-payment-auto"
                disabled={locked}
                onClick={useAutoPay}
              >
                Use Auto-pay
              </button>
            </div>
          )}
        </div>
      )}
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
                  disabled={locked || !canAddToken(view, staging, bought)}
                  onClick={() => setStaging((s) => addToken(view, s, pool, bought))}
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
          onClick={() => {
            setStaging(start);
            setBought(0);
            useAutoPay();
          }}
        >
          Reset
        </button>
        <button
          type="button"
          className="button button--primary"
          data-testid="token-confirm"
          disabled={locked || !canConfirmTokens(view, staging, bought, override)}
          onClick={() => void confirm()}
        >
          {submitting
            ? "Submitting..."
            : !gain
              ? "Confirm arrangement"
              : total === 0
                ? "No purchase"
                : bought > 0
                  ? "Confirm tokens and purchase"
                  : "Confirm tokens"}
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
