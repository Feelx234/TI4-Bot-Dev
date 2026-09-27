import React, { useRef } from "react";
import type { BoardView, PendingChoiceDto, PlacedUnitView, PlayerView } from "../protocol/types.ts";
import { InvasionLandingTray, type Landing } from "./InvasionLandingTray.tsx";
import { WorkflowShell } from "./WorkflowShell.tsx";

export const InvasionOverlay: React.FC<{
  board: BoardView;
  players?: Record<string, PlayerView>;
  choice: PendingChoiceDto | null;
  viewerSeat?: string | null;
  onSubmit: (id: string) => Promise<void>;
  onClose: () => void;
  lastError?: string | null;
  landingDraft?: Landing[];
  onLandingDraftChange?: (draft: Landing[]) => void;
}> = ({
  board,
  choice,
  players,
  viewerSeat,
  onSubmit,
  onClose,
  lastError,
  landingDraft,
  onLandingDraftChange,
}) => {
  // A state update can briefly clear the offer between consecutive landings. Keep the
  // tray mounted so its confirmation pipeline survives until the fresh nonce arrives.
  const lastLandingChoice = useRef<PendingChoiceDto | null>(null);
  const invasion = board.invasion;
  if (!invasion) return null;
  if (choice?.context?.subtype === "commit_ground_forces" && choice.actor === invasion.invader)
    lastLandingChoice.current = choice;
  const trayChoice = choice ?? lastLandingChoice.current;
  const system = board.systems[invasion.system_id];
  const landing = choice?.context?.subtype === "commit_ground_forces";
  const step = invasion.last_step;
  const describe = (units: PlacedUnitView[]) =>
    units
      .map((unit) => `${unit.owner} ${unit.unit_type}${unit.damaged ? " (damaged)" : ""}`)
      .join(", ") || "None";
  return (
    <section
      className="panel decision-frame invasion-overlay"
      data-testid="invasion-overlay"
      aria-label="Invasion"
    >
      <h2>Invasion · {invasion.system_id}</h2>
      <p>
        {invasion.invader} · {invasion.phase.replaceAll("_", " ")}
      </p>
      <nav aria-label="Invasion planets">
        {invasion.planets.map((planet) => (
          <span
            key={planet}
            className={planet === invasion.current_planet ? "card card--selected" : "card"}
          >
            {planet} · {system?.planets[planet]?.controlled_by ?? "uncontrolled"}
          </span>
        ))}
      </nav>
      {invasion.current_planet && (
        <p>
          Current planet: {invasion.current_planet} · Defender: {invasion.defender ?? "none"} ·
          Ground round: {invasion.ground_round}
        </p>
      )}
      {step && (
        <section aria-label={`${step.planet} ${step.kind} result`} data-testid="invasion-step">
          <h3>
            {step.planet} · {step.kind.replaceAll("_", " ")}{" "}
            {step.round > 0 ? `· round ${step.round}` : ""}
          </h3>
          <p>Before: {describe(step.before)}</p>
          <p>
            Hits:{" "}
            {Object.entries(step.hits)
              .map(([owner, hits]) => `${owner} ${hits}`)
              .join(" · ")}
            {step.harrow_hits > 0 ? ` · Harrow ${step.harrow_hits}` : ""}
          </p>
          {step.dice.length > 0 && (
            <div aria-label="Ground dice">
              {step.dice.map((die, index) => (
                <span key={index} className="card">
                  {die.player} · {die.group}: {die.face} / {die.target}{" "}
                  {die.hit ? "hit" : "miss"}{" "}
                </span>
              ))}
            </div>
          )}
          <p>After: {describe(step.after)}</p>
        </section>
      )}
      {invasion.planets.map((planet) => (
        <div key={planet}>
          <h3>{planet}</h3>
          <p>
            {system?.units
              .filter((unit) => unit.planet === planet)
              .map((unit) => `${unit.owner} ${unit.unit_type}${unit.damaged ? " (damaged)" : ""}`)
              .join(", ") || "No forces on planet"}
          </p>
        </div>
      ))}
      {viewerSeat === invasion.invader &&
      landingDraft?.length &&
      (!landing || choice?.actor !== viewerSeat) ? (
        <p data-testid="invasion-interrupted-draft">
          Landing paused · remaining draft:{" "}
          {landingDraft
            .map((item) => `${item.unit}${item.damaged ? " (damaged)" : ""} → ${item.planet}`)
            .join(", ")}
        </p>
      ) : null}
      {trayChoice && viewerSeat === invasion.invader && (
        <div hidden={!landing || choice?.actor !== viewerSeat}>
          <InvasionLandingTray
            choice={trayChoice}
            board={board}
            players={players}
            viewerSeat={viewerSeat}
            onSubmit={onSubmit}
            onClose={onClose}
            lastError={lastError}
            draft={landingDraft}
            onDraftChange={onLandingDraftChange}
          />
        </div>
      )}
      {choice && landing && viewerSeat === choice.actor ? null : choice &&
        viewerSeat === choice.actor ? (
        <WorkflowShell
          choice={choice}
          viewerSeat={viewerSeat}
          onSubmit={onSubmit}
          lastError={lastError}
          errorTestId="invasion-error"
        >
          {({ isDirectSubmitting, submitDirect }) => (
            <>
              <p>{choice.prompt}</p>
              <div className="decision-frame__options">
                {choice.options.map((option) => (
                  <button
                    key={option.id}
                    type="button"
                    className="button button--secondary"
                    disabled={isDirectSubmitting}
                    onClick={() => void submitDirect(option.id)}
                  >
                    {choice.context?.subtype === "fight_ground_combat_round"
                      ? "Fight next round"
                      : option.label}
                    {option.description && <small> · {option.description}</small>}
                  </button>
                ))}
              </div>
            </>
          )}
        </WorkflowShell>
      ) : (
        <p>Waiting for {choice?.actor ?? "invasion resolution"}</p>
      )}
    </section>
  );
};
