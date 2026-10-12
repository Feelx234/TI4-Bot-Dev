import React from "react";
import type { ObjectiveProgressView } from "../protocol/types.ts";
import { usePlayerIdentity } from "../presentation/PlayerIdentity.tsx";
import { neighbourProgressDisplay } from "../presentation/neighbourProgress.ts";
import "./NeighbourProgress.css";

const GLYPH = { ahead: "▲", tied: "=", behind: "▼" } as const;

/** Dark text on light seat colours, white on dark ones (seat 3 green, 5 blue, 8 black). */
function inkOn(hex: string): string {
  const m = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex);
  if (!m) return "#0b1220";
  const [r, g, b] = m.slice(1).map((part) => parseInt(part, 16) / 255);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b > 0.55 ? "#0b1220" : "#fff";
}

/**
 * Push Boundaries, per neighbour: "12/10" (our planets / theirs) in blue when ahead, red when
 * tied or behind, with the neighbour's seat symbol and a ▲ / = / ▼ so colour is never the only
 * cue; the summary says how many are beaten and how many the card needs.
 */
export const NeighbourProgress: React.FC<{
  progress: ObjectiveProgressView | undefined;
  /** Test id suffix, e.g. "push_boundaries-seat_a". */
  testId: string;
  className?: string;
  /** A leading label, e.g. "You:" when shown outside the player's own column. */
  heading?: string;
}> = ({ progress, testId, className, heading }) => {
  const display = usePlayerIdentity();
  const model = neighbourProgressDisplay(progress, display);
  if (!model) return null;
  return (
    <div
      className={`neighbour-progress${className ? ` ${className}` : ""}`}
      data-testid={`neighbour-progress-${testId}`}
    >
      {heading && <div className="neighbour-progress__heading">{heading}</div>}
      <ul className="neighbour-progress__rows" aria-label="Planets against each neighbor">
        {model.rows.map((row) => (
          <li
            key={row.player}
            className={`neighbour-progress__row neighbour-progress__row--${row.standing}`}
            data-testid={`neighbour-row-${testId}-${row.player}`}
            data-standing={row.standing}
            title={row.aria}
            aria-label={row.aria}
          >
            <span
              className="neighbour-progress__who"
              style={{ background: row.color, color: inkOn(row.color) }}
              aria-hidden="true"
            >
              {row.position ?? row.symbol}
            </span>
            <span className="neighbour-progress__count" aria-hidden="true">
              {row.text}
            </span>
            <span className="neighbour-progress__glyph" aria-hidden="true">
              {GLYPH[row.standing]}
            </span>
          </li>
        ))}
      </ul>
      <div
        className={`neighbour-progress__summary${
          model.ahead >= model.threshold ? " neighbour-progress__summary--met" : ""
        }`}
      >
        {model.summary}
      </div>
    </div>
  );
};
