import React from "react";
import {
  EDGE_RADIUS,
  edgePoint,
  hyperlaneEdges,
  hyperlaneLabel,
  hyperlaneTitle,
  lanePath,
  parseHyperlaneId,
} from "./hyperlaneGeometry.ts";

const INK = "var(--hyperlane-ink, #c4b5fd)";
/** The tile colour, so the inner line of a lane reads as a hollow channel. */
const BED = "var(--hyperlane-bed, #1e1b4b)";

/**
 * Flat lane art for one hyperlane tile, centred on 0,0 (apply `translate` outside). Each lane is a
 * double line (ink outline around a channel in the tile colour), so it reads by shape and weight,
 * not by colour. A ring cap sits on every edge a lane reaches; it half-overlaps the neighbour, and
 * where that neighbour is a matching hyperlane the two halves close into one ring.
 */
const Lanes: React.FC<{ systemId: string; weight: number; frame: boolean }> = ({ systemId, weight, frame }) => {
  const tile = parseHyperlaneId(systemId);
  if (!tile) return null;
  const edges = hyperlaneEdges(tile);
  return (
    <g>
      {frame && (
        <polygon
          points={Array.from({ length: 6 }, (_, k) => {
            const a = Math.PI / 6 + (Math.PI / 3) * k;
            return `${(63 * Math.cos(a)).toFixed(1)},${(63 * Math.sin(a)).toFixed(1)}`;
          }).join(" ")}
          fill="none"
          stroke={INK}
          strokeWidth={1}
          strokeDasharray="2 5"
          strokeLinecap="round"
          opacity={0.4}
        />
      )}
      <g fill="none" strokeLinecap="round">
        {tile.lanes.map((pair) => (
          <path key={pair.join("-")} d={lanePath(pair)} stroke={INK} strokeWidth={9 * weight} />
        ))}
        {tile.lanes.map((pair) => (
          <path key={`in-${pair.join("-")}`} d={lanePath(pair)} stroke={BED} strokeWidth={3.6 * weight} />
        ))}
      </g>
      {edges.map((e) => {
        const p = edgePoint(e, EDGE_RADIUS - 1);
        return (
          <g key={e}>
            <circle cx={p.x} cy={p.y} r={7.5 * weight} fill={INK} />
            <circle cx={p.x} cy={p.y} r={3 * weight} fill={BED} />
          </g>
        );
      })}
    </g>
  );
};

/** Board art: sits on the hex behind labels and tokens, never takes pointer events. */
export const HyperlaneArt: React.FC<{
  systemId: string;
  cx: number;
  cy: number;
  weight?: number;
  /** The hex fill, when it is not the board's. */
  bed?: string;
}> = ({ systemId, cx, cy, weight = 1, bed }) => {
  const tile = parseHyperlaneId(systemId);
  if (!tile) return null;
  return (
    <g
      data-testid="hyperlane-art"
      data-hyperlane-edges={tile.lanes.map((l) => l.map((e) => e + 1).join("-")).join(",")}
      transform={`translate(${cx} ${cy})`}
      opacity={0.85}
      pointerEvents="none"
      role="img"
      aria-label={hyperlaneLabel(tile)}
      style={bed ? ({ "--hyperlane-bed": bed } as React.CSSProperties) : undefined}
    >
      <title>{hyperlaneTitle(tile)}</title>
      <Lanes systemId={systemId} weight={weight} frame />
    </g>
  );
};

/**
 * The same lanes inside a map picker thumbnail or preview hex. `weight` thickens them for small
 * hexes. Decorative: the parent svg carries the name.
 */
export const HyperlaneThumb: React.FC<{
  systemId: string;
  cx: number;
  cy: number;
  weight?: number;
  /** The hex fill, so the inner line of a lane shows it. */
  bed?: string;
}> = ({ systemId, cx, cy, weight = 1.6, bed = "#312e81" }) => {
  if (!parseHyperlaneId(systemId)) return null;
  return (
    <g
      data-testid="hyperlane-thumb"
      transform={`translate(${cx} ${cy})`}
      pointerEvents="none"
      aria-hidden="true"
      style={{ "--hyperlane-bed": bed } as React.CSSProperties}
    >
      <Lanes systemId={systemId} weight={weight} frame={false} />
    </g>
  );
};

/** Stand-alone HTML diagram for the inspector: a hex with its lanes and an accessible name. */
export const HyperlaneDiagram: React.FC<{ systemId: string; size?: number }> = ({ systemId, size = 72 }) => {
  const tile = parseHyperlaneId(systemId);
  if (!tile) return null;
  const label = hyperlaneLabel(tile);
  return (
    <svg
      width={size}
      height={size}
      viewBox="-76 -76 152 152"
      role="img"
      aria-label={label}
      data-testid="hyperlane-diagram"
      style={{ flex: "none" }}
    >
      <title>{hyperlaneTitle(tile)}</title>
      <polygon
        points={Array.from({ length: 6 }, (_, k) => {
          const a = Math.PI / 6 + (Math.PI / 3) * k;
          return `${(70 * Math.cos(a)).toFixed(1)},${(70 * Math.sin(a)).toFixed(1)}`;
        }).join(" ")}
        fill={BED}
        stroke="#334155"
        strokeWidth={2}
      />
      <Lanes systemId={systemId} weight={1} frame />
    </svg>
  );
};
