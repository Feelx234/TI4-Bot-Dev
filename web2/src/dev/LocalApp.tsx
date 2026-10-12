import { type ChangeEvent, type ReactNode, useEffect, useMemo, useRef, useState } from "react";
import { SeatShape } from "../game/context";
import { GameShell } from "../game/shell/GameShell";
import { DemoApp } from "./DemoApp";
import { onlineAddress } from "./onlineStart";
import {
  type LocalGame,
  type SavedGame,
  type SeatSwitch,
  STEPPING_KEY,
  cardsKey,
  clearSavedGame,
  draftKey,
  gameAddress,
  loadSavedGame,
  parseSavedGame,
  planKey,
  secondaryKey,
  saveKey,
  storeSavedGame,
} from "../session/savedGame";
import type { Transport } from "../session/transport";
import { useLiveSession } from "../session/useLiveSession";
import { type Engine, createWasmTransport, loadEngine } from "../session/wasmTransport";
import { Button, Drawer, Hint, Segmented } from "../ui";

// Not an import: the file is built by scripts/build-wasm.sh and is not in the repository. Without
// it the rest of web2 still builds, and this page says how to build it.
export const wasmUrl = new URL("../session/ti4.wasm", import.meta.url).href;

const message = (error: unknown) => (error instanceof Error ? error.message : String(error));

/** What the page starts from: the engine, and the saved game of this address if there is one. */
type Start =
  | { kind: "loading" }
  | { kind: "failed"; message: string }
  | { kind: "ready"; engine: Engine; saved: SavedGame | null };

/**
 * A game of the engine in this page. `?local=3&players=8&humans=1`: the seed, the number of
 * seats, and which seats are played here (bit 0 is seat A). `?scenario=late-game` starts from a
 * prepared board of the engine. The answers are kept in localStorage, so a reload goes on where
 * the game was.
 */
