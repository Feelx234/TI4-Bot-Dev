import { useCallback, useEffect, useState } from 'react';
import { decodeJoinResponse, decodeLobby } from '../protocol/decode.ts';
import { LobbyDto } from '../protocol/types.ts';

export interface LobbySessionState {
  lobby: LobbyDto | null;
  playerId: string | null;
  error: string | null;
  loading: boolean;
  invalidCredential: boolean;
  setReady: (ready: boolean) => Promise<void>;
  start: () => Promise<void>;
  reorder: (slotIds: string[]) => Promise<void>;
  join: (playerId?: string) => Promise<string | undefined>;
  leave: () => Promise<boolean>;
}

export function useLobbySession(gameId: string, playerSession?: string): LobbySessionState {
  const [lobby, setLobby] = useState<LobbyDto | null>(null);
  const [playerId, setPlayerId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [invalidCredential, setInvalidCredential] = useState(false);
  const base = `/api/games/${encodeURIComponent(gameId)}/lobby`;
  const headers: Record<string, string> = playerSession ? { 'x-ti4-player-session': playerSession } : {};

  useEffect(() => {
    let active = true;
    setInvalidCredential(false);
    const load = async () => {
      try {
        // The public lobby intentionally has no viewer field. Reconnect explicitly to
        // obtain the authenticated identity; spectators only make a read-only GET.
        const response = await fetch(playerSession ? `${base}/join` : base, playerSession
          ? { method: 'POST', headers: { 'Content-Type': 'application/json', ...headers }, body: JSON.stringify({ kind: 'new' }) }
          : {});
        if (!response.ok) {
          if (response.status === 403 && playerSession && active) setInvalidCredential(true);
          throw new Error(`Lobby request failed (${response.status})`);
        }
        const joined = playerSession ? decodeJoinResponse(await response.json(), gameId) : null;
        const next = joined?.lobby ?? decodeLobby(await response.json(), gameId);
        if (active) { setLobby(next); setPlayerId(joined?.player.id ?? null); setError(null); setLoading(false); }
      } catch (cause) {
        if (active) { setError(String(cause)); setLoading(false); }
      }
    };
    void load();
    const timer = window.setInterval(() => void load(), 2_000);
    return () => { active = false; window.clearInterval(timer); };
  }, [gameId, playerSession]);

  const mutate = useCallback(async (path: 'ready' | 'start' | 'reorder', body?: object) => {
    try {
      const response = await fetch(`${base}/${path}`, { method: 'POST', headers: { 'Content-Type': 'application/json', ...headers }, body: body ? JSON.stringify(body) : undefined });
      if (!response.ok) {
        throw new Error(`Lobby ${path} failed (${response.status}): ${await response.text()}`);
      }
      setLobby(decodeLobby(await response.json(), gameId)); setError(null);
    } catch (cause) { setError(String(cause)); }
  }, [base, gameId, playerSession]);

  const join = useCallback(async (takeoverId?: string) => {
    try {
      const response = await fetch(`${base}/join`, { method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(takeoverId ? { kind: 'takeover', player_id: takeoverId } : { kind: 'new' }) });
      if (!response.ok) throw new Error(`Join failed (${response.status}): ${await response.text()}`);
      const joined = decodeJoinResponse(await response.json(), gameId);
      if (!joined.player_session) throw new Error('Join did not return a player session');
      setLobby(joined.lobby); setPlayerId(joined.player.id); setError(null);
      return joined.player_session;
    } catch (cause) { setError(String(cause)); return undefined; }
  }, [base, gameId]);

  const leave = useCallback(async (): Promise<boolean> => {
    if (!playerSession) return false;
    try {
      const response = await fetch(`${base}/leave`, { method: 'POST', headers });
      if (!response.ok) throw new Error(`Leave lobby failed (${response.status}): ${await response.text()}`);
      // Validate the successful server response before removing the only local credential.
      decodeLobby(await response.json(), gameId);
      setError(null);
      return true;
    } catch (cause) { setError(String(cause)); return false; }
  }, [base, gameId, playerSession]);

  return { lobby, playerId, error, loading, invalidCredential, setReady: (ready) => mutate('ready', { ready }),
    start: () => mutate('start'), reorder: (slot_ids) => mutate('reorder', { slot_ids }), join, leave };
}
