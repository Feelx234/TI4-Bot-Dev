import { useEffect, useState } from 'react';
import { decodeClaimSeatResponse, decodeLobby } from '../protocol/decode.ts';
import { LobbyDto } from '../protocol/types.ts';

export interface UseLobbySessionOptions {
  gameId: string;
  seatToken?: string;
}

export interface LobbySessionState {
  lobby: LobbyDto | null;
  error: string | null;
  loading: boolean;
  invalidCredential: boolean;
  setReady: (ready: boolean) => Promise<void>;
  start: () => Promise<void>;
  claim: (seat: string) => Promise<string | undefined>;
}

export function useLobbySession({ gameId, seatToken }: UseLobbySessionOptions): LobbySessionState {
  const [lobby, setLobby] = useState<LobbyDto | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [invalidCredential, setInvalidCredential] = useState(false);

  useEffect(() => {
    const controller = new AbortController();
    const load = async () => {
      try {
        const response = await fetch(`/api/games/${encodeURIComponent(gameId)}/lobby`, {
          headers: seatToken ? { 'x-ti4-seat-token': seatToken } : {}, signal: controller.signal,
        });
        if (!response.ok) {
          if (response.status === 403 && seatToken) setInvalidCredential(true);
          throw new Error(`Lobby request failed (${response.status})`);
        }
        const next = decodeLobby(await response.json(), gameId);
        if (!controller.signal.aborted) {
          setLobby(next);
          setError(null);
          setLoading(false);
        }
      } catch (cause) {
        if (!controller.signal.aborted) {
          setError(`Lobby request failed: ${String(cause)}`);
          setLoading(false);
        }
      }
    };
    void load();
    if (lobby?.phase === 'running') return () => controller.abort();
    const timer = window.setInterval(() => { if (lobby?.phase !== 'running') void load(); }, 2_000);
    return () => { controller.abort(); window.clearInterval(timer); };
  }, [gameId, seatToken, lobby?.phase]);

  const mutate = async (path: 'ready' | 'start', body?: object) => {
    try {
      const response = await fetch(`/api/games/${encodeURIComponent(gameId)}/lobby/${path}`, {
        method: 'POST', headers: { 'Content-Type': 'application/json', ...(seatToken ? { 'x-ti4-seat-token': seatToken } : {}) }, body: body ? JSON.stringify(body) : undefined,
      });
      if (!response.ok) {
        if (response.status === 403 && seatToken) setInvalidCredential(true);
        throw new Error(`Lobby ${path} failed (${response.status}): ${await response.text()}`);
      }
      const next = decodeLobby(await response.json(), gameId);
      setLobby(next);
      setError(null);
    } catch (cause) {
      setError(String(cause));
    }
  };

  const claim = async (seat: string) => {
    try {
      const response = await fetch(`/api/games/${encodeURIComponent(gameId)}/lobby/claim`, {
        method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ seat }),
      });
      if (!response.ok) throw new Error(`Seat claim failed (${response.status}): ${await response.text()}`);
      const claimed = decodeClaimSeatResponse(await response.json(), gameId);
      setLobby(claimed.lobby); setError(null);
      return claimed.credential;
    } catch (cause) { setError(String(cause)); return undefined; }
  };

  return { lobby, error, loading, invalidCredential, setReady: (ready) => mutate('ready', { ready }), start: () => mutate('start'), claim };
}
