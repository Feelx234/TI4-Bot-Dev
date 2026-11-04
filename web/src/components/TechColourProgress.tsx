import React from "react";
import type { ObjectiveProgressView } from "../protocol/types.ts";
import { techColourDisplay } from "../presentation/techColourProgress.ts";
import "./TechColourProgress.css";

/**
 * Diversify Research / Master the Sciences, per colour: "Blue 2/2 ✓ · Green 1/2 · ..." with the
 * colour's swatch, so a seat sees which colours still need a technology. Nothing for other objectives.
 */
export const TechColourProgress: React.FC<{
  objectiveId: string;
  technologies: readonly string[] | undefined;
  progress: ObjectiveProgressView | undefined;
  /** Test id suffix, e.g. "diversify-seat_a". */
  testId: string;
  className?: string;
  /** A leading label, e.g. "You:" when shown outside the player's own column. */
  heading?: string;
}> = ({ objectiveId, technologies, progress, testId, className, heading }) => {
  const model = techColourDisplay(objectiveId, technologies, progress);
  if (!model) return null;
  return (
    <div
      className={`tech-colour-progress${className ? ` ${className}` : ""}`}
      data-testid={`tech-colour-progress-${testId}`}
    >
      {heading && <div className="tech-colour-progress__heading">{heading}</div>}
      <ul className="tech-colour-progress__chips" aria-label={`Technologies per colour: ${model.aria}`}>
        {model.chips.map((chip) => (
          <li
            key={chip.track}
            className={`tech-colour-progress__chip${chip.met ? " tech-colour-progress__chip--met" : ""}`}
            data-testid={`tech-colour-${testId}-${chip.track.toLowerCase()}`}
            data-met={chip.met ? "true" : "false"}
            title={`${chip.trackName} (${chip.colour.toLowerCase()}): ${chip.count} of ${chip.needed}`}
          >
            <span
              className="tech-colour-progress__swatch"
              style={{ background: chip.accent }}
              aria-hidden="true"
            />
            <span aria-hidden="true">
              {chip.colour} {chip.count}/{chip.needed}
              {chip.met ? " ✓" : ""}
            </span>
          </li>
        ))}
      </ul>
      <div
        className={`tech-colour-progress__summary${
          model.have >= model.threshold ? " tech-colour-progress__summary--met" : ""
        }`}
      >
        {model.summary}
      </div>
    </div>
  );
};
