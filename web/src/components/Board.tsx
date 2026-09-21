import React, { useState, useRef } from 'react';
import { BoardView, PlayerView, PendingChoiceDto } from '../protocol/types.ts';
import {
  buildBoardPresentationModel,
  getPlayerColor,
  PLAYER_PALETTE,
  TilePresentation,
} from '../presentation/boardPresentation.ts';
import { SystemInspector } from './SystemInspector.tsx';
import { Tooltip, SvgButton } from '../primitives/index.ts';

export { getPlayerColor, PLAYER_PALETTE };

export interface BoardProps {
  board: BoardView;
  seatingOrder: string[];
  players?: readonly PlayerView[];
  pendingChoice?: PendingChoiceDto | null;
  viewerSeat?: string | null;
  selectedSystemId?: string | null;
  onSelectSystem?: (systemId: string) => void;
  onSelectTarget?: (systemId: string, planetId?: string) => void;
}

export const Board: React.FC<BoardProps> = ({
  board,
  seatingOrder,
  players = [],
  pendingChoice = null,
  viewerSeat = null,
  selectedSystemId: controlledSelectedSystemId,
  onSelectSystem,
  onSelectTarget,
}) => {
  const [uncontrolledSelectedSystemId, setUncontrolledSelectedSystemId] = useState<string | null>(null);
  const selectedSystemId = controlledSelectedSystemId ?? uncontrolledSelectedSystemId;
  const setSelectedSystemId = (id: string | null) => {
    if (controlledSelectedSystemId === undefined) {
      setUncontrolledSelectedSystemId(id);
    }
    onSelectSystem?.(id || '');
  };

  const [hoveredTile, setHoveredTile] = useState<{
    systemId: string;
    label: string;
    anomalies?: string[];
    wormholes?: string[];
    planets: { label: string; resources?: number; influence?: number; owner?: string | null }[];
    unitsCount: number;
    tokensCount: number;
  } | null>(null);

  // Pan and zoom state
  const [viewTransform, setViewTransform] = useState({ x: 0, y: 0, scale: 1 });
  const [isPanning, setIsPanning] = useState(false);
  const startPanRef = useRef({ x: 0, y: 0 });

  // Pure presentation derivation
  const presentation = buildBoardPresentationModel(
    board,
    seatingOrder,
    players,
    pendingChoice,
    viewerSeat,
    selectedSystemId
  );

  const handlePointerDown = (e: React.PointerEvent) => {
    if (e.button === 0) {
      setIsPanning(true);
      startPanRef.current = { x: e.clientX - viewTransform.x, y: e.clientY - viewTransform.y };
    }
  };

  const handlePointerMove = (e: React.PointerEvent) => {
    if (isPanning) {
      setViewTransform((prev) => ({
        ...prev,
        x: e.clientX - startPanRef.current.x,
        y: e.clientY - startPanRef.current.y,
      }));
    }
  };

  const handlePointerUp = () => setIsPanning(false);

  const zoomIn = () => setViewTransform((prev) => ({ ...prev, scale: Math.min(prev.scale * 1.25, 2.5) }));
  const zoomOut = () => setViewTransform((prev) => ({ ...prev, scale: Math.max(prev.scale / 1.25, 0.5) }));
  const resetView = () => setViewTransform({ x: 0, y: 0, scale: 1 });

  const handleTileClick = (tile: TilePresentation) => {
    if (tile.isCandidateTarget) {
      onSelectTarget?.(tile.systemId);
    }
    setSelectedSystemId(selectedSystemId === tile.systemId ? null : tile.systemId);
  };

  const handleTileKeyDown = (e: React.KeyboardEvent, tile: TilePresentation) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      handleTileClick(tile);
    }
  };

  return (
    <div
      className="board-container"
      data-testid="board-viewport"
      style={{
        position: 'relative',
        width: '100%',
        height: '100%',
        overflow: 'hidden',
        background: '#090d16',
        userSelect: 'none',
      }}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onPointerCancel={handlePointerUp}
    >
      {/* Pan / Zoom Control Overlay */}
      <div
        className="board-controls"
        style={{
          display: 'flex',
          gap: 6,
        }}
      >
        <Tooltip content="Zoom In">
          <button
            type="button"
            onClick={zoomIn}
            title="Zoom In"
            className="button button--secondary button--icon"
            style={{ fontSize: 16 }}
          >
            +
          </button>
        </Tooltip>
        <Tooltip content="Zoom Out">
          <button
            type="button"
            onClick={zoomOut}
            title="Zoom Out"
            className="button button--secondary button--icon"
            style={{ fontSize: 16 }}
          >
            −
          </button>
        </Tooltip>
        <Tooltip content="Reset Pan & Zoom">
          <button
            type="button"
            onClick={resetView}
            title="Reset Pan & Zoom"
            className="button button--secondary button--icon"
            style={{ fontSize: 14 }}
          >
            ⟲
          </button>
        </Tooltip>
      </div>

      <svg
        viewBox="-600 -500 1200 1000"
        style={{ width: '100%', height: '100%', display: 'block', cursor: isPanning ? 'grabbing' : 'grab' }}
        data-testid="ti4-board-svg"
      >
        <defs>
          <filter id="glow" x="-20%" y="-20%" width="140%" height="140%">
            <feGaussianBlur stdDeviation="4" result="blur" />
            <feComposite in="SourceGraphic" in2="blur" operator="over" />
          </filter>
          <filter id="target-glow" x="-30%" y="-30%" width="160%" height="160%">
            <feGaussianBlur stdDeviation="6" result="blur" />
            <feMerge>
              <feMergeNode in="blur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
          <marker
            id="vector-arrow"
            viewBox="0 0 10 10"
            refX="8"
            refY="5"
            markerWidth="6"
            markerHeight="6"
            orient="auto-start-reverse"
          >
            <path d="M 0 1 L 10 5 L 0 9 z" fill="#4ade80" />
          </marker>
        </defs>

        <g transform={`translate(${viewTransform.x}, ${viewTransform.y}) scale(${viewTransform.scale})`}>
          {presentation.tiles.map((tile, idx) => {
            const sysId = tile.systemId;
            const isSelected = selectedSystemId === sysId;
            const singlePlanetOwner =
              tile.planets.length > 0 &&
              tile.planets.every((p) => p.controlledBy && p.controlledBy === tile.planets[0].controlledBy)
                ? tile.planets[0].controlledBy
                : null;

            return (
              <SvgButton
                key={`hex-${sysId}-${idx}`}
                data-testid={`system-hex-${sysId}`}
                data-system-id={sysId}
                data-target-candidate={tile.isCandidateTarget ? 'true' : undefined}
                data-context-subject={tile.isContextSubject ? 'true' : undefined}
                data-system-selected={isSelected ? 'true' : undefined}
                isInteractive={tile.isCandidateTarget}
                label={tile.isCandidateTarget ? `Target system ${tile.label} #${sysId}` : ''}
                onActivate={() => handleTileClick(tile)}
                onClick={() => handleTileClick(tile)}
                onKeyDown={(e) => handleTileKeyDown(e, tile)}
                onMouseEnter={() => {
                  setHoveredTile({
                    systemId: sysId,
                    label: tile.label,
                    anomalies: tile.anomalies,
                    wormholes: tile.wormholes.map((w) => w.kind),
                    planets: tile.planets.map((p) => ({
                      label: p.label,
                      resources: p.resources,
                      influence: p.influence,
                      owner: p.controlledBy,
                    })),
                    unitsCount: tile.totalUnits,
                    tokensCount: tile.commandTokens.length,
                  });
                }}
                onMouseLeave={() => setHoveredTile(null)}
                style={{
                  cursor: tile.isCandidateTarget ? 'pointer' : 'default',
                  outline: 'none',
                }}
              >
                {/* Hexagon Tile */}
                <polygon
                  points={tile.points}
                  fill={tile.fillColor}
                  stroke={isSelected ? '#facc15' : tile.isCandidateTarget ? '#38bdf8' : tile.strokeColor}
                  strokeWidth={isSelected ? 4 : tile.isCandidateTarget ? 3.5 : tile.strokeWidth}
                  strokeDasharray={tile.strokeDashArray}
                  filter={tile.isCandidateTarget ? 'url(#target-glow)' : undefined}
                />

                {/* Candidate target highlight animation ring */}
                {tile.isCandidateTarget && (
                  <polygon
                    points={tile.innerPoints}
                    fill="none"
                    stroke="#38bdf8"
                    strokeWidth={2}
                    strokeDasharray="5 3"
                    className="target-pulse-ring"
                  />
                )}

                {/* Activation Mode Target Reticle */}
                {tile.isCandidateTarget && presentation.targets.isActivationMode && (
                  <g data-testid="activation-target-reticle" style={{ pointerEvents: 'none' }}>
                    <circle
                      cx={tile.center.x}
                      cy={tile.center.y}
                      r={28}
                      fill="none"
                      stroke="#38bdf8"
                      strokeWidth={2}
                      strokeDasharray="4 2"
                    />
                    <line
                      x1={tile.center.x - 34}
                      y1={tile.center.y}
                      x2={tile.center.x - 20}
                      y2={tile.center.y}
                      stroke="#38bdf8"
                      strokeWidth={2}
                    />
                    <line
                      x1={tile.center.x + 20}
                      y1={tile.center.y}
                      x2={tile.center.x + 34}
                      y2={tile.center.y}
                      stroke="#38bdf8"
                      strokeWidth={2}
                    />
                    <line
                      x1={tile.center.x}
                      y1={tile.center.y - 34}
                      x2={tile.center.x}
                      y2={tile.center.y - 20}
                      stroke="#38bdf8"
                      strokeWidth={2}
                    />
                    <line
                      x1={tile.center.x}
                      y1={tile.center.y + 20}
                      x2={tile.center.x}
                      y2={tile.center.y + 34}
                      stroke="#38bdf8"
                      strokeWidth={2}
                    />
                  </g>
                )}

                {/* Inner border for exclusive planet control */}
                {singlePlanetOwner && !tile.isCandidateTarget && (
                  <polygon
                    points={tile.innerPoints}
                    fill="none"
                    stroke={getPlayerColor(singlePlanetOwner, seatingOrder)}
                    strokeWidth={2}
                    strokeDasharray="4 2"
                  />
                )}

                {/* Tile Label / System Number */}
                <text
                  x={tile.center.x}
                  y={tile.center.y - 48}
                  textAnchor="middle"
                  fill="#cbd5e1"
                  fontSize="11"
                  fontWeight="bold"
                  pointerEvents="none"
                >
                  {sysId === '18' ? 'Mecatol Rex' : tile.label ? (tile.label.startsWith('#') ? tile.label : `#${sysId}`) : `#${sysId}`}
                </text>

                {/* Anomaly badge text */}
                {tile.anomalyLabel && (
                  <text
                    x={tile.center.x}
                    y={tile.center.y - 36}
                    textAnchor="middle"
                    fill="#fef08a"
                    fontSize="8"
                    fontWeight="bold"
                    pointerEvents="none"
                  >
                    {tile.anomalyLabel}
                  </text>
                )}

                {/* Printed Wormholes */}
                {tile.wormholes.map((wh, wIdx) => {
                  const wX = tile.center.x - 38 + wIdx * 20;
                  const wY = tile.center.y - 18;
                  return (
                    <g key={`wh-${wh.kind}-${wIdx}`}>
                      <circle cx={wX} cy={wY} r="8" fill="#08111d" stroke={wh.color} strokeWidth="2" />
                      <text
                        x={wX}
                        y={wY + 3.5}
                        textAnchor="middle"
                        fill={wh.color}
                        fontSize="9"
                        fontWeight="bold"
                        pointerEvents="none"
                      >
                        {wh.symbol}
                      </text>
                    </g>
                  );
                })}

                {/* Planets Representation */}
                {tile.planets.map((p, pIdx) => {
                  const pCount = tile.planets.length;
                  const pX = tile.center.x + (pCount === 1 ? 0 : (pIdx - (pCount - 1) / 2) * 36);
                  const pY = tile.center.y + 16;
                  const isControlled = Boolean(p.controlledBy);

                  return (
                    <SvgButton
                      key={p.id}
                      data-testid={`planet-${p.id}`}
                      data-target-candidate={p.isCandidateTarget ? 'true' : undefined}
                      isInteractive={p.isCandidateTarget}
                      label={`Target planet ${p.label}`}
                      onActivate={() => {
                        if (p.isCandidateTarget) {
                          onSelectTarget?.(sysId, p.id);
                        }
                      }}
                      onClick={(e) => {
                        if (p.isCandidateTarget) {
                          e.stopPropagation();
                          onSelectTarget?.(sysId, p.id);
                        }
                      }}
                      style={{ cursor: p.isCandidateTarget ? 'pointer' : 'inherit' }}
                    >
                      <circle
                        cx={pX}
                        cy={pY}
                        r="14"
                        fill={isControlled ? p.controllerColor : '#334155'}
                        stroke={p.isCandidateTarget ? '#38bdf8' : p.exhausted ? '#ef4444' : isControlled ? '#f8fafc' : '#64748b'}
                        strokeWidth={p.isCandidateTarget ? 3 : 2}
                      />
                      {/* Planet Abbreviation */}
                      <text
                        x={pX}
                        y={pY - 2}
                        textAnchor="middle"
                        fill="#f8fafc"
                        fontSize="8"
                        fontWeight="bold"
                        pointerEvents="none"
                      >
                        {p.id.substring(0, 3).toUpperCase()}
                      </text>
                      {/* Resources / Influence fraction */}
                      {p.resources !== undefined && p.influence !== undefined && (
                        <text
                          x={pX}
                          y={pY + 8}
                          textAnchor="middle"
                          fill="#fef08a"
                          fontSize="7"
                          fontWeight="bold"
                          pointerEvents="none"
                        >
                          {p.resources}/{p.influence}
                        </text>
                      )}
                    </SvgButton>
                  );
                })}

                {/* Units Badge */}
                {tile.totalUnits > 0 && (
                  <g>
                    <rect
                      x={tile.center.x - 26}
                      y={tile.center.y + 36}
                      width="52"
                      height="16"
                      rx="4"
                      fill="rgba(15, 23, 42, 0.9)"
                      stroke="#475569"
                      strokeWidth="1"
                    />
                    <text
                      x={tile.center.x}
                      y={tile.center.y + 48}
                      textAnchor="middle"
                      fill="#e2e8f0"
                      fontSize="9"
                      fontWeight="bold"
                      pointerEvents="none"
                    >
                      {tile.totalUnits} units
                    </text>
                  </g>
                )}

                {/* Command Tokens */}
                {tile.commandTokens.map((ct, cIdx) => (
                  <circle
                    key={`cmd-${cIdx}`}
                    cx={tile.center.x - 42 + cIdx * 12}
                    cy={tile.center.y + 56}
                    r="4"
                    fill={ct.color}
                    stroke="#f8fafc"
                    strokeWidth="1"
                  />
                ))}
              </SvgButton>
            );
          })}

          {/* Movement Vector Overlays */}
          {presentation.targets.movementVectors.map((vec) => (
            <g key={`vec-${vec.fromSystemId}-${vec.toSystemId}`}>
              <line
                data-testid="movement-vector-line"
                x1={vec.fromCenter.x}
                y1={vec.fromCenter.y}
                x2={vec.toCenter.x}
                y2={vec.toCenter.y}
                stroke="#4ade80"
                strokeWidth={3}
                strokeDasharray="8 5"
                markerEnd="url(#vector-arrow)"
                style={{ opacity: 0.85, pointerEvents: 'none' }}
              />
              <circle
                cx={(vec.fromCenter.x + vec.toCenter.x) / 2}
                cy={(vec.fromCenter.y + vec.toCenter.y) / 2}
                r={10}
                fill="#0f172a"
                stroke="#4ade80"
                strokeWidth={1.5}
                style={{ pointerEvents: 'none' }}
              />
              <text
                x={(vec.fromCenter.x + vec.toCenter.x) / 2}
                y={(vec.fromCenter.y + vec.toCenter.y) / 2 + 4}
                textAnchor="middle"
                fill="#4ade80"
                fontSize="10"
                fontWeight="bold"
                style={{ pointerEvents: 'none' }}
              >
                {vec.unitCount}
              </text>
            </g>
          ))}
        </g>
      </svg>

      {/* Selected System Inspector */}
      {presentation.selectedSystem && (
        <SystemInspector
          system={presentation.selectedSystem}
          onClose={() => setSelectedSystemId(null)}
          onSelectAction={() => {
            if (onSelectTarget && presentation.selectedSystem) {
              onSelectTarget(presentation.selectedSystem.systemId);
            }
          }}
        />
      )}

      {/* Hover Info Tooltip */}
      {hoveredTile && !presentation.selectedSystem && (
        <div
          role="tooltip"
          aria-live="polite"
          data-testid="system-tooltip"
          className="board-tooltip panel"
          style={{
            border: '1px solid #38bdf8',
            padding: '10px 14px',
            fontSize: 13,
            pointerEvents: 'none',
            maxWidth: 320,
          }}
        >
          <div style={{ fontWeight: 'bold', color: '#38bdf8', marginBottom: 4 }}>
            System #{hoveredTile.systemId} — {hoveredTile.label}
          </div>
          {hoveredTile.anomalies && hoveredTile.anomalies.length > 0 && (
            <div style={{ color: '#fef08a', fontSize: 12 }}>
              Anomaly: {hoveredTile.anomalies.join(', ')}
            </div>
          )}
          {hoveredTile.wormholes && hoveredTile.wormholes.length > 0 && (
            <div style={{ color: '#a78bfa', fontSize: 12 }}>
              Wormholes: {hoveredTile.wormholes.join(', ')}
            </div>
          )}
          <div style={{ marginTop: 4 }}>
            <span style={{ fontWeight: 'bold', color: '#cbd5e1' }}>Planets: </span>
            {hoveredTile.planets.length > 0 ? (
              hoveredTile.planets.map((p, i) => (
                <div key={i} style={{ marginLeft: 6, fontSize: 12 }}>
                  • {p.label}
                  {p.resources !== undefined && ` (${p.resources} Res / ${p.influence} Inf)`}
                  {p.owner && <span style={{ color: getPlayerColor(p.owner, seatingOrder) }}> [{p.owner}]</span>}
                </div>
              ))
            ) : (
              <span style={{ fontSize: 12, color: '#94a3b8' }}>None</span>
            )}
          </div>
          <div style={{ marginTop: 4, fontSize: 12 }}>
            Units: {hoveredTile.unitsCount} | Command Tokens: {hoveredTile.tokensCount}
          </div>
        </div>
      )}
    </div>
  );
};
