import { useEffect } from 'react';

/** Reports ephemeral presence; it does not renew the player session. */
export function usePresence(gameId: string, credential: string | undefined, onInvalid: () => void): void {
  useEffect(() => {
    if (!credential) return;
    let stopped = false;
    const renew = async () => {
      if (stopped) return;
      const response = await fetch(`/api/games/${encodeURIComponent(gameId)}/lobby/heartbeat`, {
        method: 'POST', headers: { 'x-ti4-player-session': credential },
      }).catch(() => undefined);
      if (!stopped && response?.status === 403) onInvalid();
    };
    const visible = () => void renew();
    void renew();
    document.addEventListener('visibilitychange', visible);
    const timer = window.setInterval(() => void renew(), 10_000);
    return () => { stopped = true; document.removeEventListener('visibilitychange', visible); window.clearInterval(timer); };
  }, [gameId, credential, onInvalid]);
}
