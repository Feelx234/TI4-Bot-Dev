import type { HostSave } from "../engine/host.ts";

/**
 * A game at the level of its messages: a server behind a websocket (`socketLink`) or the engine
 * in a Worker (`engineLink`). What goes in is a `ClientMessage` (or a local host command), what
 * comes out are `ServerMessage` objects and the state of the connection. Each app keeps its own
 * UI-facing transport on top of it.
 */
export interface GameLink {
  /** A `ClientMessage` or a local host command. Nothing is sent while the link is not open. */
  send(message: object): void;
  /** The engine only: a command whose reply is wanted (`save_state`, `history`, a query). */
  request?(command: object): Promise<unknown>;
  /** The listener gets the events from now on. The answer ends the subscription. */
  subscribe(listener: (event: LinkEvent) => void): () => void;
  /** Ends the link. Nothing is told after it. */
  close(): void;
}

export type LinkEvent =
  /** A `ServerMessage`, undecoded. */
  | { kind: "message"; message: unknown }
  /** The link can be used: a socket opened (and subscribed), or the engine asked its first question. */
  | { kind: "opened" }
  /** The socket was lost; another one is on its way. */
  | { kind: "offline" }
  /** The engine plays a saved game again. */
  | { kind: "progress"; done: number; total: number }
  /** The game as it stands at a question of the engine, or at its end. */
  | { kind: "save"; save: HostSave }
  /** The game cannot go on over this link. Nothing is tried after it. */
  | { kind: "fatal"; message: string };

/** The listeners of a link, told in the order they came. */
export function listeners(): {
  add(listener: (event: LinkEvent) => void): () => void;
  tell(event: LinkEvent): void;
  clear(): void;
} {
  const all = new Set<(event: LinkEvent) => void>();
  return {
    add(listener) {
      all.add(listener);
      return () => all.delete(listener);
    },
    tell(event) {
      for (const listener of [...all]) {
        listener(event);
      }
    },
    clear: () => all.clear(),
  };
}
