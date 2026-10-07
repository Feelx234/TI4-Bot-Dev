import { useCallback, useEffect, useRef, useState } from "react";
import type { TurnRedoCommand, TurnRedoStatus } from "../protocol/turnRedo.ts";

export type TurnRedoBusy = "request" | "autoplay" | "restore" | "keep" | null;

export interface UseTurnRedoOptions {
  /** Seated players only: a spectator has no credential to read the status with. */
  enabled: boolean;
  fetchStatus: () => Promise<TurnRedoStatus | null>;
  command: (command: TurnRedoCommand) => Promise<void>;
  /** The live game version: a redo in flight is re-read as the game moves. */
  gameVersion: number;
  /** Bumps whenever the timeline is replaced, by anyone. */
  generation: number;
  /** After the timeline was replaced by this client's own command. */
  onTimelineChanged?: () => void;
}

export interface UseTurnRedoResult {
  status: TurnRedoStatus | null;
  busy: TurnRedoBusy;
  error: string | null;
  request: (options?: { turns?: 1 | 2; seat?: string }) => void;
  autoplay: () => void;
  restore: () => void;
  keep: () => void;
}

/** A refusal that only means the game moved on while the request was in flight: try again later. */
const isStale = (error: unknown) =>
  error instanceof Error && /Game advanced|decision is in flight|\(409\)/.test(error.message);

/**
 * Turn redo state for one client: reads the server's status, runs the commands, and fires the
 * auto-play by itself as soon as the redoing seat's new turn is complete (the server never moves
 * on its own, so something has to ask for the round to replay).
 */
export function useTurnRedo({
  enabled,
  fetchStatus,
  command,
  gameVersion,
  generation,
  onTimelineChanged,
}: UseTurnRedoOptions): UseTurnRedoResult {
  const [status, setStatus] = useState<TurnRedoStatus | null>(null);
  const [busy, setBusy] = useState<TurnRedoBusy>(null);
  const [error, setError] = useState<string | null>(null);
  const autoplayedAt = useRef<number | null>(null);
  const busyRef = useRef<TurnRedoBusy>(null);
  busyRef.current = busy;

  const refresh = useCallback(async () => {
    if (!enabled) return;
    try {
      setStatus(await fetchStatus());
    } catch {
      // A status that cannot be read (an older server, a dropped request) is not a redo that is
      // not happening, and not worth a banner of its own: keep what we know and try again on the
      // next change.
    }
  }, [enabled, fetchStatus]);

  // On mount and whenever the timeline is replaced.
  useEffect(() => {
    void refresh();
  }, [refresh, generation]);

  // While a redo is in flight, follow the game: the new turn completes with some decision.
  const inFlight = useRef(false);
  inFlight.current = status !== null;
  useEffect(() => {
    if (inFlight.current) void refresh();
  }, [refresh, gameVersion]);

  const run = useCallback(
    async (kind: Exclude<TurnRedoBusy, null>, cmd: TurnRedoCommand, quiet = false) => {
      if (busyRef.current) return;
      setBusy(kind);
      busyRef.current = kind;
      setError(null);
      try {
        await command(cmd);
        onTimelineChanged?.();
        await refresh();
      } catch (cause) {
        if (!(quiet && isStale(cause)))
          setError(cause instanceof Error ? cause.message : String(cause));
      } finally {
        busyRef.current = null;
        setBusy(null);
      }
    },
    [command, onTimelineChanged, refresh],
  );

  const autoplay = useCallback(() => void run("autoplay", { action: "autoplay" }), [run]);

  // The new turn is complete: ask the server to replay the round, once per game version.
  useEffect(() => {
    if (
      status?.stage === "new_turn" &&
      status.turn_complete &&
      status.can_control &&
      !busyRef.current &&
      autoplayedAt.current !== gameVersion
    ) {
      autoplayedAt.current = gameVersion;
      void run("autoplay", { action: "autoplay" }, true);
    }
  }, [status, gameVersion, run]);

  return {
    status,
    busy,
    error,
    request: (options) =>
      void run("request", { action: "request", turns: options?.turns ?? 1, seat: options?.seat }),
    autoplay,
    restore: () => void run("restore", { action: "restore" }),
    keep: () => void run("keep", { action: "keep" }),
  };
}
