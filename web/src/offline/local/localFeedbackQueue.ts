import type {
  OfflineFeedbackQueue,
  QueuedFeedbackItem,
  QueuedFeedbackReport,
} from "../types.ts";

/**
 * Placeholder feedback queue: remembers the reports for this page and sends each one in the
 * background (`POST /api/feedback`, with the replay data in `context.local`). It never blocks
 * the game; a failed send stays `failed` until `retry`. The persistent queue with backoff is T10.
 */
export function createLocalFeedbackQueue(
  send: (report: QueuedFeedbackReport) => Promise<{ id?: string }> = postFeedback,
): OfflineFeedbackQueue {
  let items: QueuedFeedbackItem[] = [];
  const listeners = new Set<() => void>();
  let counter = 0;
  const update = (localId: string, patch: Partial<QueuedFeedbackItem>) => {
    items = items.map((item) => (item.localId === localId ? { ...item, ...patch } : item));
    listeners.forEach((listener) => listener());
  };
  const attempt = (localId: string) => {
    const item = items.find((entry) => entry.localId === localId);
    if (!item) return;
    update(localId, { state: "sending", attempts: item.attempts + 1 });
    send(item.report).then(
      (reply) => update(localId, { state: "sent", serverId: reply.id }),
      () => update(localId, { state: "failed" }),
    );
  };
  return {
    async enqueue(report) {
      const item: QueuedFeedbackItem = {
        localId: `fb_local_${Date.now().toString(36)}_${counter++}`,
        report,
        state: "queued",
        attempts: 0,
        queuedAt: Date.now(),
      };
      items = [...items, item];
      listeners.forEach((listener) => listener());
      queueMicrotask(() => attempt(item.localId));
      return item;
    },
    list: () => items,
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    retry: attempt,
  };
}

async function postFeedback(report: QueuedFeedbackReport): Promise<{ id?: string }> {
  const response = await fetch("/api/feedback", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      category: report.category,
      text: report.text,
      context: { ...report.context, local: report.local },
    }),
  });
  if (!response.ok) throw new Error(`feedback ${response.status}`);
  return (await response.json().catch(() => ({}))) as { id?: string };
}
