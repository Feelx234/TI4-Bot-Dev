import React from "react";
import { PendingChoiceDto } from "../protocol/types.ts";
import { getCombatPayload, ChoiceRendererModel } from "../presentation/choiceModel.ts";
import { Dialog } from "../primitives/index.ts";
import { WorkflowShell } from "./WorkflowShell.tsx";
import { usePlayerIdentity } from "../presentation/PlayerIdentity.tsx";
import { DecisionHeader } from "./DecisionHeader.tsx";

export interface CombatResolutionModalProps {
  choice: PendingChoiceDto | null;
  model?: ChoiceRendererModel | null;
  viewerSeat?: string | null;
  onSubmit: (optionId: string) => Promise<void>;
  isOpen: boolean;
  onClose: () => void;
  lastError?: string | null;
  recentDiceRolls?: { unit: string; roll: number; target: number; hit: boolean }[];
}

export const CombatResolutionModal: React.FC<CombatResolutionModalProps> = ({
  choice,
  model,
  viewerSeat,
  onSubmit,
  isOpen,
  onClose,
  lastError,
  recentDiceRolls = [],
}) => {
  const display = usePlayerIdentity();
  const subtype = choice?.context?.subtype ?? "";
  const constraints = model?.outstanding?.[0] ?? choice?.context?.outstanding?.[0];
  const hitsOwed =
    model?.selectionMode.mode === "casualty"
      ? model.selectionMode.hitsToAssign
      : model?.selectionMode.mode === "sustain"
        ? model.selectionMode.hitsRemaining
        : constraints?.amount;

  const isSustainStage = subtype === "sustain_damage" || model?.workflow === "combat_sustain";
  const isCasualtyStage =
    subtype === "assign_casualty" ||
    model?.workflow === "combat_casualty" ||
    choice?.options.some((o) => o.kind === "casualty");
  const isRetreatStage =
    subtype === "announce_retreat" ||
    subtype === "retreat_to" ||
    model?.workflow === "combat_retreat";

  const casualtyOptions =
    choice?.options.filter((o) => o.id !== "decline" && o.kind !== "decline") ?? [];

  if (!isOpen || !choice) return null;

  return (
    <Dialog.Root
      open={isOpen}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <Dialog.Content
        data-testid="combat-resolution-modal"
        className="combat-dialog choice-workflow-dialog"
      >
        <div className="panel choice-workflow-modal">
          {/* Header */}
          <Dialog.Title as="h2" className="visually-hidden">
            {choice.prompt}
          </Dialog.Title>
          <DecisionHeader
            actor={choice.actor}
            title={
              isSustainStage
                ? "Sustain damage"
                : isCasualtyStage
                  ? "Assign a casualty"
                  : isRetreatStage
                    ? subtype === "retreat_to"
                      ? "Choose a retreat destination"
                      : "Announce retreat"
                    : choice.prompt
            }
            instruction={choice.prompt}
            onMinimize={onClose}
            titleTestId="combat-stage-title"
            minimizeTestId="close-combat-modal"
          />

          {/* Dice Results Feed */}
          {recentDiceRolls.length > 0 && (
            <div data-testid="combat-dice-feed" className="workflow-inline">
              <div className="workflow-inline">Recent Combat Rolls</div>
              <div className="workflow-inline">
                {recentDiceRolls.map((d, i) => (
                  <span key={i} data-testid="dice-roll-badge" className="workflow-inline">
                    {d.unit} ({d.target}+): [{d.roll}] {d.hit ? "★ HIT" : "MISS"}
                  </span>
                ))}
              </div>
            </div>
          )}

          <WorkflowShell
            choice={choice}
            model={model}
            viewerSeat={viewerSeat}
            onSubmit={onSubmit}
            lastError={lastError}
            spectatorNotice={`Observing combat resolution in progress for ${display(choice.actor).label}...`}
            spectatorNoticeTestId="spectator-combat-notice"
            errorTestId="combat-error-banner"
          >
            {({ isActor, isDirectSubmitting, declineOption, submitDirect }) => (
              <>
                {/* Stage 1: Sustain Damage */}
                {isActor && isSustainStage && (
                  <div className="workflow-inline">
                    <div className="workflow-inline">
                      ⚠️ Caution: Opponents holding "Direct Hit" action cards may react to destroy
                      sustained ships!
                    </div>

                    <div className="workflow-inline">
                      Select capital ships to sustain damage, or choose decline to proceed to direct
                      hits:
                    </div>

                    <div className="workflow-inline">
                      {choice.options
                        .filter((o) => o.id !== "decline" && o.kind !== "decline")
                        .map((opt) => (
                          <button
                            key={opt.id}
                            type="button"
                            data-testid={`sustain-opt-${opt.id}`}
                            onClick={() => submitDirect(opt.id)}
                            disabled={isDirectSubmitting}
                            className="button button--secondary"
                          >
                            <span className="workflow-inline">{opt.label}</span>
                            <span className="workflow-inline">Sustain Hit</span>
                          </button>
                        ))}
                    </div>

                    {declineOption && (
                      <div className="workflow-inline">
                        <button
                          type="button"
                          data-testid="decline-sustain-btn"
                          onClick={() => submitDirect(declineOption.id)}
                          disabled={isDirectSubmitting}
                          className="button button--secondary"
                        >
                          {declineOption.label || "Do Not Sustain Damage"}
                        </button>
                      </div>
                    )}
                  </div>
                )}

                {/* Each offered casualty removes one unit for one hit; options are deduplicated. */}
                {isActor && isCasualtyStage && (
                  <div className="workflow-inline">
                    <p>
                      {hitsOwed ? `${hitsOwed} hits remaining. ` : ""}Assign one hit now. The next
                      legal options arrive after this decision.
                    </p>
                    {choice.context?.target && (
                      <p>
                        Location:{" "}
                        {"System" in choice.context.target
                          ? `System ${choice.context.target.System}`
                          : "Planet" in choice.context.target
                            ? choice.context.target.Planet.planet
                            : "Combat"}
                      </p>
                    )}
                    {casualtyOptions.map((opt) => (
                      <button
                        key={opt.id}
                        type="button"
                        data-testid={`casualty-opt-${opt.id}`}
                        className="button button--secondary"
                        disabled={isDirectSubmitting}
                        onClick={() => void submitDirect(opt.id)}
                      >
                        Assign this hit: {opt.label}
                        {getCombatPayload(opt).damaged ? " (damaged)" : ""}
                      </button>
                    ))}
                  </div>
                )}

                {/* Retreat Stage */}
                {isActor && isRetreatStage && (
                  <div className="workflow-inline">
                    <div className="workflow-inline">
                      {subtype === "announce_retreat"
                        ? "Choose whether to announce a retreat before combat rounds commence:"
                        : "Select an adjacent system to retreat your surviving fleet to:"}
                    </div>

                    <div className="workflow-inline">
                      {choice.options.map((opt) => (
                        <button
                          key={opt.id}
                          type="button"
                          data-testid={`retreat-opt-${opt.id}`}
                          onClick={() => submitDirect(opt.id)}
                          disabled={isDirectSubmitting}
                          className="button button--secondary"
                        >
                          {opt.label}
                        </button>
                      ))}
                    </div>
                  </div>
                )}
              </>
            )}
          </WorkflowShell>
        </div>
      </Dialog.Content>
    </Dialog.Root>
  );
};
