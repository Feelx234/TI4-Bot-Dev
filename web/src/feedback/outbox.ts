/**
 * The feedback outbox: a report is written to this device first, then sent. If the send fails
 * (no network, a tunnel, a timeout, 5xx, 429) it stays and goes out later, so a report written on
 * a train is not lost.
 *
 * - Durable: IndexedDB, falling back to localStorage, falling back to memory (see `storage.ts`).
 *   Every storage access is guarded; a failure never reaches the game.
 * - Big payloads: a body over 32 KB is stored gzip-compressed (CompressionStream) as base64.
 * - Idempotent: the report id is made when the report is written and sent as `report_id` with
 *   every attempt; the server stores it once (`duplicate: true` counts as sent).
 * - In order: oldest first, one at a time; a retryable failure stops the pass (no leapfrogging),
 *   with exponential backoff, jitter and `Retry-After`.
 * - Triggers: a new report, the `online` event, the tab becoming visible, page load (`start`), a
 *   successful round trip (the next items are then due at once), the backoff timer, "Send now".
 * - Several tabs: one flushes at a time (Web Locks, not waited for); a BroadcastChannel tells the
 *   others to re-read the storage, which is the source of truth.
 * - Each send carries `client_queued_at` and `context.delivery` {queued_at, sent_at, attempt,
 *   client_build}, so the server file shows when it was written versus received.
 */
import { postReport, type PostResult, withFlushLock } from "./transport.ts";
import { type OutboxStorage, openOutboxStorage } from "./outboxStorage.ts";
import { decodeBody, encodeBody, type EncodedBody } from "./outboxCodec.ts";
import { MAX_BODY_BYTES } from "./transport.ts";
import { WEB_COMMIT } from "../versionInfo.ts";

export type OutboxState = "queued" | "sending" | "sent" | "failed";
export type OutboxCategory = "bug" | "confusing" | "idea" | "other";

export interface OutboxItem {
  /** The report id (`report_id` on the wire). */
  id: string;
  /** Order of writing. */
  seq: number;
  category: OutboxCategory;
  /** Short preview for the history (the full text is in the body). */
  preview: string;
  state: OutboxState;
  attempts: number;
  queuedAt: number;
  sentAt?: number;
  nextAttemptAt: number;
  lastError?: string;
  lastStatus?: number | null;
  serverId?: string;
  clientBuild: string;
  /** Size of the request body before compression. */
  bytes: number;
  /** Absent once sent. */
  body?: EncodedBody;
}

export interface OutboxReport {
  category: OutboxCategory;
  text: string;
  context: Record<string, unknown>;
  reportId?: string;
}

export type SubmitOutcome =
  | { status: "sent"; id: string; serverId: string }
  | { status: "queued"; id: string; reason: string }
  | { status: "rejected"; message: string };

export interface OutboxOptions {
  storage: OutboxStorage;
  post?: (body: string) => Promise<PostResult>;
  now?: () => number;
  random?: () => number;
  newId?: () => string;
  baseDelayMs?: number;
  maxDelayMs?: number;
  maxAttempts?: number;
  keepSentMs?: number;
  maxItems?: number;
  maxHistory?: number;
  /** Total encoded bytes that may wait. */
  maxStoredBytes?: number;
  clientBuild?: string;
  isOnline?: () => boolean;
  locks?: Pick<LockManager, "request"> | null;
  channel?: Pick<BroadcastChannel, "postMessage" | "close"> & { onmessage: ((e: MessageEvent) => void) | null } | null;
  onlineTarget?: Pick<EventTarget, "addEventListener" | "removeEventListener"> | null;
  visibilityTarget?: Pick<EventTarget, "addEventListener" | "removeEventListener"> | null;
  isVisible?: () => boolean;
}

export interface Outbox {
  start(): Promise<void>;
  submit(report: OutboxReport): Promise<SubmitOutcome>;
  list(): OutboxItem[];
  subscribe(listener: () => void): () => void;
  /** Tries everything due now, ignoring the backoff ("Send now"). */
  sendNow(): Promise<void>;
  flush(): Promise<void>;
  retry(id: string): Promise<void>;
  discard(id: string): Promise<void>;
  dispose(): void;
}

