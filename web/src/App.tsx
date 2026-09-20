import React, { useState } from 'react';
import { ViewerRole } from './protocol/types.ts';
import { useGameSession } from './hooks/useGameSession.ts';
import { Board } from './components/Board.tsx';
import { TurnStatusBar } from './components/TurnStatusBar.tsx';
import { PlayerSheet } from './components/PlayerSheet.tsx';
import { PendingChoiceModal } from './components/PendingChoiceModal.tsx';
import { EventLog } from './components/EventLog.tsx';
import { Lobby } from './components/Lobby.tsx';

export const App: React.FC = () => {
  const [activeGameId, setActiveGameId] = useState<string | null>(null);
  const [viewerRole, setViewerRole] = useState<ViewerRole>({ role: 'player', seat: 'p1' });

  if (!activeGameId) {
    return (
      <Lobby
        onJoin={(gameId, role) => {
          setActiveGameId(gameId);
          setViewerRole(role);
        }}
      />
    );
  }

  return (
    <GameViewContainer
      gameId={activeGameId}
      viewer={viewerRole}
      onLeave={() => setActiveGameId(null)}
    />
  );
};

interface GameViewContainerProps {
  gameId: string;
  viewer: ViewerRole;
  onLeave: () => void;
}

const GameViewContainer: React.FC<GameViewContainerProps> = ({ gameId, viewer, onLeave }) => {
  const {
    status,
    gameVersion,
    snapshot,
    pendingChoice,
    turnStatus,
    lastError,
    events,
    submitChoice,
  } = useGameSession({ gameId, viewer });

  const userSeat = viewer.role === 'player' ? viewer.seat : undefined;

  // Compute actionable system IDs if choice context names targets/destinations
  const actionableSystemIds = React.useMemo(() => {
    if (!pendingChoice) return [];
    // Extract any system IDs mentioned in options
    const ids: string[] = [];
    for (const opt of pendingChoice.options) {
      const match = /system_?(\d+)/i.exec(opt.id);
      if (match) ids.push(match[1]);
    }
    return ids;
  }, [pendingChoice]);

  return (
    <div
      data-testid="game-container"
      style={{
        display: 'flex',
        flexDirection: 'column',
        height: '100vh',
        width: '100vw',
        background: '#090d16',
        color: '#f8fafc',
        overflow: 'hidden',
        position: 'relative',
      }}
    >
      {/* Top Turn & Status Bar */}
      <div style={{ display: 'flex', alignItems: 'stretch', zIndex: 150, position: 'relative' }}>
        <div style={{ flex: 1 }}>
          <TurnStatusBar
            status={turnStatus}
            view={snapshot?.view ?? null}
            gameVersion={gameVersion}
            connectionStatus={status}
            userSeat={userSeat}
          />
        </div>
        <button
          data-testid="leave-game-button"
          onClick={onLeave}
          style={{
            background: '#1e293b',
            color: '#94a3b8',
            border: 'none',
            borderBottom: '1px solid #1e293b',
            padding: '0 16px',
            fontSize: 12,
            cursor: 'pointer',
          }}
        >
          Exit Game
        </button>
      </div>

      {/* Main Content: SVG Board + Player Sheet Sidebar */}
      <div style={{ display: 'flex', flex: 1, position: 'relative', overflow: 'hidden' }}>
        <main style={{ flex: 1, position: 'relative', height: '100%' }}>
          {snapshot ? (
            <Board
              board={snapshot.view.board}
              actionableSystemIds={actionableSystemIds}
            />
          ) : (
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                height: '100%',
                color: '#94a3b8',
              }}
            >
              Loading game state...
            </div>
          )}
        </main>

        {/* Player Sheet Sidebar */}
        {snapshot && (
          <PlayerSheet
            players={snapshot.view.players}
            userSeat={userSeat}
          />
        )}
      </div>

      {/* Interactive Choice Dialog (Only when a choice is pending for this user's seat) */}
      <PendingChoiceModal
        choice={pendingChoice}
        onSubmit={submitChoice}
        lastError={lastError}
      />

      {/* Collapsible Event Log Drawer */}
      <EventLog events={events} />
    </div>
  );
};
