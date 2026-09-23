import React, { useState, useEffect, useMemo } from 'react';
import { PendingChoiceDto, PlayerView } from '../protocol/types.ts';
import { getMovementPayload, ChoiceRendererModel } from '../presentation/choiceModel.ts';
import { usePipelineRunner, SemanticIntent } from '../hooks/usePipelineRunner.ts';

export interface TacticalMovementOverlayProps {
  choice: PendingChoiceDto | null;
  model?: ChoiceRendererModel | null;
  viewerSeat?: string | null;
  activeSystemId?: string | null;
  player?: PlayerView | null;
  onSubmit: (optionId: string) => Promise<void>;
  isOpen: boolean;
  onClose: () => void;
  lastError?: string | null;
}

interface OriginShipGroup {
  originSystemId: string;
  unitType: string;
  totalAvailable: number;
  capacityPerUnit: number;
  isFighter: boolean;
  isGroundForce: boolean;
  options: { id: string; gravityDrive?: boolean }[];
}

export const TacticalMovementOverlay: React.FC<TacticalMovementOverlayProps> = ({
  choice,
  model,
  activeSystemId,
  player,
  onSubmit,
  isOpen,
  onClose,
  lastError,
}) => {
  // Map of `${originSystemId}:${unitType}` -> count to move
  const [stagedMoves, setStagedMoves] = useState<Record<string, number>>({});
  const [isDirectSubmitting, setIsDirectSubmitting] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);

  const { executePipeline, isRunning: isPipelineRunning, lastError: pipelineError } = usePipelineRunner(choice, onSubmit);

  const doneMovingOption = useMemo(() => {
    return (
      model?.declineOption ?? (choice?.options.find((o) => o.id === 'done_moving' || o.kind === 'decline') ?? null)
    );
  }, [choice, model]);

  // Extract and group move options
  const shipGroups = useMemo(() => {
    if (!choice) return [] as OriginShipGroup[];

    const groupMap = new Map<string, OriginShipGroup>();

    for (const opt of choice.options) {
      if (opt.id === 'done_moving' || opt.kind === 'decline') continue;

      const p = getMovementPayload(opt);
      const origin = p.origin ?? 'unknown';
      const unit = p.unit ?? opt.label.toLowerCase();
      const key = `${origin}:${unit}`;

      const isFighter = unit.includes('fighter');
      const isGroundForce = unit.includes('infantry') || unit.includes('mech');
      // Capacity comes from the engine payload; default to 0 rather than
      // hard-coding faction-specific values (Nomad, Nekro, Titans, etc.).
      const capacityPerUnit = p.capacity ?? 0;

      const existing = groupMap.get(key);
      if (existing) {
        existing.totalAvailable += 1;
        existing.options.push({ id: opt.id, gravityDrive: p.gravity_drive });
      } else {
        groupMap.set(key, {
          originSystemId: origin,
          unitType: unit,
          totalAvailable: 1,
          capacityPerUnit,
          isFighter,
          isGroundForce,
          options: [{ id: opt.id, gravityDrive: p.gravity_drive }],
        });
      }
    }

    return Array.from(groupMap.values());
  }, [choice]);

  // Reset staging on choice nonce change
  useEffect(() => {
    setStagedMoves({});
    setIsDirectSubmitting(false);
    setLocalError(null);
  }, [choice?.nonce]);

  const fleetTokens = player?.fleet_tokens ?? 3;

  // Capacity & Fleet calculations
  const { totalNonFightersMoving, totalCapacityProvided, totalCargoMoving } = useMemo(() => {
    let nonFighters = 0;
    let capacity = 0;
    let cargo = 0;

    for (const g of shipGroups) {
      const key = `${g.originSystemId}:${g.unitType}`;
      const count = stagedMoves[key] ?? 0;
      if (count === 0) continue;

      if (!g.isFighter && !g.isGroundForce) {
        nonFighters += count;
        capacity += count * g.capacityPerUnit;
      } else {
        cargo += count;
      }
    }

    return {
      totalNonFightersMoving: nonFighters,
      totalCapacityProvided: capacity,
      totalCargoMoving: cargo,
    };
  }, [shipGroups, stagedMoves]);

  const totalUnitsStaged = Object.values(stagedMoves).reduce((a, b) => a + b, 0);

  const handleUpdateCount = (key: string, delta: number, max: number) => {
    setStagedMoves((prev) => {
      const current = prev[key] ?? 0;
      const next = Math.max(0, Math.min(max, current + delta));
      return { ...prev, [key]: next };
    });
  };

  const submitFinish = async () => {
    setLocalError(null);
    if (!doneMovingOption) {
      setLocalError('Cannot finish movement: no finish option was offered.');
      return;
    }
    setIsDirectSubmitting(true);
    try {
      await onSubmit(doneMovingOption.id);
    } catch (error) {
      setLocalError(`Could not finish movement: ${String(error)}`);
    } finally {
      setIsDirectSubmitting(false);
    }
  };

  const handleCommitMoves = async () => {
    if (totalUnitsStaged === 0) {
      await submitFinish();
      return;
    }
    setLocalError(null);

    // Build semantic intent queue
    const intents: SemanticIntent[] = [];

    for (const g of shipGroups) {
      const key = `${g.originSystemId}:${g.unitType}`;
      const count = stagedMoves[key] ?? 0;
      for (let i = 0; i < count; i++) {
        intents.push({
          predicate: (opt) => {
            const p = getMovementPayload(opt);
            return p.origin === g.originSystemId && p.unit === g.unitType;
          },
        });
      }
    }

    // Append done_moving to close the movement step
    if (!doneMovingOption) {
      setLocalError('Cannot finish movement: no finish option was offered.');
      return;
    }
    intents.push({ predicate: (opt) => opt.id === doneMovingOption.id });

    executePipeline(intents);
  };

  if (!isOpen || !choice) return null;

  const destinationSystemId =
    activeSystemId ??
    (model?.selectionMode.mode === 'tactical_move' ? model.selectionMode.activeSystem : null) ??
    (choice?.context?.target && 'System' in choice.context.target ? choice.context.target.System : null);

  return (
    <aside
      role="region"
      aria-label="Tactical Fleet Rally Tray"
      data-testid="tactical-movement-tray"
      className="fleet-rally-tray panel"
    >
      {/* Header */}
      <div className="choice-workflow-row">
        <div>
          <div className="choice-workflow-eyebrow">
            Tactical Action • Fleet Movement Staging
          </div>
          <h3 className="choice-workflow-title">
            Destination System: #{destinationSystemId ?? 'Active'}
          </h3>
        </div>

        <button
          type="button"
          data-testid="close-movement-tray"
          onClick={onClose}
          aria-label="Close rally tray"
          className="button button--secondary button--icon choice-workflow-close"
        >
          ✕
        </button>
      </div>

      {/* Capacity & Fleet Supply Gauges */}
      <div className="fleet-rally-tray__gauges">
        <div
          data-testid="fleet-supply-gauge"
          className="workflow-card"
        >
          <div className="workflow-card--row">
            <span className="text-muted">Fleet Supply:</span>
            <span className="fleet-rally-tray__status" data-alert={totalNonFightersMoving > fleetTokens}>
              {totalNonFightersMoving} / {fleetTokens} Ships
            </span>
          </div>
        </div>

        <div
          data-testid="cargo-capacity-gauge"
          className="workflow-card"
        >
          <div className="workflow-card--row">
            <span className="text-muted">Cargo Capacity:</span>
            <span className="fleet-rally-tray__status" data-alert={totalCargoMoving > totalCapacityProvided}>
              {totalCargoMoving} / {totalCapacityProvided} Loaded
            </span>
          </div>
        </div>
      </div>

      {/* Ship List by Origin */}
      <div className="fleet-rally-tray__list">
        {shipGroups.length === 0 ? (
          <div className="text-muted">
            No ships eligible to move into the active system.
          </div>
        ) : (
          shipGroups.map((g) => {
            const key = `${g.originSystemId}:${g.unitType}`;
            const count = stagedMoves[key] ?? 0;

            return (
              <div
                key={key}
                data-testid={`rally-row-${g.originSystemId}-${g.unitType}`}
                className="workflow-card workflow-card--row"
              >
                <div>
                  <div className="workflow-unit-name">
                    {g.unitType}
                  </div>
                  <div className="text-muted">
                    Origin: #{g.originSystemId} • Available: {g.totalAvailable}
                  </div>
                </div>

                <div className="workflow-row">
                  <button
                    type="button"
                    data-testid={`rally-dec-${g.originSystemId}-${g.unitType}`}
                    onClick={() => handleUpdateCount(key, -1, g.totalAvailable)}
                    disabled={count <= 0 || isPipelineRunning || isDirectSubmitting}
                    className="button button--secondary button--icon workflow-button--stepper"
                  >
                    -
                  </button>
                  <span
                    data-testid={`rally-count-${g.originSystemId}-${g.unitType}`}
                    className="workflow-count"
                  >
                    {count}
                  </span>
                  <button
                    type="button"
                    data-testid={`rally-inc-${g.originSystemId}-${g.unitType}`}
                    onClick={() => handleUpdateCount(key, 1, g.totalAvailable)}
                    disabled={count >= g.totalAvailable || isPipelineRunning || isDirectSubmitting}
                    className="button button--secondary button--icon workflow-button--stepper"
                  >
                    +
                  </button>
                </div>
              </div>
            );
          })
        )}
      </div>

      {(localError || pipelineError || lastError) && (
        <div
          data-testid="movement-error-banner"
          role="alert"
          className="workflow-error"
        >
          {localError || pipelineError || lastError}
        </div>
      )}

      {/* Action Footer */}
      <div className="workflow-actions">
        {doneMovingOption && (
          <button
            type="button"
            data-testid="finish-movement-btn"
            onClick={submitFinish}
            disabled={isPipelineRunning || isDirectSubmitting}
            className="button button--secondary workflow-button--wide"
          >
            {doneMovingOption.label || 'Finish Movement'}
          </button>
        )}

        <button
          type="button"
          data-testid="commit-moves-btn"
          onClick={handleCommitMoves}
          disabled={isPipelineRunning || isDirectSubmitting}
          className="button button--primary workflow-button--wide"
        >
          {isPipelineRunning || isDirectSubmitting
            ? 'Moving Fleet...'
            : totalUnitsStaged > 0
              ? `Commit Moves (${totalUnitsStaged})`
              : 'Done Moving'}
        </button>
      </div>
    </aside>
  );
};
