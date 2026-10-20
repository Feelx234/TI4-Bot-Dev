import * as webStreams from "node:stream/web";
import { afterEach, describe, expect, it, vi } from "vitest";
import { backoff, createOutbox, type Outbox, type OutboxItem, type OutboxOptions } from "./outbox.ts";
import { localStorageOutbox, MemoryOutboxStorage, type OutboxStorage } from "./outboxStorage.ts";
import { decodeBody, encodeBody } from "./outboxCodec.ts";
import { parseRetryAfter, postReport, type PostResult, timeoutFor, withFlushLock } from "./transport.ts";

const OK: PostResult = { ok: true, serverId: "fb_1_aa", duplicate: false };
const DOWN: PostResult = { ok: false, retryable: true, status: null, message: "Could not reach the server." };
const REFUSED: PostResult = { ok: false, retryable: false, status: 413, message: "too large" };
const report = (text = "broken") => ({ category: "bug" as const, text, context: { a: 1 } });

// jsdom hides the gzip streams Node (and every browser that has IndexedDB) provides.
Object.assign(globalThis, { CompressionStream: webStreams.CompressionStream, DecompressionStream: webStreams.DecompressionStream });

let outboxes: Outbox[] = [];
function make(results: PostResult[] | ((n: number) => PostResult), extra: Partial<OutboxOptions> = {}) {
  const storage = extra.storage ?? new MemoryOutboxStorage();
  let t = 1_000_000;
  let n = 0;
  let ids = 0;
  const bodies: Array<Record<string, any>> = [];
  const online = { value: true };
  const outbox = createOutbox({
    storage,
    post: async (body) => {
      bodies.push(JSON.parse(body));
      const r = typeof results === "function" ? results(n) : results[Math.min(n, results.length - 1)]!;
      n += 1;
      return r;
    },
    now: () => t,
    random: () => 0.5,
    newId: () => `r_id${String(++ids).padStart(8, "0")}`,
    isOnline: () => online.value,
    locks: null,
    channel: null,
    onlineTarget: null,
    visibilityTarget: null,
    ...extra,
  });
  outboxes.push(outbox);
  return { outbox, storage, bodies, online, advance: (ms: number) => (t += ms), calls: () => n };
}
afterEach(() => {
  outboxes.forEach((o) => o.dispose());
  outboxes = [];
  vi.useRealTimers();
});

