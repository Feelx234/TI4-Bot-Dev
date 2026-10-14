import { useCallback, useEffect, useMemo, useState } from "react";
import { LobbyScreen } from "../game/lobby/LobbyScreen";
import { GameShell } from "../game/shell/GameShell";
import type { LobbyIntent } from "../model";
import {
  NICKNAME_KEY,
  PLAYER_COUNT_KEY,
  dropPlayer,
  keepPlayer,
  legacySession,
  moveLegacy,
  onlineKeys,
  playersOf,
} from "../session/onlineKeys";
import { hostChange, nextNickname, selectLobby } from "../session/select/lobby";
import {
  type Seat,
  type Table,
  type TableState,
  createTable,
  playerOf,
  serverTable,
} from "../session/table";
import type { Transport } from "../session/transport";
import { useLiveSession } from "../session/useLiveSession";
import { type Engine, loadEngine } from "../session/wasmTransport";
import { createWsTransport } from "../session/wsTransport";
import { Button } from "../ui";
import { Notice, wasmUrl } from "./LocalApp";
import { DEFAULT_NICKNAME, type OnlineStart, tableAddress, tableRequest } from "./onlineStart";

const message = (error: unknown) => (error instanceof Error ? error.message : String(error));

/** The way out of a table: the start screen. */
const START = { label: "Start", href: "/" };

/** The table that `?online` asked for. One for the page: React may mount twice (StrictMode). */
let created: ReturnType<typeof createTable> | null = null;

/** The page without a table or a game: what it waits for, or why it stopped. */
function Waiting({ failure, children }: { failure: string | null; children: string }) {
  return (
    <div className="flex h-dvh flex-col">
      <header className="flex min-h-12 items-center gap-3.5 border-b border-line bg-surface px-4 pt-[max(6px,env(safe-area-inset-top))] pb-1.5">
        <strong className="mr-auto text-sm font-semibold">Multiplayer</strong>
        <Button tone="quiet" size="sm" onClick={() => location.assign(START.href)}>
          {START.label}
        </Button>
      </header>
      <Notice tone={failure ? "error" : "quiet"}>{failure ?? children}</Notice>
    </div>
  );
}

/** `?online`: makes a table on the server and goes to its address, as its host. */
function NewTable({ start }: { start: OnlineStart }) {
  const [failure, setFailure] = useState<string | null>(null);
  useEffect(() => {
    const request = tableRequest(start, {
      nickname: localStorage.getItem(NICKNAME_KEY),
      players: Number(localStorage.getItem(PLAYER_COUNT_KEY) || Number.NaN),
    });
    created ??= createTable(request);
    created
      .then((made) => {
        keepPlayer(localStorage, made.gameId, made.playerId, made.playerSession);
        localStorage.setItem(NICKNAME_KEY, request.nickname);
        if (!start.fill) {
          localStorage.setItem(PLAYER_COUNT_KEY, String(request.playerCount));
        }
        // A page load: the address is the table and the seat, so a reload goes on with them.
        location.replace(tableAddress(made.gameId, made.playerId));
      })
      .catch((error: unknown) => setFailure(message(error)));
  }, [start]);
  return <Waiting failure={failure}>Making a table on the server…</Waiting>;
}

/** The running game of a table, as the player of this tab, or as a spectator. */
function OnlineGame({
  engine,
  gameId,
  seat,
}: {
  engine: Engine;
  gameId: string;
  seat: Seat | null;
}) {
  const [transport, setTransport] = useState<Transport | null>(null);
  // Made in an effect, as the game of `LocalApp`: each mount gets a connection of its own.
  useEffect(() => {
    const made = createWsTransport(engine, { gameId, playerSession: seat?.playerSession ?? null });
    setTransport(made);
    return () => {
      made.close();
      setTransport(null);
    };
  }, [engine, gameId, seat]);
  // What is staged is kept for the player: two tabs of one browser are two seats.
  const drafts = useMemo(() => {
    if (!seat) {
      return undefined;
    }
    const keys = onlineKeys(gameId, seat.playerId);
    return {
      storage: localStorage,
      key: keys.draft,
      planKey: keys.plan,
      secondaryKey: keys.secondary,
    };
  }, [gameId, seat]);
  const { session, error } = useLiveSession(transport, drafts);
  if (session && !error) {
    return (
      <div className="flex h-dvh flex-col">
        <div className="relative grid min-h-0 flex-1 grid-rows-[minmax(0,1fr)]">
          <GameShell session={session} other={START} />
        </div>
      </div>
    );
  }
  return <Waiting failure={error}>Connecting to the game…</Waiting>;
}

