import type { OutboxItem } from "./outbox.ts";

/** Where outbox items live. Every method may reject; the outbox copes. */
export interface OutboxStorage {
  readonly kind: "indexeddb" | "localstorage" | "memory";
  getAll(): Promise<OutboxItem[]>;
  put(item: OutboxItem): Promise<void>;
  delete(id: string): Promise<void>;
}

export class MemoryOutboxStorage implements OutboxStorage {
  readonly kind = "memory" as const;
  private readonly map = new Map<string, OutboxItem>();
  async getAll() {
    return [...this.map.values()].map((i) => structuredClone(i));
  }
  async put(item: OutboxItem) {
    this.map.set(item.id, structuredClone(item));
  }
  async delete(id: string) {
    this.map.delete(id);
  }
}

const DB_NAME = "ti4-feedback-outbox";
const STORE = "reports";

const request = <T,>(r: IDBRequest<T>): Promise<T> =>
  new Promise((resolve, reject) => {
    r.onsuccess = () => resolve(r.result);
    r.onerror = () => reject(r.error);
  });

export async function openIndexedDbOutbox(factory: IDBFactory = indexedDB): Promise<OutboxStorage> {
  const db = await new Promise<IDBDatabase>((resolve, reject) => {
    const open = factory.open(DB_NAME, 1);
    open.onupgradeneeded = () => open.result.createObjectStore(STORE, { keyPath: "id" });
    open.onsuccess = () => resolve(open.result);
    open.onerror = () => reject(open.error);
    open.onblocked = () => reject(new Error("blocked"));
  });
  const tx = (mode: IDBTransactionMode) => db.transaction(STORE, mode).objectStore(STORE);
  return {
    kind: "indexeddb",
    getAll: async () => (await request(tx("readonly").getAll())) as OutboxItem[],
    put: async (item) => void (await request(tx("readwrite").put(item))),
    delete: async (id) => void (await request(tx("readwrite").delete(id))),
  };
}

const LS_PREFIX = "ti4.feedbackOutbox.v1.";

export function localStorageOutbox(storage: Storage = localStorage): OutboxStorage {
  return {
    kind: "localstorage",
    async getAll() {
      const out: OutboxItem[] = [];
      for (let i = 0; i < storage.length; i++) {
        const key = storage.key(i);
        if (!key?.startsWith(LS_PREFIX)) continue;
        try {
          out.push(JSON.parse(storage.getItem(key) ?? "null") as OutboxItem);
        } catch {
          // a corrupt entry is skipped
        }
      }
      return out;
    },
    async put(item) {
      storage.setItem(LS_PREFIX + item.id, JSON.stringify(item));
    },
    async delete(id) {
      storage.removeItem(LS_PREFIX + id);
    },
  };
}

/** IndexedDB, else localStorage, else memory (lost on reload, but the page keeps working). */
export async function openOutboxStorage(): Promise<OutboxStorage> {
  try {
    if (typeof indexedDB !== "undefined") {
      const timeout = new Promise<never>((_, reject) => setTimeout(() => reject(new Error("timeout")), 4000));
      return await Promise.race([openIndexedDbOutbox(), timeout]);
    }
  } catch {
    // private window, blocked or broken: try the next one
  }
  try {
    const ls = localStorageOutbox();
    await ls.put({ id: "_probe" } as OutboxItem);
    await ls.delete("_probe");
    return ls;
  } catch {
    return new MemoryOutboxStorage();
  }
}
