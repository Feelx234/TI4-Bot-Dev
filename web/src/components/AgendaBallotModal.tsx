import React, { useState, useEffect, useMemo } from 'react';
import { PendingChoiceDto } from '../protocol/types.ts';
import { usePipelineRunner, SemanticIntent } from '../hooks/usePipelineRunner.ts';
import { Dialog } from '../primitives/index.ts';
import { getAgendaPlanetVotes, ChoiceRendererModel } from '../presentation/choiceModel.ts';
import { WorkflowShell } from './WorkflowShell.tsx';

export interface AgendaBallotModalProps {
  choice: PendingChoiceDto | null;
  model?: ChoiceRendererModel | null;
  viewerSeat?: string | null;
  onSubmit: (optionId: string) => Promise<void>;
  isOpen: boolean;
  onClose: () => void;
  lastError?: string | null;
}

export const AgendaBallotModal: React.FC<AgendaBallotModalProps> = ({
  choice,
  model,
  viewerSeat,
  onSubmit,
  isOpen,
  onClose,
  lastError,
}) => {
  const subtype = model?.workflow
    ? (model.workflow === 'agenda_vote_planets' ? 'vote_exhaust_planet'
      : model.workflow === 'agenda_vote_outcome' ? 'cast_vote'
      : choice?.context?.subtype ?? '')
    : (choice?.context?.subtype ?? '');

  const isCastVote = subtype === 'cast_vote';
  const isExhaustPlanet = subtype === 'vote_exhaust_planet';
  const isTiebreak = subtype === 'vote_tiebreak';

  // For planet basket in vote_exhaust_planet
  const [stagedPlanets, setStagedPlanets] = useState<string[]>([]);

  const { executePipeline, isRunning: isPipelineRunning } = usePipelineRunner(choice, onSubmit);

  // Reset staging on nonce change
  useEffect(() => {
    setStagedPlanets([]);
  }, [choice?.nonce]);

  // Extract vote tallies if in cast_vote
  const outcomeTallies = useMemo(() => {
    if (!choice || !isCastVote) return [];
    return choice.options
      .filter((o) => o.id !== 'decline' && o.kind !== 'decline')
      .map((opt) => ({
        id: opt.id,
        label: opt.label,
        currentVotes: typeof opt.payload?.current_votes === 'number' ? opt.payload.current_votes : 0,
      }));
  }, [choice, isCastVote]);

  // Extract planet options if in vote_exhaust_planet
  const planetOptions = useMemo(() => {
    if (!choice || !isExhaustPlanet) return [];
    return choice.options
      .filter((o) => o.id !== 'decline' && o.kind !== 'decline')
      .map((opt) => {
        const votes = getAgendaPlanetVotes(opt);
        const payloadPlanetName = typeof opt.payload?.planet_name === 'string'
          ? opt.payload.planet_name
          : null;
        return {
          id: opt.id,
          label: opt.label,
          planetName: payloadPlanetName || opt.label || 'Planet',
          votes,
        };
      });
  }, [choice, isExhaustPlanet]);

  const totalStagedVotes = useMemo(() => {
    return stagedPlanets.reduce((sum, planetId) => {
      const p = planetOptions.find((opt) => opt.id === planetId);
      return sum + (p ? p.votes : 0);
    }, 0);
  }, [stagedPlanets, planetOptions]);

  const togglePlanetStage = (planetId: string) => {
    setStagedPlanets((prev) =>
      prev.includes(planetId) ? prev.filter((p) => p !== planetId) : [...prev, planetId]
    );
  };

  const handleCommitPlanetVotes = (isDirectSubmitting: boolean) => {
    if (stagedPlanets.length === 0 || isPipelineRunning || isDirectSubmitting) return;

    const intents: SemanticIntent[] = stagedPlanets.map((planetId) => ({
      predicate: (opt) => opt.id === planetId,
    }));

    // After exhausting staged planets, finish with decline (end of planet exhaustion)
    intents.push({
      predicate: (opt) => opt.id === 'decline' || opt.kind === 'decline',
    });

    executePipeline(intents);
  };

  if (!isOpen || !choice) return null;

  return (
    <Dialog.Root open={isOpen} onOpenChange={(open) => { if (!open) onClose(); }}>
      <Dialog.Content
        data-testid="agenda-ballot-modal"
        className="agenda-dialog"
        style={{
          background: 'rgba(3, 7, 18, 0.85)',
          backdropFilter: 'blur(8px)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
        }}
      >
        <div
          className="panel"
          style={{
            border: '2px solid #a855f7',
            padding: 24,
            maxWidth: 580,
            width: '92%',
            display: 'flex',
            flexDirection: 'column',
            gap: 16,
            color: '#f8fafc',
            boxShadow: '0 0 32px rgba(168, 85, 247, 0.25)',
          }}
        >
          {/* Header */}
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
            <div>
              <div style={{ fontSize: 11, fontWeight: 700, color: '#c084fc', textTransform: 'uppercase' }}>
                Imperial Council Ballot • Seat: {choice.actor}
              </div>
              <Dialog.Title
                as="h2"
                data-testid="agenda-ballot-title"
                style={{ fontSize: 18, fontWeight: 700, margin: '4px 0 0 0', color: '#f8fafc' }}
              >
                {isCastVote
                  ? 'Step 1: Choose Voting Outcome'
                  : isExhaustPlanet
                    ? 'Step 2: Commit Planet Influence'
                    : isTiebreak
                      ? '⚖️ Speaker Tiebreaker Decision'
                      : choice.prompt}
              </Dialog.Title>
            </div>

            <button
              type="button"
              data-testid="close-agenda-modal"
              onClick={onClose}
              className="button button--secondary button--icon"
              aria-label="Close agenda dialog"
              style={{ minWidth: 28, height: 28 }}
            >
              ✕
            </button>
          </div>

          <WorkflowShell
            choice={choice}
            model={model}
            viewerSeat={viewerSeat}
            onSubmit={onSubmit}
            lastError={lastError}
            spectatorNotice={`Observing council voting in progress for seat ${choice.actor}...`}
            spectatorNoticeTestId="spectator-agenda-notice"
            errorTestId="agenda-error-banner"
          >
            {({ isActor, isDirectSubmitting, declineOption, submitDirect }) => <>

          {/* Stage 1: Cast Vote Outcome Selection */}
          {isActor && isCastVote && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
              <div style={{ fontSize: 13, color: '#cbd5e1' }}>
                Select an outcome to cast your votes on, or choose to abstain:
              </div>

              {/* Live Outcome Tallies */}
              <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                {outcomeTallies.map((outcome) => (
                  <button
                    key={outcome.id}
                    type="button"
                    data-testid={`vote-outcome-opt-${outcome.id}`}
                    onClick={() => submitDirect(outcome.id)}
                    disabled={isDirectSubmitting}
                    className="button button--secondary"
                    style={{
                      display: 'flex',
                      justifyContent: 'space-between',
                      alignItems: 'center',
                      padding: '12px 16px',
                      background: '#1e293b',
                      border: '1px solid #475569',
                      borderRadius: 6,
                    }}
                  >
                    <span style={{ fontSize: 14, fontWeight: 600, color: '#f8fafc' }}>
                      {outcome.label}
                    </span>
                    <span
                      data-testid={`outcome-tally-${outcome.id}`}
                      style={{
                        fontSize: 13,
                        fontWeight: 700,
                        color: '#c084fc',
                        background: '#0f172a',
                        padding: '3px 8px',
                        borderRadius: 4,
                      }}
                    >
                      {outcome.currentVotes} Votes
                    </span>
                  </button>
                ))}
              </div>

              {declineOption && (
                <div style={{ display: 'flex', justifyContent: 'flex-end', marginTop: 4 }}>
                  <button
                    type="button"
                    data-testid="abstain-vote-btn"
                    onClick={() => submitDirect(declineOption.id)}
                    disabled={isDirectSubmitting}
                    className="button button--secondary"
                    style={{ fontSize: 13 }}
                  >
                    {declineOption.label || 'Abstain from Voting'}
                  </button>
                </div>
              )}
            </div>
          )}

          {/* Stage 2: Planet Exhaustion Influence Basket */}
          {isActor && isExhaustPlanet && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
              <div style={{ fontSize: 13, color: '#cbd5e1' }}>
                {choice.prompt}. Select planets to exhaust for influence votes:
              </div>

              {/* Basket Tally Header */}
              <div
                style={{
                  background: '#1e293b',
                  borderRadius: 6,
                  padding: '10px 14px',
                  border: '1px solid #a855f7',
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                }}
              >
                <span style={{ fontSize: 13, color: '#94a3b8' }}>Staged Votes:</span>
                <span
                  data-testid="staged-votes-counter"
                  style={{ fontSize: 15, fontWeight: 700, color: '#c084fc' }}
                >
                  +{totalStagedVotes} Votes
                </span>
              </div>

              {/* Planet Grid */}
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8 }}>
                {planetOptions.map((planet) => {
                  const isStaged = stagedPlanets.includes(planet.id);
                  return (
                    <button
                      key={planet.id}
                      type="button"
                      data-testid={`planet-card-${planet.id}`}
                      onClick={() => togglePlanetStage(planet.id)}
                       disabled={isPipelineRunning || isDirectSubmitting}
                      className="button button--secondary"
                      style={{
                        padding: '10px 12px',
                        textAlign: 'left',
                        display: 'flex',
                        justifyContent: 'space-between',
                        alignItems: 'center',
                        background: isStaged ? 'rgba(168, 85, 247, 0.2)' : '#1e293b',
                        border: isStaged ? '2px solid #a855f7' : '1px solid #334155',
                        borderRadius: 6,
                      }}
                    >
                      <span style={{ fontSize: 13, fontWeight: 600, color: '#f8fafc' }}>
                        {planet.planetName}
                      </span>
                      <span
                        style={{
                          fontSize: 12,
                          fontWeight: 700,
                          color: '#c084fc',
                          background: '#0f172a',
                          padding: '2px 6px',
                          borderRadius: 4,
                        }}
                      >
                        {planet.votes} v
                      </span>
                    </button>
                  );
                })}
              </div>

              {/* Commit & Done Actions */}
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: 4 }}>
                {declineOption ? (
                  <button
                    type="button"
                    data-testid="done-voting-planets-btn"
                    onClick={() => submitDirect(declineOption.id)}
                     disabled={isPipelineRunning || isDirectSubmitting}
                    className="button button--secondary"
                    style={{ fontSize: 13 }}
                  >
                    Done Voting
                  </button>
                ) : <div />}

                <button
                  type="button"
                  data-testid="commit-planet-votes-btn"
                   onClick={() => handleCommitPlanetVotes(isDirectSubmitting)}
                   disabled={stagedPlanets.length === 0 || isPipelineRunning || isDirectSubmitting}
                  className="button button--primary"
                  style={{
                    padding: '8px 20px',
                    fontSize: 13,
                    background: stagedPlanets.length > 0 ? '#a855f7' : '#475569',
                  }}
                >
                  {isPipelineRunning ? 'Committing Votes...' : `Cast ${totalStagedVotes} Votes`}
                </button>
              </div>
            </div>
          )}

          {/* Stage 3: Speaker Tiebreaker Gavel */}
          {isActor && isTiebreak && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
              <div
                style={{
                  background: 'rgba(234, 179, 8, 0.15)',
                  border: '1px solid #eab308',
                  borderRadius: 6,
                  padding: '10px 14px',
                  fontSize: 13,
                  color: '#fef08a',
                  display: 'flex',
                  alignItems: 'center',
                  gap: 8,
                }}
              >
                <span>⚖️</span>
                <span>The council vote is tied! As Speaker, you have the sole authority to break the tie.</span>
              </div>

              <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                {choice.options.map((opt) => (
                  <button
                    key={opt.id}
                    type="button"
                    data-testid={`tiebreak-opt-${opt.id}`}
                    onClick={() => submitDirect(opt.id)}
                    disabled={isDirectSubmitting}
                    className="button button--primary"
                    style={{
                      padding: '12px 16px',
                      fontSize: 14,
                      fontWeight: 600,
                      background: '#a855f7',
                      textAlign: 'center',
                    }}
                  >
                    Resolve in Favor of: {opt.label}
                  </button>
                ))}
              </div>
            </div>
          )}

            </>}
          </WorkflowShell>
        </div>
      </Dialog.Content>
    </Dialog.Root>
  );
};
