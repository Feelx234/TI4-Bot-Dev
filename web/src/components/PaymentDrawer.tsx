import React, { useState, useEffect, useMemo } from 'react';
import { PendingChoiceDto, PlayerView } from '../protocol/types.ts';
import { getPaymentPayload, ChoiceRendererModel } from '../presentation/choiceModel.ts';
import { usePipelineRunner, SemanticIntent } from '../hooks/usePipelineRunner.ts';
import { Drawer } from '../primitives/index.ts';
import { WorkflowShell } from './WorkflowShell.tsx';

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
  const [selectedPlanetIds, setSelectedPlanetIds] = useState<string[]>([]);
  const [tradeGoodsToSpend, setTradeGoodsToSpend] = useState<number>(0);

  const { executePipeline, isRunning: isPipelineRunning } = usePipelineRunner(choice, onSubmit);

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
    <Drawer
      open={isOpen}
      onClose={onClose}
      modal={false}
      position="right"
      ariaLabel="Payment and Economy Drawer"
      data-testid="payment-drawer"
      className="payment-drawer panel"
      style={{
        position: 'absolute',
        top: 16,
        right: 16,
        bottom: 16,
        width: 360,
        zIndex: 'var(--layer-modal)',
        background: 'rgba(15, 23, 42, 0.98)',
        backdropFilter: 'blur(12px)',
        border: '1px solid #38bdf8',
        borderRadius: 8,
        padding: 20,
        boxShadow: '0 8px 32px rgba(0, 0, 0, 0.7)',
        display: 'flex',
        flexDirection: 'column',
        gap: 16,
        color: '#f8fafc',
      }}
    >
      {/* Header */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
        <div>
          <div style={{ fontSize: 11, fontWeight: 'bold', color: '#38bdf8', textTransform: 'uppercase' }}>
            Economy Settlement • Seat {choice.actor}
          </div>
          <h3
            data-testid="payment-drawer-title"
            style={{ margin: '4px 0 0 0', fontSize: 18, fontWeight: 700, color: '#f8fafc' }}
          >
            Pay {owed} {currency}
          </h3>
          <div style={{ fontSize: 12, color: '#94a3b8', marginTop: 2 }}>{choice.prompt}</div>
        </div>
        <button
          type="button"
          data-testid="close-payment-drawer"
          onClick={onClose}
          className="button button--secondary button--icon"
          aria-label="Close payment drawer"
          style={{ padding: '2px 8px', fontSize: 14, minWidth: 28, height: 28 }}
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
        spectatorNotice={`Observing payment in progress for seat ${choice.actor}...`}
        spectatorNoticeTestId="spectator-payment-notice"
        errorTestId="payment-error-banner"
      >
        {({ isActor, isDirectSubmitting, declineOption, submitDirect }) => isActor && <>
       {/* Progress and Debt Tally */}
      <div
        data-testid="payment-tally-card"
        style={{
          background: '#1e293b',
          borderRadius: 6,
          padding: 12,
          border: '1px solid #334155',
          display: 'flex',
          flexDirection: 'column',
          gap: 8,
        }}
      >
        <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 13 }}>
          <span style={{ color: '#94a3b8' }}>Total Owed:</span>
          <span style={{ fontWeight: 700, color: '#f8fafc' }}>
            {owed} {currency}
          </span>
        </div>
        <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 13 }}>
          <span style={{ color: '#94a3b8' }}>Committed:</span>
          <span
            data-testid="committed-amount"
            style={{ fontWeight: 700, color: isSettled ? '#4ade80' : '#38bdf8' }}
          >
            {totalCommitted} {currency}
          </span>
        </div>
        {credit > 0 && (
          <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 12, color: '#facc15' }}>
            <span>Credit Retained:</span>
            <span>+{credit} {currency}</span>
          </div>
        )}

        {/* Progress bar */}
        <div style={{ background: '#0f172a', borderRadius: 4, height: 8, overflow: 'hidden' }}>
          <div
            style={{
              height: '100%',
              width: `${Math.min(100, owed > 0 ? (totalCommitted / owed) * 100 : 100)}%`,
              background: isSettled ? '#4ade80' : '#38bdf8',
              transition: 'width 0.2s ease, background 0.2s ease',
            }}
          />
        </div>
      </div>

      {/* Ready Planet Cards */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: 8, flex: 1, overflowY: 'auto' }}>
        <div style={{ fontSize: 12, fontWeight: 700, color: '#94a3b8', textTransform: 'uppercase' }}>
          Ready Planets ({availablePlanets.length})
        </div>

        {availablePlanets.length === 0 ? (
          <div style={{ fontSize: 13, color: '#64748b', padding: '8px 0' }}>
            No ready planets available to exhaust.
          </div>
        ) : (
          availablePlanets.map((planet) => {
            const isSelected = selectedPlanetIds.includes(planet.id);
            return (
              <label
                key={planet.id}
                data-testid={`planet-card-${planet.id}`}
                className={`card${isSelected ? ' card--selected' : ''}`}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  padding: '8px 12px',
                  cursor: 'pointer',
                  borderRadius: 6,
                  border: isSelected ? '1px solid #38bdf8' : '1px solid #334155',
                  background: isSelected ? 'rgba(56, 189, 248, 0.1)' : '#1e293b',
                  transition: 'all 0.15s ease',
                }}
              >
                <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                  <input
                    type="checkbox"
                    checked={isSelected}
                    onChange={() => handleTogglePlanet(planet.id)}
                    disabled={isPipelineRunning || isDirectSubmitting}
                  />
                  <div>
                    <div style={{ fontWeight: 600, fontSize: 13, color: isSelected ? '#38bdf8' : '#f8fafc' }}>
                      {planet.planetName}
                    </div>
                    {planet.sourceKind && planet.sourceKind !== currency.toLowerCase() && (
                      <div style={{ fontSize: 11, color: '#facc15' }}>via {planet.sourceKind}</div>
                    )}
                  </div>
                </div>
                <span
                  style={{
                    padding: '2px 8px',
                    borderRadius: 4,
                    fontSize: 12,
                    fontWeight: 700,
                    background: '#0f172a',
                    color: '#38bdf8',
                  }}
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
            style={{
              marginTop: 8,
              padding: 12,
              background: '#1e293b',
              borderRadius: 6,
              border: '1px solid #334155',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
            }}
          >
            <div>
              <div style={{ fontWeight: 600, fontSize: 13, color: '#f8fafc' }}>Trade Goods</div>
              <div style={{ fontSize: 11, color: '#94a3b8' }}>
                1 TG = {tradeGoodWorth} {currency.slice(0, 3)} (Available: {maxTradeGoodsAvailable})
              </div>
            </div>

            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <button
                type="button"
                data-testid="tg-decrement-btn"
                onClick={() => setTradeGoodsToSpend((prev) => Math.max(0, prev - 1))}
                disabled={tradeGoodsToSpend <= 0 || isPipelineRunning || isDirectSubmitting}
                className="button button--secondary button--icon"
                style={{ minWidth: 28, height: 28 }}
              >
                -
              </button>
              <span data-testid="tg-count" style={{ fontSize: 14, fontWeight: 700, minWidth: 20, textAlign: 'center' }}>
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
                className="button button--secondary button--icon"
                style={{ minWidth: 28, height: 28 }}
              >
                +
              </button>
            </div>
          </div>
        )}
      </div>

      {lastError && (
        <div
          data-testid="payment-error-banner"
          role="alert"
          style={{
            background: 'rgba(239, 68, 68, 0.15)',
            border: '1px solid #ef4444',
            color: '#fca5a5',
            padding: '8px 12px',
            borderRadius: 6,
            fontSize: 12,
          }}
        >
          {lastError}
        </div>
      )}

      {/* Action Footer */}
      <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end', paddingTop: 8, borderTop: '1px solid #334155' }}>
        {declineOption && (
          <button
            type="button"
            data-testid="decline-payment-btn"
            onClick={() => onSubmit(declineOption.id)}
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
          style={{
            background: isSettled && !isPipelineRunning && !isDirectSubmitting ? undefined : '#475569',
          }}
        >
          {isPipelineRunning || isDirectSubmitting ? 'Paying...' : `Confirm Payment (${totalCommitted})`}
        </button>
      </div>
        </>}
      </WorkflowShell>
    </Drawer>
  );
};
