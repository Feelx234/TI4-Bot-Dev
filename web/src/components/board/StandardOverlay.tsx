import React from "react";
import { MapTargetMode, TilePresentation } from "../../presentation/boardPresentation.ts";
import { SvgButton } from "../../primitives/index.ts";
import type { PaymentMark } from "../../presentation/paymentDraft.ts";
import { usePlayerIdentity } from "../../presentation/PlayerIdentity.tsx";
import { PlanetValueGlyph, valueKind, valueLabel } from "../PlanetValueIcons.tsx";

export interface StandardOverlayProps {
  tile: TilePresentation;
  onSelectTarget?: (systemId: string, planetId?: string) => void;
  onSelectSystem?: (systemId: string | null) => void;
  /** In planet mode candidates get a reticle and every other planet is dimmed. */
  targetMode?: MapTargetMode;
  /** While paying: worth and staged state per payable planet id. */
  paymentMarks?: ReadonlyMap<string, PaymentMark>;
}

export const StandardOverlay: React.FC<StandardOverlayProps> = ({
  tile,
  onSelectTarget,
  onSelectSystem,
  targetMode = null,
  paymentMarks,
}) => {
  const display = usePlayerIdentity();
  const pCount = tile.planets.length;

  return (
    <>
      {/* Planets */}
      {tile.planets.map((p, pIdx) => {
        const pX = tile.center.x + (pCount === 1 ? 0 : (pIdx - (pCount - 1) / 2) * 38);
        const pY = tile.center.y + 16;
        const planetRadius = 15;
        const isControlled = Boolean(p.controlledBy);
        const isCandidateTarget = Boolean(p.isCandidateTarget);
        const isPlanetMode = targetMode === "planet" || targetMode === "payment";
        const isDimmed = isPlanetMode && !isCandidateTarget;
        const mark = targetMode === "payment" ? paymentMarks?.get(p.id) : undefined;

        return (
          <SvgButton
            key={p.id}
            data-testid={`planet-${p.id}`}
            data-target-candidate={isCandidateTarget ? "true" : undefined}
            data-target-dimmed={isDimmed ? "true" : undefined}
            data-payment-staged={mark ? String(mark.staged) : undefined}
            isInteractive={isCandidateTarget}
            label={
              mark
                ? `${mark.staged ? "Stop exhausting" : "Exhaust"} planet ${p.label} for ${valueLabel(valueKind(mark.unit), mark.worth)}`
                : `Target planet ${p.label}`
            }
            onActivate={() => {
              if (isCandidateTarget) {
                // Inspect first: a system inspection may reset the selection, the target must win.
                // Paying toggles planets one after another, so the inspector must not pop up.
                if (targetMode !== "payment") onSelectSystem?.(tile.systemId);
                onSelectTarget?.(tile.systemId, p.id);
              }
            }}
            onKeyDown={(e) => {
              if (isCandidateTarget && (e.key === "Enter" || e.key === " ")) {
                e.stopPropagation();
              }
            }}
            onClick={(e) => {
              if (isCandidateTarget) {
                e.stopPropagation();
                // Inspect first: a system inspection may reset the selection, the target must win.
                // Paying toggles planets one after another, so the inspector must not pop up.
                if (targetMode !== "payment") onSelectSystem?.(tile.systemId);
                onSelectTarget?.(tile.systemId, p.id);
              }
            }}
            style={{
              cursor: isCandidateTarget ? "pointer" : isDimmed ? "not-allowed" : "inherit",
              opacity: isDimmed ? 0.35 : 1,
            }}
          >
            {isPlanetMode && isCandidateTarget && (
              <g
                data-testid={`planet-target-reticle-${p.id}`}
                pointerEvents="none"
                className="planet-target-reticle"
              >
                <circle cx={pX} cy={pY} r={planetRadius + 6} fill="rgba(56, 189, 248, 0.18)" />
                <circle
                  cx={pX}
                  cy={pY}
                  r={planetRadius + 6}
                  fill="none"
                  stroke={mark?.staged ? "#4ade80" : "#38bdf8"}
                  strokeWidth={mark?.staged ? 3 : 2}
                  strokeDasharray={mark?.staged ? undefined : "4 2"}
                  className={mark?.staged ? undefined : "target-pulse-ring"}
                />
                {[0, 90, 180, 270].map((angle) => {
                  const rad = (angle * Math.PI) / 180;
                  const inner = planetRadius + 7;
                  const outer = planetRadius + 13;
                  return (
                    <line
                      key={angle}
                      x1={pX + inner * Math.cos(rad)}
                      y1={pY + inner * Math.sin(rad)}
                      x2={pX + outer * Math.cos(rad)}
                      y2={pY + outer * Math.sin(rad)}
                      stroke={mark?.staged ? "#4ade80" : "#38bdf8"}
                      strokeWidth={2}
                    />
                  );
                })}
              </g>
            )}
            <circle
              cx={pX}
              cy={pY}
              r={planetRadius}
              fill={isControlled ? p.controllerColor : "#1e293b"}
              stroke={
                isCandidateTarget
                  ? "#38bdf8"
                  : p.exhausted
                    ? "#ef4444"
                    : isControlled
                      ? "#f8fafc"
                      : "#64748b"
              }
              strokeWidth={isCandidateTarget ? 3 : 2}
            />

            {p.controlledBy && (
              <text
                x={pX + 13}
                y={pY - 11}
                textAnchor="middle"
                fill="#fff"
                stroke="#0b1220"
                strokeWidth="0.6"
                paintOrder="stroke"
                fontSize="12"
                pointerEvents="none"
              >
                {display(p.controlledBy).symbol}
              </text>
            )}

            {mark && isCandidateTarget && (
              <g
                data-testid={`payment-mark-${p.id}`}
                pointerEvents="none"
                role="img"
                aria-label={`${mark.staged ? "Staged: " : ""}${valueLabel(valueKind(mark.unit), mark.worth)}`}
              >
                <title>{valueLabel(valueKind(mark.unit), mark.worth)}</title>
                <rect
                  x={pX - 22}
                  y={pY - planetRadius - 23}
                  width={44}
                  height={16}
                  rx={8}
                  fill={mark.staged ? "#166534" : "#0c4a6e"}
                  stroke={mark.staged ? "#4ade80" : "#38bdf8"}
                  strokeWidth={1.5}
                />
                <PlanetValueGlyph
                  kind={valueKind(mark.unit)}
                  x={pX - 15}
                  y={pY - planetRadius - 21}
                  size={12}
                />
                <text
                  x={pX + 4}
                  y={pY - planetRadius - 11}
                  textAnchor="middle"
                  fill="#f8fafc"
                  fontSize="10"
                  fontWeight="bold"
                  aria-hidden="true"
                >
                  {mark.staged ? "✓ " : ""}
                  {mark.worth}
                </text>
              </g>
            )}

            {/* Planet Abbreviation */}
            <text
              x={pX}
              y={pY - 2}
              textAnchor="middle"
              fill="#f8fafc"
              fontSize="8.5"
              fontWeight="bold"
              pointerEvents="none"
            >
              {p.id.substring(0, 3).toUpperCase()}
            </text>

            {/* Resources / Influence values */}
            {p.resources !== undefined && p.influence !== undefined && (
              <g
                pointerEvents="none"
                role="img"
                aria-label={`${valueLabel("resources", p.resources)}, ${valueLabel("influence", p.influence)}`}
              >
                <title>{`${valueLabel("resources", p.resources)}, ${valueLabel("influence", p.influence)}`}</title>
                <PlanetValueGlyph kind="resources" x={pX - 14} y={pY + 2} size={8} />
                <text x={pX - 5} y={pY + 9} fill="#fef08a" fontSize="8" fontWeight="bold" aria-hidden="true">
                  {p.resources}
                </text>
                <PlanetValueGlyph kind="influence" x={pX + 2} y={pY + 2} size={8} />
                <text x={pX + 11} y={pY + 9} fill="#bae6fd" fontSize="8" fontWeight="bold" aria-hidden="true">
                  {p.influence}
                </text>
              </g>
            )}
          </SvgButton>
        );
      })}

      {/* Units Badge: ONLY shown in standard view */}
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
    </>
  );
};