describe("outbox", () => {
  it("sends a report at once and keeps a small history without the body", async () => {
    const { outbox, bodies } = make([OK]);
    const out = await outbox.submit(report());
    expect(out).toEqual({ status: "sent", id: "r_id00000001", serverId: "fb_1_aa" });
    expect(bodies[0]).toMatchObject({ report_id: "r_id00000001", text: "broken" });
    expect(bodies[0]!.client_queued_at).toBe(new Date(1_000_000).toISOString());
    expect(bodies[0]!.context.delivery).toMatchObject({ attempt: 1, via: "outbox", queued_at: new Date(1_000_000).toISOString() });
    expect(typeof bodies[0]!.context.delivery.client_build).toBe("string");
    const [item] = outbox.list();
    expect(item).toMatchObject({ state: "sent", attempts: 1, serverId: "fb_1_aa" });
    expect(item!.body).toBeUndefined();
  });

  it("offline: saved on the device without a network call, and nothing is lost", async () => {
    const { outbox, online, calls, storage } = make([OK]);
    online.value = false;
    const out = await outbox.submit(report());
    expect(out).toMatchObject({ status: "queued", reason: "offline" });
    expect(calls()).toBe(0);
    expect((await storage.getAll())[0]).toMatchObject({ state: "queued", attempts: 0 });
  });

  it("a failed send is kept and retried with backoff; the same report id every time", async () => {
    const { outbox, bodies, advance } = make([DOWN, DOWN, OK]);
    const out = await outbox.submit(report());
    expect(out.status).toBe("queued");
    const first = outbox.list()[0]!;
    expect(first).toMatchObject({ state: "queued", attempts: 1, lastError: "Could not reach the server." });
    expect(first.nextAttemptAt).toBe(1_000_000 + backoff(1, 15_000, 30 * 60_000, 0.5));
    await outbox.flush(); // not due yet
    expect(bodies).toHaveLength(1);
    advance(20_000);
    await outbox.flush();
    expect(bodies).toHaveLength(2);
    expect(outbox.list()[0]!.nextAttemptAt - 1_020_000).toBe(backoff(2, 15_000, 30 * 60_000, 0.5));
    advance(60_000);
    await outbox.flush();
    expect(outbox.list()[0]).toMatchObject({ state: "sent", attempts: 3 });
    expect(new Set(bodies.map((b) => b.report_id)).size).toBe(1);
    expect(bodies.map((b) => b.context.delivery.attempt)).toEqual([1, 2, 3]);
    // The write time stays the first one; sent_at moves.
    expect(new Set(bodies.map((b) => b.client_queued_at)).size).toBe(1);
  });

  it("backoff grows, is capped and jittered", () => {
    expect(backoff(1, 15_000, 1_800_000, 0.5)).toBe(15_000);
    expect(backoff(4, 15_000, 1_800_000, 0.5)).toBe(120_000);
    expect(backoff(20, 15_000, 1_800_000, 0.5)).toBe(1_800_000);
    expect(backoff(1, 15_000, 1_800_000, 0)).toBe(11_250);
    expect(backoff(1, 15_000, 1_800_000, 1)).toBe(18_750);
  });

  it("a refusal that retrying cannot fix is reported and not queued (413, 400)", async () => {
    const { outbox } = make([REFUSED]);
    const out = await outbox.submit(report());
    expect(out).toEqual({ status: "rejected", message: "too large" });
    expect(outbox.list()).toHaveLength(0);
  });

  it("429 waits at least Retry-After", async () => {
    const limited: PostResult = { ok: false, retryable: true, status: 429, message: "slow", retryAfterMs: 600_000 };
    const { outbox } = make([limited]);
    await outbox.submit(report());
    expect(outbox.list()[0]!.nextAttemptAt - 1_000_000).toBe(600_000);
  });

  it("keeps the order: a stuck older report is not overtaken, and all go once the server answers", async () => {
    const { outbox, bodies, online, advance } = make((n) => (n < 1 ? DOWN : OK));
    online.value = false;
    await outbox.submit(report("one"));
    await outbox.submit(report("two"));
    await outbox.submit(report("three"));
    online.value = true;
    advance(1);
    await outbox.sendNow(); // "one" fails first (n=0), pass stops
    expect(bodies.map((b) => b.text)).toEqual(["one"]);
    await outbox.sendNow();
    expect(bodies.map((b) => b.text)).toEqual(["one", "one", "two", "three"]);
    expect(outbox.list().map((i) => i.state)).toEqual(["sent", "sent", "sent"]);
  });

  it("success makes the rest due at once, even in backoff", async () => {
    const { outbox, bodies, online } = make((n) => (n === 0 || n === 1 ? DOWN : OK));
    online.value = false;
    await outbox.submit(report("a"));
    await outbox.submit(report("b"));
    online.value = true;
    await outbox.sendNow(); // a fails
    expect(outbox.list().map((i) => i.state)).toEqual(["queued", "queued"]);
    await outbox.sendNow(); // a fails again (n=1)
    await outbox.sendNow(); // a ok, then b without waiting
    expect(bodies.map((b) => b.text)).toEqual(["a", "a", "a", "b"]);
  });

  it("the online event, visibilitychange and start() trigger a send", async () => {
    const target = new EventTarget();
    const vis = new EventTarget();
    const { outbox, bodies, online, storage } = make([OK], { onlineTarget: target, visibilityTarget: vis });
    online.value = false;
    await outbox.submit(report("train"));
    online.value = true;
    target.dispatchEvent(new Event("online"));
    await vi.waitFor(() => expect(bodies).toHaveLength(1));
    expect(outbox.list()[0]!.state).toBe("sent");

    // A page closed with items queued: the next page loads them and sends.
    online.value = false;
    await outbox.submit(report("closed tab"));
    const second = make([OK], { storage });
    await second.outbox.start();
    await vi.waitFor(() => expect(second.bodies.map((b) => b.text)).toEqual(["closed tab"]));

    // visibilitychange
    online.value = false;
    const third = make([OK], { visibilityTarget: vis, storage: new MemoryOutboxStorage() });
    third.online.value = false;
    await third.outbox.submit(report("hidden"));
    third.online.value = true;
    vis.dispatchEvent(new Event("visibilitychange"));
    await vi.waitFor(() => expect(third.bodies).toHaveLength(1));
  });

  it("a crash while sending is retried on the next start with the same id", async () => {
    const storage = new MemoryOutboxStorage();
    const a = make([DOWN], { storage });
    a.online.value = false;
    await a.outbox.submit(report());
    const [stored] = await storage.getAll();
    await storage.put({ ...stored!, state: "sending", attempts: 1 });
    const b = make([OK], { storage });
    await b.outbox.start();
    await vi.waitFor(() => expect(b.bodies).toHaveLength(1));
    expect(b.bodies[0]!.report_id).toBe(stored!.id);
  });

  it("idempotent submit: the same report id is queued once", async () => {
    const { outbox, online } = make([OK]);
    online.value = false;
    await outbox.submit({ ...report(), reportId: "r_fixed_0001" });
    await outbox.submit({ ...report(), reportId: "r_fixed_0001" });
    expect(outbox.list()).toHaveLength(1);
  });

  it("a duplicate answer from the server counts as sent", async () => {
    const { outbox } = make([{ ok: true, serverId: "fb_first", duplicate: true }]);
    expect((await outbox.submit(report())).status).toBe("sent");
  });

  it("gives up after the attempt limit (failed), and retry() starts again", async () => {
    const { outbox, advance } = make([DOWN, DOWN, OK], { maxAttempts: 2 });
    await outbox.submit(report()); // attempt 1 -> rejected? still queued
    advance(60_000);
    await outbox.flush(); // attempt 2 -> failed
    const item = outbox.list()[0]!;
    expect(item.state).toBe("failed");
    await outbox.retry(item.id);
    expect(outbox.list()[0]!.state).toBe("sent");
  });

  it("caps: count, size and the 3 MB body limit", async () => {
    const { outbox, online } = make([OK], { maxItems: 2 });
    online.value = false;
    expect((await outbox.submit(report("1"))).status).toBe("queued");
    expect((await outbox.submit(report("2"))).status).toBe("queued");
    expect(await outbox.submit(report("3"))).toMatchObject({ status: "rejected" });
    const big = await outbox.submit({ ...report(), context: { blob: "x".repeat(3 * 1024 * 1024) } });
    expect(big).toMatchObject({ status: "rejected", message: expect.stringContaining("too large") });
  });

  it("a big save is stored compressed and sent intact", async () => {
    const { outbox, online, storage, bodies } = make([OK]);
    online.value = false;
    const save = { answers: Array.from({ length: 20_000 }, (_, i) => `option_${i % 50}`) };
    await outbox.submit({ ...report(), context: { local: { save } } });
    const [stored] = await storage.getAll();
    expect(stored!.body!.enc).toBe("gzip");
    expect(stored!.body!.data.length).toBeLessThan(stored!.bytes / 3);
    online.value = true;
    await outbox.sendNow();
    expect(bodies[0]!.context.local.save.answers).toHaveLength(20_000);
  });

  it("a storage that throws never breaks the game: the send still happens", async () => {
    const broken: OutboxStorage = {
      kind: "memory",
      getAll: async () => { throw new Error("denied"); },
      put: async () => { throw new Error("quota"); },
      delete: async () => { throw new Error("denied"); },
    };
    const { outbox, bodies } = make([OK], { storage: broken });
    expect((await outbox.submit(report())).status).toBe("sent");
    expect(bodies).toHaveLength(1);
    // Offline and unsavable: the user is told, the text stays in the sheet.
    const b = make([OK], { storage: broken });
    b.online.value = false;
    expect(await b.outbox.submit(report())).toMatchObject({ status: "rejected" });
  });

  it("history keeps the last N sent reports", async () => {
    const { outbox } = make([OK], { maxHistory: 2 });
    for (const t of ["a", "b", "c"]) {
      await outbox.submit(report(t));
    }
    await outbox.flush();
    expect(outbox.list().filter((i) => i.state === "sent")).toHaveLength(2);
  });
});

