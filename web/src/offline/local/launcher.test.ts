import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { OFFLINE_FACTIONS, createLocalLauncher, engineSetup } from "./launcher.ts";
import { MemoryGameStore, SessionGameStore, type StoredGame } from "./store.ts";
import { FakePort, GAME, envelope, save, snapshot } from "./testKit.ts";
import { LocalTransport } from "./LocalTransport.ts";
import { Host, hasJspi } from "./hostApi.ts";
import type { OfflineSetup } from "../types.ts";

const setup: OfflineSetup = { playerCount: 4, humanSeat: 2, humanFaction: "hacan", nickname: "Ada" };

describe("engineSetup", () => {
  it("maps the 1-based seat to the engine's 0-based seat and keeps the nickname out", () => {
    const mapped = engineSetup(setup, "off_x");
    expect(mapped).toMatchObject({ game_id: "off_x", players: 4, human_seat: 1, human_faction: "hacan" });
    expect(JSON.stringify(mapped)).not.toContain("Ada");
    expect(Number.isSafeInteger(mapped.seed)).toBe(true);
  });
  it("leaves a random seat and faction to the engine and honours a dev seed", () => {
    const mapped = engineSetup({ ...setup, humanSeat: null, humanFaction: null, seed: 42 }, "off_x");
    expect(mapped).toEqual({ game_id: "off_x", seed: 42, players: 4 });
  });
});

describe("placeholder stores", () => {
  const game = (id: string, updatedAt = 1): StoredGame => ({
    gameId: id, save: save(["a"]), setup, engineHash: "h", createdAt: 1, updatedAt, resumeMs: null,
  });
  class FakeStorage {
    data = new Map<string, string>();
    full = false;
    get length() { return this.data.size; }
    key(i: number) { return [...this.data.keys()][i] ?? null; }
    getItem(k: string) { return this.data.get(k) ?? null; }
    setItem(k: string, v: string) { if (this.full) throw new Error("quota"); this.data.set(k, v); }
    removeItem(k: string) { this.data.delete(k); }
  }
  it("keeps games in memory", async () => {
    const store = new MemoryGameStore();
    await store.put(game("a", 1));
    await store.put(game("b", 2));
    expect((await store.list()).map((g) => g.gameId)).toEqual(["b", "a"]);
    expect((await store.get("a"))?.save.answers).toEqual(["a"]);
    await store.remove("a");
    expect(await store.get("a")).toBeNull();
  });
  it("survives a reload through sessionStorage and falls back to memory when it is full", async () => {
    const storage = new FakeStorage();
    const first = new SessionGameStore(() => storage as unknown as Storage);
    await first.put(game("a", 5));
    const second = new SessionGameStore(() => storage as unknown as Storage); // a new page
    expect((await second.get("a"))?.updatedAt).toBe(5);
    expect(await second.list()).toHaveLength(1);
    storage.full = true;
    await second.put(game("b", 6));
    expect((await second.get("b"))?.gameId).toBe("b");
    await second.remove("a");
    expect(await second.get("a")).toBeNull();
  });
  it("has no storage at all (private mode): memory only", async () => {
    const store = new SessionGameStore(() => null);
    await store.put(game("a"));
    expect(await store.list()).toHaveLength(1);
  });
});

/** A session module whose engine is a FakePort: no wasm, no worker. */
function fakeSession(ports: FakePort[], saves: ReturnType<typeof save>[]) {
  return async () =>
    ({
      loadEngine: async () => ({ module: {} as WebAssembly.Module, hash: "abc123" }),
      openTransport: (_engine: unknown, options: ConstructorParameters<typeof LocalTransport>[0]) =>
        new LocalTransport({
          ...options,
          createPort: async () => {
            const port = new FakePort();
            ports.push(port);
            setTimeout(() => {
              port.emit({ t: "ask", envelope: envelope("ask", { messages: [snapshot(1, "n1")] }), save: saves[0] });
            }, 0);
            return port;
          },
        }),
    }) as unknown as typeof import("./session.ts");
}

