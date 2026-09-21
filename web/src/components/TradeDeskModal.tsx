import React, { useState, useEffect, useMemo } from 'react';
import { PendingChoiceDto } from '../protocol/types.ts';
import { decodeTradeOption, DecodedTradeOffer, TradeCategory } from '../presentation/tradeDecoder.ts';
import { Dialog } from '../primitives/index.ts';

export interface TradeDeskModalProps {
  choice: PendingChoiceDto | null;
  viewerSeat?: string | null;
  onSubmit: (optionId: string) => Promise<void>;
  isOpen: boolean;
  onClose: () => void;
  lastError?: string | null;
}

const CATEGORY_NAMES: Record<TradeCategory, string> = {
  commodity_swap: 'Commodities',
  goods_exchange: 'Goods Exchange',
  promissory: 'Promissory Notes',
  mutual_support: 'Mutual Support',
  other: 'Special Offers',
};

export const TradeDeskModal: React.FC<TradeDeskModalProps> = ({
  choice,
  viewerSeat,
  onSubmit,
  isOpen,
  onClose,
  lastError,
}) => {
  const isActor = Boolean(choice && viewerSeat && choice.actor === viewerSeat);
  const subtype = choice?.context?.subtype ?? '';
  const isAnswering = subtype === 'answer_transaction';
  const partnerSeat = choice?.context?.target && 'Player' in choice.context.target
    ? choice.context.target.Player
    : 'Unknown Partner';

  const [activeTab, setActiveTab] = useState<TradeCategory>('commodity_swap');
  const [selectedOfferId, setSelectedOfferId] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  // Parse all propose offers
  const decodedOffers = useMemo<DecodedTradeOffer[]>(() => {
    if (!choice || isAnswering) return [];
    return choice.options
      .filter((o) => o.id !== 'decline' && o.kind !== 'decline')
      .map(decodeTradeOption);
  }, [choice, isAnswering]);

  // Available categories
  const availableCategories = useMemo<TradeCategory[]>(() => {
    const categories = new Set<TradeCategory>();
    for (const off of decodedOffers) {
      categories.add(off.category);
    }
    const order: TradeCategory[] = [
      'commodity_swap',
      'goods_exchange',
      'promissory',
      'mutual_support',
      'other',
    ];
    return order.filter((c) => categories.has(c));
  }, [decodedOffers]);

  // Ensure activeTab is valid when options change
  useEffect(() => {
    if (availableCategories.length > 0 && !availableCategories.includes(activeTab)) {
      setActiveTab(availableCategories[0]);
    }
  }, [availableCategories, activeTab]);

  // Reset selection on nonce change
  useEffect(() => {
    setSelectedOfferId(null);
    setIsSubmitting(false);
  }, [choice?.nonce]);

  const declineOption = useMemo(() => {
    return choice?.options.find((o) => o.id === 'decline' || o.kind === 'decline') ?? null;
  }, [choice]);

  const handleSubmitOption = async (optionId: string) => {
    if (isSubmitting) return;
    setIsSubmitting(true);
    try {
      await onSubmit(optionId);
    } finally {
      setIsSubmitting(false);
    }
  };

  const currentTabOffers = decodedOffers.filter((o) => o.category === activeTab);
  const selectedOffer = decodedOffers.find((o) => o.id === selectedOfferId) ?? null;

  if (!isOpen || !choice) return null;

  return (
    <Dialog.Root open={isOpen} onOpenChange={(open) => { if (!open) onClose(); }}>
      <Dialog.Content
        data-testid="trade-desk-modal"
        className="trade-dialog"
        style={{
          background: 'rgba(3, 7, 18, 0.85)',
          backdropFilter: 'blur(8px)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
        }}
      >
        <div
          className="panel"
          style={{
            border: '2px solid #38bdf8',
            padding: 24,
            maxWidth: 620,
            width: '92%',
            display: 'flex',
            flexDirection: 'column',
            gap: 16,
            color: '#f8fafc',
            boxShadow: '0 0 32px rgba(56, 189, 248, 0.2)',
          }}
        >
          {/* Header */}
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
            <div>
              <div style={{ fontSize: 11, fontWeight: 700, color: '#38bdf8', textTransform: 'uppercase' }}>
                Bilateral Trade Negotiation
              </div>
              <Dialog.Title
                as="h2"
                data-testid="trade-desk-title"
                style={{ fontSize: 18, fontWeight: 700, margin: '4px 0 0 0', color: '#f8fafc' }}
              >
                {isAnswering
                  ? `Inbound Trade Offer from Seat ${partnerSeat}`
                  : `Trade Desk: You (${choice.actor}) ↔ ${partnerSeat}`}
              </Dialog.Title>
            </div>

            <button
              type="button"
              data-testid="close-trade-modal"
              onClick={onClose}
              className="button button--secondary button--icon"
              aria-label="Close trade desk"
              style={{ minWidth: 28, height: 28 }}
            >
              ✕
            </button>
          </div>

          {/* Spectator Notice */}
          {!isActor && (
            <div
              data-testid="spectator-trade-notice"
              style={{
                background: 'rgba(56, 189, 248, 0.15)',
                border: '1px solid #38bdf8',
                borderRadius: 6,
                padding: 10,
                fontSize: 13,
                color: '#38bdf8',
              }}
            >
              Observing bilateral trade negotiations between {choice.actor} and {partnerSeat}...
            </div>
          )}

          {/* Answer Mode */}
          {isActor && isAnswering && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
              <div
                style={{
                  background: '#1e293b',
                  borderRadius: 6,
                  padding: 14,
                  border: '1px solid #334155',
                  fontSize: 14,
                  lineHeight: 1.5,
                  color: '#f8fafc',
                }}
              >
                {choice.prompt}
              </div>

              <div style={{ display: 'flex', gap: 10, justifyContent: 'flex-end', marginTop: 8 }}>
                {choice.options.map((opt) => {
                  const isAccept = opt.id === 'accept';
                  const isRefuse = opt.id === 'refuse' || opt.kind === 'decline';
                  const isCounter = opt.id === 'counter';

                  let btnStyle: React.CSSProperties = {};
                  if (isAccept) {
                    btnStyle = { background: '#22c55e', color: '#fff' };
                  } else if (isRefuse) {
                    btnStyle = { background: '#ef4444', color: '#fff' };
                  } else if (isCounter) {
                    btnStyle = { background: '#3b82f6', color: '#fff' };
                  }

                  return (
                    <button
                      key={opt.id}
                      type="button"
                      data-testid={`answer-opt-${opt.id}`}
                      onClick={() => handleSubmitOption(opt.id)}
                      disabled={isSubmitting}
                      className={`button ${isAccept ? 'button--primary' : 'button--secondary'}`}
                      style={btnStyle}
                    >
                      {opt.label}
                    </button>
                  );
                })}
              </div>
            </div>
          )}

          {/* Propose Mode */}
          {isActor && !isAnswering && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
              {/* Category Tabs */}
              {availableCategories.length > 0 && (
                <div
                  role="tablist"
                  style={{
                    display: 'flex',
                    gap: 6,
                    borderBottom: '1px solid #334155',
                    paddingBottom: 6,
                    overflowX: 'auto',
                  }}
                >
                  {availableCategories.map((cat) => (
                    <button
                      key={cat}
                      role="tab"
                      aria-selected={activeTab === cat}
                      data-testid={`trade-tab-${cat}`}
                      type="button"
                      onClick={() => setActiveTab(cat)}
                      className={`button ${activeTab === cat ? 'button--primary' : 'button--secondary'}`}
                      style={{
                        fontSize: 12,
                        padding: '6px 12px',
                        borderRadius: 4,
                      }}
                    >
                      {CATEGORY_NAMES[cat]}
                    </button>
                  ))}
                </div>
              )}

              {/* Offer Options List */}
              <div
                style={{
                  display: 'flex',
                  flexDirection: 'column',
                  gap: 8,
                  maxHeight: 220,
                  overflowY: 'auto',
                  paddingRight: 4,
                }}
              >
                {currentTabOffers.length === 0 ? (
                  <div style={{ color: '#94a3b8', fontSize: 13, padding: 12, textAlign: 'center' }}>
                    No available offers in this category.
                  </div>
                ) : (
                  currentTabOffers.map((offer) => {
                    const isSelected = selectedOfferId === offer.id;
                    return (
                      <button
                        key={offer.id}
                        type="button"
                        data-testid={`trade-opt-${offer.id}`}
                        onClick={() => setSelectedOfferId(offer.id)}
                        className="button button--secondary"
                        style={{
                          display: 'flex',
                          justifyContent: 'space-between',
                          alignItems: 'center',
                          padding: '10px 14px',
                          borderRadius: 6,
                          border: isSelected ? '2px solid #38bdf8' : '1px solid #334155',
                          background: isSelected ? 'rgba(56, 189, 248, 0.15)' : '#1e293b',
                          textAlign: 'left',
                        }}
                      >
                        <span style={{ fontSize: 13, fontWeight: 500, color: '#f8fafc' }}>
                          {offer.label}
                        </span>
                        {offer.net !== undefined && (
                          <span
                            style={{
                              fontSize: 12,
                              fontWeight: 700,
                              color: offer.net >= 0 ? '#4ade80' : '#f87171',
                              background: '#0f172a',
                              padding: '2px 8px',
                              borderRadius: 4,
                            }}
                          >
                            {offer.net >= 0 ? `+${offer.net}` : offer.net} Value
                          </span>
                        )}
                      </button>
                    );
                  })
                )}
              </div>

              {/* Selected Offer Summary */}
              {selectedOffer && (
                <div
                  data-testid="selected-trade-summary"
                  style={{
                    background: 'rgba(56, 189, 248, 0.1)',
                    border: '1px solid #38bdf8',
                    borderRadius: 6,
                    padding: '8px 12px',
                    fontSize: 12,
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                  }}
                >
                  <span>Selected Deal: <strong>{selectedOffer.label}</strong></span>
                  {selectedOffer.net !== undefined && (
                    <span>Net Gain: <strong>{selectedOffer.net >= 0 ? `+${selectedOffer.net}` : selectedOffer.net}</strong></span>
                  )}
                </div>
              )}

              {/* Actions */}
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: 4 }}>
                {declineOption ? (
                  <button
                    type="button"
                    data-testid="decline-trade-btn"
                    onClick={() => handleSubmitOption(declineOption.id)}
                    disabled={isSubmitting}
                    className="button button--secondary"
                    style={{ fontSize: 13 }}
                  >
                    {declineOption.label || 'Offer Nothing'}
                  </button>
                ) : <div />}

                <button
                  type="button"
                  data-testid="propose-trade-btn"
                  onClick={() => selectedOfferId && handleSubmitOption(selectedOfferId)}
                  disabled={!selectedOfferId || isSubmitting}
                  className="button button--primary"
                  style={{ padding: '8px 20px', fontSize: 13 }}
                >
                  {isSubmitting ? 'Proposing...' : 'Propose Deal'}
                </button>
              </div>
            </div>
          )}

          {/* Error Banner */}
          {lastError && (
            <div
              data-testid="trade-error-banner"
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
        </div>
      </Dialog.Content>
    </Dialog.Root>
  );
};
