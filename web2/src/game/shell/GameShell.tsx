import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";
import { shortcuts, type ExplorationDeck, type GameSession, type Intent } from "../../model";
import {
  Badge,
  Button,
  Dialog,
  Drawer,
  InlineNote,
  LiveRegion,
  Toast,
  cx,
  usePhone,
} from "../../ui";
import { ActionFooter, ActionPanel, footerOf } from "../action/ActionPanel";
import { Board } from "../board/Board";
import { GameProvider } from "../context";
import { LinkProvider } from "../link";
import { PlayerTable } from "../players/PlayerTable";
import { ReferenceDrawer } from "./Reference";
import { OVERLAY, keyIsFree } from "./shortcuts";
import { type OtherGame, Toolbar } from "./Toolbar";

const ACCENT = {
  draft: "var(--color-gold)",
  live: "var(--color-cyan)",
  done: "var(--color-green)",
};

const PANES = [
  ["map", "Map"],
  ["players", "Players"],
  ["action", "Action"],
] as const;
type Pane = (typeof PANES)[number][0];

/**
 * The game screen: one toolbar, then the board and the right column (player table, action panel).
 * The right column has one fixed width. The design target is 1920×1080 or larger; see AGENTS.md.
 * A portrait phone shows the same three parts one at a time, with the footer of the action under
 * each of them.
 * It needs a `GameSession` and nothing else. State that only changes what is in view lives here.
 */
