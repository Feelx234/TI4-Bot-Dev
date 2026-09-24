import React, { useEffect, useState } from "react";
import type { BoardView, PendingChoiceDto } from "../protocol/types.ts";
import { DecisionHeader } from "./DecisionHeader.tsx";
import { WorkflowShell } from "./WorkflowShell.tsx";

/** Each landing is one decision; subsequent offers must be checked anew. */
export const InvasionLandingTray: React.FC<{
  choice: PendingChoiceDto;
  board?: BoardView;
  viewerSeat?: string | null;
  selectedOptionId?: string;
  onSubmit: (id: string) => Promise<void>;
  onClose: () => void;
  lastError?: string | null;
}> = ({ choice, board, viewerSeat, selectedOptionId, onSubmit, onClose, lastError }) => {
  const [localSelection, setLocalSelection] = useState<string | null>(null);
  useEffect(() => setLocalSelection(null), [choice.nonce]);
  useEffect(() => {
    if (selectedOptionId) setLocalSelection(selectedOptionId);
  }, [selectedOptionId]);
  const finish = choice.options.find(
    (option) => option.id === "done_committing" || option.kind === "decline",
  );
  const landings = choice.options.filter((option) => option !== finish);
  const selected = landings.find((option) => option.id === localSelection);
  const active =
    choice.context?.target && "System" in choice.context.target
      ? choice.context.target.System
      : board?.active_system;
  return (
    <section
      className="panel decision-frame"
      data-testid="invasion-landing-tray"
      aria-label="Ground force landing"
    >
      <DecisionHeader
        actor={choice.actor}
        title="Land ground forces"
        instruction={choice.prompt}
        progress={active ? `Active system: ${active}` : undefined}
        onMinimize={onClose}
      />
      <WorkflowShell
        choice={choice}
        viewerSeat={viewerSeat}
        onSubmit={onSubmit}
        lastError={lastError}
        errorTestId="invasion-error"
      >
        {({ isActor, isDirectSubmitting, submitDirect }) =>
          isActor && (
            <>
              {!board && (
                <p className="text-muted">
                  Board projection unavailable; only currently offered landings are shown.
                </p>
              )}
              <div className="decision-frame__options">
                {landings.map((option) => {
                  const planet =
                    typeof option.payload?.planet === "string" ? option.payload.planet : undefined;
                  const unit =
                    typeof option.payload?.unit === "string" ? option.payload.unit : undefined;
                  const stock =
                    active && board && unit
                      ? board.systems[active]?.units.filter(
                          (piece) =>
                            piece.owner === choice.actor &&
                            piece.unit_type === unit &&
                            !piece.planet &&
                            Boolean(piece.damaged) === (option.payload?.damaged === true),
                        ).length
                      : undefined;
                  return (
                    <label
                      key={option.id}
                      className={`card${selected?.id === option.id ? " card--selected" : ""}`}
                    >
                      <input
                        type="radio"
                        name="landing"
                        checked={selected?.id === option.id}
                        onChange={() => setLocalSelection(option.id)}
                      />
                      <span>
                        {option.label}
                        {planet ? ` · ${planet}` : ""}
                        {stock !== undefined ? ` · ${stock} in space` : ""}
                      </span>
                    </label>
                  );
                })}
              </div>
              <div className="workflow-actions">
                {selected && (
                  <button
                    type="button"
                    className="button button--secondary"
                    onClick={() => setLocalSelection(null)}
                  >
                    Reset selection
                  </button>
                )}
                {finish && (
                  <button
                    type="button"
                    className="button button--secondary"
                    disabled={isDirectSubmitting}
                    onClick={() => void submitDirect(finish.id)}
                  >
                    {finish.label}
                  </button>
                )}
                <button
                  type="button"
                  className="button button--primary"
                  disabled={!selected || isDirectSubmitting}
                  onClick={() => {
                    if (selected) void submitDirect(selected.id);
                  }}
                >
                  Land forces
                </button>
              </div>
              <p className="text-muted">
                One landing is submitted now. Recheck offered destinations before landing another
                unit.
              </p>
            </>
          )
        }
      </WorkflowShell>
    </section>
  );
};
