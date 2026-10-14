import { useSyncExternalStore } from "react";
import type { OfflineFeedbackQueue, QueuedFeedbackItem } from "./types.ts";

/** The queue's items, re-rendered on change. Items are replaced, not mutated, by the queue. */
export function useFeedbackQueue(queue: OfflineFeedbackQueue | null): QueuedFeedbackItem[] {
  return useSyncExternalStore(
    (cb) => (queue ? queue.subscribe(cb) : () => undefined),
    () => (queue ? queue.list() : EMPTY),
  );
}
const EMPTY: QueuedFeedbackItem[] = [];

/** Short text for the small status chip; null when there is nothing to say. */
export function feedbackChipText(items: readonly QueuedFeedbackItem[]): string | null {
  const waiting = items.filter((i) => i.state === "queued" || i.state === "sending").length;
  const failed = items.filter((i) => i.state === "failed").length;
  if (failed) return `${failed} feedback report${failed === 1 ? "" : "s"} could not be sent yet`;
  if (waiting) return `${waiting} feedback report${waiting === 1 ? "" : "s"} will be sent`;
  return null;
}
