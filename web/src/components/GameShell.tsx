import React, { useEffect, useState } from 'react';
import { GameLogEntry } from '../hooks/useGameSession.ts';
import { PendingChoiceDto } from '../protocol/types.ts';
import { EventLog } from './EventLog.tsx';
import { PendingChoiceModal } from './PendingChoiceModal.tsx';

export interface GameShellProps {
  header: React.ReactNode;
  board: React.ReactNode;
  playerSheet: React.ReactNode;
  events: GameLogEntry[];
  choice: PendingChoiceDto | null;
  onSubmitChoice: (optionId: string) => Promise<void>;
  lastError?: string | null;
}

export const GameShell: React.FC<GameShellProps> = ({
  header,
  board,
  playerSheet,
  events,
  choice,
  onSubmitChoice,
  lastError,
}) => {
  const [openDrawer, setOpenDrawer] = useState<'events' | 'players' | null>(null);
  const [isChoiceMinimized, setIsChoiceMinimized] = useState(false);

  useEffect(() => {
    setIsChoiceMinimized(false);
  }, [choice?.nonce]);

  return (
    <div data-testid="game-container" className="app-shell">
      <div className="app-shell__header">{header}</div>
      <div className="app-shell__content">
        <main className="app-shell__board">{board}</main>
        <aside
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
          onClick={() => setOpenDrawer((drawer) => drawer === 'players' ? null : 'players')}
        >
          Players
        </button>
        <button
          type="button"
          data-testid="event-log-mobile-toggle"
          className="button button--secondary"
          aria-expanded={openDrawer === 'events'}
          onClick={() => setOpenDrawer((drawer) => drawer === 'events' ? null : 'events')}
        >
          Events
        </button>
      </div>

      <section
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
        />
      </div>
    </div>
  );
};
