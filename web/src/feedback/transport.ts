/**
 * The one way a feedback report goes to `POST /api/feedback`, shared by the online outbox and the
 * offline-game queue: a timeout that grows with the payload (a 3 MB save on a train connection
 * must not hang forever nor be cut after 10 s), the answer sorted into "retry later" and "give
 * up", and `Retry-After` of a 429 honoured.
 */

export const FEEDBACK_URL = "/api/feedback";
/** The server's body cap (`MAX_BODY_BYTES`). */
export const MAX_BODY_BYTES = 3 * 1024 * 1024;

export type PostResult =
  | { ok: true; serverId: string; duplicate: boolean }
  | {
      ok: false;
      /** Worth another attempt later (network, timeout, 408/429/5xx, password gate). */
      retryable: boolean;
      status: number | null;
      message: string;
      retryAfterMs?: number;
    };

/** 20 s plus 15 s per MB, so a 3 MB save gets about 65 s. */
export function timeoutFor(bytes: number): number {
  return Math.round(20_000 + (bytes / (1024 * 1024)) * 15_000);
}

export function parseRetryAfter(value: string | null, now = Date.now()): number | undefined {
  if (!value) return undefined;
  const seconds = Number(value);
  if (Number.isFinite(seconds)) return Math.max(0, Math.min(seconds, 3600)) * 1000;
  const at = Date.parse(value);
  return Number.isNaN(at) ? undefined : Math.max(0, Math.min(at - now, 3_600_000));
}

export interface PostOptions {
  fetchImpl?: typeof fetch;
  url?: string;
  timeoutMs?: number;
}

/** `body` is the JSON text (so its size is known). */
export async function postReport(body: string, options: PostOptions = {}): Promise<PostResult> {
  const doFetch = options.fetchImpl ?? ((...a: Parameters<typeof fetch>) => fetch(...a));
  const bytes = new TextEncoder().encode(body).length;
  if (bytes > MAX_BODY_BYTES) {
    return { ok: false, retryable: false, status: 413, message: "The report is too large (over 3 MB)." };
  }
  const controller = typeof AbortController === "undefined" ? null : new AbortController();
  const timer = setTimeout(() => controller?.abort(), options.timeoutMs ?? timeoutFor(bytes));
  try {
    const res = await doFetch(options.url ?? FEEDBACK_URL, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body,
      keepalive: bytes < 60_000,
      ...(controller ? { signal: controller.signal } : {}),
    });
    if (res.ok) {
      const data = (await res.json().catch(() => ({}))) as { id?: string; duplicate?: boolean };
      return { ok: true, serverId: data.id ?? "", duplicate: data.duplicate === true };
    }
    const status = res.status;
    if (status === 429) {
      return {
        ok: false,
        retryable: true,
        status,
        message: "Too many reports in a short time; waiting.",
        retryAfterMs: parseRetryAfter(res.headers?.get?.("retry-after") ?? null),
      };
    }
    if (status === 401) return { ok: false, retryable: true, status, message: "Enter the playtest password first." };
    if (status === 413) return { ok: false, retryable: false, status, message: "The report is too large for the server." };
    if (status === 408 || status >= 500) {
      return { ok: false, retryable: true, status, message: `The server could not save the report (HTTP ${status}).` };
    }
    return { ok: false, retryable: false, status, message: `The server refused the report (HTTP ${status}).` };
  } catch (error) {
    const timedOut = error instanceof Error && error.name === "AbortError";
    return {
      ok: false,
      retryable: true,
      status: null,
      message: timedOut ? "The server did not answer in time." : "Could not reach the server.",
    };
  } finally {
    clearTimeout(timer);
  }
}

/**
 * Runs `work` in at most one tab: a Web Lock (when the browser has them) that is not waited for.
 * Without Web Locks every tab runs it (the server stores a report id once anyway).
 */
export async function withFlushLock<T>(
  name: string,
  work: () => Promise<T>,
  locks: Pick<LockManager, "request"> | undefined = typeof navigator === "undefined" ? undefined : navigator.locks,
): Promise<T | undefined> {
  if (!locks?.request) return work();
  let result: T | undefined;
  await locks.request(name, { ifAvailable: true }, async (lock) => {
    if (lock) result = await work();
  });
  return result;
}

/** Text for the chip: "just now", "5 min ago", "2 h ago". */
export function ago(at: number, now = Date.now()): string {
  const s = Math.max(0, Math.round((now - at) / 1000));
  if (s < 45) return "just now";
  if (s < 3600) return `${Math.round(s / 60)} min ago`;
  if (s < 86_400) return `${Math.round(s / 3600)} h ago`;
  return `${Math.round(s / 86_400)} d ago`;
}
