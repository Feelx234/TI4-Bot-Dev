import React, { useEffect, useState } from 'react';
import { GameLogEntry } from '../hooks/useGameSession.ts';
import { PendingChoiceDto } from '../protocol/types.ts';
import { EventLog } from './EventLog.tsx';
import { PendingChoiceModal } from './PendingChoiceModal.tsx';
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
}

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
}) => {
  const [openDrawer, setOpenDrawer] = useState<'events' | 'players' | null>(null);
  const [isChoiceMinimized, setIsChoiceMinimized] = useState(false);

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
        <PendingChoiceModal
          choice={choice}
          onSubmit={onSubmitChoice}
          lastError={lastError}
          isMinimized={isChoiceMinimized}
          onMinimizedChange={setIsChoiceMinimized}
          selectedOptionId={selectedOptionId}
          onSelectOption={onSelectOption}
        />
      </div>
    </div>
  );
};
