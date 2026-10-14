import React, { useEffect, useRef, useState } from "react";
import { OfflineBanner } from "./OfflineBanner.tsx";
import { ResumeProgress } from "./ResumeProgress.tsx";
import { CapabilityProvider } from "./capabilities.ts";
import type { OfflineGameHandle, OfflineLauncher, OfflineProgress } from "./types.ts";
import "./offline.css";

/**
 * /offline/games/<id>: opens (resumes) the game with the progress bar, then renders the game
 * through `renderGame`. T5 passes the real game screen there (wrapped in the transport provider
 * T1 defines); until then a placeholder shows the banner and the queued-feedback state.
 */
export const OfflineGameRoute: React.FC<{
  launcher: OfflineLauncher;
  gameId: string;
  /** A handle just created or resumed by OfflineHome, so the game is not replayed twice. */
  handle?: OfflineGameHandle | null;
  renderGame?: (handle: OfflineGameHandle, banner: React.ReactNode) => React.ReactNode;
  onLeave: () => void;
}> = ({ launcher, gameId, handle: given, renderGame, onLeave }) => {
  const [handle, setHandle] = useState<OfflineGameHandle | null>(given ?? null);
  const [progress, setProgress] = useState<OfflineProgress>({ phase: "loading-engine", done: 0, total: null });
  const [error, setError] = useState<string | null>(null);
  const started = useRef(false);
  useEffect(() => {
    if (handle || started.current) return;
    started.current = true;
    const controller = new AbortController();
    launcher
      .resume(gameId, setProgress, controller.signal)
      .then((h) => !controller.signal.aborted && setHandle(h))
      .catch((cause: unknown) => {
        if (!controller.signal.aborted)
          setError(cause instanceof Error ? cause.message : "The offline game could not be opened.");
      });
    return () => controller.abort();
  }, [handle, launcher, gameId]);

  const banner = <OfflineBanner capabilities={launcher.capabilities} feedback={launcher.feedback} />;
  return (
    <CapabilityProvider value={launcher.capabilities}>
      {handle ? (
        renderGame ? (
          renderGame(handle, banner)
        ) : (
          <main className="lobby-page" data-testid="offline-game-placeholder">
            <section className="panel lobby-panel offline-panel">
              {banner}
              <h1>Offline game {gameId}</h1>
              <p className="text-muted">The game screen is attached by the offline host (T5).</p>
              <button type="button" className="button button--outline" onClick={onLeave}>
                Back to My offline games
              </button>
            </section>
          </main>
        )
      ) : (
        <main className="lobby-page" data-testid="offline-resuming">
          <div className="offline-panel">
            {error ? (
              <section className="panel lobby-panel">
                <div className="session-error offline-error" role="alert" data-testid="offline-error">
                  {error}
                </div>
                <button type="button" className="button button--outline" onClick={onLeave}>
                  Back
                </button>
              </section>
            ) : (
              <ResumeProgress progress={progress} onCancel={onLeave} />
            )}
          </div>
        </main>
      )}
    </CapabilityProvider>
  );
};
