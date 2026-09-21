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

  return (
    <>
      {/* Minimized Decision Pill for dedicated drawers/modals */}
      {isMinimized && workflow !== 'generic_selection' && workflow !== 'system_activation' && (
        <div
          className="choice-banner"
          data-testid="choice-minimized-pill"
          style={{
            position: 'fixed',
            bottom: 24,
            left: '50%',
            transform: 'translateX(-50%)',
            background: 'rgba(15, 23, 42, 0.95)',
            border: '2px solid #38bdf8',
            borderRadius: 8,
            padding: '8px 16px',
            display: 'flex',
            alignItems: 'center',
            gap: 12,
            boxShadow: '0 4px 20px rgba(0,0,0,0.6)',
            zIndex: 1000,
          }}
        >
          <span style={{ fontSize: 13, fontWeight: 600, color: '#f8fafc' }}>{choice.prompt}</span>
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

      {/* 1. Payment Drawer */}
      {workflow === 'payment' && (
        <PaymentDrawer
          choice={choice}
          model={model}
          viewerSeat={viewerSeat}
          player={choice ? players?.[choice.actor] : null}
          onSubmit={onSubmit}
          isOpen={!isMinimized}
          onClose={() => onMinimizedChange(true)}
          lastError={lastError}
        />
      )}

      {/* 2. Tactical Movement Overlay */}
      {workflow === 'tactical_movement' && (
        <TacticalMovementOverlay
          choice={choice}
          model={model}
          viewerSeat={viewerSeat}
          activeSystemId={
            model?.selectionMode.mode === 'tactical_move'
              ? model.selectionMode.activeSystem
              : (choice?.context?.target && 'System' in choice.context.target ? choice.context.target.System : null)
          }
          player={choice ? players?.[choice.actor] : null}
          onSubmit={onSubmit}
          isOpen={!isMinimized}
          onClose={() => onMinimizedChange(true)}
          lastError={lastError}
        />
      )}

      {/* 3. Combat Resolution Modal */}
      {(workflow === 'combat_sustain' || workflow === 'combat_casualty' || workflow === 'combat_retreat') && (
        <CombatResolutionModal
          choice={choice}
          model={model}
          viewerSeat={viewerSeat}
          onSubmit={onSubmit}
          isOpen={!isMinimized}
          onClose={() => onMinimizedChange(true)}
          lastError={lastError}
        />
      )}

      {/* 4. Trade Desk Modal */}
      {(workflow === 'transaction_propose' || workflow === 'transaction_answer') && (
        <TradeDeskModal
          choice={choice}
          model={model}
          viewerSeat={viewerSeat}
          onSubmit={onSubmit}
          isOpen={!isMinimized}
          onClose={() => onMinimizedChange(true)}
          lastError={lastError}
        />
      )}

      {/* 5. Agenda Ballot Modal */}
      {(workflow === 'agenda_vote_outcome' || workflow === 'agenda_vote_planets') && (
        <AgendaBallotModal
          choice={choice}
          model={model}
          viewerSeat={viewerSeat}
          onSubmit={onSubmit}
          isOpen={!isMinimized}
          onClose={() => onMinimizedChange(true)}
          lastError={lastError}
        />
      )}

      {/* 6. Reaction Status Bar */}
      {workflow === 'action_card_reaction' && (
        <ReactionStatusBar
          choice={choice}
          model={model}
          viewerSeat={viewerSeat}
          onSubmit={onSubmit}
          isOpen={!isMinimized}
          onClose={() => onMinimizedChange(true)}
          lastError={lastError}
        />
      )}

      {/* 7. Production Builder Drawer */}
      {workflow === 'production' && (
        <ProductionBuilderDrawer
          choice={choice}
          model={model}
          viewerSeat={viewerSeat}
          onSubmit={onSubmit}
          isOpen={!isMinimized}
          onClose={() => onMinimizedChange(true)}
          lastError={lastError}
        />
      )}

      {/* 8. Fallback / Generic Selection & System Activation Modal */}
      {(workflow === 'generic_selection' || workflow === 'system_activation' || workflow === 'objective_scoring') && (
        <PendingChoiceModal
          choice={choice}
          model={model}
          onSubmit={onSubmit}
          lastError={lastError}
          isMinimized={isMinimized}
          onMinimizedChange={onMinimizedChange}
          selectedOptionId={selectedOptionId}
          onSelectOption={onSelectOption}
        />
      )}
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