describe("outbox across tabs", () => {
  function fakeBus() {
    const peers: Array<{ onmessage: ((e: MessageEvent) => void) | null }> = [];
    return () => {
      const peer = {
        onmessage: null as ((e: MessageEvent) => void) | null,
        postMessage(message: unknown) {
          for (const other of peers) if (other !== peer) other.onmessage?.({ data: message } as MessageEvent);
        },
        close() {},
      };
      peers.push(peer);
      return peer;
    };
  }
  /** navigator.locks with ifAvailable: the second holder gets null. */
  function fakeLocks() {
    const held = new Set<string>();
    return {
      request: async (name: string, _o: unknown, cb: (lock: unknown) => Promise<unknown>) => {
        if (held.has(name)) return cb(null);
        held.add(name);
        try {
          return await cb({ name });
        } finally {
          held.delete(name);
        }
      },
    } as unknown as LockManager;
  }

  it("two tabs flushing at once send a report once (lock), and the other tab sees the result", async () => {
    const storage = new MemoryOutboxStorage();
    const locks = fakeLocks();
    const bus = fakeBus();
    let release!: () => void;
    const gate = new Promise<void>((r) => (release = r));
    let posted = 0;
    const slow = async (): Promise<PostResult> => {
      posted += 1;
      await gate;
      return OK;
    };
    const a = createOutbox({ storage, post: slow, locks, channel: bus(), onlineTarget: null, visibilityTarget: null, isOnline: () => true });
    const b = createOutbox({ storage, post: slow, locks, channel: bus(), onlineTarget: null, visibilityTarget: null, isOnline: () => true });
    outboxes.push(a, b);
    const submitting = a.submit(report("shared"));
    await vi.waitFor(() => expect(posted).toBe(1));
    await b.start(); // tab B loads while A is mid-send: the lock is taken, B skips
    await b.flush();
    expect(posted).toBe(1);
    release();
    await submitting;
    await vi.waitFor(() => expect(b.list()[0]?.state).toBe("sent"));
    expect(posted).toBe(1);
  });

  it("without locks both tabs may send; the shared report id keeps it one report on the server", async () => {
    const seen = new Map<string, number>();
    const server = async (body: string): Promise<PostResult> => {
      const id = JSON.parse(body).report_id as string;
      seen.set(id, (seen.get(id) ?? 0) + 1);
      return { ok: true, serverId: "fb_x", duplicate: seen.get(id)! > 1 };
    };
    const storage = new MemoryOutboxStorage();
    const mk = () => createOutbox({ storage, post: server, locks: null, channel: null, onlineTarget: null, visibilityTarget: null, isOnline: () => true });
    const a = mk();
    const b = mk();
    outboxes.push(a, b);
    await a.submit({ ...report(), reportId: "r_same_0001" });
    await b.submit({ ...report(), reportId: "r_same_0001" });
    expect([...seen.keys()]).toEqual(["r_same_0001"]);
  });

  it("an item discarded in one tab disappears in the other", async () => {
    const storage = new MemoryOutboxStorage();
    const bus = fakeBus();
    const mk = () => createOutbox({ storage, post: async () => DOWN, locks: null, channel: bus(), onlineTarget: null, visibilityTarget: null, isOnline: () => false });
    const a = mk();
    const b = mk();
    outboxes.push(a, b);
    await a.submit(report());
    await vi.waitFor(() => expect(b.list()).toHaveLength(1));
    await a.discard(a.list()[0]!.id);
    await vi.waitFor(() => expect(b.list()).toHaveLength(0));
  });
});