/** Who this tab is at the table: a player that the browser kept, or a visitor. */
function useSeat(gameId: string, asked: string | null): { seat: Seat | null } | null {
  const [who, setWho] = useState<{ seat: Seat | null } | null>(null);
  useEffect(() => {
    let stale = false;
    const resolve = () => {
      if (stale) {
        return;
      }
      const mine = playersOf(localStorage, gameId);
      const ids = Object.keys(mine);
      // Without `seat`, the one player of this browser is the tab; of several, none is.
      const id = asked !== null ? asked : ids.length === 1 ? ids[0] : null;
      const seat = id !== null && mine[id] ? { playerId: id, playerSession: mine[id] } : null;
      if ((seat?.playerId ?? null) !== asked) {
        history.replaceState(null, "", tableAddress(gameId, seat?.playerId));
      }
      setWho({ seat });
    };
    // An older page kept one player session for the game, with no id: the server says whose.
    const legacy = legacySession(localStorage, gameId);
    if (legacy === null) {
      resolve();
    } else {
      playerOf(gameId, legacy)
        .then((id) => id !== null && moveLegacy(localStorage, gameId, id))
        .catch(() => {})
        .then(resolve);
    }
    return () => {
      stale = true;
    };
  }, [gameId, asked]);
  return who;
}

/**
 * A table on the server, at the address of the page (`/api`, `/ws`): `?online=<game_id>` is the
 * table, and `&seat=<player_id>` says which player of this browser the tab is. Before the game
 * the page is the lobby; when the table says that the game runs, the page is the game, as that
 * player. The printed cards are those of the engine of this page.
 */
