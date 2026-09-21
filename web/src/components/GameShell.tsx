import React, { useEffect, useState, useMemo } from 'react';
import { GameLogEntry } from '../hooks/useGameSession.ts';
import { PendingChoiceDto, PlayerView } from '../protocol/types.ts';
import { EventLog } from './EventLog.tsx';
import { PendingChoiceModal } from './PendingChoiceModal.tsx';
import { PaymentDrawer } from './PaymentDrawer.tsx';
import { TacticalMovementOverlay } from './TacticalMovementOverlay.tsx';
import { CombatResolutionModal } from './CombatResolutionModal.tsx';
import { TradeDeskModal } from './TradeDeskModal.tsx';
import { AgendaBallotModal } from './AgendaBallotModal.tsx';
import { ReactionStatusBar } from './ReactionStatusBar.tsx';
import { ProductionBuilderDrawer } from './ProductionBuilderDrawer.tsx';
import { deriveChoiceRendererModel, ChoiceRendererModel } from '../presentation/choiceModel.ts';
import { overlayStack } from '../primitives/index.ts';

export interface GameShellProps {
  header: React.ReactNode;
  board: React.ReactNode;
  playerSheet: React.ReactNode;
  events: GameLogEntry[];
  choice: PendingChoiceDto | null;
  onSubmitChoice: (optionId: string) => Promise<void>;
  lastError?: string | null;
  selectedOptionId?: string;
  onSelectOption?: (optionId: string) => void;
  viewerSeat?: string | null;
  players?: Record<string, PlayerView> | PlayerView[];
}

export interface ChoiceRendererDispatcherProps {
  choice: PendingChoiceDto | null;
  model?: ChoiceRendererModel | null;
  viewerSeat?: string | null;
  onSubmit: (optionId: string) => Promise<void>;
  lastError?: string | null;
  selectedOptionId?: string;
  onSelectOption?: (optionId: string) => void;
  isMinimized: boolean;
  onMinimizedChange: (minimized: boolean) => void;
  players?: Record<string, PlayerView>;
}

type WorkflowRenderer = (props: Omit<ChoiceRendererDispatcherProps, 'model'> & {
  choice: PendingChoiceDto;
  model: ChoiceRendererModel | null;
}) => React.ReactNode;

const workflowRenderers = new Map<ChoiceRendererModel['workflow'], WorkflowRenderer>([
  ['payment', ({ choice, model, viewerSeat, players, onSubmit, isMinimized, onMinimizedChange, lastError }) => (
    <PaymentDrawer choice={choice} model={model} viewerSeat={viewerSeat} player={players?.[choice.actor] ?? null}
      onSubmit={onSubmit} isOpen={!isMinimized} onClose={() => onMinimizedChange(true)} lastError={lastError} />
  )],
  ['tactical_movement', ({ choice, model, viewerSeat, players, onSubmit, isMinimized, onMinimizedChange, lastError }) => (
    <TacticalMovementOverlay choice={choice} model={model} viewerSeat={viewerSeat}
      activeSystemId={model?.selectionMode.mode === 'tactical_move' ? model.selectionMode.activeSystem
        : (choice.context?.target && 'System' in choice.context.target ? choice.context.target.System : null)}
      player={players?.[choice.actor] ?? null} onSubmit={onSubmit} isOpen={!isMinimized}
      onClose={() => onMinimizedChange(true)} lastError={lastError} />
  )],
  ['combat_sustain', renderCombat],
  ['combat_casualty', renderCombat],
  ['combat_retreat', renderCombat],
  ['transaction_propose', renderTrade],
  ['transaction_answer', renderTrade],
  ['agenda_vote_outcome', renderAgenda],
  ['agenda_vote_planets', renderAgenda],
  ['action_card_reaction', ({ choice, model, viewerSeat, onSubmit, isMinimized, onMinimizedChange, lastError }) => (
    <ReactionStatusBar choice={choice} model={model} viewerSeat={viewerSeat} onSubmit={onSubmit}
      isOpen={!isMinimized} onClose={() => onMinimizedChange(true)} lastError={lastError} />
  )],
  ['production', ({ choice, model, viewerSeat, onSubmit, isMinimized, onMinimizedChange, lastError }) => (
    <ProductionBuilderDrawer choice={choice} model={model} viewerSeat={viewerSeat} onSubmit={onSubmit}
      isOpen={!isMinimized} onClose={() => onMinimizedChange(true)} lastError={lastError} />
  )],
  ['generic_selection', renderGeneric],
  ['system_activation', renderGeneric],
  ['objective_scoring', renderGeneric],
]);

function renderCombat({ choice, model, viewerSeat, onSubmit, isMinimized, onMinimizedChange, lastError }: Parameters<WorkflowRenderer>[0]) {
  return <CombatResolutionModal choice={choice} model={model} viewerSeat={viewerSeat} onSubmit={onSubmit}
    isOpen={!isMinimized} onClose={() => onMinimizedChange(true)} lastError={lastError} />;
}

function renderTrade({ choice, model, viewerSeat, onSubmit, isMinimized, onMinimizedChange, lastError }: Parameters<WorkflowRenderer>[0]) {
  return <TradeDeskModal choice={choice} model={model} viewerSeat={viewerSeat} onSubmit={onSubmit}
    isOpen={!isMinimized} onClose={() => onMinimizedChange(true)} lastError={lastError} />;
}