describe("storage and codec", () => {
  it("localStorage backend round-trips and skips foreign and corrupt keys", async () => {
    const ls = localStorage;
    ls.clear();
    ls.setItem("other", "1");
    ls.setItem("ti4.feedbackOutbox.v1.bad", "{nope");
    const s = localStorageOutbox(ls);
    const item = { id: "r_1", seq: 1 } as OutboxItem;
    await s.put(item);
    expect(await s.getAll()).toEqual([item]);
    await s.delete("r_1");
    expect(await s.getAll()).toEqual([]);
  });

  it("gzip round trip, and small bodies stay plain", async () => {
    const big = JSON.stringify({ x: "abc".repeat(40_000) });
    const enc = await encodeBody(big);
    expect(enc.enc).toBe("gzip");
    expect(await decodeBody(enc)).toBe(big);
    expect((await encodeBody("{}")).enc).toBe("json");
  });
});

describe("transport", () => {
  const res = (status: number, body: unknown = {}, headers: Record<string, string> = {}) =>
    vi.fn(async () => new Response(JSON.stringify(body), { status, headers })) as unknown as typeof fetch;

  it("sorts answers into retry or give up", async () => {
    expect(await postReport("{}", { fetchImpl: res(200, { id: "fb_1", duplicate: true }) })).toEqual({ ok: true, serverId: "fb_1", duplicate: true });
    for (const status of [408, 500, 502, 503]) expect(await postReport("{}", { fetchImpl: res(status) })).toMatchObject({ ok: false, retryable: true });
    for (const status of [400, 404, 413]) expect(await postReport("{}", { fetchImpl: res(status) })).toMatchObject({ ok: false, retryable: false });
    expect(await postReport("{}", { fetchImpl: res(401) })).toMatchObject({ retryable: true });
    const limited = await postReport("{}", { fetchImpl: res(429, {}, { "retry-after": "90" }) });
    expect(limited).toMatchObject({ ok: false, retryable: true, status: 429, retryAfterMs: 90_000 });
  });

  it("a network error and a timeout are retryable", async () => {
    const down = vi.fn(async () => { throw new TypeError("Failed to fetch"); }) as unknown as typeof fetch;
    expect(await postReport("{}", { fetchImpl: down })).toMatchObject({ ok: false, retryable: true, status: null });
    const hang = vi.fn((_u: unknown, init?: RequestInit) => new Promise((_r, reject) => {
      init?.signal?.addEventListener("abort", () => reject(Object.assign(new Error("aborted"), { name: "AbortError" })));
    })) as unknown as typeof fetch;
    const out = await postReport("{}", { fetchImpl: hang, timeoutMs: 20 });
    expect(out).toMatchObject({ ok: false, retryable: true, message: expect.stringContaining("in time") });
  });

  it("refuses a body over 3 MB without sending it; the timeout scales with size", async () => {
    const f = res(200);
    const out = await postReport("x".repeat(3 * 1024 * 1024 + 1), { fetchImpl: f });
    expect(out).toMatchObject({ ok: false, retryable: false, status: 413 });
    expect(f).not.toHaveBeenCalled();
    expect(timeoutFor(3 * 1024 * 1024)).toBeGreaterThan(timeoutFor(1000) + 40_000);
  });

  it("Retry-After in seconds or as a date, capped at an hour", () => {
    expect(parseRetryAfter("30")).toBe(30_000);
    expect(parseRetryAfter("999999")).toBe(3_600_000);
    expect(parseRetryAfter(new Date(10_000).toUTCString(), 4_000)).toBe(6_000);
    expect(parseRetryAfter(null)).toBeUndefined();
  });

  it("withFlushLock runs without Web Locks and skips when the lock is held", async () => {
    expect(await withFlushLock("n", async () => 1, undefined)).toBe(1);
    const busy = { request: async (_n: string, _o: unknown, cb: (l: unknown) => Promise<unknown>) => cb(null) } as unknown as LockManager;
    expect(await withFlushLock("n", async () => 1, busy)).toBeUndefined();
  });
});
