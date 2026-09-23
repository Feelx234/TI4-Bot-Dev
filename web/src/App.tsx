import React, { useCallback, useEffect, useLayoutEffect, useState } from 'react';
import { ViewerRole } from './protocol/types.ts';
import { useGameSession } from './hooks/useGameSession.ts';
import { useLobbySession } from './hooks/useLobbySession.ts';
import { Board } from './components/Board.tsx';
import { TurnStatusBar } from './components/TurnStatusBar.tsx';
import { PlayerSheet } from './components/PlayerSheet.tsx';
import { CreateLobby, LobbyStatus } from './components/Lobby.tsx';
import { GameShell } from './components/GameShell.tsx';
import { usePresence } from './hooks/usePresence.ts';
import { PlayerIdentityProvider } from './presentation/PlayerIdentity.tsx';
import { participantText } from './presentation/participantText.ts';

const storageKey = (gameId: string) => `ti4.player-session:${gameId}`;
const pathGameId = () => /^\/games\/([^/]+)$/.exec(window.location.pathname)?.[1] ? decodeURIComponent(/^\/games\/([^/]+)$/.exec(window.location.pathname)![1]) : null;

export const App: React.FC = () => {
  const [gameId, setGameId] = useState(pathGameId);
  const [error, setError] = useState<string | null>(null);
  const [token, setToken] = useState(() => gameId ? sessionStorage.getItem(storageKey(gameId)) ?? undefined : undefined);
  const navigate = (id: string | null, nextToken?: string) => { if (id) { if (nextToken) sessionStorage.setItem(storageKey(id), nextToken); history.pushState({}, '', `/games/${encodeURIComponent(id)}`); } else history.pushState({}, '', '/'); setGameId(id); setToken(nextToken); };
  useLayoutEffect(() => {
    const receive = () => { const id = pathGameId(); setGameId(id); setToken(id ? sessionStorage.getItem(storageKey(id)) ?? undefined : undefined); };
    window.addEventListener('popstate', receive);
    return () => window.removeEventListener('popstate', receive);
  }, []);
  if (!gameId) return <><CreateLobby onError={setError} onCreated={(created) => navigate(created.game_id, created.player_session)} />{error && <div className="session-error" role="alert">{error}</div>}</>;
  return <GameRoute key={gameId} gameId={gameId} token={token} onCredential={(credential) => { sessionStorage.setItem(storageKey(gameId), credential); setToken(credential); }} onCredentialInvalid={() => { sessionStorage.removeItem(storageKey(gameId)); setToken(undefined); }} onForget={() => { sessionStorage.removeItem(storageKey(gameId)); navigate(null); }} />;
};

const GameRoute: React.FC<{ gameId: string; token?: string; onCredential: (credential: string) => void; onCredentialInvalid: () => void; onForget: () => void }> = ({ gameId, token, onCredential, onCredentialInvalid, onForget }) => {
  const { lobby, playerId, error, loading, invalidCredential, pendingAction, setReady, start, reorder, join, leave } = useLobbySession(gameId, token);
  const [watching, setWatching] = useState(false);
  const invalidate = useCallback(() => onCredentialInvalid(), [onCredentialInvalid]);
  usePresence(gameId, token, invalidate);
  useEffect(() => { if (invalidCredential) onCredentialInvalid(); }, [invalidCredential, onCredentialInvalid]);
  if (!lobby) return <main className="lobby-page"><div className="panel lobby-panel">{loading ? 'Loading lobby...' : 'Unable to load lobby.'}</div></main>;
  const viewer: ViewerRole = token && playerId ? { role: 'player', seat: playerId, playerSession: token } : { role: 'spectator' };
  const enter = async (nickname: string, id?: string) => { const credential = await join(nickname, id); if (credential) { setWatching(false); onCredential(credential); } };
  const leaveLobby = async () => { if (await leave()) onForget(); };
   return <>{error && <div className="session-error" role="alert">{participantText(error, lobby, [])} Check the lobby and try again.</div>}{lobby.phase === 'running' && (playerId || watching) ? <GameViewContainer key={`${gameId}:${token ?? 'watch'}`} gameId={gameId} lobby={lobby} viewer={viewer} onLeave={onForget} /> : <LobbyStatus lobby={lobby} playerId={playerId} watching={watching} pendingAction={pendingAction} onReady={(ready) => void setReady(ready)} onStart={() => void start()} onLeave={() => void leaveLobby()} onJoin={(name) => void enter(name)} onTakeover={(id, name) => void enter(name, id)} onReorder={(ids) => void reorder(ids)} onWatch={() => setWatching(true)} />}</>;
};

const GameViewContainer: React.FC<{ gameId: string; lobby: import('./protocol/types.ts').LobbyDto; viewer: ViewerRole; onLeave: () => void }> = ({ gameId, lobby, viewer, onLeave }) => {
  const { status, gameVersion, snapshot, pendingChoice, turnStatus, lastError, events, submitChoice } = useGameSession({ gameId, viewer });
  const userSeat = viewer.role === 'player' ? viewer.seat : undefined;
  const [selectedOptionId, setSelectedOptionId] = useState<string>(); const [selectedSystemId, setSelectedSystemId] = useState<string | null>(null);
  useEffect(() => setSelectedOptionId(undefined), [pendingChoice?.nonce]);
  const handleSelectTarget = (systemId: string, planetId?: string) => { if (!pendingChoice || pendingChoice.actor !== userSeat) return; const match = pendingChoice.options.find((option) => planetId ? option.payload?.planet === planetId : String(option.payload?.system ?? option.payload?.to ?? option.id) === systemId); if (match) setSelectedOptionId(match.id); };
  return <PlayerIdentityProvider lobby={lobby} seatingOrder={snapshot?.view.seating_order ?? []}><GameShell header={<div className="game-header"><TurnStatusBar status={turnStatus} view={snapshot?.view ?? null} gameVersion={gameVersion} connectionStatus={status} userSeat={userSeat} /><button data-testid="leave-game-button" onClick={onLeave} className="button button--secondary game-header__exit">Exit Game</button></div>} board={snapshot ? <Board board={snapshot.view.board} seatingOrder={snapshot.view.seating_order} players={snapshot.view.players} pendingChoice={pendingChoice} viewerSeat={userSeat} selectedSystemId={selectedSystemId} onSelectSystem={setSelectedSystemId} onSelectTarget={handleSelectTarget} /> : <div className="game-loading">Loading game state...</div>} playerSheet={snapshot ? <PlayerSheet players={snapshot.view.players} userSeat={userSeat} /> : null} events={events} choice={pendingChoice} viewerSeat={userSeat} players={snapshot?.view.players} onSubmitChoice={submitChoice} lastError={lastError} selectedOptionId={selectedOptionId} onSelectOption={setSelectedOptionId} /></PlayerIdentityProvider>;
};
