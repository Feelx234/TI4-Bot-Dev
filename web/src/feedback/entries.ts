import { useMemo, useSyncExternalStore } from "react";
import { getOfflineLauncher, subscribeLauncher } from "../offline/registry.ts";
import type { OfflineFeedbackQueue, QueuedFeedbackItem } from "../offline/types.ts";
import { getFeedbackOutbox, type Outbox, type OutboxItem, peekFeedbackOutbox } from "./outbox.ts";

/** One line of the feedback history, whichever queue holds the report. */
export interface FeedbackEntry {
  key: string;
  source: "outbox" | "offline-game";
  category: string;
  preview: string;
  state: "queued" | "sending" | "sent" | "failed";
  attempts: number;
  queuedAt: number;
  sentAt?: number;
  lastError?: string;
  serverId?: string;
}

const fromOutbox = (i: OutboxItem): FeedbackEntry => ({
  key: `o:${i.id}`,
  source: "outbox",
  category: i.category,
  preview: i.preview,
  state: i.state,
  attempts: i.attempts,
  queuedAt: i.queuedAt,
  sentAt: i.sentAt,
  lastError: i.lastError,
  serverId: i.serverId,
});

const fromQueue = (i: QueuedFeedbackItem): FeedbackEntry => ({
  key: `q:${i.localId}`,
  source: "offline-game",
  category: i.report.category,
  preview: i.report.text.slice(0, 120),
  state: i.state,
  attempts: i.attempts,
  queuedAt: i.queuedAt,
  serverId: i.serverId,
});

const EMPTY: never[] = [];

function useStore<T>(subscribe: (cb: () => void) => () => void, get: () => T): T {
  return useSyncExternalStore(subscribe, get, get);
}

/** The outbox (opened on first use) and the offline launcher's queue, newest first. */
export function useFeedbackEntries(): FeedbackEntry[] {
  const outbox = useStore<Outbox | null>(
    (cb) => {
      let off = () => undefined as void;
      void getFeedbackOutbox().then((o) => {
        off = o.subscribe(cb);
        cb();
      }, () => undefined);
      return () => off();
    },
    peekFeedbackOutbox,
  );
  const outboxItems = useStore<OutboxItem[]>(
    (cb) => (outbox ? outbox.subscribe(cb) : () => undefined),
    () => outbox?.list() ?? (EMPTY as OutboxItem[]),
  );
  const queue = useStore<OfflineFeedbackQueue | null>(
    (cb) => subscribeLauncher(cb),
    () => getOfflineLauncher()?.feedback ?? null,
  );
  const queueItems = useStore<QueuedFeedbackItem[]>(
    (cb) => (queue ? queue.subscribe(cb) : () => undefined),
    () => queue?.list() ?? (EMPTY as QueuedFeedbackItem[]),
  );
  return useMemo(
    () => [...outboxItems.map(fromOutbox), ...queueItems.map(fromQueue)].sort((a, b) => b.queuedAt - a.queuedAt),
    [outboxItems, queueItems],
  );
}

/** "Send now": every queue tries at once, ignoring the backoff. */
export async function sendAllNow(): Promise<void> {
  const queue = getOfflineLauncher()?.feedback;
  if (queue) {
    for (const item of queue.list()) if (item.state === "queued" || item.state === "failed") queue.retry(item.localId);
  }
  try {
    await (await getFeedbackOutbox()).sendNow();
    for (const item of (await getFeedbackOutbox()).list()) {
      if (item.state === "failed") await (await getFeedbackOutbox()).retry(item.id);
    }
  } catch {
    // the outbox could not open: nothing to send
  }
}

export const pendingCount = (entries: readonly FeedbackEntry[]): number =>
  entries.filter((e) => e.state === "queued" || e.state === "sending").length;
export const failedCount = (entries: readonly FeedbackEntry[]): number => entries.filter((e) => e.state === "failed").length;
