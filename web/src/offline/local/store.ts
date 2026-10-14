import type { HostSave } from "./hostApi.ts";
import type { OfflineSetup } from "../types.ts";

/** One offline game as stored: the latest save plus what the form made. */
export interface StoredGame {
  gameId: string;
  save: HostSave;
  setup: OfflineSetup;
  /** The hash in the file name of the engine (`ti4_wasm-<hash>.wasm`) the game was played with. */
  engineHash: string;
  /** Unix seconds. */
  createdAt: number;
  updatedAt: number;
  /** How long the last replay of this game took (ms); null before the first resume. */
  resumeMs: number | null;
}

/**
 * Where offline games live. T6 provides the IndexedDB implementation; this task ships two
 * placeholders that need neither IndexedDB nor localStorage: {@link MemoryGameStore} and
 * {@link SessionGameStore}.
 */
export interface OfflineGameStore {
  list(): Promise<StoredGame[]>;
  get(gameId: string): Promise<StoredGame | null>;
  put(game: StoredGame): Promise<void>;
  remove(gameId: string): Promise<void>;
}

/** Lives as long as the page. */
export class MemoryGameStore implements OfflineGameStore {
  private readonly games = new Map<string, StoredGame>();
  async list(): Promise<StoredGame[]> {
    return [...this.games.values()].map(clone).sort((a, b) => b.updatedAt - a.updatedAt);
  }
  async get(gameId: string): Promise<StoredGame | null> {
    const game = this.games.get(gameId);
    return game ? clone(game) : null;
  }
  async put(game: StoredGame): Promise<void> {
    this.games.set(game.gameId, clone(game));
  }
  async remove(gameId: string): Promise<void> {
    this.games.delete(gameId);
  }
}

const PREFIX = "ti4.offline.game:";

/**
 * Placeholder until T6: keeps games in `sessionStorage`, so a refresh of the tab resumes the game
 * (a closed tab forgets it). Falls back to memory when the browser refuses (quota, private mode).
 */
export class SessionGameStore implements OfflineGameStore {
  private readonly memory = new MemoryGameStore();
  constructor(private readonly storage: () => Storage | null = defaultStorage) {}

  async list(): Promise<StoredGame[]> {
    const found = new Map<string, StoredGame>();
    for (const game of await this.memory.list()) found.set(game.gameId, game);
    const storage = this.storage();
    if (storage) {
      try {
        for (let i = 0; i < storage.length; i++) {
          const key = storage.key(i);
          if (!key?.startsWith(PREFIX)) continue;
          const game = parse(storage.getItem(key));
          if (game && (found.get(game.gameId)?.updatedAt ?? -1) <= game.updatedAt)
            found.set(game.gameId, game);
        }
      } catch {
        // Unreadable storage: the memory copy is all there is.
      }
    }
    return [...found.values()].sort((a, b) => b.updatedAt - a.updatedAt);
  }

  async get(gameId: string): Promise<StoredGame | null> {
    const memory = await this.memory.get(gameId);
    try {
      const stored = parse(this.storage()?.getItem(PREFIX + gameId) ?? null);
      if (stored && (!memory || stored.updatedAt >= memory.updatedAt)) return stored;
    } catch {
      // fall through
    }
    return memory;
  }

  async put(game: StoredGame): Promise<void> {
    await this.memory.put(game);
    try {
      this.storage()?.setItem(PREFIX + game.gameId, JSON.stringify(game));
    } catch {
      // Over quota: the memory copy still serves this page.
    }
  }

  async remove(gameId: string): Promise<void> {
    await this.memory.remove(gameId);
    try {
      this.storage()?.removeItem(PREFIX + gameId);
    } catch {
      // nothing to remove
    }
  }
}

function defaultStorage(): Storage | null {
  try {
    return window.sessionStorage;
  } catch {
    return null;
  }
}

function parse(text: string | null): StoredGame | null {
  if (!text) return null;
  try {
    const value = JSON.parse(text) as StoredGame;
    return value && typeof value.gameId === "string" && value.save ? value : null;
  } catch {
    return null;
  }
}

const clone = <T>(value: T): T => structuredClone(value);
