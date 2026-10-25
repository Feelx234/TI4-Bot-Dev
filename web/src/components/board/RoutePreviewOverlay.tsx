import React from "react";
import type { RoutePreview } from "../../presentation/RoutePreviewContext.tsx";

export interface RoutePreviewOverlayProps {
  previews: readonly RoutePreview[];
  /** Where each system is drawn. */
  centers: ReadonlyMap<string, { x: number; y: number }>;
}

/** Route colours: the movement dialog's green, and red for a route that rolls for a rift. */
export const ROUTE_COLORS = { safe: "#38bdf8", risky: "#f87171" } as const;

/**
 * The route a staged ship takes: a line through the centres of the systems it enters, with a dot
 * on each system it passes. Drawn over the movement arrows, which only join origin and destination.
 */
export const RoutePreviewOverlay: React.FC<RoutePreviewOverlayProps> = ({ previews, centers }) => (
  <>
    {previews.map((preview) => {
      const points = preview.path
        .map((id) => centers.get(id))
        .filter((point): point is { x: number; y: number } => point !== undefined);
      if (points.length < 2) return null;
      const color = preview.risky ? ROUTE_COLORS.risky : ROUTE_COLORS.safe;
      return (
        <g key={preview.key} data-testid="route-preview" data-risky={preview.risky || undefined}>
          <polyline
            points={points.map((p) => `${p.x},${p.y}`).join(" ")}
            fill="none"
            stroke={color}
            strokeWidth={9}
            strokeLinejoin="round"
            strokeLinecap="round"
            style={{ opacity: 0.28, pointerEvents: "none" }}
          />
          <polyline
            points={points.map((p) => `${p.x},${p.y}`).join(" ")}
            fill="none"
            stroke={color}
            strokeWidth={3.5}
            strokeLinejoin="round"
            strokeLinecap="round"
            strokeDasharray={preview.risky ? "2 7" : undefined}
            style={{ pointerEvents: "none" }}
          />
          {points.slice(1, -1).map((p, index) => (
            <circle
              key={`${preview.key}-${index}`}
              cx={p.x}
              cy={p.y}
              r={9}
              fill="#0f172a"
              stroke={color}
              strokeWidth={3}
              style={{ pointerEvents: "none" }}
            />
          ))}
        </g>
      );
    })}
  </>
);
