import React, { useState } from "react";
import {
  canConfirmTokens,
  confirmBlocker,
  initialStaging,
  tokenOutcome,
  type CommandTokenView,
  type TokenOutcome,
} from "../presentation/commandTokens.ts";
import { useSharedTokenDraft, useTokenDraftState } from "../presentation/CommandTokenDraftContext.tsx";

export interface TokenPaymentBarProps {
  view: CommandTokenView;
  onConfirm: (outcome: TokenOutcome) => Promise<void>;
  /** Reopens the token panel. */
  onOpenPanel: () => void;
}

/**
 * The confirm bar for Leadership's purchase while the token panel is minimised: the planets that
 * can pay are the controls on the map, this bar shows paid against the bill and confirms. It
 * shares the staged purchase with the panel and the map; nothing is sent before Confirm.
 */
export const TokenPaymentBar: React.FC<TokenPaymentBarProps> = ({ view, onConfirm, onOpenPanel }) => {
  const shared = useSharedTokenDraft();
  const local = useTokenDraftState(null);
  const { draft, update } = shared ?? local;
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const purchase = view.purchase;
  if (!purchase) return null;

  const staging = draft.staging ?? initialStaging(view);
  const bought = draft.bought;
  const check = shared?.mapPayment?.check ?? null;
  const goods = shared?.mapPayment?.draft.tradeGoods ?? 0;
  const bill = purchase.cost * bought;
  const paid = check?.paid ?? 0;
  const covered = bought > 0 && paid >= bill;
  const blocker = confirmBlocker(view, staging, bought);
  const ready = canConfirmTokens(view, staging, bought, draft.override) && !busy;
  const planetCount = shared?.mapPayment?.draft.planetIds.length ?? 0;

  const confirm = async () => {
    const outcome = tokenOutcome(view, staging, bought, draft.override);
    if (!outcome || busy) return;
    setBusy(true);
    setError(null);
    try {
      await onConfirm(outcome);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      update(() => ({ staging: null, bought: 0, override: null }));
    } finally {
      setBusy(false);
    }
  };

  return (
    <aside
      data-testid="token-payment-bar"
      className="choice-banner panel system-activation-bar payment-bar"
      aria-label="Payment"
    >
      <div className="system-activation-bar__body">
        <div className="system-activation-bar__prompt-row">
          <span className="badge badge--primary">
            {bought > 0
              ? `Pay ${bill} influence for ${bought} token${bought === 1 ? "" : "s"}`
              : "No tokens bought"}
          </span>
          <span className="system-activation-bar__hint text-muted">
            {bought > 0
              ? "(Click highlighted planets on the map)"
              : "(Open the panel to buy tokens, then pay here)"}
          </span>
        </div>
        {bought > 0 && (
          <div className="payment-bar__tally" data-testid="token-bar-tally" data-settled={covered}>
            <span>
              Paid <strong data-testid="token-bar-paid">{paid}</strong> / {bill} influence
            </span>
            <span className="text-muted">
              from {planetCount} planet{planetCount === 1 ? "" : "s"}
              {purchase.tradeGoods > 0 && (
                <>
                  , {goods} trade good{goods === 1 ? "" : "s"}
                </>
              )}
            </span>
            <span
              data-testid="token-bar-remaining"
              className={covered && !check?.problem ? "text-success" : "text-warning"}
            >
              {covered
                ? check && check.waste > 0
                  ? `Overpaying by ${check.waste}`
                  : "Covered"
                : `${Math.max(0, bill - paid)} remaining`}
            </span>
          </div>
        )}
        {bought > 0 && check?.problem && (
          <div role="status" className="payment-bar__problem text-warning" data-testid="token-bar-problem">
            {check.problem}
          </div>
        )}
        {blocker && (
          <div role="status" className="payment-bar__problem text-warning" data-testid="token-bar-blocker">
            {blocker}
          </div>
        )}
        <div className="system-activation-bar__actions payment-bar__actions">
          <button
            type="button"
            className="button button--primary"
            data-testid="token-bar-confirm"
            disabled={!ready}
            onClick={() => void confirm()}
          >
            {busy ? "Submitting..." : bought > 0 ? "Confirm tokens and purchase" : "Confirm tokens"}
          </button>
          {bought > 0 && purchase.tradeGoods > 0 && (
            <span className="token-panel__stepper" data-testid="token-bar-goods">
              Trade goods {goods}
              <button
                type="button"
                className="button button--secondary button--sm"
                aria-label="Spend one trade good less"
                data-testid="token-bar-goods-minus"
                disabled={busy || goods === 0}
                onClick={() => shared?.stepTradeGoods(-1)}
              >
                −
              </button>
              <button
                type="button"
                className="button button--secondary button--sm"
                aria-label="Spend one trade good more"
                data-testid="token-bar-goods-plus"
                disabled={busy || goods >= purchase.tradeGoods}
                onClick={() => shared?.stepTradeGoods(1)}
              >
                +
              </button>
            </span>
          )}
          {bought > 0 && (
            <button
              type="button"
              className="button button--secondary"
              data-testid="token-bar-auto"
              disabled={busy || draft.override === null}
              title="Back to the Auto-pay suggestion; nothing is paid until you confirm"
              onClick={() => update((d) => ({ ...d, override: null }))}
            >
              Auto-pay
            </button>
          )}
          <button
            type="button"
            className="button button--secondary"
            data-testid="resume-choice-button"
            onClick={onOpenPanel}
          >
            Open token panel
          </button>
        </div>
        {error && (
          <div role="alert" className="system-activation-bar__error text-danger" data-testid="token-bar-error">
            {error}
          </div>
        )}
      </div>
    </aside>
  );
};