export function LocalApp({ game }: { game: LocalGame }) {
  const [start, setStart] = useState<Start>({ kind: "loading" });
  /** The saved game was played by another engine, and the player said to play it anyway. */
  const [anyway, setAnyway] = useState(false);
  const [answers, setAnswers] = useState(0);
  /** The seat changes of the game ("Play as"). The last one says which seats are played here. */
  const [switches, setSwitches] = useState<readonly SeatSwitch[]>([]);
  /** Counts the seat changes of this visit: what was staged for the seat before is dropped. */
  const [seatChanges, setSeatChanges] = useState(0);
  const [importError, setImportError] = useState<string | null>(null);
  const [transport, setTransport] = useState<Transport | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  // A scenario names its seed and its seats, and the engine knows them.
  const scenarios = start.kind === "ready" ? start.engine.scenarios : null;
  const played = useMemo(() => {
    const scenario = scenarios?.find((item) => item.id === game.scenario);
    return scenario ? { ...game, seed: scenario.seed, players: scenario.players } : game;
  }, [game, scenarios]);
  /** The other seats wait for "Step" before each decision. A setting of the page. */
  const [stepping, setStepping] = useState(() => localStorage.getItem(STEPPING_KEY) === "1");
  // The game that is played is told, and the next one starts with it.
  const steps = useRef(stepping);
  steps.current = stepping;
  // Without the shell nothing else closes the Dev sheet: Esc does, as in the shell.
  useEffect(() => {
    if (!settingsOpen) {
      return;
    }
    const onKey = (event: KeyboardEvent) => event.key === "Escape" && setSettingsOpen(false);
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [settingsOpen]);

  useEffect(() => {
    let stale = false;
    loadEngine(wasmUrl)
      .then((engine) => ({
        kind: "ready" as const,
        engine,
        saved: loadSavedGame(localStorage, game),
      }))
      .catch((error: unknown) => ({ kind: "failed" as const, message: message(error) }))
      .then((next) => stale || setStart(next));
    return () => {
      stale = true;
    };
  }, [game]);

  const foreign =
    start.kind === "ready" && start.saved !== null && start.saved.engine !== start.engine.hash;
  const waits = foreign && !anyway;

  // Made in an effect, not in a memo: the game is closed when the page lets go of it, and React
  // may do that and mount again (StrictMode). Each mount gets a game of its own.
  useEffect(() => {
    if (start.kind !== "ready" || waits) {
      return;
    }
    const { engine, saved } = start;
    setAnswers(saved?.answers.length ?? 0);
    setSwitches(saved?.switches ?? []);
    const made = createWasmTransport(engine, played, {
      answers: saved?.answers,
      switches: saved?.switches,
      stepping: steps.current,
      never: readNever(game),
      onNever: (never) => localStorage.setItem(cardsKey(game), JSON.stringify(never)),
      onAnswers(list, changes) {
        storeSavedGame(localStorage, {
          format: 1,
          engine: engine.hash,
          ...played,
          answers: [...list],
          ...(changes.length ? { switches: [...changes] } : {}),
        });
        setAnswers(list.length);
        setSwitches([...changes]);
      },
    });
    setTransport(made);
    return () => {
      made.close();
      setTransport(null);
    };
  }, [start, waits, played, game]);

  // A new object after a seat change: the session then starts with nothing staged.
  // biome-ignore lint/correctness/useExhaustiveDependencies: seatChanges is the reason to make it again
  const drafts = useMemo(
    () => ({
      storage: localStorage,
      key: draftKey(game),
      planKey: planKey(game),
      secondaryKey: secondaryKey(game),
    }),
    [game, seatChanges],
  );
  const stepSeats = (on: boolean) => {
    setStepping(on);
    localStorage.setItem(STEPPING_KEY, on ? "1" : "0");
    transport?.capabilities.stepping?.set(on);
  };
  const { session, error, replaying } = useLiveSession(transport, drafts);
  // The shell takes an open Dev sheet over. A page without the shell starts with it closed.
  const shown = !waits && !!session;
  useEffect(() => {
    if (shown) {
      setSettingsOpen(false);
    }
  }, [shown]);

  /** The seats that are played here (bit 0 is seat A). */
  const humans = switches.at(-1)?.humans ?? game.humans;
  const playsHere = (seat: string) => (humans >> (seat.charCodeAt(0) - 97)) & 1;
  /** Plays `seat` here in place of the seat that is shown. The draft of that seat is dropped. */
  const playAs = (seat: string) => {
    localStorage.removeItem(draftKey(game));
    localStorage.removeItem(planKey(game));
    localStorage.removeItem(secondaryKey(game));
    setSeatChanges((count) => count + 1);
    transport?.capabilities.switchSeat?.(seat);
  };
  const newGame = () => {
    clearSavedGame(localStorage, game);
    setAnyway(false);
    setSettingsOpen(false);
    setStart((now) => (now.kind === "ready" ? { ...now, saved: null } : now));
  };
  const exportGame = () => {
    const text = localStorage.getItem(saveKey(game));
    if (text === null) {
      return;
    }
    const link = document.createElement("a");
    link.href = URL.createObjectURL(new Blob([text], { type: "application/json" }));
    link.download = `ti4-${game.scenario ?? `seed${game.seed}`}-${answers}answers.json`;
    link.click();
    URL.revokeObjectURL(link.href);
  };
  const importGame = async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) {
      return;
    }
    try {
      const saved = parseSavedGame(await file.text());
      storeSavedGame(localStorage, saved);
      // The file says which game it is. Its address opens it.
      window.location.search = gameAddress(saved);
    } catch (failure) {
      setImportError(message(failure));
    }
  };
  const file = useRef<HTMLInputElement>(null);

  // A game that starts from a scenario, or from round 1, is another game with its own saved
  // answers: its address opens it, as Import does.
  const startRow = (id: string | undefined, title: string, description: string) => (
    <div
      key={id ?? ""}
      className="row flex min-h-9 items-center gap-2 pr-2.5 [.row+&]:border-t [.row+&]:border-line/50"
    >
      <button
        type="button"
        className="flex min-h-9 flex-1 cursor-pointer items-center gap-2.5 px-3.5 text-left text-base hover:bg-cyan/[.06]"
        aria-current={game.scenario === id}
        onClick={() => {
          if (game.scenario !== id) {
            window.location.search = gameAddress(
              id === undefined
                ? { seed: game.seed, players: game.players, humans: game.humans }
                : { ...game, scenario: id },
            );
          }
        }}
      >
        <strong className="font-mid">{title}</strong>
        {game.scenario === id && <small className="text-xs text-muted">✓ Selected</small>}
      </button>
      <Hint label={`About ${title}`}>{description}</Hint>
    </div>
  );

  // What the game can do besides play: each part of the Dev sheet is shown from it. Before there
  // is a game, the sheet has every part, as a local game does.
  const made = transport?.capabilities;
  const can = {
    stepping: !made || !!made.stepping,
    switchSeat: !made || !!made.switchSeat,
    scenarios: !made || !!made.scenarios,
    save: !made || !!made.save,
  };
  // The Dev sheet. The shell shows it in its sheet slot; a page without the shell shows it below.
  const settingsView = (close: () => void) => (
    <>
      <p className="text-muted">
        {`Local game · ${game.scenario === undefined ? "" : `scenario ${game.scenario} · `}seed ${played.seed} · ${played.players} seats · ${answers} answers saved`}
      </p>
      {importError && (
        <p role="alert" className="mt-3 text-red">
          {importError}
        </p>
      )}
      {can.stepping && (
        <div className="mt-4 flex flex-wrap items-center gap-2.5">
          <span className="text-sm text-muted">Other seats</span>
          <Segmented
            label="Other seats"
            options={[
              { id: "run", label: "Run" },
              { id: "step", label: "Step" },
            ]}
            value={stepping ? "step" : "run"}
            onChange={(mode) => stepSeats(mode === "step")}
          />
          <Hint label="About the other seats">
            The other seats decide at random. With Step the game waits before each of their
            decisions, and the Step button of the action panel lets the seat decide.
          </Hint>
        </div>
      )}
      {session && can.switchSeat && (
        <section aria-label="Play as" className="mt-4">
          <h3 className="flex items-center gap-2 text-sm text-muted">
            Play as
            <Hint label="About Play as">
              You play the chosen seat from now on. The seat that you leave decides at random, from
              its open decision on. Undo does not go back before the change.
            </Hint>
          </h3>
          <div className="mt-1.5 rounded border border-line">
            {Object.values(session.view.seats).map((seat) => (
              <button
                key={seat.id}
                type="button"
                className="row flex min-h-9 w-full cursor-pointer items-center gap-2.5 px-3.5 text-left text-base hover:bg-cyan/[.06] disabled:cursor-default disabled:hover:bg-transparent [.row+&]:border-t [.row+&]:border-line/50"
                aria-current={!!playsHere(seat.id)}
                disabled={!!playsHere(seat.id) || !!error || !!replaying}
                onClick={() => {
                  playAs(seat.id);
                  close();
                }}
              >
                <svg viewBox="-6 -6 12 12" className="size-[1em] shrink-0" aria-hidden="true">
                  <SeatShape symbol={seat.symbol} x={0} y={0} size={11.2} color={seat.color} />
                </svg>
                <strong className="font-mid">{seat.name}</strong>
                <span className="min-w-0 flex-1 truncate text-muted">{seat.faction}</span>
                <small className="text-xs text-muted">
                  {playsHere(seat.id) ? "✓ Playing" : "Random"}
                </small>
              </button>
            ))}
          </div>
        </section>
      )}
      {scenarios && can.scenarios && (
        <section aria-label="Start from" className="mt-4">
          <h3 className="text-sm text-muted">Start from</h3>
          <div className="mt-1.5 rounded border border-line">
            {startRow(undefined, "Round 1", "A new game from its setup, on the seed of this page.")}
            {scenarios.map((scenario) =>
              startRow(scenario.id, scenario.title, scenario.description),
            )}
          </div>
        </section>
      )}
      <section
        aria-label="Multiplayer"
        className="mt-4 flex min-h-9 items-center gap-2 rounded border border-line pr-2.5"
      >
        <button
          type="button"
          className="flex min-h-9 flex-1 cursor-pointer items-center gap-2.5 px-3.5 text-left text-base hover:bg-cyan/[.06]"
          onClick={() => {
            // A page load: the game on the server is another game, with an address of its own.
            window.location.search = onlineAddress({ seed: played.seed });
          }}
        >
          <strong className="font-mid">Multiplayer</strong>
          <small className="text-xs text-muted">{`seed ${played.seed} · 8 seats`}</small>
        </button>
        <Hint label="About Multiplayer">
          A new game of eight on the server, on the seed of this page. You play the first seat, and
          the other seats decide at random.
        </Hint>
      </section>
      {can.save && (
        <div className="mt-4 flex flex-wrap gap-2">
          <Button
            tone="quiet"
            size="sm"
            disabled={answers === 0}
            onClick={() => {
              newGame();
              close();
            }}
          >
            New game
          </Button>
          <Button tone="quiet" size="sm" disabled={answers === 0} onClick={exportGame}>
            Export
          </Button>
          <Button tone="quiet" size="sm" onClick={() => file.current?.click()}>
            Import
          </Button>
        </div>
      )}
    </>
  );

  // The engine did not load: the demo is shown with the reason, so the page is never empty.
  if (start.kind === "failed") {
    return <DemoApp notice={`Local game unavailable. ${start.message}`} />;
  }

  let body: ReactNode;
  if (waits) {
    body = (
      <Notice tone="error">
        The saved game ({start.saved?.answers.length} answers) was played by another build of the
        engine. This build may play the same answers differently.
        <span className="mt-3 flex gap-2">
          <Button onClick={() => setAnyway(true)}>Replay anyway</Button>
          <Button onClick={newGame}>New game</Button>
        </span>
      </Notice>
    );
  } else if (!session) {
    body = (
      <Notice tone={error ? "error" : "quiet"}>
        {error ??
          (replaying
            ? `Playing the saved game again: answer ${replaying.done} of ${replaying.total}…`
            : "Starting the game…")}
      </Notice>
    );
  } else {
    body = (
      <GameShell
        session={session}
        other={{ label: "Demo", href: "/?example=live-picker" }}
        settings={settingsView}
        settingsOpen={settingsOpen}
      />
    );
  }

  // The shell has the toolbar, and the toolbar has the Dev entry. Without the shell the page
  // has a row of its own, so Import and New game stay in reach.
  const inShell = shown;
  return (
    <div className="flex h-dvh flex-col">
      {!inShell && (
        <header className="flex min-h-12 items-center gap-3.5 border-b border-line bg-surface px-4 pt-[max(6px,env(safe-area-inset-top))] pb-1.5">
          <strong className="mr-auto text-sm font-semibold">Local game</strong>
          <Button tone="quiet" size="sm" onClick={() => setSettingsOpen(true)}>
            Dev
          </Button>
        </header>
      )}
      <div className="relative grid min-h-0 flex-1 grid-rows-[minmax(0,1fr)]">
        {body}
        {!inShell && settingsOpen && (
          <Drawer title="Dev" onClose={() => setSettingsOpen(false)}>
            {settingsView(() => setSettingsOpen(false))}
          </Drawer>
        )}
      </div>
      <input
        ref={file}
        type="file"
        accept="application/json,.json"
        aria-label="Saved game file"
        hidden
        onChange={importGame}
      />
    </div>
  );
}

