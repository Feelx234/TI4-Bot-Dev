import React, { useState, useEffect, useMemo } from 'react';
import { PendingChoiceDto, PlayerView } from '../protocol/types.ts';
import { getMovementPayload } from '../presentation/choiceModel.ts';
import { usePipelineRunner, SemanticIntent } from '../hooks/usePipelineRunner.ts';

export interface TacticalMovementOverlayProps {
  choice: PendingChoiceDto | null;
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

  const { executePipeline, isRunning: isPipelineRunning } = usePipelineRunner(choice, onSubmit);

  const doneMovingOption = useMemo(() => {
    return (
      choice?.options.find((o) => o.id === 'done_moving' || o.kind === 'decline') ?? null
    );
  }, [choice]);

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
      const capacityPerUnit = p.capacity ?? (unit.includes('carrier') ? 4 : unit.includes('dreadnought') ? 1 : 0);

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

  const handleCommitMoves = async () => {
    if (totalUnitsStaged === 0) {
      // Nothing staged, just finish movement
      if (doneMovingOption) {
        setIsDirectSubmitting(true);
        try {
          await onSubmit(doneMovingOption.id);
        } finally {
          setIsDirectSubmitting(false);
        }
      }
      return;
    }

    // Build semantic intent queue
    const intents: SemanticIntent[] = [];

    for (const g of shipGroups) {
      const key = `${g.originSystemId}:${g.unitType}`;
      const count = stagedMoves[key] ?? 0;
      for (let i = 0; i < count; i++) {
        intents.push({
          kind: 'movement',
          predicate: (opt) => {
            const p = getMovementPayload(opt);
            return p.origin === g.originSystemId && p.unit === g.unitType;
          },
          description: `Move ${g.unitType} from #${g.originSystemId}`,
        });
      }
    }

    // Append done_moving to close the movement step
    if (doneMovingOption) {
      intents.push({
        kind: 'movement',
        predicate: (opt) => opt.id === 'done_moving' || opt.kind === 'decline',
        description: 'Finish movement',
      });
    }

    executePipeline(intents);
  };

  if (!isOpen || !choice) return null;

