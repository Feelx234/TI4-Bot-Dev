import React, { useState, useEffect } from 'react';
import { getStrategyCardMeta } from '../protocol/contentCatalog.ts';
import { PendingChoiceDto } from '../protocol/types.ts';

export interface PendingChoiceModalProps {
  choice: PendingChoiceDto | null;
  onSubmit: (optionId: string) => Promise<void>;
  lastError?: string | null;
}

export const PendingChoiceModal: React.FC<PendingChoiceModalProps> = ({
  choice,
  onSubmit,
  lastError,
}) => {
  const [selectedOptionId, setSelectedOptionId] = useState<string>('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isMinimized, setIsMinimized] = useState(false);

  // Auto-select the first option and expand when a new choice arrives
  useEffect(() => {
    if (choice && choice.options.length > 0) {
      setSelectedOptionId(choice.options[0].id);
      setIsSubmitting(false);
      setIsMinimized(false);
    }
  }, [choice?.nonce]);

  if (!choice) return null;

  // Minimized floating banner allowing inspection of map, players, and tables
  if (isMinimized) {
    return (
      <div
        data-testid="minimized-choice-banner"
        style={{
          position: 'fixed',
          bottom: 24,
          left: '50%',
          transform: 'translateX(-50%)',
          background: '#0f172a',
          border: '2px solid #38bdf8',
          borderRadius: 30,
          padding: '8px 20px',
          color: '#f8fafc',
          boxShadow: '0 8px 32px rgba(0, 0, 0, 0.8), 0 0 16px rgba(56, 189, 248, 0.3)',
          display: 'flex',
          alignItems: 'center',
          gap: 16,
          zIndex: 90,
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span
            style={{
              width: 10,
              height: 10,
              borderRadius: '50%',
              background: '#38bdf8',
              display: 'inline-block',
              boxShadow: '0 0 8px #38bdf8',
            }}
          />
          <span style={{ fontSize: 13, fontWeight: 'bold', color: '#38bdf8' }}>
            Decision Required ({choice.actor}):
          </span>
          <span
            style={{
              fontSize: 13,
              color: '#f8fafc',
              maxWidth: 360,
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
            }}
          >
            {choice.prompt}
          </span>
        </div>

        <button
          type="button"
          data-testid="resume-choice-button"
          onClick={() => setIsMinimized(false)}
          style={{
            background: '#0284c7',
            color: '#ffffff',
            border: 'none',
            borderRadius: 16,
            padding: '6px 14px',
            fontSize: 12,
            fontWeight: 'bold',
            cursor: 'pointer',
            display: 'flex',
            alignItems: 'center',
            gap: 6,
          }}
        >
          <span>Open Decision</span>
          <span>▲</span>
        </button>
      </div>
    );
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedOptionId || isSubmitting) return;

    setIsSubmitting(true);
    try {
      await onSubmit(selectedOptionId);
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div
      data-testid="pending-choice-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="choice-prompt-title"
      style={{
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        background: 'rgba(3, 7, 18, 0.75)',
        backdropFilter: 'blur(4px)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 100,
      }}
    >
      <div
        style={{
          background: '#0f172a',
          border: '1px solid #38bdf8',
          borderRadius: 12,
          padding: 24,
          maxWidth: 540,
          width: '90%',
          color: '#f8fafc',
          boxShadow: '0 10px 30px rgba(0, 0, 0, 0.8)',
          display: 'flex',
          flexDirection: 'column',
          gap: 16,
        }}
      >
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
          <div>
            <div style={{ fontSize: 12, fontWeight: 'bold', color: '#38bdf8', textTransform: 'uppercase' }}>
              Decision Required • Seat: {choice.actor}
            </div>
            <h2
              id="choice-prompt-title"
              data-testid="choice-prompt"
              style={{ fontSize: 18, fontWeight: 'bold', margin: '6px 0 0 0', color: '#f8fafc' }}
            >
              {choice.prompt}
            </h2>
            {choice.context && (
              <div style={{ fontSize: 12, color: '#94a3b8', marginTop: 4 }}>
                Context: {choice.context.subtype}
              </div>
            )}
          </div>

          <button
            type="button"
            data-testid="minimize-choice-button"
            onClick={() => setIsMinimized(true)}
            title="Minimize decision dialog to inspect map and player sheets"
            style={{
              background: '#1e293b',
              border: '1px solid #475569',
              color: '#94a3b8',
              borderRadius: 6,
              padding: '5px 12px',
              fontSize: 12,
              fontWeight: 500,
              cursor: 'pointer',
              display: 'flex',
              alignItems: 'center',
              gap: 6,
              flexShrink: 0,
            }}
          >
            <span>Inspect Map</span>
            <span style={{ fontWeight: 'bold' }}>—</span>
          </button>
        </div>

        {/* Error banner if rejected */}
        {lastError && (
          <div
            data-testid="choice-error-banner"
            role="alert"
            style={{
              background: 'rgba(239, 68, 68, 0.15)',
              border: '1px solid #ef4444',
              color: '#fca5a5',
              padding: '8px 12px',
              borderRadius: 6,
              fontSize: 13,
            }}
          >
            {lastError}
          </div>
        )}

        <form onSubmit={handleSubmit} style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8, maxHeight: 260, overflowY: 'auto' }}>
            {choice.options.map((opt) => {
              const isChecked = selectedOptionId === opt.id;
              const scMeta = getStrategyCardMeta(opt.id);
              const displayDesc =
                opt.description ||
                (scMeta.primaryText ? `Primary: ${scMeta.primaryText}` : undefined);
              const tooltip =
                scMeta.secondaryText ? `Secondary Ability:\n${scMeta.secondaryText}` : undefined;

              return (
                <label
                  key={opt.id}
                  data-testid="choice-option"
                  data-option-id={opt.id}
                  data-actionable="true"
                  title={tooltip}
                  style={{
                    display: 'flex',
                    alignItems: 'flex-start',
                    gap: 10,
                    padding: '10px 14px',
                    borderRadius: 8,
                    background: isChecked ? '#1e293b' : '#090d16',
                    border: isChecked ? '1px solid #38bdf8' : '1px solid #334155',
                    cursor: 'pointer',
                    fontSize: 14,
                    transition: 'all 0.15s ease',
                  }}
                >
                  <input
                    type="radio"
                    name="choice-option"
                    value={opt.id}
                    checked={isChecked}
                    onChange={() => setSelectedOptionId(opt.id)}
                    style={{ marginTop: 3 }}
                  />
                  <div>
                    <div style={{ fontWeight: 600, color: isChecked ? '#38bdf8' : '#e2e8f0' }}>
                      {opt.label}
                    </div>
                    {displayDesc && (
                      <div style={{ fontSize: 12, color: '#94a3b8', marginTop: 2, whiteSpace: 'pre-line' }}>
                        {displayDesc}
                      </div>
                    )}
                  </div>
                </label>
              );
            })}
          </div>

          <div style={{ display: 'flex', justifyContent: 'flex-end', marginTop: 8 }}>
            <button
              type="submit"
              data-testid="submit-choice-button"
              disabled={!selectedOptionId || isSubmitting}
              style={{
                background: selectedOptionId && !isSubmitting ? '#0284c7' : '#475569',
                color: '#ffffff',
                border: 'none',
                borderRadius: 6,
                padding: '10px 20px',
                fontSize: 14,
                fontWeight: 'bold',
                cursor: selectedOptionId && !isSubmitting ? 'pointer' : 'not-allowed',
                transition: 'background 0.15s ease',
              }}
            >
              {isSubmitting ? 'Submitting...' : 'Confirm Choice'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