/** The cards that the player is never offered in this game, as they were kept. */
function readNever(game: LocalGame): string[] {
  try {
    const kept: unknown = JSON.parse(localStorage.getItem(cardsKey(game)) || "[]");
    return Array.isArray(kept) ? kept.filter((name) => typeof name === "string") : [];
  } catch {
    return [];
  }
}

export function Notice({ tone, children }: { tone: "quiet" | "error"; children: ReactNode }) {
  return (
    <p role="status" className={tone === "error" ? "p-5 text-red" : "p-5 text-muted"}>
      {children}
    </p>
  );
}

/** The game of the bare address `/`: seed 42, eight seats, and only the first seat is played here. */
export const DEFAULT_GAME: LocalGame = { seed: 42, players: 8, humans: 1 };

/**
 * The game that the address asks for. Without `local`, the bare address gets DEFAULT_GAME, and
 * the demo (`example`) and the gallery have their own addresses, so they get none. With
 * `scenario`, the engine says which seed and how many seats the game has.
 */
export function localGame(params: URLSearchParams): LocalGame | null {
  const number = (name: string, otherwise: number) => {
    const value = Number(params.get(name) || otherwise);
    return Number.isInteger(value) && value >= 0 ? value : otherwise;
  };
  const scenario = params.get("scenario");
  if (scenario) {
    return { ...DEFAULT_GAME, humans: number("humans", 1), scenario };
  }
  if (!params.has("local")) {
    return params.has("example") || params.has("gallery") ? null : DEFAULT_GAME;
  }
  return { seed: number("local", 3), players: number("players", 8), humans: number("humans", 1) };
}
