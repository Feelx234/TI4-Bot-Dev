import React, { useCallback, useEffect, useRef, useState } from "react";
import { OfflineSetupForm } from "./OfflineSetupForm.tsx";
import { OfflineGamesList } from "./OfflineGamesList.tsx";
import { ResumeProgress } from "./ResumeProgress.tsx";
import { OfflineBanner } from "./OfflineBanner.tsx";
import { CapabilityProvider } from "./capabilities.ts";
import type {
  OfflineGameHandle,
  OfflineGameSummary,
  OfflineLauncher,
  OfflineProgress,
  OfflineSetup,
} from "./types.ts";
import "./offline.css";

type Busy = { kind: "create" | "resume"; progress: OfflineProgress; estimateMs: number | null };

/**
 * /offline: the setup form and "My offline games". Starting or resuming shows the progress bar;
 * when the launcher returns a handle, `onEnter` navigates to the game (T5 renders it).
 */
export const OfflineHome: React.FC<{
  launcher: OfflineLauncher;
  onEnter: (handle: OfflineGameHandle) => void;
  onBack: () => void;
}> = ({ launcher, onEnter, onBack }) => {
  const [games, setGames] = useState<OfflineGameSummary[] | null>(null);
  const [busy, setBusy] = useState<Busy | null>(null);
  const [error, setError] = useState<string | null>(null);
  const abort = useRef<AbortController | null>(null);
  const support = launcher.supported();

  const refresh = useCallback(async () => {
    try {
      setGames(await launcher.list());
    } catch {
      setGames([]);
      setError("Could not read the saved games on this device.");
    }
  }, [launcher]);
  useEffect(() => {
    void refresh();
  }, [refresh]);

  const run = async (
    kind: Busy["kind"],
    estimateMs: number | null,
    start: (cb: (p: OfflineProgress) => void, signal: AbortSignal) => Promise<OfflineGameHandle>,
  ) => {
    const controller = new AbortController();
    abort.current = controller;
    setError(null);
    setBusy({ kind, estimateMs, progress: { phase: "loading-engine", done: 0, total: null } });
    try {
      const handle = await start((progress) => setBusy({ kind, estimateMs, progress }), controller.signal);
      if (!controller.signal.aborted) onEnter(handle);
    } catch (cause) {
      if (!controller.signal.aborted)
        setError(cause instanceof Error ? cause.message : "The offline game could not be opened.");
    } finally {
      abort.current = null;
      setBusy(null);
      void refresh();
    }
  };

  const create = (setup: OfflineSetup) =>
    void run("create", null, (cb, signal) => launcher.create(setup, cb, signal));
  const resume = (gameId: string) =>
    void run("resume", games?.find((g) => g.gameId === gameId)?.resumeEstimateMs ?? null, (cb, signal) =>
      launcher.resume(gameId, cb, signal),
    );
  const exportGame = async (gameId: string) => {
    try {
      const { filename, blob } = await launcher.exportGame(gameId);
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = filename;
      link.click();
      setTimeout(() => URL.revokeObjectURL(url), 1000);
    } catch {
      setError("Could not export that game.");
    }
  };

  return (
    <CapabilityProvider value={launcher.capabilities}>
      <main data-testid="offline-home" className="lobby-page">
        <section className="panel lobby-panel offline-panel">
          <OfflineBanner capabilities={launcher.capabilities} feedback={launcher.feedback} />
          <h1>Play offline</h1>
          <p className="text-muted">
            One human against rule bots, on this device. No account or connection needed.
          </p>
          {!support.ok && (
            <div className="session-error offline-error" role="alert" data-testid="offline-unsupported">
              {support.reason}
            </div>
          )}
          {busy ? (
            <ResumeProgress
              progress={busy.progress}
              estimateMs={busy.estimateMs}
              onCancel={() => abort.current?.abort()}
            />
          ) : (
            <>
              {support.ok && <OfflineSetupForm factions={launcher.factions} onStart={create} />}
              <h2 className="offline-heading">My offline games</h2>
              {games === null ? (
                <p className="text-muted">Loading…</p>
              ) : (
                <OfflineGamesList
                  games={games}
                  disabled={!support.ok}
                  onResume={resume}
                  onExport={exportGame}
                  onDelete={async (id) => {
                    await launcher.remove(id);
                    await refresh();
                  }}
                />
              )}
            </>
          )}
          {error && (
            <div className="session-error offline-error" role="alert" data-testid="offline-error">
              {error}
            </div>
          )}
          <button type="button" className="button button--outline" data-testid="offline-back" onClick={onBack}>
            Back to the start screen
          </button>
        </section>
      </main>
    </CapabilityProvider>
  );
};
