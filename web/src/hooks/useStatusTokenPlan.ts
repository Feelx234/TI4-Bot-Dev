import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { GameEvent, HistoryStatus, PendingChoiceDto } from "../protocol/types.ts";
import type { TokenStep } from "../presentation/commandTokens.ts";
import {
  openParts,
  parseStoredStatusPlan,
  resolveStatusTokens,
  serializeStoredStatusPlan,
  type StatusTokenPart,
  type StatusTokenPlan,
  type StoredStatusPlan,
} from "../presentation/statusTokenPlan.ts";
import { useSecondaryAutoPlay } from "./useSecondaryAutoPlay.ts";

export const STATUS_PLAN_KEY_PREFIX = "ti4_status_tokens_plan";
const key = (gameId: string, seat: string) => `${STATUS_PLAN_KEY_PREFIX}:${gameId}:${seat}`;

/** Reads the viewer's stored status plan for this game; never throws. */
export function loadStatusPlan(gameId: string, seat: string): StoredStatusPlan | null {
  try {
    return parseStoredStatusPlan(localStorage.getItem(key(gameId, seat)));
  } catch {
    return null;
  }
}

function saveStatusPlan(gameId: string, seat: string, stored: StoredStatusPlan | null) {
  try {
    if (stored) localStorage.setItem(key(gameId, seat), serializeStoredStatusPlan(stored));
    else localStorage.removeItem(key(gameId, seat));
  } catch {
    // No storage: the plan then lives in memory only.
  }
}

export interface UseStatusTokenPlanInput {
  gameId: string | undefined;
  viewerSeat: string | null | undefined;
  /** The round in progress; a plan is for one round's status phase. */
  round: number | undefined;
  /** The game is running (a plan is only made for a game in progress). */
  ready: boolean;
  history?: HistoryStatus;
  events: readonly GameEvent[];
  realChoice: PendingChoiceDto | null;
  busy?: boolean;
  submitChoice: (optionId: string) => Promise<void>;
  submitBatch?: (batch: { kind: "tokens"; steps: TokenStep[] }) => Promise<void>;
}

/**
 * The viewer's prepared status-phase token decisions (gain per 81.5 / 52.4, then redistribution).
 * Stored on this device only for one round; resolved against the real question and, in auto mode,
 * sent through the same click-equivalent path as a prepared secondary (`useSecondaryAutoPlay`).
 */
export function useStatusTokenPlan({
  gameId,
  viewerSeat,
  round,
  ready,
  history,
  events,
  realChoice,
  busy,
  submitChoice,
  submitBatch,
}: UseStatusTokenPlanInput) {
  const [stored, setStored] = useState<StoredStatusPlan | null>(() =>
    gameId && viewerSeat ? loadStatusPlan(gameId, viewerSeat) : null,
  );
  const identity = `${gameId ?? ""}:${viewerSeat ?? ""}`;
  const [seenIdentity, setSeenIdentity] = useState(identity);
  if (seenIdentity !== identity) {
    setSeenIdentity(identity);
    setStored(gameId && viewerSeat ? loadStatusPlan(gameId, viewerSeat) : null);
  }
  const persist = useCallback(
    (next: StoredStatusPlan | null) => {
      setStored(next);
      if (gameId && viewerSeat) saveStatusPlan(gameId, viewerSeat, next);
    },
    [gameId, viewerSeat],
  );

  // A later round has started: the plan can never apply again.
  useEffect(() => {
    if (stored && ready && round !== undefined && round > stored.round) persist(null);
  }, [stored, ready, round, persist]);

  // Same rule as the secondary plan: only a log that merely grew (a committed batch) keeps armed.
  const logSeen = useRef<{ length: number; lastId?: string }>({ length: 0 });
  const logExtended =
    logSeen.current.length === 0 ||
    (events.length >= logSeen.current.length &&
      events[logSeen.current.length - 1]?.id === logSeen.current.lastId);
  useEffect(() => {
    logSeen.current = { length: events.length, lastId: events[events.length - 1]?.id };
  });

  const cursor = history?.cursor;
  const open = useMemo(() => openParts(stored, cursor), [stored, cursor]);
  const resolved = useMemo(
    () => resolveStatusTokens(open, realChoice, viewerSeat),
    [open, realChoice, viewerSeat],
  );

  const markApplied = useCallback(
    (part: StatusTokenPart | null) => {
      if (!part || !stored) return;
      persist({ ...stored, applied: { ...stored.applied, [part]: cursor ?? 0 } });
    },
    [stored, cursor, persist],
  );

  const sendOption = useCallback(
    async (optionId: string) => {
      const part = resolved.part;
      await submitChoice(optionId);
      markApplied(part);
    },
    [resolved.part, submitChoice, markApplied],
  );
  const sendTokens = useMemo(
    () =>
      submitBatch
        ? async (steps: TokenStep[]) => {
            const part = resolved.part;
            await submitBatch({ kind: "tokens", steps });
            markApplied(part);
          }
        : undefined,
    [submitBatch, resolved.part, markApplied],
  );

  const { pending, played, cancel, holding } = useSecondaryAutoPlay({
    choice: resolved.part ? realChoice : null,
    viewerSeat,
    history,
    logExtended,
    busy,
    resolution: resolved.resolution,
    submitOption: sendOption,
    submitTokens: sendTokens,
  });

  /** Sends the prepared answer for the viewer's real decision (the Review bar's Confirm). */
  const play = useCallback(async () => {
    const target = resolved.resolution;
    if (target.kind === "option") await sendOption(target.optionId);
    else if (target.kind === "tokens") await sendTokens?.(target.steps);
  }, [resolved.resolution, sendOption, sendTokens]);

  const save = useCallback(
    (plan: StatusTokenPlan) => {
      if (!gameId || !viewerSeat || round === undefined) return;
      if (!plan.gain && !plan.redistribute) {
        persist(null);
        return;
      }
      persist({ v: 1, round, plan });
    },
    [gameId, viewerSeat, round, persist],
  );
  const clear = useCallback(() => persist(null), [persist]);

  return {
    /** The plan as stored (including parts already answered), or `null`. */
    stored,
    /** The parts still to answer. */
    plan: open,
    hasPlan: Boolean(open.gain || open.redistribute),
    part: resolved.part,
    resolution: resolved.resolution,
    prefill: resolved.prefill,
    save,
    clear,
    play,
    pending,
    played,
    cancel,
    holding,
  };
}

export type StatusTokenPlanApi = ReturnType<typeof useStatusTokenPlan>;
