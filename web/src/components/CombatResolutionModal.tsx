import React, { useState, useEffect, useMemo } from 'react';
import { PendingChoiceDto } from '../protocol/types.ts';
import { getCombatPayload, ChoiceRendererModel } from '../presentation/choiceModel.ts';
import { usePipelineRunner, SemanticIntent } from '../hooks/usePipelineRunner.ts';
import { Dialog } from '../primitives/index.ts';
import { WorkflowShell } from './WorkflowShell.tsx';

export interface CombatResolutionModalProps {
  choice: PendingChoiceDto | null;
  model?: ChoiceRendererModel | null;
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
  model,
  viewerSeat,
  onSubmit,
  isOpen,
  onClose,
  lastError,
  recentDiceRolls = [],
}) => {
  // Staged casualties: unitType -> count
  const [stagedCasualties, setStagedCasualties] = useState<Record<string, number>>({});

  const { executePipeline, isRunning: isPipelineRunning } = usePipelineRunner(choice, onSubmit);

  const subtype = choice?.context?.subtype ?? '';
  const constraints = model?.outstanding?.[0] ?? choice?.context?.outstanding?.[0];
  const hitsOwed = model?.selectionMode.mode === 'casualty'
    ? model.selectionMode.hitsToAssign
    : model?.selectionMode.mode === 'sustain'
      ? model.selectionMode.hitsRemaining
      : (constraints?.amount ?? 1);

  const isSustainStage = subtype === 'sustain_damage' || model?.workflow === 'combat_sustain';
  const isCasualtyStage = subtype === 'assign_casualty' || model?.workflow === 'combat_casualty' || choice?.options.some((o) => o.kind === 'casualty');
  const isRetreatStage = subtype === 'announce_retreat' || subtype === 'retreat_to' || model?.workflow === 'combat_retreat';

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

  const handleConfirmCasualties = async (isDirectSubmitting: boolean, submitDirect: (optionId: string) => Promise<void>) => {
    if (!isCasualtyAllocationValid || isPipelineRunning || isDirectSubmitting || !choice) return;

    const intents: SemanticIntent[] = [];

    for (const g of casualtyGroups) {
      const count = stagedCasualties[g.unitType] ?? 0;
      for (let i = 0; i < count; i++) {
        intents.push({
          predicate: (opt) => {
            const p = getCombatPayload(opt);
            if (p.unit && p.unit.toLowerCase() === g.unitType.toLowerCase()) return true;
            if (opt.id === `destroy|${g.unitType}` || opt.id.startsWith(`destroy|${g.unitType}|`)) return true;
            const parts = opt.id.toLowerCase().split('|');
            return parts.includes(g.unitType.toLowerCase());
          },
        });
      }
    }

    if (intents.length === 1) {
        const opt = choice.options.find(intents[0].predicate);
        if (opt) await submitDirect(opt.id);
    } else {
      executePipeline(intents);
    }
  };

  if (!isOpen || !choice) return null;

  return (
    <Dialog.Root open={isOpen} onOpenChange={(open) => { if (!open) onClose(); }}>
      <Dialog.Content
        data-testid="combat-resolution-modal"
        className="combat-dialog choice-workflow-dialog"

      >
        <div
          className="panel choice-workflow-modal"

        >
          {/* Header */}
          <div className="workflow-inline" >
            <div>
              <div className="workflow-inline" >
                Space Combat Arena • Seat: {choice.actor}
              </div>
              <Dialog.Title
                as="h2"
                data-testid="combat-stage-title"
                className="workflow-inline"
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

            >
              ✕
            </button>
          </div>

          {/* Dice Results Feed */}
          {recentDiceRolls.length > 0 && (
            <div
              data-testid="combat-dice-feed"
              className="workflow-inline"
            >
              <div className="workflow-inline" >
                Recent Combat Rolls
              </div>
              <div className="workflow-inline" >
                {recentDiceRolls.map((d, i) => (
                  <span
                    key={i}
                    data-testid="dice-roll-badge"
                    className="workflow-inline"
                  >
                    {d.unit} ({d.target}+): [{d.roll}] {d.hit ? '★ HIT' : 'MISS'}
                  </span>
                ))}
              </div>
            </div>
          )}

          <WorkflowShell
            choice={choice}
            model={model}
            viewerSeat={viewerSeat}
            onSubmit={onSubmit}
            lastError={lastError}
            spectatorNotice={`Observing combat resolution in progress for seat ${choice.actor}...`}
            spectatorNoticeTestId="spectator-combat-notice"
            errorTestId="combat-error-banner"
          >
            {({ isActor, isDirectSubmitting, declineOption, submitDirect }) => <>

          {/* Stage 1: Sustain Damage */}
          {isActor && isSustainStage && (
            <div className="workflow-inline" >
              <div
                className="workflow-inline"
              >
                ⚠️ Caution: Opponents holding "Direct Hit" action cards may react to destroy sustained ships!
              </div>

              <div className="workflow-inline" >
                Select capital ships to sustain damage, or choose decline to proceed to direct hits:
              </div>

              <div className="workflow-inline" >
                {choice.options
                  .filter((o) => o.id !== 'decline' && o.kind !== 'decline')
                  .map((opt) => (
                    <button
                      key={opt.id}
                      type="button"
                      data-testid={`sustain-opt-${opt.id}`}
                       onClick={() => submitDirect(opt.id)}
                      disabled={isDirectSubmitting}
                      className="button button--secondary"

                    >
                      <span className="workflow-inline" >{opt.label}</span>
                      <span className="workflow-inline" >
                        Sustain Hit
                      </span>
                    </button>
                  ))}
              </div>

              {declineOption && (
                <div className="workflow-inline" >
                  <button
                    type="button"
                    data-testid="decline-sustain-btn"
                     onClick={() => submitDirect(declineOption.id)}
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
            <div className="workflow-inline" >
              <div className="workflow-inline" >
                <span className="workflow-inline" >
                  Allocate exactly {hitsOwed} hits to destroy your units:
                </span>
                <button
                  type="button"
                  data-testid="auto-cheapest-btn"
                  onClick={handleAutoCheapest}
                  className="button button--secondary"

                >
                  Auto-Cheapest
                </button>
              </div>

              {/* Tally Card */}
              <div
                className="workflow-inline"
              >
                <span className="workflow-inline" >Hits Allocated:</span>
                <span
                  data-testid="casualty-allocated-count"
                  className="workflow-inline"
                >
                  {totalCasualtiesAllocated} / {hitsOwed} Hits
                </span>
              </div>

              {/* Grouped Unit Rows */}
              <div className="workflow-inline" >
                {casualtyGroups.map((g) => {
                  const count = stagedCasualties[g.unitType] ?? 0;
                  return (
                    <div
                      key={g.unitType}
                      data-testid={`casualty-row-${g.unitType}`}
                      className="workflow-inline"
                    >
                      <div>
                        <div className="workflow-inline" >
                          {g.unitType}
                        </div>
                        <div className="workflow-inline" >Available: {g.totalAvailable}</div>
                      </div>

                      <div className="workflow-inline" >
                        <button
                          type="button"
                          data-testid={`casualty-dec-${g.unitType}`}
                          onClick={() => handleUpdateCasualty(g.unitType, -1, g.totalAvailable)}
                          disabled={count <= 0 || isPipelineRunning || isDirectSubmitting}
                          className="button button--secondary button--icon"

                        >
                          -
                        </button>
                        <span
                          data-testid={`casualty-count-${g.unitType}`}
                          className="workflow-inline"
                        >
                          {count}
                        </span>
                        <button
                          type="button"
                          data-testid={`casualty-inc-${g.unitType}`}
                          onClick={() => handleUpdateCasualty(g.unitType, 1, g.totalAvailable)}
                          disabled={count >= g.totalAvailable || isPipelineRunning || isDirectSubmitting}
                          className="button button--secondary button--icon"

                        >
                          +
                        </button>
                      </div>
                    </div>
                  );
                })}
              </div>

              <div className="workflow-inline" >
                <button
                  type="button"
                  data-testid="confirm-casualties-btn"
                   onClick={() => handleConfirmCasualties(isDirectSubmitting, submitDirect)}
                  disabled={!isCasualtyAllocationValid || isPipelineRunning || isDirectSubmitting}
                  className="button button--primary"

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
            <div className="workflow-inline" >
              <div className="workflow-inline" >
                {subtype === 'announce_retreat'
                  ? 'Choose whether to announce a retreat before combat rounds commence:'
                  : 'Select an adjacent system to retreat your surviving fleet to:'}
              </div>

              <div className="workflow-inline" >
                {choice.options.map((opt) => (
                  <button
                    key={opt.id}
                    type="button"
                    data-testid={`retreat-opt-${opt.id}`}
                    onClick={() => submitDirect(opt.id)}
                    disabled={isDirectSubmitting}
                    className="button button--secondary"

                  >
                    {opt.label}
                  </button>
                ))}
              </div>
            </div>
          )}

            </>}
          </WorkflowShell>
        </div>
      </Dialog.Content>
    </Dialog.Root>
  );
};
