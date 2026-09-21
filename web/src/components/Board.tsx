import React, { useState, useRef } from 'react';
import { BoardView, BoardTileView } from '../protocol/types.ts';

export interface BoardProps {
  board: BoardView;
  seatingOrder: string[];
}

const PLAYER_COLORS = ['#ef4444', '#38bdf8', '#facc15', '#4ade80', '#c084fc', '#fb923c'];

export function getPlayerColor(owner: string | null | undefined, seatingOrder: readonly string[]): string {
  if (!owner) return '#64748b';
  const seatIndex = seatingOrder.indexOf(owner);
  return seatIndex === -1 ? '#94a3b8' : PLAYER_COLORS[seatIndex % PLAYER_COLORS.length];
}

function getHexPoints(cx: number, cy: number, radius = 70): string {
  const points: string[] = [];
  for (let k = 0; k < 6; k++) {
    const angle = Math.PI / 6 + (Math.PI / 3) * k;
    const px = cx + radius * Math.cos(angle);
    const py = cy + radius * Math.sin(angle);
    points.push(`${px.toFixed(1)},${py.toFixed(1)}`);
  }
  return points.join(' ');
}

function getInnerPoints(cx: number, cy: number, radius = 64): string {
  return getHexPoints(cx, cy, radius);
}

function getAnomalyColor(anomalies?: string[]): string | null {
  if (!anomalies || anomalies.length === 0) return null;
  const lower = anomalies.map((a) => a.toLowerCase());
  if (lower.some((a) => a.includes('supernova'))) return '#6b271a';
  if (lower.some((a) => a.includes('gravity rift'))) return '#38235f';
  if (lower.some((a) => a.includes('nebula'))) return '#173f57';
  if (lower.some((a) => a.includes('asteroid'))) return '#3f3b35';
  if (lower.some((a) => a.includes('scar'))) return '#4a2025';
  return null;
}

function getWormholeColor(kind: string): { color: string; symbol: string } {
  const k = kind.toLowerCase();
  if (k.includes('alpha')) return { color: '#38bdf8', symbol: 'α' };
  if (k.includes('beta')) return { color: '#f43f5e', symbol: 'β' };
  if (k.includes('gamma')) return { color: '#4ade80', symbol: 'γ' };
  if (k.includes('delta')) return { color: '#fb923c', symbol: 'δ' };
  return { color: '#94a3b8', symbol: 'ω' };
}