export function GameShell({
  session,
  other,
  settings,
  settingsOpen = false,
}: {
  session: GameSession;
  other?: OtherGame;
  /** The content of the Dev sheet. `close` shuts the sheet, for an action that ends it. */
  settings?: (close: () => void) => ReactNode;
  /** The Dev sheet was open before the shell was there: it stays open. */
  settingsOpen?: boolean;
}) {
  const { view } = session;
  const [drawer, setDrawer] = useState<string | null>(settingsOpen ? "settings" : null);
  const [tableOpen, setTableOpen] = useState(true);
  const [applyOpen, setApplyOpen] = useState(false);
  /** The exploration deck that a planet type opened: the sheet shows it on top. */
  const [deck, setDeck] = useState<ExplorationDeck | null>(null);
  const toggleDrawer = (id: string) => {
    setDeck(null);
    setDrawer((now) => (now === id ? null : id));
  };
  const phone = usePhone();
  const [pane, setPane] = useState<Pane>("action");
  // The pane follows the choice: a choice on the board or in the player table opens that pane.
  const choiceOn: Pane | null = view.players.pick
    ? "players"
    : view.board.task?.interactive || view.board.targeting
      ? "map"
      : null;
  useEffect(() => setPane(choiceOn ?? "action"), [choiceOn]);
  // After an undo the pane follows the choice that is open again, also one of the same kind.
  const undone = useRef(false);
  // biome-ignore lint/correctness/useExhaustiveDependencies: the next view is the answer to the undo
  useEffect(() => {
    if (undone.current) {
      undone.current = false;
      setPane(choiceOn ?? "action");
    }
  }, [view]);

  const dispatch = useCallback(
    (intent: Intent) => {
      if (intent.type === "openApply") {
        return setApplyOpen(true);
      }
      if (intent.type === "openReference") {
        setDeck(intent.sheet === "exploration" ? intent.deck : null);
        return setDrawer(intent.sheet);
      }
      if (intent.type === "inspectLogEntry") {
        setDrawer(null);
      }
      if (intent.type === "undo") {
        undone.current = true;
      }
      session.dispatch(intent);
    },
    [session],
  );
  useEffect(() => {
    if (!view.apply) {
      setApplyOpen(false);
    }
  }, [view.apply]);

  // Shortcut keys of the open action. Esc closes an open reference sheet first.
  const keys = useRef({ view: view.action, drawer, dispatch });
  keys.current = { view: view.action, drawer, dispatch };
  useEffect(() => {
    // Focus that Tab moved belongs to the keyboard; focus that a click left behind does not.
    let focusByKeyboard = false;
    const onPointer = () => (focusByKeyboard = false);
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Tab") {
        focusByKeyboard = true;
      }
      const now = keys.current;
      // Esc closes one thing: an open dialog, menu or rule text goes first.
      if (event.key === "Escape" && now.drawer && !document.querySelector(OVERLAY)) {
        return setDrawer(null);
      }
      if (!keyIsFree(event, focusByKeyboard)) {
        return;
      }
      const intent = shortcuts(now.view).get(event.key.toLowerCase());
      if (!intent) {
        return;
      }
      // The key is used: the control that has focus must not act on it too.
      event.preventDefault();
      event.stopPropagation();
      now.dispatch(intent);
    };
    document.addEventListener("keydown", onKey, true);
    document.addEventListener("pointerdown", onPointer, true);
    return () => {
      document.removeEventListener("keydown", onKey, true);
      document.removeEventListener("pointerdown", onPointer, true);
    };
  }, []);

  const reference = drawer && (
    <ReferenceDrawer
      id={drawer}
      deck={deck}
      view={view.reference}
      players={view.players.rows}
      onClose={() => setDrawer(null)}
    />
  );
  const close = () => setDrawer(null);
  // Dev is a sheet like the reference sheets, in the same place.
  const sheet =
    drawer === "settings"
      ? settings && (
          <Drawer title="Dev" onClose={close}>
            {settings(close)}
          </Drawer>
        )
      : reference;
  const players = (
    <PlayerTable
      view={view.players}
      open={tableOpen || phone}
      onToggle={() => setTableOpen(!tableOpen)}
      onOpenSeat={(seat) => toggleDrawer(`player:${seat}`)}
    />
  );
  const board = <Board view={view.board} />;
  const paneClass = (id: Pane) =>
    cx(
      "absolute inset-0 grid grid-cols-[minmax(0,1fr)] grid-rows-[minmax(0,1fr)]",
      pane !== id && "invisible",
    );

  return (
    <GameProvider seats={view.seats} dispatch={dispatch}>
      <LinkProvider>
        <main
          className="flex min-h-0 flex-col"
          style={{ "--accent": ACCENT[view.accent] } as CSSProperties}
        >
          <Toolbar
            view={view.toolbar}
            drawer={drawer}
            onDrawer={toggleDrawer}
            phone={phone}
            other={other}
            settings={!!settings}
          />
          {phone ? (
            <>
              <div className="relative min-h-0 flex-1">
                {/* Every pane stays in the page, so the board keeps its camera and its size. */}
                <div className={paneClass("map")} inert={pane !== "map"}>
                  {board}
                </div>
                <div
                  className={cx(paneClass("players"), "block! overflow-auto bg-board")}
                  inert={pane !== "players"}
                >
                  {players}
                </div>
                <div className={paneClass("action")} inert={pane !== "action"}>
                  <ActionPanel view={view.action} reveal={view.reveal} footer={false} />
                </div>
                {sheet}
              </div>
              {/* An open fleet has the room of the footer, and so have the open landings and the
                  open build of a production. The check mark of the sheet gives the footer back.
                  A sheet that covers the pane has the room of the footer and of the tabs. */}
              {!sheet &&
                !(
                  pane === "map" &&
                  (view.board.origin || view.board.landing || view.board.production)
                ) && <ActionFooter view={footerOf(view.action)} />}
              {!sheet && (
                <nav
                  aria-label="Panes"
                  className="flex border-t border-line bg-well pb-(--inset-bottom,env(safe-area-inset-bottom))"
                >
                  {PANES.map(([id, label]) => (
                    <button
                      key={id}
                      type="button"
                      aria-pressed={pane === id}
                      onClick={() => setPane(id)}
                      className="min-h-12 flex-1 border-t-2 border-transparent text-sm font-strong text-muted aria-pressed:border-accent aria-pressed:text-accent"
                    >
                      {label}
                      {choiceOn === id && pane !== id && " ·"}
                    </button>
                  ))}
                </nav>
              )}
            </>
          ) : (
            <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_1040px]">
              <Board view={view.board}>{sheet}</Board>
              <div className="flex min-h-0 min-w-0 flex-col border-l border-line">
                {players}
                <ActionPanel view={view.action} reveal={view.reveal} />
              </div>
            </div>
          )}
          <Dialog
            open={applyOpen && !!view.apply}
            onClose={() => setApplyOpen(false)}
            lead={<Badge tone="draft">Private draft</Badge>}
            title="Apply to Live?"
            description="These recorded choices are committed to the live game in order."
            actions={
              <>
                <Button tone="quiet" autoFocus onClick={() => setApplyOpen(false)}>
                  Cancel
                </Button>
                <Button
                  tone="primary"
                  iconAfter="arrow"
                  onClick={() => {
                    setApplyOpen(false);
                    session.dispatch({ type: "confirmApply" });
                  }}
                >
                  Apply to Live
                </Button>
              </>
            }
          >
            <ul className="my-3.5 rounded border border-line">
              {view.apply?.items.map((item) => (
                <li key={item} className="px-3 py-[9px] text-sm [li+&]:border-t [li+&]:border-line">
                  {item}
                </li>
              ))}
            </ul>
            <InlineNote strong={view.apply?.cost}>{view.apply?.note}</InlineNote>
          </Dialog>
          <Toast message={view.toast} />
          <LiveRegion text={view.announcement} />
        </main>
      </LinkProvider>
    </GameProvider>
  );
}
