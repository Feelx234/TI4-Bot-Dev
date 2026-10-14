/**
 * Durable store-and-forward queue for feedback written during offline games. Implements
 * `OfflineFeedbackQueue` of ../types.ts (what T2's `useFeedbackQueue` reads) on top of the
 * store's `feedbackBackend`.
 *
 * - Durable: every state change is written before the next step; a crash while "sending" reverts
 *   to "queued" on the next start.
 * - Idempotent: each report has a `reportId` created at enqueue and sent with every attempt, so a
 *   re-send after a lost response can be recognised by the receiver. `enqueue` with a known
 *   `reportId` returns the existing item.
 * - Retry with exponential backoff and jitter; a non-retryable answer (4xx other than 408/429)
 *   or too many attempts ends in "failed" until `retry()`.
 * - Never blocks the game: `enqueue` only touches local storage; sending is in the background.
 */
import type { QueuedFeedbackItem, QueuedFeedbackReport } from "../types.ts";
import {
  type DurableFeedbackQueue,
  type FeedbackBackend,
  type FeedbackPayload,
  type FeedbackSendResult,
  type FeedbackSender,
  LIMITS,
  StoreError,
  type StoredFeedbackItem,
} from "./types.ts";

export interface FeedbackQueueOptions {
  backend: FeedbackBackend;
  send: FeedbackSender;
  now?: () => number;
  random?: () => number;
  newId?: () => string;
  /** First retry delay; doubles each attempt. Default 30 s. */
  baseDelayMs?: number;
  maxDelayMs?: number;
  /** After this many failed attempts the item becomes "failed" (manual retry). Default 12. */
  maxAttempts?: number;
  /** Sent items are forgotten after this long. Default 24 h. */
  keepSentMs?: number;
  /** Where `online` is listened for; default window when present. */
  onlineTarget?: Pick<EventTarget, "addEventListener" | "removeEventListener"> | null;
}

export function backoffDelay(attempts: number, base: number, max: number, random: number): number {
  const exp = Math.min(max, base * 2 ** Math.max(0, attempts - 1));
  return Math.round(exp * (0.75 + 0.5 * random));
}

export function createDurableFeedbackQueue(opts: FeedbackQueueOptions): DurableFeedbackQueue {
  const now = opts.now ?? Date.now;
  const random = opts.random ?? Math.random;
  const newId = opts.newId ?? (() => crypto.randomUUID());
  const base = opts.baseDelayMs ?? 30_000;
  const max = opts.maxDelayMs ?? 3_600_000;
  const maxAttempts = opts.maxAttempts ?? 12;
  const keepSent = opts.keepSentMs ?? 24 * 3_600_000;

  let items: StoredFeedbackItem[] = [];
  let snapshot: QueuedFeedbackItem[] = [];
  const listeners = new Set<() => void>();
  let timer: ReturnType<typeof setTimeout> | null = null;
  let flushing: Promise<void> | null = null;
  let disposed = false;

  const publish = () => {
    // Replace, never mutate: useSyncExternalStore compares by identity.
    snapshot = items.map(toPublic);
    for (const l of [...listeners]) l();
  };
  const replace = (item: StoredFeedbackItem) => {
    items = items.map((i) => (i.localId === item.localId ? item : i));
  };
  const save = async (item: StoredFeedbackItem) => {
    replace(item);
    publish();
    await opts.backend.put(item);
  };

  const schedule = () => {
    if (timer) clearTimeout(timer);
    timer = null;
    if (disposed) return;
    const due = items.filter((i) => i.state === "queued").map((i) => i.nextAttemptAt);
    if (due.length === 0) return;
    const wait = Math.max(0, Math.min(...due) - now());
    // setTimeout cannot hold more than ~24 days; the cap keeps it far below.
    timer = setTimeout(() => void flush(), Math.min(wait, 2 ** 30));
  };

  const attempt = async (item: StoredFeedbackItem) => {
    const attempts = item.attempts + 1;
    await save({ ...item, state: "sending", attempts, lastAttemptAt: now() });
    let result: FeedbackSendResult;
    try {
      result = await opts.send(toPayload(item));
    } catch (e) {
      result = { ok: false, retryable: true, message: e instanceof Error ? e.message : "send failed" };
    }
    const current = items.find((i) => i.localId === item.localId)!;
    if (result.ok) {
      await save({ ...current, state: "sent", serverId: result.serverId, lastError: undefined });
    } else if (!result.retryable || attempts >= maxAttempts) {
      await save({ ...current, state: "failed", lastError: result.message });
    } else {
      await save({
        ...current,
        state: "queued",
        lastError: result.message,
        nextAttemptAt: now() + backoffDelay(attempts, base, max, random()),
      });
    }
  };

  const flush = (): Promise<void> => {
    if (flushing) return flushing;
    flushing = (async () => {
      await Promise.resolve(); // `flushing` must be assigned before the body can finish
      try {
        // Oldest first, one at a time: a flaky network is not hammered.
        for (const item of [...items].sort((a, b) => a.queuedAt - b.queuedAt)) {
          if (disposed) return;
          const fresh = items.find((i) => i.localId === item.localId);
          if (fresh?.state === "queued" && fresh.nextAttemptAt <= now()) await attempt(fresh);
        }
      } finally {
        flushing = null;
        schedule();
      }
    })();
    return flushing;
  };

  const onOnline = () => {
    // Back online: due now, whatever the backoff said.
    items = items.map((i) => (i.state === "queued" ? { ...i, nextAttemptAt: 0 } : i));
    void flush();
  };
  const onlineTarget =
    opts.onlineTarget === undefined ? (typeof window !== "undefined" ? window : null) : opts.onlineTarget;
  onlineTarget?.addEventListener("online", onOnline);

  return {
    async start() {
      const stored = await opts.backend.getAll();
      const t = now();
      const keep: StoredFeedbackItem[] = [];
      for (const raw of stored) {
        if (!isStoredItem(raw)) continue;
        if (raw.state === "sent" && t - raw.queuedAt > keepSent) {
          await opts.backend.delete(raw.localId);
          continue;
        }
        // A crash during "sending": the report may or may not have arrived; reportId makes a re-send safe.
        keep.push(raw.state === "sending" ? { ...raw, state: "queued", nextAttemptAt: 0 } : raw);
      }
      items = keep;
      publish();
      void flush();
    },

    async enqueue(report: QueuedFeedbackReport, options?: { reportId?: string }) {
      if (options?.reportId) {
        const existing = items.find((i) => i.reportId === options.reportId);
        if (existing) return toPublic(existing);
      }
      checkReport(report);
      if (items.filter((i) => i.state !== "sent").length >= LIMITS.maxFeedbackItems) {
        throw new StoreError("too-many-games", "Too many feedback reports are waiting to be sent.");
      }
      const item: StoredFeedbackItem = {
        localId: newId(),
        reportId: options?.reportId ?? newId(),
        report,
        state: "queued",
        attempts: 0,
        queuedAt: now(),
        nextAttemptAt: 0,
      };
      items = [...items, item];
      publish();
      await opts.backend.put(item);
      void flush();
      return toPublic(item);
    },

    list: () => snapshot,
    subscribe(listener) {
      listeners.add(listener);
      return () => void listeners.delete(listener);
    },
    retry(localId) {
      const item = items.find((i) => i.localId === localId);
      if (!item || (item.state !== "failed" && item.state !== "queued")) return;
      void save({ ...item, state: "queued", attempts: 0, nextAttemptAt: 0 }).then(flush);
    },
    flush,
    dispose() {
      disposed = true;
      if (timer) clearTimeout(timer);
      onlineTarget?.removeEventListener("online", onOnline);
    },
  } as DurableFeedbackQueue;
}