export const Board: React.FC<BoardProps> = ({ board, seatingOrder }) => {
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

  const systemsMap = board.systems || {};

  // Build unified tile representation: from board.map_tiles if available, else derive from board.systems
  const tiles: BoardTileView[] = board.map_tiles && board.map_tiles.length > 0
    ? board.map_tiles
    : Object.keys(systemsMap).map((id, index) => {
        // Fallback for tests/mocks without map_tiles
        let q = 0;
        let r = 0;
        if (id === '18' || index === 0) {
          q = 0;
          r = 0;
        } else if (id === '34' || index === 1) {
          q = 1;
          r = 0;
        } else {
          q = index;
          r = -index;
        }
        return {
          system_id: id,
          label: id === '18' ? 'Mecatol Rex' : `#${id}`,
          q,
          r,
        };
      });

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
        <button
          onClick={zoomIn}
          title="Zoom In"
          className="button button--secondary button--icon"
          style={{
            fontSize: 16,
          }}
        >
          +
        </button>
        <button
          onClick={zoomOut}
          title="Zoom Out"
          className="button button--secondary button--icon"
          style={{
            fontSize: 16,
          }}
        >
          −
        </button>
        <button
          onClick={resetView}
          title="Reset Pan & Zoom"
          className="button button--secondary button--icon"
          style={{
            fontSize: 14,
          }}
        >
          ⟲
        </button>
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
        </defs>

        <g transform={`translate(${viewTransform.x}, ${viewTransform.y}) scale(${viewTransform.scale})`}>
          {tiles.map((tile, idx) => {
            const sysId = tile.system_id;
            const dynamicSystem = systemsMap[sysId];

            // Axial coordinates to Cartesian pixel center
            let x = 150 * (tile.q + tile.r / 2);
            let y = 130 * tile.r;
            if (tile.special_area === 'fracture') {
              x = (tile.q - 3) * 130;
              y = 420;
            } else if (tile.special_area === 'nexus') {
              x = -500;
              y = 420;
            }

            const points = getHexPoints(x, y, 70);
            const innerPoints = getInnerPoints(x, y, 64);
            // Anomaly / System background fill
            const anomalyColor = getAnomalyColor(tile.anomalies);
            let fillColor = sysId === '18' ? '#1e1b4b' : anomalyColor || (tile.hyperlane ? '#1e1b4b' : '#0f172a');
            if (tile.special_area === 'fracture') fillColor = '#112f39';
            if (tile.special_area === 'nexus') fillColor = '#29304d';

            // Space control: check if single player has space units
            const spaceUnits = (dynamicSystem?.units || []).filter((u) => !u.planet);
            const spaceOwners = Array.from(new Set(spaceUnits.map((u) => u.owner)));
            const singleSpaceOwner = spaceOwners.length === 1 ? spaceOwners[0] : null;

            // Planet control
            const dynamicPlanets = dynamicSystem?.planets || {};
            const planetOwners = Array.from(
              new Set(
                Object.values(dynamicPlanets)
                  .map((p) => p.controlled_by)
                  .filter((o): o is string => Boolean(o))
              )
            );
            const singlePlanetOwner = planetOwners.length === 1 ? planetOwners[0] : null;

            // Border stroke
            let strokeColor = '#334155';
            let strokeWidth = 1.5;
            if (singleSpaceOwner) {
              strokeColor = getPlayerColor(singleSpaceOwner, seatingOrder);
              strokeWidth = 3;
            }

            // Planets to display (merge static metadata with dynamic state)
            const planetsToDisplay: {
              id: string;
              label: string;
              resources?: number;
              influence?: number;
              traits?: string[];
              specialties?: string[];
              controlled_by?: string | null;
              exhausted?: boolean;
            }[] = (tile.planets || []).map((sp) => ({
              id: sp.id,
              label: sp.label,
              resources: sp.resources,
              influence: sp.influence,
              traits: sp.traits,
              specialties: sp.tech_specialties,
              controlled_by: dynamicPlanets[sp.id]?.controlled_by,
              exhausted: dynamicPlanets[sp.id]?.exhausted,
            }));

            // If no static planets but dynamic planets exist (e.g. in legacy tests)
            if (planetsToDisplay.length === 0 && dynamicSystem?.planets) {
              Object.values(dynamicSystem.planets).forEach((dp) => {
                planetsToDisplay.push({
                  id: dp.planet_id,
                  label: dp.planet_id,
                  controlled_by: dp.controlled_by,
                  exhausted: dp.exhausted,
                });
              });
            }

            const totalUnits = dynamicSystem?.units?.length || 0;
            const commandTokens = dynamicSystem?.command_tokens || [];

            return (
              <g
                key={`hex-${sysId}-${idx}`}
                data-testid={`system-hex-${sysId}`}
                data-system-id={sysId}
                onMouseEnter={() => {
                  setHoveredTile({
                    systemId: sysId,
                    label: tile.label,
                    anomalies: tile.anomalies,
                    wormholes: tile.wormholes,
                    planets: planetsToDisplay.map((p) => ({
                      label: p.label,
                      resources: p.resources,
                      influence: p.influence,
                      owner: p.controlled_by,
                    })),
                    unitsCount: totalUnits,
                    tokensCount: commandTokens.length,
                  });
                }}
                onMouseLeave={() => setHoveredTile(null)}
                style={{ cursor: 'default', outline: 'none' }}
              >
                {/* Hexagon Tile */}
                <polygon
                  points={points}
                  fill={fillColor}
                  stroke={strokeColor}
                  strokeWidth={strokeWidth}
                />

                {/* Inner border for exclusive planet control */}
                {singlePlanetOwner && (
                  <polygon
                    points={innerPoints}
                    fill="none"
                    stroke={getPlayerColor(singlePlanetOwner, seatingOrder)}
                    strokeWidth={2}
                    strokeDasharray="4 2"
                  />
                )}

                {/* Tile Label / System Number */}
                <text
                  x={x}
                  y={y - 48}
                  textAnchor="middle"
                  fill="#cbd5e1"
                  fontSize="11"
                  fontWeight="bold"
                  pointerEvents="none"
                >
                  {sysId === '18' ? 'Mecatol Rex' : tile.label ? (tile.label.startsWith('#') ? tile.label : `#${sysId}`) : `#${sysId}`}
                </text>

                {/* Anomaly badge text */}
                {tile.anomalies && tile.anomalies.length > 0 && (
                  <text
                    x={x}
                    y={y - 36}
                    textAnchor="middle"
                    fill="#fef08a"
                    fontSize="8"
                    fontWeight="bold"
                    pointerEvents="none"
                  >
                    {tile.anomalies[0].toUpperCase()}
                  </text>
                )}

                {/* Printed Wormholes */}
                {(tile.wormholes || []).map((wh, wIdx) => {
                  const { color, symbol } = getWormholeColor(wh);
                  const wX = x - 38 + wIdx * 20;
                  const wY = y - 18;
                  return (
                    <g key={`wh-${wh}-${wIdx}`}>
                      <circle cx={wX} cy={wY} r="8" fill="#08111d" stroke={color} strokeWidth="2" />
                      <text
                        x={wX}
                        y={wY + 3.5}
                        textAnchor="middle"
                        fill={color}
                        fontSize="9"
                        fontWeight="bold"
                        pointerEvents="none"
                      >
                        {symbol}
                      </text>
                    </g>
                  );
                })}

                {/* Planets Representation */}
                {planetsToDisplay.map((p, pIdx) => {
                  const pCount = planetsToDisplay.length;
                  const pX = x + (pCount === 1 ? 0 : (pIdx - (pCount - 1) / 2) * 36);
                  const pY = y + 16;
                  const ownerColor = getPlayerColor(p.controlled_by, seatingOrder);
                  const isControlled = Boolean(p.controlled_by);

                  return (
                    <g key={p.id}>
                      <circle
                        cx={pX}
                        cy={pY}
                        r="14"
                        fill={isControlled ? ownerColor : '#334155'}
                        stroke={p.exhausted ? '#ef4444' : isControlled ? '#f8fafc' : '#64748b'}
                        strokeWidth="2"
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
                    </g>
                  );
                })}

                {/* Units Badge */}
                {totalUnits > 0 && (
                  <g>
                    <rect
                      x={x - 26}
                      y={y + 36}
                      width="52"
                      height="16"
                      rx="4"
                      fill="rgba(15, 23, 42, 0.9)"
                      stroke="#475569"
                      strokeWidth="1"
                    />
                    <text
                      x={x}
                      y={y + 48}
                      textAnchor="middle"
                      fill="#e2e8f0"
                      fontSize="9"
                      fontWeight="bold"
                      pointerEvents="none"
                    >
                      {totalUnits} units
                    </text>
                  </g>
                )}

                {/* Command Tokens */}
                {commandTokens.map((owner, cIdx) => (
                  <circle
                    key={`cmd-${cIdx}`}
                    cx={x - 42 + cIdx * 12}
                    cy={y + 56}
                    r="4"
                    fill={getPlayerColor(owner, seatingOrder)}
                    stroke="#f8fafc"
                    strokeWidth="1"
                  />
                ))}
              </g>
            );
          })}
        </g>
      </svg>

      {/* Hover Info Tooltip */}
      {hoveredTile && (
        <div
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