function renderAgenda({ choice, model, viewerSeat, onSubmit, isMinimized, onMinimizedChange, lastError }: Parameters<WorkflowRenderer>[0]) {
  return <AgendaBallotModal choice={choice} model={model} viewerSeat={viewerSeat} onSubmit={onSubmit}
    isOpen={!isMinimized} onClose={() => onMinimizedChange(true)} lastError={lastError} />;
}

function renderGeneric({ choice, model, onSubmit, lastError, isMinimized, onMinimizedChange, selectedOptionId, onSelectOption }: Parameters<WorkflowRenderer>[0]) {
  return <PendingChoiceModal choice={choice} model={model} onSubmit={onSubmit} lastError={lastError}
    isMinimized={isMinimized} onMinimizedChange={onMinimizedChange} selectedOptionId={selectedOptionId}
    onSelectOption={onSelectOption} />;
}

export const ChoiceRendererDispatcher: React.FC<ChoiceRendererDispatcherProps> = ({
  choice,
  model: propModel,
  viewerSeat,
  onSubmit,
  lastError,
  selectedOptionId,
  onSelectOption,
  isMinimized,
  onMinimizedChange,
  players,
}) => {
  const derivedModel = useMemo(() => {
    return choice ? deriveChoiceRendererModel(choice, viewerSeat ?? null) : null;
  }, [choice, viewerSeat]);

  if (!choice) return null;

  const model = propModel ?? derivedModel;
  const workflow = model?.workflow ?? 'generic_selection';
  const renderer = workflowRenderers.get(workflow) ?? workflowRenderers.get('generic_selection')!;

  return (
    <>
      {/* Minimized Decision Pill for dedicated drawers/modals */}
      {isMinimized && workflow !== 'generic_selection' && workflow !== 'system_activation' && (
        <div
          className="choice-banner choice-minimized-pill"
          data-testid="choice-minimized-pill"
        >
          <span className="choice-minimized-pill__prompt">{choice.prompt}</span>
          <button
            type="button"
            data-testid="resume-decision-btn"
            onClick={() => onMinimizedChange(false)}
            className="button button--primary button--sm"
          >
            Resume Decision
          </button>
        </div>
      )}

       {renderer({ choice, model, viewerSeat, onSubmit, lastError, selectedOptionId, onSelectOption,
         isMinimized, onMinimizedChange, players })}
    </>
  );
};

export const GameShell: React.FC<GameShellProps> = ({
  header,
  board,
  playerSheet,
  events,
  choice,
  onSubmitChoice,
  lastError,
  selectedOptionId,
  onSelectOption,
  viewerSeat,
  players,
}) => {
  const [openDrawer, setOpenDrawer] = useState<'events' | 'players' | null>(null);
  const [isChoiceMinimized, setIsChoiceMinimized] = useState(false);

  const playersMap = React.useMemo<Record<string, PlayerView>>(() => {
    if (!players) return {};
    if (Array.isArray(players)) {
      return Object.fromEntries(players.map((p) => [p.id, p]));
    }
    return players;
  }, [players]);

  useEffect(() => {
    setIsChoiceMinimized(false);
  }, [choice?.nonce]);

  // Register open drawer in overlayStack for Escape dismissal
  useEffect(() => {
    if (!openDrawer) return;
    const unregister = overlayStack.register({
      id: `drawer-${openDrawer}`,
      modal: false,
      onDismiss: () => setOpenDrawer(null),
    });
    return unregister;
  }, [openDrawer]);

  return (
    <div data-testid="game-container" className="app-shell">
      <div className="app-shell__header">{header}</div>
      <div className="app-shell__content">
        <main className="app-shell__board">{board}</main>
        <aside
          id="player-sheet-drawer"
          data-testid="player-sheet-drawer"
          className={`app-shell__player-sheet${openDrawer === 'players' ? ' app-shell__drawer--open' : ''}`}
        >
          {playerSheet}
        </aside>
      </div>

      <div className="app-shell__mobile-actions" aria-label="Game panels">
        <button
          type="button"
          data-testid="player-sheet-toggle"
          className="button button--secondary"
          aria-expanded={openDrawer === 'players'}
          aria-controls="player-sheet-drawer"
          onClick={() => setOpenDrawer((drawer) => drawer === 'players' ? null : 'players')}
        >
          Players
        </button>
        <button
          type="button"
          data-testid="event-log-mobile-toggle"
          className="button button--secondary"
          aria-expanded={openDrawer === 'events'}
          aria-controls="event-log-drawer"
          onClick={() => setOpenDrawer((drawer) => drawer === 'events' ? null : 'events')}
        >
          Events
        </button>
      </div>

      <section
        id="event-log-drawer"
        className={`app-shell__event-log${openDrawer === 'events' ? ' app-shell__drawer--open' : ''}`}
        aria-label="Event log"
      >
        <EventLog
          events={events}
          isOpen={openDrawer === 'events'}
          onToggle={() => setOpenDrawer((drawer) => drawer === 'events' ? null : 'events')}
        />
      </section>

      <div className="app-shell__overlays">
        <ChoiceRendererDispatcher
          choice={choice}
          viewerSeat={viewerSeat}
          players={playersMap}
          onSubmit={onSubmitChoice}
          lastError={lastError}
          selectedOptionId={selectedOptionId}
          onSelectOption={onSelectOption}
          isMinimized={isChoiceMinimized}
          onMinimizedChange={setIsChoiceMinimized}
        />
      </div>
    </div>
  );
};
