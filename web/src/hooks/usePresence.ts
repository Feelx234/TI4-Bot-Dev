import { useEffect } from "react";

const devHeartbeatMs = Number(import.meta.env.VITE_TI4_DEV_PRESENCE_HEARTBEAT_MS);
const heartbeatMs = import.meta.env.DEV && devHeartbeatMs > 0 ? devHeartbeatMs : 10_000;

/** Reports ephemeral presence; it does not renew the player session. */
export function usePresence(
  gameId: string,
  credential: string | undefined,
  onInvalid: () => void,
): void {
  useEffect(() => {
    if (!credential) return;
    let stopped = false;
    // A slow server must not stack one more heartbeat every tick (the browser runs out of sockets).
    let inFlight = false;
    const renew = async () => {
      if (stopped || inFlight) return;
      inFlight = true;
      const response = await fetch(`/api/games/${encodeURIComponent(gameId)}/lobby/heartbeat`, {
        method: "POST",
        headers: { "x-ti4-player-session": credential },
      }).catch(() => undefined);
      inFlight = false;
      if (!stopped && response?.status === 403) onInvalid();
    };
    const visible = () => void renew();
    void renew();
    document.addEventListener("visibilitychange", visible);
    const timer = window.setInterval(() => void renew(), heartbeatMs);
    return () => {
      stopped = true;
      document.removeEventListener("visibilitychange", visible);
      window.clearInterval(timer);
    };
  }, [gameId, credential, onInvalid]);
}
