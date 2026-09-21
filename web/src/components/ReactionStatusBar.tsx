import React, { useState, useEffect, useMemo } from 'react';
import { PendingChoiceDto } from '../protocol/types.ts';
import { ChoiceRendererModel } from '../presentation/choiceModel.ts';

export interface ReactionStatusBarProps {
  choice: PendingChoiceDto | null;
  model?: ChoiceRendererModel | null;
  viewerSeat?: string | null;
  onSubmit: (optionId: string) => Promise<void>;
  isOpen: boolean;
  onClose?: () => void;
  lastError?: string | null;
  autoPassTimeoutSeconds?: number;
}

export const ReactionStatusBar: React.FC<ReactionStatusBarProps> = ({
  choice,
  model,
  viewerSeat,
  onSubmit,
  isOpen,
  lastError,
  autoPassTimeoutSeconds,
}) => {
  const isActor = Boolean(choice && viewerSeat && choice.actor === viewerSeat);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [secondsRemaining, setSecondsRemaining] = useState<number | null>(
    autoPassTimeoutSeconds ?? null
  );
  const [isPinned, setIsPinned] = useState(false);

  // Reset state on nonce change
  useEffect(() => {
    setIsSubmitting(false);
    setSecondsRemaining(autoPassTimeoutSeconds ?? null);
  }, [choice?.nonce, autoPassTimeoutSeconds]);

  // Use model.declineOption when available (centralized extraction), fallback to local search
  const declineOption = useMemo(() => {
    return model?.declineOption ?? choice?.options.find((o) => o.id === 'decline' || o.kind === 'decline') ?? null;
  }, [choice, model]);

  const reactionOptions = useMemo(() => {
    if (!choice) return [];
    return choice.options.filter((o) => o.id !== 'decline' && o.kind !== 'decline');
  }, [choice]);

  const handleAction = async (optionId: string) => {
    if (isSubmitting) return;
    setIsSubmitting(true);
    try {
      await onSubmit(optionId);
    } finally {
      setIsSubmitting(false);
    }
  };

  // Keyboard navigation (Spacebar -> Pass, Enter -> Play Reaction)
  useEffect(() => {
    if (!isOpen || !choice || !isActor || isSubmitting) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      // Ignore if focused on an input element
      const activeTag = document.activeElement?.tagName?.toLowerCase();
      if (activeTag === 'input' || activeTag === 'textarea' || activeTag === 'select') {
        return;
      }

      if (e.key === ' ' || e.code === 'Space') {
        if (declineOption) {
          e.preventDefault();
          handleAction(declineOption.id);
        }
      } else if (e.key === 'Enter') {
        if (reactionOptions.length > 0) {
          e.preventDefault();
          handleAction(reactionOptions[0].id);
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, choice, isActor, isSubmitting, declineOption, reactionOptions]);

  // Countdown timer for auto-pass (if configured and not pinned)
  useEffect(() => {
    if (
      !isOpen ||
      !choice ||
      !isActor ||
      isPinned ||
      secondsRemaining === null ||
      isSubmitting
    ) {
      return;
    }

    if (secondsRemaining <= 0) {
      if (declineOption) {
        handleAction(declineOption.id);
      }
      return;
    }

    const timer = setTimeout(() => {
      setSecondsRemaining((prev) => (prev !== null ? prev - 1 : null));
    }, 1000);

    return () => clearTimeout(timer);
  }, [isOpen, choice, isActor, isPinned, secondsRemaining, isSubmitting, declineOption]);

  if (!isOpen || !choice) return null;

  return (
    <div
      role="region"
      aria-label="Reaction Window"
      data-testid="reaction-status-bar"
      style={{
        position: 'fixed',
        bottom: 24,
        left: '50%',
        transform: 'translateX(-50%)',
        zIndex: 9999,
        background: 'rgba(15, 23, 42, 0.95)',
        backdropFilter: 'blur(12px)',
        border: '2px solid #eab308',
        borderRadius: 12,
        padding: '12px 20px',
        boxShadow: '0 8px 32px rgba(234, 179, 8, 0.35)',
        color: '#f8fafc',
        display: 'flex',
        alignItems: 'center',
        gap: 16,
        maxWidth: 720,
        width: 'calc(100% - 32px)',
        boxSizing: 'border-box',
      }}
    >
      {/* Icon & Details */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 10, flex: 1, minWidth: 0 }}>
        <span style={{ fontSize: 20 }} role="img" aria-label="Reaction opportunity">
          ⚡
        </span>
        <div style={{ display: 'flex', flexDirection: 'column', minWidth: 0 }}>
          <div
            data-testid="reaction-bar-title"
            style={{
              fontSize: 11,
              fontWeight: 700,
              color: '#fef08a',
              textTransform: 'uppercase',
              letterSpacing: 0.5,
            }}
          >
            Reaction Opportunity
            {secondsRemaining !== null && !isPinned && (
              <span style={{ marginLeft: 8, color: '#94a3b8' }}>({secondsRemaining}s)</span>
            )}
          </div>
          <div
            data-testid="reaction-bar-prompt"
            style={{
              fontSize: 13,
              fontWeight: 600,
              whiteSpace: 'nowrap',
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              color: '#f8fafc',
            }}
          >
            {choice.prompt}
          </div>
        </div>
      </div>

      {/* Spectator Notice */}
      {!isActor ? (
        <div
          data-testid="spectator-reaction-notice"
          style={{ fontSize: 12, color: '#94a3b8', fontStyle: 'italic' }}
        >
          Waiting for seat {choice.actor}...
        </div>
      ) : (
        /* Action Buttons */
        <div style={{ display: 'flex', alignItems: 'center', gap: 10, flexShrink: 0 }}>
          {/* Reaction Cards */}
          {reactionOptions.map((opt) => (
            <button
              key={opt.id}
              type="button"
              data-testid={`play-reaction-btn-${opt.id}`}
              onClick={() => handleAction(opt.id)}
              disabled={isSubmitting}
              className="button button--primary"
              style={{
                background: '#eab308',
                color: '#0f172a',
                fontWeight: 700,
                fontSize: 12,
                padding: '6px 14px',
              }}
            >
              Play {opt.label}
            </button>
          ))}

          {/* Pass Button */}
          {declineOption && (
            <button
              type="button"
              data-testid="pass-reaction-btn"
              onClick={() => handleAction(declineOption.id)}
              disabled={isSubmitting}
              className="button button--secondary"
              style={{
                fontSize: 12,
                padding: '6px 14px',
                borderColor: '#64748b',
                color: '#cbd5e1',
              }}
            >
              Pass (Spacebar)
            </button>
          )}

          {/* Pinned Toggle */}
          <label
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 4,
              fontSize: 11,
              color: '#94a3b8',
              cursor: 'pointer',
              userSelect: 'none',
            }}
          >
            <input
              type="checkbox"
              data-testid="pin-reaction-toggle"
              checked={isPinned}
              onChange={(e) => setIsPinned(e.target.checked)}
              style={{ cursor: 'pointer' }}
            />
            Pin
          </label>
        </div>
      )}

      {/* Error alert if present */}
      {lastError && (
        <div
          data-testid="reaction-error-badge"
          style={{
            position: 'absolute',
            top: -28,
            left: 20,
            background: '#ef4444',
            color: '#fff',
            fontSize: 11,
            fontWeight: 600,
            padding: '2px 8px',
            borderRadius: 4,
          }}
        >
          {lastError}
        </div>
      )}
    </div>
  );
};
