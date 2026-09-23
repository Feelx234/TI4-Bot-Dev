import React, { useState, useEffect, useMemo } from 'react';
import { PendingChoiceDto, PlayerView } from '../protocol/types.ts';
import { getPaymentPayload, ChoiceRendererModel } from '../presentation/choiceModel.ts';
import { usePipelineRunner, SemanticIntent } from '../hooks/usePipelineRunner.ts';
import { WorkflowShell } from './WorkflowShell.tsx';
import { usePlayerIdentity } from '../presentation/PlayerIdentity.tsx';

export interface PaymentDrawerProps {
  choice: PendingChoiceDto | null;
  model?: ChoiceRendererModel | null;
  viewerSeat?: string | null;
  player?: PlayerView | null;
  onSubmit: (optionId: string) => Promise<void>;
  isOpen: boolean;
  onClose: () => void;
  lastError?: string | null;
}

interface DraftPlanet {
  id: string;
  planetName: string;
  worth: number;
  label: string;
  sourceKind?: string;
}

export const PaymentDrawer: React.FC<PaymentDrawerProps> = ({
  choice,
  model,
  viewerSeat,
  player,
  onSubmit,
  isOpen,
  onClose,
  lastError,
}) => {
  const display = usePlayerIdentity();
  const [selectedPlanetIds, setSelectedPlanetIds] = useState<string[]>([]);
  const [tradeGoodsToSpend, setTradeGoodsToSpend] = useState<number>(0);

  const { executePipeline, isRunning: isPipelineRunning, lastError: pipelineError } = usePipelineRunner(choice, onSubmit);

  const constraints = model?.outstanding?.[0] ?? choice?.context?.outstanding?.[0];
  const totalAmount = model?.selectionMode.mode === 'quantity'
    ? model.selectionMode.target
    : (constraints?.amount ?? 0);
  const alreadyPaid = model?.selectionMode.mode === 'quantity'
    ? model.selectionMode.paid
    : (constraints?.paid ?? 0);
  const owed = Math.max(0, totalAmount - alreadyPaid);
  const currency = model?.selectionMode.mode === 'quantity'
    ? (model.selectionMode.unit === 'influence' ? 'Influence' : 'Resources')
    : (choice?.context?.subtype === 'pay_influence' || constraints?.kind?.toLowerCase() === 'influence'
      ? 'Influence'
      : 'Resources');

  // Extract payment options from legal options
  const { availablePlanets, hasTradeGoodOption, tradeGoodWorth } = useMemo(() => {
    if (!choice) {
      return {
        availablePlanets: [] as DraftPlanet[],
        hasTradeGoodOption: false,
        tradeGoodWorth: 1,
      };
    }

    const planets: DraftPlanet[] = [];
    let hasTG = false;
    let tgWorth = 1;

    for (const opt of choice.options) {
      if (opt.id === 'decline' || opt.kind === 'decline') {
        continue;
      }
      if (opt.id === 'trade_good') {
        hasTG = true;
        const p = getPaymentPayload(opt);
        if (p.worth > 0) tgWorth = p.worth;
        continue;
      }
      if (opt.id.startsWith('exhaust|') || opt.kind === 'pay') {
        const p = getPaymentPayload(opt);
        planets.push({
          id: opt.id,
          planetName: p.planetName || opt.label || 'Planet',
          worth: p.worth > 0 ? p.worth : 1,
          label: opt.label,
          sourceKind: p.source,
        });
      }
    }

    return {
      availablePlanets: planets,
      hasTradeGoodOption: hasTG,
      tradeGoodWorth: tgWorth,
    };
  }, [choice, model]);

  // Reset draft state on new choice nonce
  useEffect(() => {
    setSelectedPlanetIds([]);
    setTradeGoodsToSpend(0);
  }, [choice?.nonce]);

  const maxTradeGoodsAvailable = player?.trade_goods ?? (hasTradeGoodOption ? 10 : 0);

  const committedFromPlanets = selectedPlanetIds.reduce((sum, id) => {
    const planet = availablePlanets.find((p) => p.id === id);
    return sum + (planet?.worth ?? 0);
  }, 0);

  const committedFromTG = tradeGoodsToSpend * tradeGoodWorth;
  const totalCommitted = committedFromPlanets + committedFromTG;
  const credit = Math.max(0, totalCommitted - owed);
  const isSettled = totalCommitted >= owed && owed > 0;

  const handleTogglePlanet = (id: string) => {
    setSelectedPlanetIds((prev) =>
      prev.includes(id) ? prev.filter((p) => p !== id) : [...prev, id]
    );
  };

  const handleConfirmPayment = async (isDirectSubmitting: boolean, submitDirect: (optionId: string) => Promise<void>) => {
    if (!isSettled || isPipelineRunning || isDirectSubmitting || !choice) return;

    // Build execution list
    const intents: SemanticIntent[] = [];

    // 1. Planets to exhaust
    for (const planetId of selectedPlanetIds) {
      intents.push({
        predicate: (opt) => opt.id === planetId,
      });
    }

    // 2. Trade goods to spend
    for (let i = 0; i < tradeGoodsToSpend; i++) {
      intents.push({
        predicate: (opt) => opt.id === 'trade_good',
      });
    }

    if (intents.length === 1) {
        const targetOption = choice.options.find(intents[0].predicate);
        if (targetOption) await submitDirect(targetOption.id);
    } else if (intents.length > 1) {
      executePipeline(intents);
    }
  };

  if (!choice) return null;

  return (
    isOpen && <section role="region" aria-label="Payment and Economy" data-testid="payment-drawer" className="payment-drawer panel">
      {/* Header */}
      <div className="choice-workflow-header">
        <div>
          <div className="choice-workflow-eyebrow">
             Economy Settlement • {display(choice.actor).label}
          </div>
          <h3
            data-testid="payment-drawer-title"
            className="choice-workflow-title"
          >
            Pay {owed} {currency}
          </h3>
          <div className="text-muted">{choice.prompt}</div>
        </div>
        <button
          type="button"
          data-testid="close-payment-drawer"
          onClick={onClose}
          aria-label="Close payment drawer"
          className="button button--secondary button--icon choice-workflow-close"
        >
          ✕
        </button>
      </div>

      <WorkflowShell
        choice={choice}
        model={model}
        viewerSeat={viewerSeat}
        onSubmit={onSubmit}
        lastError={lastError}
         spectatorNotice={`Observing payment in progress for ${display(choice.actor).label}...`}
        spectatorNoticeTestId="spectator-payment-notice"
        errorTestId="payment-error-banner"
      >
        {({ isActor, isDirectSubmitting, declineOption, submitDirect }) => isActor && <>
       {/* Progress and Debt Tally */}
      <div
        data-testid="payment-tally-card"
        className="workflow-card payment-drawer__tally"
      >
        <div className="workflow-card--row">
          <span className="text-muted">Total Owed:</span>
          <span>
            {owed} {currency}
          </span>
        </div>
        <div className="workflow-card--row">
          <span className="text-muted">Committed:</span>
          <span
            data-testid="committed-amount"
            className={isSettled ? 'text-success' : 'text-accent'}
          >
            {totalCommitted} {currency}
          </span>
        </div>
        {credit > 0 && (
          <div className="workflow-card--row text-warning">
            <span>Credit Retained:</span>
            <span>+{credit} {currency}</span>
          </div>
        )}

        {/* Progress bar */}
        <progress className="payment-drawer__progress" data-settled={isSettled} max={Math.max(owed, 1)} value={Math.min(totalCommitted, owed)} />
      </div>

      {/* Ready Planet Cards */}
      <div className="payment-drawer__list">
        <div className="choice-workflow-eyebrow text-muted">
          Ready Planets ({availablePlanets.length})
        </div>

        {availablePlanets.length === 0 ? (
          <div className="text-faint">
            No ready planets available to exhaust.
          </div>
        ) : (
          availablePlanets.map((planet) => {
            const isSelected = selectedPlanetIds.includes(planet.id);
            return (
              <label
                key={planet.id}
                data-testid={`planet-card-${planet.id}`}
                className={`card payment-drawer__planet${isSelected ? ' card--selected' : ''}`}
                data-selected={isSelected}
              >
                <div className="workflow-row">
                  <input
                    type="checkbox"
                    checked={isSelected}
                    onChange={() => handleTogglePlanet(planet.id)}
                    disabled={isPipelineRunning || isDirectSubmitting}
                  />
                  <div>
                    <div className="payment-drawer__planet-name" data-selected={isSelected}>
                      {planet.planetName}
                    </div>
                    {planet.sourceKind && planet.sourceKind !== currency.toLowerCase() && (
                      <div className="text-warning">via {planet.sourceKind}</div>
                    )}
                  </div>
                </div>
                <span
                  className="workflow-badge"
                >
                  +{planet.worth} {currency.slice(0, 3)}
                </span>
              </label>
            );
          })
        )}

        {/* Trade Goods Stepper */}
        {hasTradeGoodOption && (
          <div
            data-testid="trade-goods-stepper"
            className="workflow-card workflow-card--row"
          >
            <div>
              <div>Trade Goods</div>
              <div className="text-muted">
                1 TG = {tradeGoodWorth} {currency.slice(0, 3)} (Available: {maxTradeGoodsAvailable})
              </div>
            </div>

            <div className="workflow-row">
              <button
                type="button"
                data-testid="tg-decrement-btn"
                onClick={() => setTradeGoodsToSpend((prev) => Math.max(0, prev - 1))}
                disabled={tradeGoodsToSpend <= 0 || isPipelineRunning || isDirectSubmitting}
                className="button button--secondary button--icon choice-workflow-close"
              >
                -
              </button>
              <span data-testid="tg-count" className="workflow-count">
                {tradeGoodsToSpend}
              </span>
              <button
                type="button"
                data-testid="tg-increment-btn"
                onClick={() =>
                  setTradeGoodsToSpend((prev) =>
                    Math.min(maxTradeGoodsAvailable, prev + 1)
                  )
                }
                disabled={
                  tradeGoodsToSpend >= maxTradeGoodsAvailable ||
                  isPipelineRunning ||
                  isDirectSubmitting
                }
                className="button button--secondary button--icon choice-workflow-close"
              >
                +
              </button>
            </div>
          </div>
        )}
      </div>

      {pipelineError && (
        <div
          role="alert"
          className="workflow-error"
        >
          {pipelineError}
        </div>
      )}

      {/* Action Footer */}
      <div className="payment-drawer__footer">
        {declineOption && (
          <button
            type="button"
            data-testid="decline-payment-btn"
             onClick={() => { void submitDirect(declineOption.id); }}
            disabled={isPipelineRunning || isDirectSubmitting}
            className="button button--secondary"
          >
            {declineOption.label || 'Decline'}
          </button>
        )}
        <button
          type="button"
          data-testid="confirm-payment-btn"
           onClick={() => handleConfirmPayment(isDirectSubmitting, submitDirect)}
          disabled={!isSettled || isPipelineRunning || isDirectSubmitting}
          className="button button--primary"
          data-ready={isSettled && !isPipelineRunning && !isDirectSubmitting}
        >
          {isPipelineRunning || isDirectSubmitting ? 'Paying...' : `Confirm Payment (${totalCommitted})`}
        </button>
      </div>
        </>}
      </WorkflowShell>
    </section>
  );
};
