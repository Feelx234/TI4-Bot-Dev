import React, { useState, useEffect, useRef, useMemo } from 'react';
import { PendingChoiceDto } from '../protocol/types.ts';
import { Dialog, Tooltip } from '../primitives/index.ts';

export interface PendingChoiceModalProps {
  choice: PendingChoiceDto | null;
  onSubmit: (optionId: string) => Promise<void>;
  lastError?: string | null;
  isMinimized?: boolean;
  onMinimizedChange?: (isMinimized: boolean) => void;
  selectedOptionId?: string;
  onSelectOption?: (optionId: string) => void;
  selectedOptionIds?: string[];
  onSelectOptions?: (optionIds: string[]) => void;
}

export const PendingChoiceModal: React.FC<PendingChoiceModalProps> = ({
  choice,
  onSubmit,
  lastError,
  isMinimized: controlledIsMinimized,
  onMinimizedChange,
  selectedOptionId: controlledSelectedOptionId,
  onSelectOption,
  selectedOptionIds: controlledSelectedOptionIds,
  onSelectOptions,
}) => {
  const [uncontrolledSelectedOptionId, setUncontrolledSelectedOptionId] = useState<string>('');
  const [uncontrolledSelectedOptionIds, setUncontrolledSelectedOptionIds] = useState<string[]>([]);
  const [searchQuery, setSearchQuery] = useState<string>('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [uncontrolledIsMinimized, setUncontrolledIsMinimized] = useState(false);
  const dialogRef = useRef<HTMLDivElement>(null);
  const priorFocusRef = useRef<HTMLElement | null>(null);

  const isMinimized = controlledIsMinimized ?? uncontrolledIsMinimized;
  const setIsMinimized = (next: boolean) => {
    if (controlledIsMinimized === undefined) setUncontrolledIsMinimized(next);
    onMinimizedChange?.(next);
  };

  const constraints = choice?.constraints ?? choice?.context?.outstanding?.[0];
  const minSelection = constraints?.min_selection ?? 1;
  const maxSelection = constraints?.max_selection ?? (constraints?.min_selection ? constraints.min_selection : 1);
  const isMultiSelect = maxSelection > 1;

  const selectedOptionId = controlledSelectedOptionId ?? uncontrolledSelectedOptionId;
  const setSelectedOptionId = (id: string) => {
    if (controlledSelectedOptionId === undefined) setUncontrolledSelectedOptionId(id);
    onSelectOption?.(id);
  };

  const selectedOptionIds = useMemo(() => {
    return controlledSelectedOptionIds ?? uncontrolledSelectedOptionIds;
  }, [controlledSelectedOptionIds, uncontrolledSelectedOptionIds]);

  const setSelectedOptionIds = (ids: string[]) => {
    if (controlledSelectedOptionIds === undefined) setUncontrolledSelectedOptionIds(ids);
    onSelectOptions?.(ids);
  };

  // Auto-select initial state when new choice arrives
  useEffect(() => {
    if (!choice || choice.options.length === 0) return;

    if (!isMultiSelect) {
      if (controlledSelectedOptionId === undefined) {
        setUncontrolledSelectedOptionId(choice.options[0].id);
      }
    } else {
      if (controlledSelectedOptionIds === undefined) {
        setUncontrolledSelectedOptionIds([]);
      }
    }
    setSearchQuery('');
    setIsSubmitting(false);
    setIsMinimized(false);
  }, [choice?.nonce, isMultiSelect]);

  useEffect(() => {
    if (!choice || isMinimized) return;
    priorFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    dialogRef.current?.focus();
    return () => priorFocusRef.current?.focus();
  }, [choice?.nonce, isMinimized]);

  if (!choice) return null;

  // Filtered options based on search query
  const filteredOptions = choice.options.filter((opt) => {
    if (!searchQuery.trim()) return true;
    const q = searchQuery.toLowerCase();
    return opt.label.toLowerCase().includes(q) || (opt.description?.toLowerCase().includes(q) ?? false);
  });

  const handleToggleOption = (id: string) => {
    if (isMultiSelect) {
      if (selectedOptionIds.includes(id)) {
        const next = selectedOptionIds.filter((item) => item !== id);
        setSelectedOptionIds(next);
      } else {
        if (selectedOptionIds.length < maxSelection) {
          const next = [...selectedOptionIds, id];
          setSelectedOptionIds(next);
        }
      }
    } else {
      setSelectedOptionId(id);
    }
  };

  // Minimized floating banner allowing inspection of map, players, and tables
  if (isMinimized) {
    return (
      <div
        data-testid="minimized-choice-banner"
        className="choice-banner panel"
        style={{
          border: '2px solid #38bdf8',
          padding: '8px 20px',
          display: 'flex',
          alignItems: 'center',
          gap: 16,
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
          className="button button--primary"
          style={{
            borderRadius: 16,
            padding: '6px 14px',
            fontSize: 12,
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

  const isSelectionValid = isMultiSelect
    ? selectedOptionIds.length >= minSelection && selectedOptionIds.length <= maxSelection
    : Boolean(selectedOptionId);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!isSelectionValid || isSubmitting) return;

    setIsSubmitting(true);
    try {
      if (isMultiSelect) {
        // Submit first chosen option or joined selection
        await onSubmit(selectedOptionIds[0] ?? '');
      } else {
        await onSubmit(selectedOptionId);
      }
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <Dialog.Root open={!isMinimized} onOpenChange={(open) => setIsMinimized(!open)}>
      <Dialog.Content
        ref={dialogRef}
        data-testid="pending-choice-dialog"
        className="choice-dialog"
        onEscape={() => setIsMinimized(true)}
        initialFocusRef={dialogRef}
        style={{
          background: 'rgba(3, 7, 18, 0.75)',
          backdropFilter: 'blur(4px)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
        }}
      >
        <div
          className="choice-dialog__panel panel"
          style={{
            border: '1px solid #38bdf8',
            padding: 24,
            maxWidth: 560,
            width: '90%',
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
              <Dialog.Title
                as="h2"
                id="choice-prompt-title"
                data-testid="choice-prompt"
                style={{ fontSize: 18, fontWeight: 'bold', margin: '6px 0 0 0', color: '#f8fafc' }}
              >
                {choice.prompt}
              </Dialog.Title>
              {choice.context && (
                <div style={{ fontSize: 12, color: '#94a3b8', marginTop: 4 }}>
                  Context: {choice.context.subtype}
                </div>
              )}
            </div>

            <Tooltip content="Minimize decision dialog to inspect map and player sheets">
              <button
                type="button"
                data-testid="minimize-choice-button"
                onClick={() => setIsMinimized(true)}
                className="button button--secondary"
                style={{
                  padding: '5px 12px',
                  fontSize: 12,
                  display: 'flex',
                  alignItems: 'center',
                  gap: 6,
                  flexShrink: 0,
                }}
              >
                <span>Inspect Map</span>
                <span style={{ fontWeight: 'bold' }}>—</span>
              </button>
            </Tooltip>
          </div>

          {/* Bounded Multi-Select Status Indicator */}
          {isMultiSelect && (
            <div
              data-testid="multi-selection-badge"
              style={{
                fontSize: 12,
                fontWeight: 600,
                color: selectedOptionIds.length >= minSelection ? '#4ade80' : '#facc15',
                background: 'rgba(30, 41, 59, 0.8)',
                padding: '4px 10px',
                borderRadius: 4,
                border: '1px solid #334155',
                display: 'inline-block',
              }}
            >
              Selected: {selectedOptionIds.length} of {maxSelection} (Minimum: {minSelection})
            </div>
          )}

          {/* Search Bar for long option lists */}
          {choice.options.length >= 6 && (
            <input
              type="search"
              data-testid="choice-search-input"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search options..."
              aria-label="Filter options"
              style={{
                background: '#0f172a',
                border: '1px solid #334155',
                borderRadius: 6,
                padding: '6px 12px',
                color: '#f8fafc',
                fontSize: 13,
              }}
            />
          )}

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
            <div style={{ display: 'flex', flexDirection: 'column', gap: 8, maxHeight: 280, overflowY: 'auto' }}>
              {filteredOptions.length === 0 ? (
                <div style={{ padding: 16, textAlign: 'center', color: '#94a3b8', fontSize: 13 }}>
                  No matching options found.
                </div>
              ) : (
                filteredOptions.map((opt) => {
                  const isChecked = isMultiSelect
                    ? selectedOptionIds.includes(opt.id)
                    : selectedOptionId === opt.id;
                  const isMaxReached = isMultiSelect && selectedOptionIds.length >= maxSelection && !isChecked;

                  return (
                    <label
                      key={opt.id}
                      data-testid="choice-option"
                      data-option-id={opt.id}
                      data-actionable={!isMaxReached}
                      className={`card${isChecked ? ' card--selected' : ''}`}
                      style={{
                        display: 'flex',
                        alignItems: 'flex-start',
                        gap: 10,
                        padding: '10px 14px',
                        cursor: isMaxReached ? 'not-allowed' : 'pointer',
                        opacity: isMaxReached ? 0.5 : 1,
                        fontSize: 14,
                        transition: 'all 0.15s ease',
                      }}
                    >
                      <input
                        type={isMultiSelect ? 'checkbox' : 'radio'}
                        name="choice-option"
                        value={opt.id}
                        checked={isChecked}
                        onChange={() => handleToggleOption(opt.id)}
                        disabled={isSubmitting || isMaxReached}
                        style={{ marginTop: 3 }}
                        aria-checked={isChecked}
                      />
                      <div>
                        <div style={{ fontWeight: 600, color: isChecked ? '#38bdf8' : '#e2e8f0' }}>
                          {opt.label}
                        </div>
                        {opt.description && (
                          <div style={{ fontSize: 12, color: '#94a3b8', marginTop: 2, whiteSpace: 'pre-line' }}>
                            {opt.description}
                          </div>
                        )}
                      </div>
                    </label>
                  );
                })
              )}
            </div>

            <div style={{ display: 'flex', justifyContent: 'flex-end', marginTop: 8 }}>
              <button
                type="submit"
                data-testid="submit-choice-button"
                disabled={!isSelectionValid || isSubmitting}
                className="button button--primary"
                style={{
                  padding: '10px 20px',
                  background: isSelectionValid && !isSubmitting ? undefined : '#475569',
                  transition: 'background 0.15s ease',
                }}
              >
                {isSubmitting ? 'Submitting...' : 'Confirm Choice'}
              </button>
            </div>
          </form>
        </div>
      </Dialog.Content>
    </Dialog.Root>
  );
};
