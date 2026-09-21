import React, { useState, useEffect } from 'react';
import { ViewerRole } from './protocol/types.ts';
import { useGameSession } from './hooks/useGameSession.ts';
import { Board } from './components/Board.tsx';
import { TurnStatusBar } from './components/TurnStatusBar.tsx';
import { PlayerSheet } from './components/PlayerSheet.tsx';
import { Lobby } from './components/Lobby.tsx';
import { GameShell } from './components/GameShell.tsx';

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
  const [selectedOptionId, setSelectedOptionId] = useState<string | undefined>();
  const [selectedSystemId, setSelectedSystemId] = useState<string | null>(null);

  useEffect(() => {
    setSelectedOptionId(undefined);
  }, [pendingChoice?.nonce]);

  const handleSelectTarget = (systemId: string, planetId?: string) => {
    if (pendingChoice && userSeat && pendingChoice.actor === userSeat) {
      const match = pendingChoice.options.find((opt) => {
        const payload = opt.payload;
        if (planetId && payload?.planet === planetId) return true;
        if (payload?.system !== undefined && String(payload.system) === systemId) return true;
        if (payload?.to !== undefined && String(payload.to) === systemId) return true;
        if (opt.kind === 'activate' && String(payload?.system || opt.id) === systemId) return true;
        return false;
      });
      if (match) {
        setSelectedOptionId(match.id);
      }
    }
  };

  return (
    <GameShell
      header={(
        <div className="game-header">
          <TurnStatusBar
            status={turnStatus}
            view={snapshot?.view ?? null}
            gameVersion={gameVersion}
            connectionStatus={status}
            userSeat={userSeat}
          />
          <button
            data-testid="leave-game-button"
            onClick={onLeave}
            className="button button--secondary game-header__exit"
          >
            Exit Game
          </button>
        </div>
      )}
      board={snapshot ? (
        <Board
          board={snapshot.view.board}
          seatingOrder={snapshot.view.seating_order}
          players={snapshot.view.players}
          pendingChoice={pendingChoice}
          viewerSeat={userSeat}
          selectedSystemId={selectedSystemId}
          onSelectSystem={setSelectedSystemId}
          onSelectTarget={handleSelectTarget}
        />
      ) : (
        <div className="game-loading">Loading game state...</div>
      )}
      playerSheet={snapshot ? <PlayerSheet players={snapshot.view.players} userSeat={userSeat} /> : null}
      events={events}
      choice={pendingChoice}
      onSubmitChoice={submitChoice}
      lastError={lastError}
      selectedOptionId={selectedOptionId}
      onSelectOption={setSelectedOptionId}
    />
  );
};
