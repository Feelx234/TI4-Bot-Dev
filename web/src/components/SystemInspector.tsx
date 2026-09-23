import React from 'react';
import { SelectedSystemDetails } from '../presentation/boardPresentation.ts';
import { Drawer } from '../primitives/index.ts';
import { SeatBadge, useParticipantText, usePlayerIdentity } from '../presentation/PlayerIdentity.tsx';

export interface SystemInspectorProps {
  system: SelectedSystemDetails | null;
  onClose: () => void;
  onSelectAction?: (optionId: string) => void;
}

export const SystemInspector: React.FC<SystemInspectorProps> = ({
  system,
  onClose,
  onSelectAction,
}) => {
  const display = usePlayerIdentity();
  const present = useParticipantText();
  if (!system) return null;

  return (
    <Drawer
      open={true}
      onClose={onClose}
      modal={false}
      position="right"
      ariaLabel={`System ${system.systemId} details`}
      data-testid="system-inspector"
      className="system-inspector panel"
      style={{
        position: 'absolute',
        top: 16,
        right: 16,
        bottom: 'auto',
        width: 320,
        maxHeight: 'calc(100% - 32px)',
        overflowY: 'auto',
        zIndex: 'var(--layer-drawer)',
        background: 'rgba(15, 23, 42, 0.95)',
        backdropFilter: 'blur(8px)',
        border: '1px solid #38bdf8',
        borderRadius: 8,
        padding: 16,
        boxShadow: '0 8px 24px rgba(0, 0, 0, 0.5)',
        display: 'flex',
        flexDirection: 'column',
        gap: 12,
        color: '#f8fafc',
        fontSize: 13,
      }}
    >
      {/* Header */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
        <div>
          <div style={{ fontSize: 11, fontWeight: 'bold', color: '#38bdf8', textTransform: 'uppercase' }}>
            System Details
          </div>
          <h3
            data-testid="inspector-system-title"
            style={{ margin: '2px 0 0 0', fontSize: 16, fontWeight: 700, color: '#f8fafc' }}
          >
            {system.label} <span style={{ color: '#94a3b8', fontSize: 13 }}>#{system.systemId}</span>
          </h3>
          {system.isActiveSystem && (
            <span
              style={{
                display: 'inline-block',
                marginTop: 4,
                padding: '2px 6px',
                borderRadius: 4,
                fontSize: 10,
                fontWeight: 700,
                background: 'rgba(56, 189, 248, 0.2)',
                color: '#38bdf8',
                border: '1px solid #38bdf8',
              }}
            >
              ACTIVE SYSTEM
            </span>
          )}
        </div>
        <button
          type="button"
          data-testid="close-inspector-button"
          onClick={onClose}
          className="button button--secondary button--icon"
          aria-label="Close inspector"
          style={{ padding: '2px 8px', fontSize: 14, minWidth: 28, height: 28 }}
        >
          ✕
        </button>
      </div>

      {/* Anomalies & Wormholes */}
      {(system.anomalies.length > 0 || system.wormholes.length > 0) && (
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
          {system.anomalies.map((a, i) => (
            <span
              key={i}
              data-testid="inspector-anomaly"
              style={{
                padding: '2px 6px',
                borderRadius: 4,
                fontSize: 11,
                fontWeight: 600,
                background: '#451a03',
                color: '#fef08a',
                border: '1px solid #d97706',
              }}
            >
              {a.toUpperCase()}
            </span>
          ))}
          {system.wormholes.map((wh, i) => (
            <span
              key={i}
              data-testid="inspector-wormhole"
              style={{
                padding: '2px 6px',
                borderRadius: 4,
                fontSize: 11,
                fontWeight: 600,
                background: '#2e1065',
                color: '#c084fc',
                border: '1px solid #9333ea',
              }}
            >
              WORMHOLE: {wh.toUpperCase()}
            </span>
          ))}
        </div>
      )}

      {/* Planets */}
      <div>
        <h4 style={{ margin: '0 0 6px 0', fontSize: 12, fontWeight: 700, color: '#94a3b8', textTransform: 'uppercase' }}>
          Planets ({system.planets.length})
        </h4>
        {system.planets.length === 0 ? (
          <div style={{ color: '#64748b', fontSize: 12 }}>No planets in this system.</div>
        ) : (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            {system.planets.map((p) => (
              <div
                key={p.id}
                data-testid={`inspector-planet-${p.id}`}
                style={{
                  padding: '8px 10px',
                  borderRadius: 6,
                  background: 'rgba(30, 41, 59, 0.7)',
                  border: `1px solid ${p.isCandidateTarget ? '#38bdf8' : '#334155'}`,
                }}
              >
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <span style={{ fontWeight: 600, color: '#f8fafc' }}>
                    {p.label}{' '}
                    {p.legendary && (
                      <span style={{ fontSize: 10, color: '#facc15', fontWeight: 700 }} title="Legendary Planet">
                        ★
                      </span>
                    )}
                  </span>
                  <span style={{ fontSize: 12, color: '#facc15', fontWeight: 600 }}>
                    {p.resources} Res / {p.influence} Inf
                  </span>
                </div>

                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: 4, fontSize: 11 }}>
                  <span>
                    Control:{' '}
                    {p.controlledBy ? (
                       <span style={{ color: '#f8fafc', borderLeft: `3px solid ${p.controllerColor}`, paddingLeft: 3, fontWeight: 600 }}>
                        {display(p.controlledBy).position && <SeatBadge position={display(p.controlledBy).position!} />} {display(p.controlledBy).label} {p.exhausted ? '(Exhausted)' : '(Ready)'}
                      </span>
                    ) : (
                      <span style={{ color: '#64748b' }}>Uncontrolled</span>
                    )}
                  </span>
                  {p.traits.length > 0 && (
                    <span style={{ color: '#94a3b8' }}>{p.traits.join(', ')}</span>
                  )}
                </div>

                {p.attachments.length > 0 && (
                  <div style={{ marginTop: 4, fontSize: 11, color: '#a78bfa' }}>
                    Attachments: {p.attachments.join(', ')}
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Units */}
      <div>
        <h4 style={{ margin: '0 0 6px 0', fontSize: 12, fontWeight: 700, color: '#94a3b8', textTransform: 'uppercase' }}>
          Units ({system.spaceUnits.length + Object.values(system.planetUnits).flat().length})
        </h4>

        {/* Space Units */}
        <div style={{ marginBottom: 6 }}>
          <div style={{ fontSize: 11, fontWeight: 600, color: '#cbd5e1' }}>Space Roster:</div>
          {system.spaceUnits.length === 0 ? (
            <div style={{ color: '#64748b', fontSize: 11, marginLeft: 6 }}>No space units</div>
          ) : (
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4, marginTop: 2, marginLeft: 6 }}>
              {system.spaceUnits.map((u, i) => (
                <span
                  key={i}
                  data-testid="inspector-space-unit"
                  style={{
                    padding: '2px 6px',
                    borderRadius: 4,
                    fontSize: 11,
                    background: '#1e293b',
                    borderLeft: `3px solid ${u.ownerColor}`,
                    color: u.damaged ? '#fca5a5' : '#e2e8f0',
                  }}
                >
                   {u.unitType} {u.damaged && '(Damaged)'} [{display(u.owner).position && <SeatBadge position={display(u.owner).position!} />} {display(u.owner).label}]
                </span>
              ))}
            </div>
          )}
        </div>

        {/* Ground Units */}
        {Object.entries(system.planetUnits).map(([pId, pUnits]) => (
          <div key={pId} style={{ marginTop: 4 }}>
            <div style={{ fontSize: 11, fontWeight: 600, color: '#cbd5e1' }}>
              On {system.planets.find((p) => p.id === pId)?.label || pId}:
            </div>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4, marginTop: 2, marginLeft: 6 }}>
              {pUnits.map((u, i) => (
                <span
                  key={i}
                  data-testid="inspector-ground-unit"
                  style={{
                    padding: '2px 6px',
                    borderRadius: 4,
                    fontSize: 11,
                    background: '#1e293b',
                    borderLeft: `3px solid ${u.ownerColor}`,
                    color: u.damaged ? '#fca5a5' : '#e2e8f0',
                  }}
                >
                   {u.unitType} {u.damaged && '(Damaged)'} [{display(u.owner).position && <SeatBadge position={display(u.owner).position!} />} {display(u.owner).label}]
                </span>
              ))}
            </div>
          </div>
        ))}
      </div>

      {/* Command Tokens */}
      {system.commandTokens.length > 0 && (
        <div>
          <h4 style={{ margin: '0 0 4px 0', fontSize: 12, fontWeight: 700, color: '#94a3b8', textTransform: 'uppercase' }}>
            Command Tokens
          </h4>
          <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
            {system.commandTokens.map((ct, i) => (
              <span
                key={i}
                data-testid="inspector-command-token"
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  gap: 4,
                  fontSize: 11,
                  padding: '2px 6px',
                  borderRadius: 4,
                  background: '#1e293b',
                  color: '#e2e8f0',
                }}
              >
                 <span style={{ width: 8, height: 8, borderRadius: '50%', background: ct.color, outline: '1px solid #f8fafc' }} />
                 {display(ct.owner).position && <SeatBadge position={display(ct.owner).position!} />} {display(ct.owner).label}
              </span>
            ))}
          </div>
        </div>
      )}

      {/* Available Actions (When viewer has legal choices targeting this system) */}
      {system.availableActions.length > 0 && (
        <div style={{ marginTop: 4, paddingTop: 8, borderTop: '1px solid #334155' }}>
          <h4 style={{ margin: '0 0 6px 0', fontSize: 12, fontWeight: 700, color: '#38bdf8', textTransform: 'uppercase' }}>
            Available Decision Actions
          </h4>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
            {system.availableActions.map((act) => (
              <button
                key={act.optionId}
                type="button"
                data-testid={`inspector-action-${act.optionId}`}
                onClick={() => onSelectAction?.(act.optionId)}
                className="button button--primary"
                style={{
                  padding: '6px 12px',
                  fontSize: 12,
                  textAlign: 'left',
                  borderRadius: 6,
                }}
              >
                 {present(act.label)}
              </button>
            ))}
          </div>
        </div>
      )}
    </Drawer>
  );
};