  return (
    <aside
      role="region"
      aria-label="Tactical Fleet Rally Tray"
      data-testid="tactical-movement-tray"
      className="fleet-rally-tray panel"
      style={{
        position: 'absolute',
        bottom: 16,
        left: 16,
        right: 16,
        maxWidth: 720,
        margin: '0 auto',
        zIndex: 25,
        background: 'rgba(15, 23, 42, 0.95)',
        backdropFilter: 'blur(10px)',
        border: '1px solid #38bdf8',
        borderRadius: 10,
        padding: '14px 20px',
        boxShadow: '0 8px 32px rgba(0, 0, 0, 0.7)',
        display: 'flex',
        flexDirection: 'column',
        gap: 12,
        color: '#f8fafc',
      }}
    >
      {/* Header */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div>
          <div style={{ fontSize: 11, fontWeight: 700, color: '#38bdf8', textTransform: 'uppercase' }}>
            Tactical Action • Fleet Movement Staging
          </div>
          <h3 style={{ margin: '2px 0 0 0', fontSize: 16, fontWeight: 700, color: '#f8fafc' }}>
            Destination System: #{activeSystemId ?? 'Active'}
          </h3>
        </div>

        <button
          type="button"
          data-testid="close-movement-tray"
          onClick={onClose}
          className="button button--secondary button--icon"
          aria-label="Close rally tray"
          style={{ padding: '2px 8px', fontSize: 14, minWidth: 28, height: 28 }}
        >
          ✕
        </button>
      </div>

      {/* Capacity & Fleet Supply Gauges */}
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
        <div
          data-testid="fleet-supply-gauge"
          style={{
            background: '#1e293b',
            borderRadius: 6,
            padding: '8px 12px',
            border: '1px solid #334155',
            fontSize: 12,
          }}
        >
          <div style={{ display: 'flex', justifyContent: 'space-between' }}>
            <span style={{ color: '#94a3b8' }}>Fleet Supply:</span>
            <span style={{ fontWeight: 700, color: totalNonFightersMoving > fleetTokens ? '#ef4444' : '#4ade80' }}>
              {totalNonFightersMoving} / {fleetTokens} Ships
            </span>
          </div>
        </div>

        <div
          data-testid="cargo-capacity-gauge"
          style={{
            background: '#1e293b',
            borderRadius: 6,
            padding: '8px 12px',
            border: '1px solid #334155',
            fontSize: 12,
          }}
        >
          <div style={{ display: 'flex', justifyContent: 'space-between' }}>
            <span style={{ color: '#94a3b8' }}>Cargo Capacity:</span>
            <span style={{ fontWeight: 700, color: totalCargoMoving > totalCapacityProvided ? '#ef4444' : '#38bdf8' }}>
              {totalCargoMoving} / {totalCapacityProvided} Loaded
            </span>
          </div>
        </div>
      </div>

      {/* Ship List by Origin */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: 8, maxHeight: 180, overflowY: 'auto' }}>
        {shipGroups.length === 0 ? (
          <div style={{ padding: 12, textAlign: 'center', color: '#94a3b8', fontSize: 13 }}>
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
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  background: '#1e293b',
                  borderRadius: 6,
                  padding: '6px 12px',
                  border: '1px solid #334155',
                }}
              >
                <div>
                  <div style={{ fontSize: 13, fontWeight: 600, color: '#f8fafc', textTransform: 'capitalize' }}>
                    {g.unitType}
                  </div>
                  <div style={{ fontSize: 11, color: '#94a3b8' }}>
                    Origin: #{g.originSystemId} • Available: {g.totalAvailable}
                  </div>
                </div>

                <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                  <button
                    type="button"
                    data-testid={`rally-dec-${g.originSystemId}-${g.unitType}`}
                    onClick={() => handleUpdateCount(key, -1, g.totalAvailable)}
                    disabled={count <= 0 || isPipelineRunning || isDirectSubmitting}
                    className="button button--secondary button--icon"
                    style={{ minWidth: 26, height: 26 }}
                  >
                    -
                  </button>
                  <span
                    data-testid={`rally-count-${g.originSystemId}-${g.unitType}`}
                    style={{ fontSize: 13, fontWeight: 700, minWidth: 20, textAlign: 'center' }}
                  >
                    {count}
                  </span>
                  <button
                    type="button"
                    data-testid={`rally-inc-${g.originSystemId}-${g.unitType}`}
                    onClick={() => handleUpdateCount(key, 1, g.totalAvailable)}
                    disabled={count >= g.totalAvailable || isPipelineRunning || isDirectSubmitting}
                    className="button button--secondary button--icon"
                    style={{ minWidth: 26, height: 26 }}
                  >
                    +
                  </button>
                </div>
              </div>
            );
          })
        )}
      </div>

      {lastError && (
        <div
          data-testid="movement-error-banner"
          role="alert"
          style={{
            background: 'rgba(239, 68, 68, 0.15)',
            border: '1px solid #ef4444',
            color: '#fca5a5',
            padding: '6px 10px',
            borderRadius: 6,
            fontSize: 12,
          }}
        >
          {lastError}
        </div>
      )}

      {/* Action Footer */}
      <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 10, paddingTop: 4 }}>
        {doneMovingOption && (
          <button
            type="button"
            data-testid="finish-movement-btn"
            onClick={() => onSubmit(doneMovingOption.id)}
            disabled={isPipelineRunning || isDirectSubmitting}
            className="button button--secondary"
            style={{ fontSize: 13, padding: '8px 16px' }}
          >
            {doneMovingOption.label || 'Finish Movement'}
          </button>
        )}

        <button
          type="button"
          data-testid="commit-moves-btn"
          onClick={handleCommitMoves}
          disabled={isPipelineRunning || isDirectSubmitting}
          className="button button--primary"
          style={{ fontSize: 13, padding: '8px 20px' }}
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