describe("the local launcher", () => {
  it("creates, lists, resumes from the stored save, exports and removes a game", async () => {
    const store = new MemoryGameStore();
    const ports: FakePort[] = [];
    const first = { ...save(["o1", "o2"]), human_seat: 1, setup: { game_id: "ignored", seed: 7, players: 4, human_faction: "hacan" } };
    const launcher = createLocalLauncher({ store, session: fakeSession(ports, [first]) });
    expect(launcher.supported()).toEqual(hasJspi() ? { ok: true } : expect.anything());
    expect(launcher.factions).toHaveLength(24);

    const progress: string[] = [];
    const handle = await launcher.create(setup, (p) => progress.push(p.phase), new AbortController().signal);
    expect(handle.gameId).toMatch(/^off_/);
    expect(progress[0]).toBe("loading-engine");
    expect(ports[0].posted[0]).toMatchObject({ t: "start", config: { setup: { human_seat: 1, players: 4, human_faction: "hacan" } } });

    await new Promise((r) => setTimeout(r, 0));
    const [summary] = await launcher.list();
    expect(summary).toMatchObject({ playerCount: 3, humanSeat: 2, humanFaction: "hacan", answers: 2, status: "in_progress" });
    const stored = await store.get(handle.gameId);
    expect(stored?.engineHash).toBe("abc123");
    expect(stored?.setup.nickname).toBe("Ada");

    // The same page opens the same game again: no second engine.
    const again = await launcher.resume(handle.gameId, () => undefined, new AbortController().signal);
    expect(again.transport).toBe(handle.transport);
    expect(ports).toHaveLength(1);

    const file = await launcher.exportGame(handle.gameId);
    const body = JSON.parse(await file.blob.text());
    expect(body).toMatchObject({ seed: 7, answers: ["o1", "o2"], engineHash: "abc123", playerCount: 3 });

    await launcher.remove(handle.gameId);
    expect(await launcher.list()).toEqual([]);
    expect(ports[0].terminated).toBe(true);
  });

  it("resumes a stored game from its save with replay progress", async () => {
    const store = new MemoryGameStore();
    const stored = save(["a", "b", "c"]);
    await store.put({ gameId: GAME, save: stored, setup, engineHash: "old", createdAt: 1, updatedAt: 2, resumeMs: null });
    const ports: FakePort[] = [];
    const launcher = createLocalLauncher({ store, session: fakeSession(ports, [stored]) });
    const phases: string[] = [];
    await launcher.resume(GAME, (p) => phases.push(`${p.phase}:${p.total}`), new AbortController().signal);
    expect(ports[0].posted[0]).toMatchObject({ t: "start", config: { setup: stored.setup, resume: { answers: ["a", "b", "c"] } } });
    expect(phases).toContain("replaying:3");
    await new Promise((r) => setTimeout(r, 0));
    expect((await store.get(GAME))?.resumeMs).not.toBeNull();
  });

  it("says so when the game is not there, and honours an abort", async () => {
    const launcher = createLocalLauncher({ store: new MemoryGameStore(), session: fakeSession([], [save()]) });
    await expect(launcher.resume("nope", () => undefined, new AbortController().signal)).rejects.toThrow(/not on this device/);
    const controller = new AbortController();
    controller.abort();
    await expect(launcher.create(setup, () => undefined, controller.signal)).rejects.toThrow();
  });

  it("explains a browser without JSPI", async () => {
    const wa = WebAssembly as unknown as { Suspending?: unknown };
    const original = wa.Suspending;
    Object.defineProperty(WebAssembly, "Suspending", { value: undefined, configurable: true, writable: true });
    try {
      const launcher = createLocalLauncher({ store: new MemoryGameStore() });
      expect(launcher.supported()).toMatchObject({ ok: false, reason: expect.stringMatching(/Chrome or Edge 137/) });
      await expect(launcher.create(setup, () => undefined, new AbortController().signal)).rejects.toThrow(/JSPI/);
    } finally {
      Object.defineProperty(WebAssembly, "Suspending", { value: original, configurable: true, writable: true });
    }
  });
});

describe("the engine file", () => {
  const wasm = resolve(process.cwd(), "src/offline/local/engine/ti4_wasm.wasm");
  it.skipIf(!existsSync(wasm) || !hasJspi())("offers exactly the factions the launcher lists", async () => {
    const host = await Host.create(await WebAssembly.compile(readFileSync(wasm)));
    const form = host.query<{ faction_groups: { factions: { id: string }[] }[] }>({ query: "describe" });
    const ids = form.faction_groups.flatMap((group) => group.factions.map((f) => f.id));
    expect([...OFFLINE_FACTIONS].sort()).toEqual([...ids].sort());
  });
});