export function newReportId(): string {
  const bytes = new Uint8Array(12);
  crypto.getRandomValues(bytes);
  return `r_${Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("")}`;
}

export function backoff(attempts: number, base: number, max: number, random: number): number {
  const exp = Math.min(max, base * 2 ** Math.max(0, attempts - 1));
  return Math.round(exp * (0.75 + 0.5 * random));
}

const LOCK_NAME = "ti4-feedback-outbox-flush";
const CHANNEL_NAME = "ti4-feedback-outbox";

export function createOutbox(opts: OutboxOptions): Outbox {
  const now = opts.now ?? Date.now;
  const random = opts.random ?? Math.random;
  const newId = opts.newId ?? newReportId;
  const base = opts.baseDelayMs ?? 15_000;
  const max = opts.maxDelayMs ?? 30 * 60_000;
  const maxAttempts = opts.maxAttempts ?? 20;
  const keepSent = opts.keepSentMs ?? 7 * 86_400_000;
  const maxItems = opts.maxItems ?? 30;
  const maxHistory = opts.maxHistory ?? 10;
  const maxStored = opts.maxStoredBytes ?? 12 * 1024 * 1024;
  const build = opts.clientBuild ?? WEB_COMMIT;
  const isOnline = opts.isOnline ?? (() => typeof navigator === "undefined" || navigator.onLine !== false);
  const isVisible = opts.isVisible ?? (() => typeof document === "undefined" || document.visibilityState !== "hidden");
  const post = opts.post ?? ((body: string) => postReport(body));
  const locks = opts.locks === undefined ? (typeof navigator === "undefined" ? undefined : navigator.locks) : (opts.locks ?? undefined);

  let items: OutboxItem[] = [];
  let snapshot: OutboxItem[] = [];
  let seqCounter = 0;
  const listeners = new Set<() => void>();
  let timer: ReturnType<typeof setTimeout> | null = null;
  let flushing: Promise<void> | null = null;
  let disposed = false;
  const unsaved = new Set<string>();

  const publish = () => {
    snapshot = [...items].sort((a, b) => a.seq - b.seq);
    for (const l of [...listeners]) l();
  };
  const channel =
    opts.channel === undefined
      ? typeof BroadcastChannel === "undefined"
        ? null
        : (() => {
            try {
              return new BroadcastChannel(CHANNEL_NAME);
            } catch {
              return null;
            }
          })()
      : opts.channel;
  const tell = () => {
    try {
      channel?.postMessage("changed");
    } catch {
      // a closed channel: other tabs find out at their next trigger
    }
  };
  const write = async (item: OutboxItem): Promise<boolean> => {
    items = items.some((i) => i.id === item.id) ? items.map((i) => (i.id === item.id ? item : i)) : [...items, item];
    publish();
    try {
      await opts.storage.put(item);
      unsaved.delete(item.id);
      tell();
      return true;
    } catch {
      unsaved.add(item.id);
      return false;
    }
  };
  const remove = async (id: string) => {
    items = items.filter((i) => i.id !== id);
    unsaved.delete(id);
    publish();
    try {
      await opts.storage.delete(id);
      tell();
    } catch {
      // stays in storage; dropped by the next cleanup
    }
  };
  /** The storage is the truth shared by all tabs. */
  const reload = async () => {
    let stored: OutboxItem[];
    try {
      stored = (await opts.storage.getAll()).filter(isItem);
    } catch {
      return;
    }
    // Items whose write failed exist only in memory and are kept; the rest comes from storage
    // (an item another tab removed is gone here too).
    const memoryOnly = items.filter((i) => unsaved.has(i.id) && !stored.some((s) => s.id === i.id));
    items = [...stored, ...memoryOnly];
    for (const i of items) seqCounter = Math.max(seqCounter, i.seq);
    publish();
  };
  if (channel) {
    channel.onmessage = () => {
      if (!disposed) void reload().then(schedule);
    };
  }

  function schedule() {
    if (timer) clearTimeout(timer);
    timer = null;
    if (disposed) return;
    const due = items.filter((i) => i.state === "queued").map((i) => i.nextAttemptAt);
    if (due.length === 0) return;
    timer = setTimeout(() => void flush(), Math.min(Math.max(0, Math.min(...due) - now()), 2 ** 30));
  }

  const trimHistory = async () => {
    const t = now();
    const sent = items.filter((i) => i.state === "sent").sort((a, b) => (b.sentAt ?? 0) - (a.sentAt ?? 0));
    for (const [index, item] of sent.entries()) {
      if (index >= maxHistory || t - (item.sentAt ?? item.queuedAt) > keepSent) await remove(item.id);
    }
  };

  /** One send attempt; returns the result. */
  const attempt = async (item: OutboxItem): Promise<PostResult> => {
    const attempts = item.attempts + 1;
    const sending = { ...item, state: "sending" as const, attempts };
    await write(sending);
    let result: PostResult;
    try {
      const body = JSON.parse(await decodeBody(item.body!)) as Record<string, unknown>;
      const context = { ...((body.context as Record<string, unknown> | undefined) ?? {}) };
      context.delivery = {
        queued_at: new Date(item.queuedAt).toISOString(),
        sent_at: new Date(now()).toISOString(),
        attempt: attempts,
        client_build: item.clientBuild,
        via: "outbox",
      };
      result = await post(JSON.stringify({ ...body, context }));
    } catch {
      result = { ok: false, retryable: true, status: null, message: "Could not send the report." };
    }
    const current = items.find((i) => i.id === item.id) ?? sending;
    if (result.ok) {
      const { body: _gone, ...rest } = current;
      await write({ ...rest, state: "sent", sentAt: now(), serverId: result.serverId, lastError: undefined, lastStatus: 200 });
    } else if (!result.retryable || attempts >= maxAttempts) {
      await write({ ...current, state: "failed", lastError: result.message, lastStatus: result.status });
    } else {
      const wait = Math.max(backoff(attempts, base, max, random()), result.retryAfterMs ?? 0);
      await write({ ...current, state: "queued", lastError: result.message, lastStatus: result.status, nextAttemptAt: now() + wait });
    }
    return result;
  };

  const pass = async (force: boolean) => {
    await reload();
    for (const item of [...items].sort((a, b) => a.seq - b.seq)) {
      if (disposed) return;
      const fresh = items.find((i) => i.id === item.id);
      if (!fresh || fresh.state !== "queued" || !fresh.body) continue;
      if (!force && fresh.nextAttemptAt > now()) return; // keep the order: do not skip ahead
      const result = await attempt(fresh);
      if (result.ok) {
        // The server is reachable: what waited for its backoff goes now.
        items = items.map((i) => (i.state === "queued" ? { ...i, nextAttemptAt: 0 } : i));
      } else if (result.retryable) {
        return; // oldest first: a flaky connection is not hammered, order stays
      }
    }
  };

  function flush(force = false): Promise<void> {
    if (flushing) return flushing;
    flushing = (async () => {
      await Promise.resolve();
      try {
        if (!isOnline() && !force) return;
        await withFlushLock(LOCK_NAME, () => pass(force), locks);
        await trimHistory();
      } finally {
        flushing = null;
        schedule();
      }
    })();
    return flushing;
  }

  const onOnline = () => {
    items = items.map((i) => (i.state === "queued" ? { ...i, nextAttemptAt: 0 } : i));
    void flush();
  };
  const onVisible = () => {
    if (isVisible()) void flush();
  };
  const onlineTarget = opts.onlineTarget === undefined ? (typeof window === "undefined" ? null : window) : opts.onlineTarget;
  const visibilityTarget =
    opts.visibilityTarget === undefined ? (typeof document === "undefined" ? null : document) : opts.visibilityTarget;
  onlineTarget?.addEventListener("online", onOnline);
  visibilityTarget?.addEventListener("visibilitychange", onVisible);

  return {
    async start() {
      await reload();
      // A crash while sending: the report may or may not have arrived; the report id makes a re-send safe.
      for (const i of items.filter((x) => x.state === "sending")) await write({ ...i, state: "queued", nextAttemptAt: 0 });
      await trimHistory();
      schedule();
      void flush();
    },

    async submit(report) {
      const text = report.text.trim();
      if (!text) return { status: "rejected", message: "The report is empty." };
      const id = report.reportId ?? newId();
      const existing = items.find((i) => i.id === id);
      if (existing) return outcome(existing);
      const queuedAt = now();
      const body = JSON.stringify({
        category: report.category,
        text,
        context: report.context,
        report_id: id,
        client_queued_at: new Date(queuedAt).toISOString(),
      });
      const bytes = new TextEncoder().encode(body).length;
      if (bytes > MAX_BODY_BYTES) return { status: "rejected", message: "The report is too large (over 3 MB)." };
      const encoded = await encodeBody(body);
      const waiting = items.filter((i) => i.state !== "sent");
      const stored = waiting.reduce((sum, i) => sum + (i.body ? i.body.data.length : 0), 0);
      if (waiting.length >= maxItems || stored + encoded.data.length > maxStored) {
        return { status: "rejected", message: "Too many reports are waiting to be sent from this device." };
      }
      await reload(); // another tab may have written meanwhile: keep a shared order
      const item: OutboxItem = {
        id,
        seq: ++seqCounter,
        category: report.category,
        preview: text.slice(0, 120),
        state: "queued",
        attempts: 0,
        queuedAt,
        nextAttemptAt: 0,
        clientBuild: build,
        bytes,
        body: encoded,
      };
      const saved = await write(item);
      if (!isOnline()) {
        return saved
          ? { status: "queued", id, reason: "offline" }
          : { status: "rejected", message: "Could not save the report on this device and there is no connection." };
      }
      await flush();
      const after = items.find((i) => i.id === id) ?? item;
      // Never saved and never sent: the user keeps the text and sees why.
      if (!saved && after.state !== "sent") {
        await remove(id);
        return { status: "rejected", message: after.lastError ?? "Could not save the report on this device." };
      }
      if (after.state === "failed") {
        await remove(id);
        return { status: "rejected", message: after.lastError ?? "The server refused the report." };
      }
      return outcome(after);
    },

    list: () => snapshot,
    subscribe(listener) {
      listeners.add(listener);
      return () => void listeners.delete(listener);
    },
    sendNow: () => flush(true),
    flush: () => flush(),
    async retry(id) {
      const item = items.find((i) => i.id === id);
      if (!item || item.state === "sent" || item.state === "sending") return;
      await write({ ...item, state: "queued", attempts: 0, nextAttemptAt: 0 });
      await flush(true);
    },
    discard: (id) => remove(id),
    dispose() {
      disposed = true;
      if (timer) clearTimeout(timer);
      onlineTarget?.removeEventListener("online", onOnline);
      visibilityTarget?.removeEventListener("visibilitychange", onVisible);
      if (channel) channel.onmessage = null;
      try {
        channel?.close();
      } catch {
        // already closed
      }
    },
  };
}

