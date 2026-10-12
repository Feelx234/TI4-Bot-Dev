import { useEffect, useMemo, useState } from "react";
import { GameShell } from "../game/shell/GameShell";
import type { Transport } from "../session/transport";
import { useLiveSession } from "../session/useLiveSession";
import { type Engine, loadEngine } from "../session/wasmTransport";
import { createWsTransport, onlineKeys } from "../session/wsTransport";
import { Button } from "../ui";
import { Notice, wasmUrl } from "./LocalApp";
import { type OnlineStart, createRequest } from "./onlineStart";

const message = (error: unknown) => (error instanceof Error ? error.message : String(error));

/** What the server answers to `POST /api/games`: the part that the page keeps. */
interface CreatedGame {
  game_id: string;
  player_session: string;
}

/** The game that `?online` asked for. One for the page: React may mount twice (StrictMode). */
let created: Promise<CreatedGame> | null = null;

/** A game of eight on the server: this page has the first seat, the other seats decide at random. */
function createGame(start: OnlineStart): Promise<CreatedGame> {
  created ??= fetch("/api/games", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(createRequest(start)),
  }).then(async (response) => {
    if (!response.ok) {
      throw new Error(
        `The server did not start a game (${response.status}). ${await response.text()}`,
      );
    }
    return response.json();
  });
  return created;
}

/**
 * A game on the server, at the address of the page (`/api`, `/ws`). `?online` starts one and
 * goes to its address; `?online=<game_id>` is that game, as the player that this browser kept
 * for it, or as a spectator. The printed cards are those of the engine of this page.
 *
 * Without the game (it is being made or reached, or the server no longer knows it) the page
 * has a row of its own, with the way back to the local game.
 */
export function OnlineApp({ gameId, start }: { gameId: string | null; start: OnlineStart }) {
  const [engine, setEngine] = useState<Engine | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [transport, setTransport] = useState<Transport | null>(null);

  useEffect(() => {
    let stale = false;
    loadEngine(wasmUrl)
      .then((loaded) => stale || setEngine(loaded))
      .catch((error: unknown) => stale || setFailure(message(error)));
    return () => {
      stale = true;
    };
  }, []);

  useEffect(() => {
    if (gameId !== null) {
      return;
    }
    createGame(start)
      .then((made) => {
        localStorage.setItem(onlineKeys(made.game_id).session, made.player_session);
        // A page load: the address is the game, so a reload goes on with it.
        location.replace(`?online=${encodeURIComponent(made.game_id)}`);
      })
      .catch((error: unknown) => setFailure(message(error)));
  }, [gameId, start]);

  // Made in an effect, as the game of `LocalApp`: each mount gets a connection of its own.
  useEffect(() => {
    if (!engine || gameId === null) {
      return;
    }
    const made = createWsTransport(engine, {
      gameId,
      playerSession: localStorage.getItem(onlineKeys(gameId).session),
    });
    setTransport(made);
    return () => {
      made.close();
      setTransport(null);
    };
  }, [engine, gameId]);

  const drafts = useMemo(() => {
    const keys = gameId === null ? null : onlineKeys(gameId);
    return keys
      ? {
          storage: localStorage,
          key: keys.draft,
          planKey: keys.plan,
          secondaryKey: keys.secondary,
        }
      : undefined;
  }, [gameId]);
  const { session, error } = useLiveSession(transport, drafts);

  if (session && !error) {
    return (
      <div className="flex h-dvh flex-col">
        <div className="relative grid min-h-0 flex-1 grid-rows-[minmax(0,1fr)]">
          <GameShell session={session} other={{ label: "Local game", href: "/" }} />
        </div>
      </div>
    );
  }
  const stopped = failure ?? error;
  return (
    <div className="flex h-dvh flex-col">
      <header className="flex min-h-12 items-center gap-3.5 border-b border-line bg-surface px-4 pt-[max(6px,env(safe-area-inset-top))] pb-1.5">
        <strong className="mr-auto text-sm font-semibold">Multiplayer</strong>
        <Button tone="quiet" size="sm" onClick={() => location.assign("/")}>
          Local game
        </Button>
      </header>
      <Notice tone={stopped ? "error" : "quiet"}>
        {stopped ??
          (gameId === null ? "Starting a game on the server…" : "Connecting to the game…")}
      </Notice>
    </div>
  );
}
