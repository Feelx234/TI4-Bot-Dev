import React, { useState, useEffect, useMemo } from 'react';
import { PendingChoiceDto } from '../protocol/types.ts';
import { Dialog } from '../primitives/index.ts';

export interface ProductionBuilderDrawerProps {
  choice: PendingChoiceDto | null;
  viewerSeat?: string | null;
  onSubmit: (optionId: string) => Promise<void>;
  isOpen: boolean;
  onClose: () => void;
  lastError?: string | null;
}

export const ProductionBuilderDrawer: React.FC<ProductionBuilderDrawerProps> = ({
  choice,
  viewerSeat,
  onSubmit,
  isOpen,
  onClose,
  lastError,
}) => {
  const isActor = Boolean(choice && viewerSeat && choice.actor === viewerSeat);
  const subtype = choice?.context?.subtype ?? '';
  const isPlaceUnit = subtype === 'place_unit';
  const isProduceUnit = subtype === 'produce_unit' || !isPlaceUnit;

  const [isSubmitting, setIsSubmitting] = useState(false);

  // Reset submitting state on nonce change
  useEffect(() => {
    setIsSubmitting(false);
  }, [choice?.nonce]);

  const declineOption = useMemo(() => {
    return choice?.options.find((o) => o.id === 'decline' || o.kind === 'decline') ?? null;
  }, [choice]);

  const productionOptions = useMemo(() => {
    if (!choice) return [];
    return choice.options.filter((o) => o.id !== 'decline' && o.kind !== 'decline');
  }, [choice]);

  const constraints = choice?.context?.outstanding?.[0];
  const capacityLimit = constraints?.amount ?? 0;
  const capacitySpent = constraints?.paid ?? 0;
  const capacityRemaining = Math.max(0, capacityLimit - capacitySpent);

  const systemId = choice?.context?.target && 'System' in choice.context.target
    ? choice.context.target.System
    : '';

  const handleSelectOption = async (optionId: string) => {
    if (isSubmitting) return;
    setIsSubmitting(true);
    try {
      await onSubmit(optionId);
    } finally {
      setIsSubmitting(false);
    }
  };

  if (!isOpen || !choice) return null;

  return (
    <Dialog.Root open={isOpen} onOpenChange={(open) => { if (!open) onClose(); }}>
      <Dialog.Content
        data-testid="production-builder-drawer"
        className="production-drawer"
        style={{
          position: 'fixed',
          top: 0,
          right: 0,
          bottom: 0,
          width: 440,
          maxWidth: '100vw',
          background: 'rgba(15, 23, 42, 0.96)',
          backdropFilter: 'blur(16px)',
          borderLeft: '2px solid #10b981',
          boxShadow: '-8px 0 32px rgba(16, 185, 129, 0.25)',
          color: '#f8fafc',
          display: 'flex',
          flexDirection: 'column',
          zIndex: 9000,
          boxSizing: 'border-box',
          padding: 24,
          overflowY: 'auto',
        }}
      >
        {/* Header */}
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: 16 }}>
          <div>
            <div style={{ fontSize: 11, fontWeight: 700, color: '#34d399', textTransform: 'uppercase' }}>
              Space Dock Production {systemId ? `• System ${systemId}` : ''}
            </div>
            <Dialog.Title
              as="h2"
              data-testid="production-drawer-title"
              style={{ fontSize: 18, fontWeight: 700, margin: '4px 0 0 0', color: '#f8fafc' }}
            >
              {isPlaceUnit ? 'Place Produced Unit' : 'Unit Production Builder'}
            </Dialog.Title>
          </div>

          <button
            type="button"
            data-testid="close-production-drawer"
            onClick={onClose}
            className="button button--secondary button--icon"
            aria-label="Close production builder"
            style={{ minWidth: 28, height: 28 }}
          >
            ✕
          </button>
        </div>

        {/* Spectator Notice */}
        {!isActor && (
          <div
            data-testid="spectator-production-notice"
            style={{
              background: 'rgba(16, 185, 129, 0.15)',
              border: '1px solid #10b981',
              borderRadius: 6,
              padding: 10,
              fontSize: 13,
              color: '#6ee7b7',
              marginBottom: 16,
            }}
          >
            Observing unit production in progress for seat {choice.actor}...
          </div>
        )}

        {/* Produce Unit Mode */}
        {isActor && isProduceUnit && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 16, flex: 1 }}>
            {/* Capacity Progress Meter */}
            {capacityLimit > 0 && (
              <div
                style={{
                  background: '#1e293b',
                  borderRadius: 8,
                  padding: '12px 16px',
                  border: '1px solid #334155',
                  display: 'flex',
                  flexDirection: 'column',
                  gap: 8,
                }}
              >
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <span style={{ fontSize: 13, color: '#94a3b8' }}>Production Capacity:</span>
                  <span
                    data-testid="production-capacity-counter"
                    style={{ fontSize: 14, fontWeight: 700, color: '#34d399' }}
                  >
                    {capacitySpent} / {capacityLimit} Units ({capacityRemaining} Left)
                  </span>
                </div>

                <div
                  style={{
                    height: 6,
                    background: '#0f172a',
                    borderRadius: 3,
                    overflow: 'hidden',
                  }}
                >
                  <div
                    style={{
                      height: '100%',
                      width: `${Math.min(100, (capacitySpent / capacityLimit) * 100)}%`,
                      background: capacityRemaining > 0 ? '#10b981' : '#f59e0b',
                      transition: 'width 0.2s ease',
                    }}
                  />
                </div>
              </div>
            )}

            <div style={{ fontSize: 13, color: '#cbd5e1' }}>
              Select a unit to build, or click Done to proceed to payment:
            </div>

            {/* Units Grid */}
            <div
              data-testid="production-options-grid"
              style={{
                display: 'grid',
                gridTemplateColumns: '1fr 1fr',
                gap: 10,
                flex: 1,
              }}
            >
              {productionOptions.map((opt) => (
                <button
                  key={opt.id}
                  type="button"
                  data-testid={`produce-unit-btn-${opt.id}`}
                  onClick={() => handleSelectOption(opt.id)}
                  disabled={isSubmitting}
                  className="button button--secondary"
                  style={{
                    display: 'flex',
                    flexDirection: 'column',
                    alignItems: 'center',
                    justifyContent: 'center',
                    padding: '14px 10px',
                    borderRadius: 8,
                    background: '#1e293b',
                    border: '1px solid #334155',
                    color: '#f8fafc',
                    cursor: 'pointer',
                    gap: 4,
                  }}
                >
                  <span style={{ fontSize: 13, fontWeight: 600 }}>{opt.label}</span>
                </button>
              ))}
            </div>

            {/* Done Producing Action */}
            {declineOption && (
              <div style={{ marginTop: 'auto', paddingTop: 16 }}>
                <button
                  type="button"
                  data-testid="done-producing-btn"
                  onClick={() => handleSelectOption(declineOption.id)}
                  disabled={isSubmitting}
                  className="button button--primary"
                  style={{
                    width: '100%',
                    padding: '12px 16px',
                    fontSize: 14,
                    fontWeight: 700,
                    background: '#10b981',
                  }}
                >
                  {declineOption.label || 'Done Producing'}
                </button>
              </div>
            )}
          </div>
        )}

        {/* Place Unit Mode */}
        {isActor && isPlaceUnit && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 14, flex: 1 }}>
            <div style={{ fontSize: 13, color: '#cbd5e1' }}>
              {choice.prompt}. Select destination location:
            </div>

            <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
              {productionOptions.map((opt) => (
                <button
                  key={opt.id}
                  type="button"
                  data-testid={`place-spot-btn-${opt.id}`}
                  onClick={() => handleSelectOption(opt.id)}
                  disabled={isSubmitting}
                  className="button button--secondary"
                  style={{
                    padding: '12px 16px',
                    textAlign: 'left',
                    background: '#1e293b',
                    border: '1px solid #334155',
                    borderRadius: 6,
                    color: '#f8fafc',
                  }}
                >
                  {opt.label}
                </button>
              ))}
            </div>
          </div>
        )}

        {/* Error Banner */}
        {lastError && (
          <div
            data-testid="production-error-banner"
            role="alert"
            style={{
              background: 'rgba(239, 68, 68, 0.15)',
              border: '1px solid #ef4444',
              color: '#fca5a5',
              padding: '8px 12px',
              borderRadius: 6,
              fontSize: 12,
              marginTop: 12,
            }}
          >
            {lastError}
          </div>
        )}
      </Dialog.Content>
    </Dialog.Root>
  );
};