function TableApp({ gameId, asked }: { gameId: string; asked: string | null }) {
  const [engine, setEngine] = useState<Engine | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const who = useSeat(gameId, asked);
  const seat = who?.seat ?? null;
  const [table, setTable] = useState<Table | null>(null);
  const [state, setState] = useState<TableState | null>(null);
  const [offline, setOffline] = useState(false);
  const [toast, setToast] = useState<{ id: number; text: string } | null>(null);
  const [inspected, setInspected] = useState<string | null>(null);
  const [mine, setMine] = useState<string[]>([]);
  const [nickname, setNickname] = useState(
    () => localStorage.getItem(NICKNAME_KEY) ?? DEFAULT_NICKNAME,
  );
  const running = state?.lobby.phase === "running";

  useEffect(() => {
    let stale = false;
    loadEngine(wasmUrl)
      .then((loaded) => stale || setEngine(loaded))
      .catch((error: unknown) => stale || setFailure(message(error)));
    return () => {
      stale = true;
    };
  }, []);

  // The time is the id: the screen has messages of its own, and shows the newest.
  const say = useCallback((text: string) => setToast({ id: performance.now(), text }), []);
  // The socket of the table is open while the table is a lobby: the game has its own.
  useEffect(() => {
    if (!who || running) {
      return;
    }
    setMine(Object.keys(playersOf(localStorage, gameId)));
    const made = serverTable(gameId, who.seat);
    let before: TableState["lobby"] | null = null;
    const stop = made.subscribe((event) => {
      switch (event.kind) {
        case "table": {
          const { lobby, viewer } = event;
          if (who.seat && viewer === null && lobby.phase === "lobby") {
            // The table no longer has the player of this tab: the tab is a visitor.
            dropPlayer(localStorage, gameId, who.seat.playerId);
            return location.replace(tableAddress(gameId));
          }
          // A change of the host is told to the others once.
          const changed =
            before && viewer !== lobby.host_player_id ? hostChange(before, lobby) : null;
          if (changed) {
            say(changed);
          }
          before = lobby;
          setOffline(false);
          return setState(event);
        }
        case "offline":
          return setOffline(true);
        case "rejected":
          return say(event.reason);
        case "error":
          return setFailure(event.message);
      }
    });
    setTable(made);
    return () => {
      stop();
      made.close();
      setTable(null);
    };
  }, [gameId, who, running, say]);

  const view = useMemo(
    () => state && selectLobby(state, { mine, inspected, offline }, engine?.catalog ?? null),
    [state, mine, inspected, offline, engine],
  );

  if (failure) {
    return <Waiting failure={failure}>…</Waiting>;
  }
  if (!state || !view) {
    return <Waiting failure={null}>Connecting to the table…</Waiting>;
  }
  if (running) {
    return engine ? (
      <OnlineGame engine={engine} gameId={gameId} seat={seat} />
    ) : (
      <Waiting failure={null}>Loading the game…</Waiting>
    );
  }

  const dispatch = (intent: LobbyIntent) => {
    switch (intent.type) {
      case "inspectSystem":
        return setInspected(intent.system);
      case "closeInspector":
        return setInspected(null);
      case "join":
        localStorage.setItem(NICKNAME_KEY, nickname.trim());
        return void table?.join(nickname.trim()).then((joined) => {
          if (joined) {
            keepPlayer(localStorage, gameId, joined.playerId, joined.playerSession);
            location.replace(tableAddress(gameId, joined.playerId));
          }
        });
      case "joinTab": {
        if (!table || state.viewer === null) {
          return;
        }
        // The tab is opened with the click: a browser blocks one that is opened later.
        const tab = window.open("", "_blank");
        return void table.join(nextNickname(state.lobby, state.viewer)).then((joined) => {
          if (!joined) {
            return tab?.close();
          }
          keepPlayer(localStorage, gameId, joined.playerId, joined.playerSession);
          setMine(Object.keys(playersOf(localStorage, gameId)));
          const address = tableAddress(gameId, joined.playerId);
          if (tab) {
            tab.location.replace(address);
          } else {
            say("The browser did not open a tab.");
          }
        });
      }
      case "become":
        return location.assign(tableAddress(gameId, intent.player));
      case "addBot":
        return table?.addBots(intent.kind);
      case "fillBots":
        return table?.addBots(intent.kind, true);
      case "remove":
        return table?.removeSeat(intent.player);
      case "set":
        if (intent.change.players !== undefined) {
          // The next table of this browser has as many seats.
          localStorage.setItem(PLAYER_COUNT_KEY, String(intent.change.players));
        }
        return table?.setSettings(intent.change);
      case "rename":
        localStorage.setItem(NICKNAME_KEY, intent.name);
        return table?.rename(intent.name);
      case "start":
        return table?.start();
    }
  };

  return (
    <div className="flex h-dvh flex-col">
      <div className="relative grid min-h-0 flex-1 grid-rows-[minmax(0,1fr)]">
        <LobbyScreen
          view={view}
          dispatch={dispatch}
          nickname={nickname}
          onNickname={setNickname}
          link={`${location.origin}/${tableAddress(gameId)}`}
          toast={toast}
          other={START}
        />
      </div>
    </div>
  );
}

export function OnlineApp({
  gameId,
  seat,
  start,
}: {
  /** Null: `?online` with no game, which makes a table. */
  gameId: string | null;
  /** The player of this tab, from the address. */
  seat: string | null;
  start: OnlineStart;
}) {
  return gameId === null ? <NewTable start={start} /> : <TableApp gameId={gameId} asked={seat} />;
}
