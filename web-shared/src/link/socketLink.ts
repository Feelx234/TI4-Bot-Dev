import { type Socket, backoff, connect } from "../net/socket.ts";
import { type GameLink, listeners } from "./link.ts";

export interface SocketLinkOptions {
  /** What is sent on each socket that opens: the subscription to the game. */
  subscribe(): object;
  /** The heartbeat, sent every `HEARTBEAT_MS` while a socket is open. Without it: a `ping`. */
  ping?(sequence: number): object;
  /** Added to every message that is sent (the protocol version, say). */
  stamp?: object;
  /** How long to wait before try `attempt` (from 0) to reach the game again, in ms. Without it: `backoff`. */
  retryAfter?(attempt: number): number;
  /**
   * Told when a socket ended, before anything else. `wait` replaces the wait of `retryAfter`;
   * `quiet` says that the loss was expected (the server replaced the game), so no `offline` is
   * told for it.
   */
  lost?(event: { code?: number; opened: boolean }): { wait?: number; quiet?: boolean } | undefined;
}

/**
 * The game on a server: one socket of `open` at a time, opened again when it is lost. Each socket
 * that opens is subscribed and then told as `opened`; its messages are parsed from JSON. The wait
 * between tries grows until a socket delivers a message.
 */
export function socketLink(open: () => Socket, options: SocketLinkOptions): GameLink {
  const all = listeners();
  const retryAfter = options.retryAfter ?? backoff;
  let attempt = 0;
  let sequence = 0;
  let offline = false;
  let closed = false;

  const send = (message: object) => {
    connection.send(JSON.stringify({ ...options.stamp, ...message }));
  };
  const close = () => {
    closed = true;
    connection.close();
    all.clear();
  };

  const connection = connect(open, {
    opened() {
      send(options.subscribe());
      offline = false;
      all.tell({ kind: "opened" });
    },
    beat() {
      sequence += 1;
      send(options.ping ? options.ping(sequence) : { type: "ping", sequence });
    },
    message(text) {
      let message: unknown;
      try {
        message = JSON.parse(text);
      } catch {
        all.tell({ kind: "fatal", message: "The server sent a message that is not JSON" });
        close();
        return;
      }
      attempt = 0;
      all.tell({ kind: "message", message });
    },
    lost(event) {
      const told = options.lost?.(event);
      const wait = told?.wait ?? retryAfter(attempt++);
      if (!closed && !offline && !told?.quiet) {
        offline = true;
        all.tell({ kind: "offline" });
      }
      return wait;
    },
  });

  return {
    send,
    subscribe: all.add,
    close,
  };
}
