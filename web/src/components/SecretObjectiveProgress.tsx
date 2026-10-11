import React from "react";
import type { ObjectiveProgressView } from "../protocol/types.ts";
import { secretNoProgressNote, secretProgressDisplay } from "../presentation/secretProgress.ts";
import "./SecretObjectiveProgress.css";

/**
 * "4 / 6 systems with at least one of your ships" with a bar, for one of the viewer's own held
 * secrets — the same have / threshold the public objectives show, worded for the secret. Event
 * secrets get a one-line note instead; anything else without a count renders nothing.
 */
export const SecretObjectiveProgress: React.FC<{
  alias: string;
  progress: ObjectiveProgressView | undefined;
}> = ({ alias, progress }) => {
  const display = secretProgressDisplay(alias, progress);
  if (!display) {
    const note = secretNoProgressNote(alias);
    return note ? (
      <div className="secret-progress__note" data-testid={`secret-progress-note-${alias}`}>
        {note}
      </div>
    ) : null;
  }
  return (
    <div
      className={`secret-progress${display.satisfied ? " secret-progress--met" : ""}`}
      data-testid={`secret-progress-${alias}`}
      data-satisfied={display.satisfied ? "true" : "false"}
      aria-label={`${display.text}${display.satisfied ? ", requirement met" : ""}`}
    >
      <div className="secret-progress__label">
        <span className="secret-progress__count">
          {display.have} / {display.threshold}
        </span>{" "}
        <span className="secret-progress__unit">{display.unit}</span>
        {display.satisfied && <span className="secret-progress__met"> · met</span>}
      </div>
      <div className="secret-progress__bar" aria-hidden="true">
        <div className="secret-progress__fill" style={{ width: `${display.percent}%` }} />
      </div>
    </div>
  );
};
