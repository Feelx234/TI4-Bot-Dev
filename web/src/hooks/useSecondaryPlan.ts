import { useCallback, useEffect, useMemo, useState } from "react";
import {
  normalizePlan,
  parseStoredPlan,
  serializeStoredPlan,
  type SecondaryPlan,
  type StoredPlan,
} from "../presentation/secondaryPlan.ts";

export const PREPARED_KEY_PREFIX = "ti4_secondary_prepared";
const key = (gameId: string, seat: string) => `${PREPARED_KEY_PREFIX}:${gameId}:${seat}`;

/** Reads the viewer's stored plan for this game; never throws. */
export function loadPlan(gameId: string, seat: string): StoredPlan | null {
  try {
    return parseStoredPlan(localStorage.getItem(key(gameId, seat)));
  } catch {
    return null;
  }
}

function savePlan(gameId: string, seat: string, stored: StoredPlan | null) {
  try {
    if (stored) localStorage.setItem(key(gameId, seat), serializeStoredPlan(stored));
    else localStorage.removeItem(key(gameId, seat));
  } catch {
    // No storage: the plan then lives in memory only.
  }
}

export interface UsePreparedPlanInput {
  gameId: string | undefined;
  viewerSeat: string | null | undefined;
  /** Identity of the strategic action in progress, or `null` when there is none. */
  actionKey: string | null;
  generation: number;
  /** The log and view are loaded; only then may a missing action count as "it ended". */
  ready: boolean;
}

/**
 * The viewer's prepared secondary for the action in progress. It lives on this device only (never
 * sent anywhere) and is valid for one action and one history generation:
 *  - a different action (a new strategic action) or none in progress (finished or cancelled), or
 *  - a different generation (undo, redo, restore, load)
 * drops it. `clear()` is the player's own revoke.
 */
export function usePreparedPlan({
  gameId,
  viewerSeat,
  actionKey,
  generation,
  ready,
}: UsePreparedPlanInput) {
  const [stored, setStored] = useState<StoredPlan | null>(() =>
    gameId && viewerSeat ? loadPlan(gameId, viewerSeat) : null,
  );

  // Another game or seat: read what that one stored.
  const identity = `${gameId ?? ""}:${viewerSeat ?? ""}`;
  const [seenIdentity, setSeenIdentity] = useState(identity);
  if (seenIdentity !== identity) {
    setSeenIdentity(identity);
    setStored(gameId && viewerSeat ? loadPlan(gameId, viewerSeat) : null);
  }

  const valid =
    stored !== null &&
    actionKey !== null &&
    stored.actionKey === actionKey &&
    stored.generation === generation;

  useEffect(() => {
    if (!stored || valid || !ready) return;
    // Wrong action or generation (or the action is over): it can never apply again.
    setStored(null);
    if (gameId && viewerSeat) savePlan(gameId, viewerSeat, null);
  }, [stored, valid, ready, gameId, viewerSeat]);

  const set = useCallback(
    (plan: SecondaryPlan) => {
      if (!gameId || !viewerSeat || !actionKey) return;
      const next: StoredPlan = { v: 1, actionKey, generation, plan: normalizePlan(plan) };
      setStored(next);
      savePlan(gameId, viewerSeat, next);
    },
    [gameId, viewerSeat, actionKey, generation],
  );

  const clear = useCallback(() => {
    setStored(null);
    if (gameId && viewerSeat) savePlan(gameId, viewerSeat, null);
  }, [gameId, viewerSeat]);

  const plan = useMemo(() => (valid ? stored.plan : null), [valid, stored]);
  return { plan, set, clear };
}
