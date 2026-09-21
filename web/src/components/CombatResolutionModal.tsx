import React, { useState, useEffect, useMemo } from 'react';
import { PendingChoiceDto } from '../protocol/types.ts';
import { getCombatPayload } from '../presentation/choiceModel.ts';
import { usePipelineRunner, SemanticIntent } from '../hooks/usePipelineRunner.ts';
import { Dialog } from '../primitives/index.ts';

export interface CombatResolutionModalProps {
  choice: PendingChoiceDto | null;
  viewerSeat?: string | null;
  onSubmit: (optionId: string) => Promise<void>;
  isOpen: boolean;
  onClose: () => void;
  lastError?: string | null;
  recentDiceRolls?: { unit: string; roll: number; target: number; hit: boolean }[];
}

interface CasualtyGroup {
  unitType: string;
  totalAvailable: number;
  options: string[];
}

export const CombatResolutionModal: React.FC<CombatResolutionModalProps> = ({
  choice,
  viewerSeat,
  onSubmit,
  isOpen,
  onClose,
  lastError,
  recentDiceRolls = [],
}) => {
  // Staged casualties: unitType -> count
  const [stagedCasualties, setStagedCasualties] = useState<Record<string, number>>({});
  const [isDirectSubmitting, setIsDirectSubmitting] = useState(false);

  const { executePipeline, isRunning: isPipelineRunning } = usePipelineRunner(choice, onSubmit);

  const isActor = Boolean(choice && viewerSeat && choice.actor === viewerSeat);
  const subtype = choice?.context?.subtype ?? '';
  const constraints = choice?.context?.outstanding?.[0];
  const hitsOwed = constraints?.amount ?? 1;

  const isSustainStage = subtype === 'sustain_damage';
  const isCasualtyStage = subtype === 'assign_casualty' || choice?.options.some((o) => o.kind === 'casualty');
  const isRetreatStage = subtype === 'announce_retreat' || subtype === 'retreat_to';

  const declineOption = useMemo(() => {
    return choice?.options.find((o) => o.id === 'decline' || o.kind === 'decline') ?? null;
  }, [choice]);

  // Group casualty options
  const casualtyGroups = useMemo(() => {
    if (!choice || !isCasualtyStage) return [] as CasualtyGroup[];

    const map = new Map<string, CasualtyGroup>();
    for (const opt of choice.options) {
      if (opt.id === 'decline' || opt.kind === 'decline') continue;

      const p = getCombatPayload(opt);
      const unit = p.unit ?? opt.label.replace('destroy ', '').split(' ')[0].toLowerCase();

      const existing = map.get(unit);
      if (existing) {
        existing.totalAvailable += 1;
        existing.options.push(opt.id);
      } else {
        map.set(unit, {
          unitType: unit,
          totalAvailable: 1,
          options: [opt.id],
        });
      }
    }

    return Array.from(map.values());
  }, [choice, isCasualtyStage]);

  // Reset staging on nonce change
  useEffect(() => {
    setStagedCasualties({});
    setIsDirectSubmitting(false);
  }, [choice?.nonce]);

  const totalCasualtiesAllocated = Object.values(stagedCasualties).reduce((a, b) => a + b, 0);
  const isCasualtyAllocationValid = totalCasualtiesAllocated === hitsOwed;

  const handleUpdateCasualty = (unitType: string, delta: number, max: number) => {
    setStagedCasualties((prev) => {
      const current = prev[unitType] ?? 0;
      const next = Math.max(0, Math.min(max, current + delta));
      return { ...prev, [unitType]: next };
    });
  };

  const handleAutoCheapest = () => {
    // Cheap units first: fighters, then infantry, then others
    const priority = ['fighter', 'infantry', 'destroyer', 'cruiser', 'carrier', 'dreadnought', 'mech', 'warsun'];
    const sorted = [...casualtyGroups].sort((a, b) => {
      const idxA = priority.indexOf(a.unitType);
      const idxB = priority.indexOf(b.unitType);
      return (idxA === -1 ? 99 : idxA) - (idxB === -1 ? 99 : idxB);
    });

    let remaining = hitsOwed;
    const allocation: Record<string, number> = {};

    for (const g of sorted) {
      if (remaining <= 0) break;
      const take = Math.min(g.totalAvailable, remaining);
      allocation[g.unitType] = take;
      remaining -= take;
    }

    setStagedCasualties(allocation);
  };

  const handleConfirmCasualties = async () => {
    if (!isCasualtyAllocationValid || isPipelineRunning || isDirectSubmitting || !choice) return;

    const intents: SemanticIntent[] = [];

    for (const g of casualtyGroups) {
      const count = stagedCasualties[g.unitType] ?? 0;
      for (let i = 0; i < count; i++) {
        intents.push({
          kind: 'casualty',
          predicate: (opt) => {
            const p = getCombatPayload(opt);
            if (p.unit && p.unit.toLowerCase() === g.unitType.toLowerCase()) return true;
            if (opt.id === `destroy|${g.unitType}` || opt.id.startsWith(`destroy|${g.unitType}|`)) return true;
            const parts = opt.id.toLowerCase().split('|');
            return parts.includes(g.unitType.toLowerCase());
          },
          description: `Destroy ${g.unitType}`,
        });
      }
    }

    if (intents.length === 1) {
      setIsDirectSubmitting(true);
      try {
        const opt = choice.options.find(intents[0].predicate);
        if (opt) await onSubmit(opt.id);
      } finally {
        setIsDirectSubmitting(false);
      }
    } else {
      executePipeline(intents);
    }
  };

  const handleConfirmSustain = async (optionId: string) => {
    setIsDirectSubmitting(true);
    try {
      await onSubmit(optionId);
    } finally {
      setIsDirectSubmitting(false);
    }
  };

  if (!isOpen || !choice) return null;

  return (
    <Dialog.Root open={isOpen} onOpenChange={(open) => { if (!open) onClose(); }}>
      <Dialog.Content
        data-testid="combat-resolution-modal"
        className="combat-dialog"
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
            border: '2px solid #ef4444',
            padding: 24,
            maxWidth: 600,
            width: '92%',
            display: 'flex',
            flexDirection: 'column',
            gap: 16,
            color: '#f8fafc',
            boxShadow: '0 0 32px rgba(239, 68, 68, 0.25)',
          }}
        >
          {/* Header */}
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
            <div>
              <div style={{ fontSize: 11, fontWeight: 700, color: '#ef4444', textTransform: 'uppercase' }}>
                Space Combat Arena • Seat: {choice.actor}
              </div>
              <Dialog.Title
                as="h2"
                data-testid="combat-stage-title"
                style={{ fontSize: 18, fontWeight: 700, margin: '4px 0 0 0', color: '#f8fafc' }}
              >
                {isSustainStage
                  ? 'Stage 1: Sustain Damage'
                  : isCasualtyStage
                    ? `Stage 3: Allocate Casualties (${hitsOwed} Hits)`
                    : isRetreatStage
                      ? 'Combat Movement: Announce Retreat'
                      : choice.prompt}
              </Dialog.Title>
            </div>

            <button
              type="button"
              data-testid="close-combat-modal"
              onClick={onClose}
              className="button button--secondary button--icon"
              aria-label="Close combat dialog"
              style={{ minWidth: 28, height: 28 }}
            >
              ✕
            </button>
          </div>

          {/* Dice Results Feed */}
          {recentDiceRolls.length > 0 && (
            <div
              data-testid="combat-dice-feed"
              style={{
                background: '#0f172a',
                borderRadius: 6,
                padding: '10px 14px',
                border: '1px solid #334155',
                display: 'flex',
                flexDirection: 'column',
                gap: 6,
              }}
            >
              <div style={{ fontSize: 11, fontWeight: 700, color: '#94a3b8', textTransform: 'uppercase' }}>
                Recent Combat Rolls
              </div>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                {recentDiceRolls.map((d, i) => (
                  <span
                    key={i}
                    data-testid="dice-roll-badge"
                    style={{
                      padding: '3px 8px',
                      borderRadius: 4,
                      fontSize: 12,
                      fontWeight: 700,
                      background: d.hit ? 'rgba(74, 222, 128, 0.2)' : 'rgba(100, 116, 139, 0.2)',
                      border: d.hit ? '1px solid #4ade80' : '1px solid #475569',
                      color: d.hit ? '#4ade80' : '#94a3b8',
                    }}
                  >
                    {d.unit} ({d.target}+): [{d.roll}] {d.hit ? '★ HIT' : 'MISS'}
                  </span>
                ))}
              </div>
            </div>
          )}

          {/* Spectator Notice */}
          {!isActor && (
            <div
              data-testid="spectator-combat-notice"
              style={{
                background: 'rgba(56, 189, 248, 0.15)',
                border: '1px solid #38bdf8',
                borderRadius: 6,
                padding: 10,
                fontSize: 13,
                color: '#38bdf8',
              }}
            >
              Observing combat resolution in progress for seat {choice.actor}...
            </div>
          )}

          {/* Stage 1: Sustain Damage */}
          {isActor && isSustainStage && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
              <div
                style={{
                  background: 'rgba(234, 179, 8, 0.15)',
                  border: '1px solid #eab308',
                  borderRadius: 6,
                  padding: '8px 12px',
                  fontSize: 12,
                  color: '#fef08a',
                }}
              >
                ⚠️ Caution: Opponents holding "Direct Hit" action cards may react to destroy sustained ships!
              </div>

              <div style={{ fontSize: 13, color: '#cbd5e1' }}>
                Select capital ships to sustain damage, or choose decline to proceed to direct hits:
              </div>

              <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                {choice.options
                  .filter((o) => o.id !== 'decline' && o.kind !== 'decline')
                  .map((opt) => (
                    <button
                      key={opt.id}
                      type="button"
                      data-testid={`sustain-opt-${opt.id}`}
                      onClick={() => handleConfirmSustain(opt.id)}
                      disabled={isDirectSubmitting}
                      className="button button--secondary"
                      style={{
                        padding: '10px 16px',
                        display: 'flex',
                        justifyContent: 'space-between',
                        alignItems: 'center',
                        border: '1px solid #eab308',
                        color: '#fef08a',
                      }}
                    >
                      <span style={{ fontWeight: 600 }}>{opt.label}</span>
                      <span style={{ fontSize: 11, background: '#0f172a', padding: '2px 6px', borderRadius: 4 }}>
                        Sustain Hit
                      </span>
                    </button>
                  ))}
              </div>

              {declineOption && (
                <div style={{ display: 'flex', justifyContent: 'flex-end', marginTop: 8 }}>
                  <button
                    type="button"
                    data-testid="decline-sustain-btn"
                    onClick={() => handleConfirmSustain(declineOption.id)}
                    disabled={isDirectSubmitting}
                    className="button button--secondary"
                  >
                    {declineOption.label || 'Do Not Sustain Damage'}
                  </button>
                </div>
              )}
            </div>
          )}

          {/* Stage 3: Casualty Allocation Steppers */}
          {isActor && isCasualtyStage && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <span style={{ fontSize: 13, color: '#cbd5e1' }}>
                  Allocate exactly {hitsOwed} hits to destroy your units:
                </span>
                <button
                  type="button"
                  data-testid="auto-cheapest-btn"
                  onClick={handleAutoCheapest}
                  className="button button--secondary"
                  style={{ fontSize: 11, padding: '4px 10px' }}
                >
                  Auto-Cheapest
                </button>
              </div>

              {/* Tally Card */}
              <div
                style={{
                  background: '#1e293b',
                  borderRadius: 6,
                  padding: '8px 12px',
                  border: isCasualtyAllocationValid ? '1px solid #4ade80' : '1px solid #ef4444',
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                }}
              >
                <span style={{ fontSize: 13, color: '#94a3b8' }}>Hits Allocated:</span>
                <span
                  data-testid="casualty-allocated-count"
                  style={{
                    fontSize: 14,
                    fontWeight: 700,
                    color: isCasualtyAllocationValid ? '#4ade80' : '#ef4444',
                  }}
                >
                  {totalCasualtiesAllocated} / {hitsOwed} Hits
                </span>
              </div>

              {/* Grouped Unit Rows */}
              <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                {casualtyGroups.map((g) => {
                  const count = stagedCasualties[g.unitType] ?? 0;
                  return (
                    <div
                      key={g.unitType}
                      data-testid={`casualty-row-${g.unitType}`}
                      style={{
                        display: 'flex',
                        justifyContent: 'space-between',
                        alignItems: 'center',
                        background: '#1e293b',
                        padding: '8px 12px',
                        borderRadius: 6,
                        border: '1px solid #334155',
                      }}
                    >
                      <div>
                        <div style={{ fontSize: 13, fontWeight: 600, textTransform: 'capitalize' }}>
                          {g.unitType}
                        </div>
                        <div style={{ fontSize: 11, color: '#94a3b8' }}>Available: {g.totalAvailable}</div>
                      </div>

                      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                        <button
                          type="button"
                          data-testid={`casualty-dec-${g.unitType}`}
                          onClick={() => handleUpdateCasualty(g.unitType, -1, g.totalAvailable)}
                          disabled={count <= 0 || isPipelineRunning || isDirectSubmitting}
                          className="button button--secondary button--icon"
                          style={{ minWidth: 26, height: 26 }}
                        >
                          -
                        </button>
                        <span
                          data-testid={`casualty-count-${g.unitType}`}
                          style={{ minWidth: 20, textAlign: 'center', fontSize: 13, fontWeight: 700 }}
                        >
                          {count}
                        </span>
                        <button
                          type="button"
                          data-testid={`casualty-inc-${g.unitType}`}
                          onClick={() => handleUpdateCasualty(g.unitType, 1, g.totalAvailable)}
                          disabled={count >= g.totalAvailable || isPipelineRunning || isDirectSubmitting}
                          className="button button--secondary button--icon"
                          style={{ minWidth: 26, height: 26 }}
                        >
                          +
                        </button>
                      </div>
                    </div>
                  );
                })}
              </div>

              <div style={{ display: 'flex', justifyContent: 'flex-end', marginTop: 6 }}>
                <button
                  type="button"
                  data-testid="confirm-casualties-btn"
                  onClick={handleConfirmCasualties}
                  disabled={!isCasualtyAllocationValid || isPipelineRunning || isDirectSubmitting}
                  className="button button--primary"
                  style={{
                    padding: '8px 20px',
                    background: isCasualtyAllocationValid && !isPipelineRunning && !isDirectSubmitting ? '#ef4444' : '#475569',
                  }}
                >
                  {isPipelineRunning || isDirectSubmitting
                    ? 'Destroying Units...'
                    : `Confirm Casualties (${totalCasualtiesAllocated})`}
                </button>
              </div>
            </div>
          )}

          {/* Retreat Stage */}
          {isActor && isRetreatStage && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
              <div style={{ fontSize: 13, color: '#cbd5e1' }}>
                {subtype === 'announce_retreat'
                  ? 'Choose whether to announce a retreat before combat rounds commence:'
                  : 'Select an adjacent system to retreat your surviving fleet to:'}
              </div>

              <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                {choice.options.map((opt) => (
                  <button
                    key={opt.id}
                    type="button"
                    data-testid={`retreat-opt-${opt.id}`}
                    onClick={() => handleConfirmSustain(opt.id)}
                    disabled={isDirectSubmitting}
                    className="button button--secondary"
                    style={{ textAlign: 'left', padding: '10px 14px' }}
                  >
                    {opt.label}
                  </button>
                ))}
              </div>
            </div>
          )}

          {lastError && (
            <div
              data-testid="combat-error-banner"
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