function toPublic(i: StoredFeedbackItem): QueuedFeedbackItem {
  const { reportId: _r, nextAttemptAt: _n, lastError: _e, lastAttemptAt: _l, ...pub } = i;
  return pub;
}

function toPayload(i: StoredFeedbackItem): FeedbackPayload {
  return { reportId: i.reportId, category: i.report.category, text: i.report.text, context: i.report.context, local: i.report.local };
}

function checkReport(r: QueuedFeedbackReport) {
  const bad = (m: string) => new StoreError("invalid", m);
  if (!["bug", "confusing", "idea", "other"].includes(r.category)) throw bad("category");
  if (typeof r.text !== "string" || r.text.trim() === "") throw bad("The report is empty.");
  if ([...r.text].length > LIMITS.maxFeedbackTextChars) throw bad("The report text is too long.");
  const bytes = new TextEncoder().encode(JSON.stringify({ c: r.context, l: r.local })).length;
  if (bytes > LIMITS.maxFeedbackPayloadBytes) throw new StoreError("too-large", "The report is too large.");
}

function isStoredItem(v: unknown): v is StoredFeedbackItem {
  if (typeof v !== "object" || v === null) return false;
  const o = v as Record<string, unknown>;
  return (
    typeof o.localId === "string" &&
    typeof o.reportId === "string" &&
    typeof o.queuedAt === "number" &&
    typeof o.attempts === "number" &&
    typeof o.nextAttemptAt === "number" &&
    ["queued", "sending", "sent", "failed"].includes(o.state as string) &&
    typeof o.report === "object" &&
    o.report !== null
  );
}

/**
 * Default sender: `POST /api/feedback` as the online sheet does. The server rejects unknown
 * top-level fields, so `reportId` and `local` travel inside `context.offline`. (Server-side
 * de-duplication by `context.offline.report_id` is not built; until then a lost response can
 * produce a duplicate file, which triage can recognise by that id.)
 */
export function httpFeedbackSender(fetchImpl: typeof fetch = (...a) => fetch(...a), url = "/api/feedback"): FeedbackSender {
  return async (p) => {
    try {
      const res = await fetchImpl(url, {
        method: "POST",
        headers: { "content-type": "application/json", "idempotency-key": p.reportId },
        body: JSON.stringify({
          category: p.category,
          text: p.text,
          context: { ...p.context, offline: { report_id: p.reportId, local: p.local } },
        }),
      });
      if (res.ok) {
        const data = (await res.json().catch(() => ({}))) as { id?: string };
        return { ok: true, serverId: data.id ?? "" };
      }
      const retryable = res.status === 408 || res.status === 429 || res.status >= 500 || res.status === 401;
      return { ok: false, retryable, message: `HTTP ${res.status}` };
    } catch {
      return { ok: false, retryable: true, message: "offline" };
    }
  };
}