function outcome(item: OutboxItem): SubmitOutcome {
  if (item.state === "sent") return { status: "sent", id: item.id, serverId: item.serverId ?? "" };
  if (item.state === "failed") return { status: "rejected", message: item.lastError ?? "The server refused the report." };
  return { status: "queued", id: item.id, reason: item.lastError ?? "waiting" };
}

function isItem(v: unknown): v is OutboxItem {
  if (typeof v !== "object" || v === null) return false;
  const o = v as Record<string, unknown>;
  return (
    typeof o.id === "string" &&
    typeof o.seq === "number" &&
    typeof o.queuedAt === "number" &&
    typeof o.attempts === "number" &&
    typeof o.nextAttemptAt === "number" &&
    ["queued", "sending", "sent", "failed"].includes(o.state as string) &&
    (o.state === "sent" || (typeof o.body === "object" && o.body !== null))
  );
}

// The page-wide instance --------------------------------------------------------------------

let shared: Promise<Outbox> | null = null;
let sharedNow: Outbox | null = null;

/** The page's outbox; opens the storage and flushes what an earlier page left. */
export function getFeedbackOutbox(): Promise<Outbox> {
  shared ??= (async () => {
    const outbox = createOutbox({ storage: await openOutboxStorage() });
    sharedNow = outbox;
    await outbox.start();
    return outbox;
  })();
  return shared;
}

/** The outbox if it is open already (for hooks). */
export const peekFeedbackOutbox = (): Outbox | null => sharedNow;

export function resetFeedbackOutboxForTests(): void {
  sharedNow?.dispose();
  shared = null;
  sharedNow = null;
}
