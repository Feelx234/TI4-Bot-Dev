import React from "react";
import type { BoardView, PendingChoiceDto, PlayerView } from "../protocol/types.ts";
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
}> = ({ board, choice, players, viewerSeat, onSubmit, onClose, lastError, landingDraft, onLandingDraftChange }) => {
  const invasion = board.invasion;
  if (!invasion) return null;
  const system = board.systems[invasion.system_id];
  const landing = choice?.context?.subtype === "commit_ground_forces";
  return (
    <section className="panel decision-frame" data-testid="invasion-overlay" aria-label="Invasion">
      <h2>Invasion · {invasion.system_id}</h2>
      <p>{invasion.invader} · {invasion.phase.replaceAll("_", " ")}</p>
      <nav aria-label="Invasion planets">
        {invasion.planets.map((planet) => (
          <span key={planet} className={planet === invasion.current_planet ? "card card--selected" : "card"}>
            {planet} · {system?.planets[planet]?.controlled_by ?? "uncontrolled"}
          </span>
        ))}
      </nav>
      {invasion.current_planet && <p>Current planet: {invasion.current_planet} · Defender: {invasion.defender ?? "none"} · Ground round: {invasion.ground_round}</p>}
      {invasion.planets.map((planet) => (
        <div key={planet}>
          <h3>{planet}</h3>
          <p>{system?.units.filter((unit) => unit.planet === planet).map((unit) => `${unit.owner} ${unit.unit_type}${unit.damaged ? " (damaged)" : ""}`).join(", ") || "No forces on planet"}</p>
        </div>
      ))}
      {choice && viewerSeat === invasion.invader && <div hidden={!landing || choice.actor !== viewerSeat}>
        <InvasionLandingTray choice={choice} board={board} players={players} viewerSeat={viewerSeat} onSubmit={onSubmit} onClose={onClose} lastError={lastError} draft={landingDraft} onDraftChange={onLandingDraftChange} />
      </div>}
      {choice && landing && viewerSeat === choice.actor ? null : choice && viewerSeat === choice.actor ? (
        <WorkflowShell choice={choice} viewerSeat={viewerSeat} onSubmit={onSubmit} lastError={lastError} errorTestId="invasion-error">
          {({ isDirectSubmitting, submitDirect }) => <>
            <p>{choice.prompt}</p>
            <div className="decision-frame__options">{choice.options.map((option) => <button key={option.id} type="button" className="button button--secondary" disabled={isDirectSubmitting} onClick={() => void submitDirect(option.id)}>{choice.context?.subtype === "fight_ground_combat_round" ? "Fight next round" : option.label}{option.description && <small> · {option.description}</small>}</button>)}</div>
          </>}
        </WorkflowShell>
      ) : <p>Waiting for {choice?.actor ?? "invasion resolution"}</p>}
    </section>
  );
};
