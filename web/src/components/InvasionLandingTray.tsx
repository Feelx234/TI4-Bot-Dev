import React, { useEffect, useRef, useState } from "react";
import type { BoardView, PendingChoiceDto, PlayerView } from "../protocol/types.ts";
import { fetchGroundOdds, normalizeFaction, type GroundOddsRequest } from "../services/advisorService.ts";
import { DecisionHeader } from "./DecisionHeader.tsx";
import { WorkflowShell } from "./WorkflowShell.tsx";

export type Landing = { planet: string; unit: string; damaged: boolean };
const same = (a: Landing, b: Landing) => a.planet === b.planet && a.unit === b.unit && a.damaged === b.damaged;
const fromOption = (option: PendingChoiceDto["options"][number]): Landing | null =>
  typeof option.payload?.planet === "string" && typeof option.payload.unit === "string"
    ? { planet: option.payload.planet, unit: option.payload.unit, damaged: option.payload.damaged === true || (option.payload.damaged === undefined && option.label.includes("(damaged)")) }
    : null;

/** Confirmation submits one fresh engine offer per copy and pauses on any interruption. */
export const InvasionLandingTray: React.FC<{
  choice: PendingChoiceDto;
  board?: BoardView;
  players?: Record<string, PlayerView>;
  viewerSeat?: string | null;
  selectedOptionId?: string;
  onSubmit: (id: string) => Promise<void>;
  onClose: () => void;
  lastError?: string | null;
  draft?: Landing[];
  onDraftChange?: (draft: Landing[]) => void;
}> = ({ choice, board, players, viewerSeat, onSubmit, onClose, lastError, draft: controlledDraft, onDraftChange }) => {
  const system = board?.invasion?.system_id ?? board?.active_system ?? "";
  const [planet, setPlanet] = useState<string | null>(null);
  const [localDraft, setLocalDraft] = useState<Landing[]>([]);
  const draft = controlledDraft ?? localDraft;
  const draftRef = useRef(draft);
  draftRef.current = draft;
  const setDraft = (update: Landing[] | ((current: Landing[]) => Landing[])) => {
    const next = typeof update === "function" ? update(draftRef.current) : update;
    draftRef.current = next;
    if (onDraftChange) onDraftChange(next);
    else setLocalDraft(next);
  };
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [odds, setOdds] = useState<{ key: string; value: number | "loading" | "unavailable" | "no-battle" } | null>(null);
  const submitting = useRef(false);
  const submittedNonce = useRef<string | null>(null);
  const onSubmitRef = useRef(onSubmit);
  onSubmitRef.current = onSubmit;
  const origin = useRef({ actor: choice.actor, system });
  const options = choice.options.map((option) => ({ option, landing: fromOption(option) })).filter((entry): entry is { option: PendingChoiceDto["options"][number]; landing: Landing } => entry.landing !== null);
  const planets = [...new Set(options.map(({ landing }) => landing.planet))];

  const units = board?.systems[system]?.units ?? [];
  const previewKey = JSON.stringify([board?.invasion, planet, units, draft, players && Object.values(players).map(({ id, faction }) => [id, faction])]);
  useEffect(() => {
    if (!planet || !board?.invasion || board.invasion.phase !== "landing" || viewerSeat !== choice.actor) { setOdds(null); return; }
    const local = units.filter((unit) => unit.planet === planet);
    const defender = board.systems[system]?.planets[planet]?.controlled_by;
    const opponents = [...new Set(local.filter((unit) => unit.owner !== choice.actor && (unit.unit_type.includes("infantry") || unit.unit_type.includes("mech") || unit.unit_type.startsWith("titans_pds"))).map((unit) => unit.owner))];
    const opponent = defender && opponents.includes(defender) ? defender : opponents[0];
    const mine = local.filter((unit) => unit.owner === choice.actor && (unit.unit_type.includes("infantry") || unit.unit_type.includes("mech") || unit.unit_type.startsWith("titans_pds")));
    const planned = draft.filter((item) => item.planet === planet);
    if (!opponent) { setOdds({ key: previewKey, value: "no-battle" }); return; }
    if (mine.length + planned.length === 0 || !players?.[choice.actor] || !players[opponent]) { setOdds(null); return; }
    const count = (entries: { unit_type: string; damaged: boolean }[]) => {
      const units: Record<string, number> = {}, damaged: Record<string, number> = {};
      for (const entry of entries) { units[entry.unit_type] = (units[entry.unit_type] ?? 0) + 1; if (entry.damaged) damaged[entry.unit_type] = (damaged[entry.unit_type] ?? 0) + 1; }
      return { units, damaged };
    };
    const defenderForces = local.filter((unit) => unit.owner === opponent && (unit.unit_type.includes("infantry") || unit.unit_type.includes("mech") || unit.unit_type.startsWith("titans_pds")));
    const guns: Record<string, number> = {};
    for (const unit of local.filter((unit) => unit.owner === opponent && unit.unit_type.includes("pds") && !unit.unit_type.startsWith("titans_pds"))) guns[unit.unit_type] = (guns[unit.unit_type] ?? 0) + 1;
    const attack = count([...mine, ...planned.map((item) => ({ unit_type: item.unit, damaged: item.damaged }))]);
    const request: GroundOddsRequest = {
      attacker: { faction: normalizeFaction(players[choice.actor].faction), ...attack },
      defender: { faction: normalizeFaction(players[opponent].faction), ...count(defenderForces), guns },
      simulations: 2000,
    };
    const controller = new AbortController();
    setOdds({ key: previewKey, value: "loading" });
    void fetchGroundOdds(request, controller.signal).then((result) => { if (!controller.signal.aborted) setOdds({ key: previewKey, value: result.attacker_win_rate }); }).catch(() => { if (!controller.signal.aborted) setOdds({ key: previewKey, value: "unavailable" }); });
    return () => controller.abort();
  }, [previewKey, choice.actor, viewerSeat]);

  useEffect(() => {
    if (!running || submitting.current || submittedNonce.current === choice.nonce || !draft.length) return;
    if (choice.actor !== origin.current.actor || system !== origin.current.system || choice.context?.subtype !== "commit_ground_forces") {
      setRunning(false);
      setError("Landing paused: another decision intervened. Accepted landings remain committed; review the remaining draft.");
      return;
    }
    const next = draft[0];
    const offered = options.find(({ landing }) => same(landing, next));
    if (!offered) {
      setRunning(false);
      setError("Landing paused: the next force is no longer offered. Edit the remaining draft.");
      return;
    }
    submitting.current = true;
    const nonce = choice.nonce;
    void onSubmitRef.current(offered.option.id).then(() => {
      submittedNonce.current = nonce;
      setDraft((current) => current.slice(1));
    }).catch((cause: unknown) => {
      setRunning(false);
      setError(cause instanceof Error ? cause.message : String(cause));
    }).finally(() => { submitting.current = false; });
  }, [running, draft, choice.nonce, choice.actor, choice.context?.subtype, system]);

  useEffect(() => { if (running && draft.length === 0) setRunning(false); }, [running, draft.length]);
  const stock = (unit: string, damaged: boolean) => board?.systems[system]?.units.filter((piece) => piece.owner === choice.actor && !piece.planet && piece.unit_type === unit && piece.damaged === damaged).length ?? 1;
  const visibleOdds = odds?.key === previewKey ? odds.value : "loading";
  const available = (landing: Landing) => stock(landing.unit, landing.damaged) - draft.filter((item) => item.unit === landing.unit && item.damaged === landing.damaged).length;
  return (
    <section className="panel decision-frame" data-testid="invasion-landing-tray" aria-label="Ground force landing">
      <DecisionHeader actor={choice.actor} title="Plan landings" instruction={choice.prompt} progress={system ? `Active system: ${system}` : undefined} onMinimize={onClose} />
      <WorkflowShell choice={choice} viewerSeat={viewerSeat} onSubmit={onSubmit} lastError={lastError} errorTestId="invasion-error">
        {({ isActor, isDirectSubmitting, submitDirect }) => isActor && <>
          <p>Choose a planet, then stage forces. Only confirmed landings are public.</p>
          <div className="workflow-actions" aria-label="Landing destination">{planets.map((name) => <button key={name} type="button" className={planet === name ? "button button--primary" : "button button--secondary"} onClick={() => setPlanet(name)}>{name}</button>)}</div>
          {planet && <>
            <p>Already on {planet}: {board?.systems[system]?.units.filter((piece) => piece.owner === choice.actor && piece.planet === planet).length ?? 0} · Draft: {draft.filter((item) => item.planet === planet).length}</p>
            {board?.invasion?.phase === "landing" && <p data-testid="invasion-odds">{visibleOdds === "no-battle" ? "No ground battle expected" : visibleOdds === "loading" ? "Calculating…" : visibleOdds === "unavailable" ? "Odds unavailable" : `Projected odds if these forces land: ${Math.round(visibleOdds * 100)}%`}</p>}
            {typeof visibleOdds === "number" && <p className="text-muted">Conditional preview for this planet and its immediate defender; excludes cards, Parley, optional deploy and unmodeled modifiers.</p>}
            <div className="decision-frame__options">{options.filter(({ landing }) => landing.planet === planet).map(({ option, landing }) => <button key={option.id} type="button" className="button button--secondary" disabled={running || available(landing) <= 0} onClick={() => { setError(null); setDraft((current) => [...current, landing]); }}>{option.label} · {Math.max(0, available(landing))} in space</button>)}</div>
          </>}
          {draft.length > 0 && <p>Remaining draft: {draft.map((item) => `${item.unit}${item.damaged ? " (damaged)" : ""} → ${item.planet}`).join(", ")}</p>}
          {error && <p role="alert">{error}</p>}
          <div className="workflow-actions">
            <button type="button" className="button button--secondary" disabled={running} onClick={() => { setDraft([]); setError(null); }}>Reset draft</button>
            <button type="button" className="button button--primary" disabled={!draft.length || running} onClick={() => { origin.current = { actor: choice.actor, system }; submittedNonce.current = null; setError(null); setRunning(true); }}>Confirm landings</button>
            {choice.options.some((option) => option.id === "done_committing") && <button type="button" className="button button--secondary" disabled={running || isDirectSubmitting || draft.length > 0} onClick={() => void submitDirect("done_committing")}>Done committing</button>}
          </div>
          {running && <p>Submitting landings one at a time…</p>}
        </>}
      </WorkflowShell>
    </section>
  );
};
