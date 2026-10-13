import { useEffect, useRef, useState, type ReactNode } from "react";
import type {
  Intent,
  LobbyButtonView,
  LobbyIntent,
  LobbyRowView,
  LobbySignView,
  LobbyView,
} from "../../model";
import { Button, Hint, Kbd, Toast, cx, usePhone } from "../../ui";
import { Board } from "../board/Board";
import { GameProvider } from "../context";
import { LinkProvider } from "../link";
import { ShellFrame } from "../shell/ShellFrame";
import { keyIsFree } from "../shell/shortcuts";
import type { OtherGame } from "../shell/Toolbar";

/** A nickname has 64 bytes at most on the server. */
export const NICKNAME_MAX = 32;

const SIGNS: Record<LobbySignView["kind"], string> = {
  host: "★",
  rule: "⚙",
  random: "⚄",
  away: "◇",
  open: "",
};

/** The one sign of a row, in a column of fixed width. The word is on hover. */
function Sign({ sign }: { sign: LobbySignView | null }) {
  if (!sign) {
    return <span className="w-5 flex-none" />;
  }
  return (
    <span
      role="img"
      aria-label={sign.word}
      title={sign.word}
      className={cx(
        "grid w-5 flex-none place-items-center text-base leading-none",
        sign.kind === "away" ? "text-muted" : "text-accent",
      )}
    >
      {sign.kind === "open" ? (
        <i className="block size-3 rounded-full border border-dashed border-muted" />
      ) : (
        SIGNS[sign.kind]
      )}
    </span>
  );
}

function RowButton({
  view,
  dispatch,
}: {
  view: LobbyButtonView;
  dispatch: (intent: LobbyIntent) => void;
}) {
  return (
    <Button tone="quiet" size="sm" title={view.title} onClick={() => dispatch(view.intent)}>
      {view.label}
    </Button>
  );
}

function PlayerRow({
  row,
  dispatch,
}: {
  row: LobbyRowView;
  dispatch: (intent: LobbyIntent) => void;
}) {
  const who = (
    <>
      <Sign sign={row.sign} />
      {/* A name is cut on desktop, with the full name on hover; a phone gives it a second line. */}
      <span
        title={row.name}
        className={cx(
          "w-[300px] flex-none truncate",
          // An open seat has its buttons in the row: they go under the word when they have no room.
          row.sign?.kind === "open"
            ? "text-muted phone:w-auto"
            : "font-mid phone:w-auto phone:min-w-0 phone:flex-1 phone:whitespace-normal phone:[overflow-wrap:anywhere]",
        )}
      >
        {row.name}
        {row.you && <span className="font-normal text-muted"> (you)</span>}
        {row.become && <span className="font-normal text-accent"> (yours)</span>}
      </span>
      <span className="w-[180px] flex-none truncate text-muted phone:w-auto phone:max-w-[40%]">
        {row.faction}
      </span>
    </>
  );
  return (
    <li
      aria-current={row.you ? "true" : undefined}
      className={cx(
        "flex min-h-9 items-center gap-2.5 px-5 py-0.5 text-base phone:flex-wrap phone:px-3 phone:py-1.5 [li+&]:border-t [li+&]:border-line/50",
        row.you && "bg-cyan/[.06]",
      )}
    >
      {row.become ? (
        <button
          type="button"
          title="Play this seat in this tab"
          className="flex min-w-0 items-center gap-2.5 text-left hover:text-accent phone:flex-1"
          onClick={() => dispatch(row.become!)}
        >
          {who}
        </button>
      ) : (
        who
      )}
      <span className="ml-auto flex flex-none items-center gap-1.5">
        {row.buttons.map((button) => (
          <RowButton key={button.label} view={button} dispatch={dispatch} />
        ))}
        {row.remove && (
          <Button
            tone="quiet"
            size="icon"
            aria-label={row.remove.label}
            title={row.remove.label}
            onClick={() => dispatch(row.remove!.intent)}
          >
            ×
          </Button>
        )}
      </span>
    </li>
  );
}

function Heading({ children, aside }: { children: ReactNode; aside?: ReactNode }) {
  return (
    <div className="flex min-h-10 items-center gap-2 border-b border-line bg-surface py-1 pr-3 pl-5 phone:pl-3">
      <strong className="whitespace-nowrap">{children}</strong>
      {aside}
    </div>
  );
}

function Setup({ view }: { view: LobbyView }) {
  return (
    <section aria-label="Setup" className="flex-none">
      <Heading>Setup</Heading>
      <dl>
        {view.setup.map((row) => (
          <div
            key={row.id}
            className="flex min-h-9 items-center justify-between gap-3 px-5 py-0.5 text-base phone:px-3 [div+&]:border-t [div+&]:border-line/50"
          >
            <dt className="text-muted">{row.label}</dt>
            <dd className="min-w-0 truncate font-mid" title={row.value}>
              {row.value}
            </dd>
          </div>
        ))}
      </dl>
    </section>
  );
}

function Players({ view, dispatch }: { view: LobbyView; dispatch: (intent: LobbyIntent) => void }) {
  return (
    <section aria-label="Players" className="flex min-h-0 flex-1 flex-col">
      <Heading
        aside={
          <>
            <span className="text-sm text-muted tabular-nums">{view.filled}</span>
            {view.help.length > 0 && (
              <Hint label="About seats that the server plays">
                {view.help.map((item) => (
                  <span key={item.title} className="block">
                    <strong>{item.title}.</strong> {item.text}
                  </span>
                ))}
              </Hint>
            )}
            <span className="ml-auto" />
            {view.fill && <RowButton view={view.fill} dispatch={dispatch} />}
          </>
        }
      >
        Players
      </Heading>
      <ul className="min-h-0 flex-1 overflow-y-auto">
        {view.rows.map((row) => (
          <PlayerRow key={row.id} row={row} dispatch={dispatch} />
        ))}
      </ul>
    </section>
  );
}

/** What the main button does, and the main button. A phone shows it under every pane. */
function Footer({
  view,
  dispatch,
  nickname,
  onNickname,
  phone,
}: {
  view: LobbyView;
  dispatch: (intent: LobbyIntent) => void;
  nickname: string;
  onNickname: (name: string) => void;
  phone: boolean;
}) {
  const { footer } = view;
  if (!footer.note && !footer.main) {
    return null;
  }
  const nameless = footer.nickname && !nickname.trim();
  return (
    <form
      className="flex items-center justify-between gap-[15px] border-t border-line bg-surface px-5 py-2.5 phone:flex-col phone:items-stretch phone:gap-1.5 phone:px-3 phone:py-2"
      onSubmit={(event) => {
        event.preventDefault();
        if (footer.main && !footer.main.disabled && !nameless) {
          dispatch(footer.main.intent);
        }
      }}
    >
      <span id="lobby-note" role="status" className="min-w-0 text-sm text-muted">
        {footer.note}
      </span>
      <div className="flex shrink-0 items-center gap-2 phone:[&>button]:flex-auto">
        {footer.nickname && (
          <label className="flex min-w-0 items-center gap-2 text-sm text-muted phone:flex-1">
            Name
            <input
              value={nickname}
              maxLength={NICKNAME_MAX}
              onChange={(event) => onNickname(event.target.value)}
              className="min-h-9 w-[220px] min-w-0 rounded border border-line bg-well px-2.5 text-base text-text phone:w-auto phone:flex-1 touch:min-h-11"
            />
          </label>
        )}
        {footer.main && (
          <Button
            type="submit"
            tone="primary"
            disabled={footer.main.disabled || nameless}
            aria-describedby="lobby-note"
            aria-keyshortcuts="Enter"
            iconAfter="arrow"
          >
            {footer.main.label}
            {!phone && <Kbd>Enter</Kbd>}
          </Button>
        )}
      </div>
    </form>
  );
}

type Pane = "map" | "players" | "setup";

/**
 * The lobby of a table, in the frame of the game: the map that the table will play at the left,
 * the setup and the players in the panel, and the main button in the footer. It is one screen
 * with no dialog, and the same screen for the host, a player and a visitor: what a viewer may
 * change is in the view. A change applies at once.
 */
export function LobbyScreen({
  view,
  dispatch,
  nickname,
  onNickname,
  link,
  toast,
  other,
}: {
  view: LobbyView;
  dispatch: (intent: LobbyIntent) => void;
  /** The name that a visitor joins with. */
  nickname: string;
  onNickname: (name: string) => void;
  /** The address of the table, for the other players. */
  link: string;
  toast: { id: number; text: string } | null;
  /** The way out: the start screen. A page load. */
  other?: OtherGame;
}) {
  const phone = usePhone();
  const [pane, setPane] = useState<Pane>("players");
  const [copied, setCopied] = useState<{ id: number; text: string } | null>(null);

  // Enter is the main button, as in the game.
  const now = useRef({ view, dispatch, nickname });
  now.current = { view, dispatch, nickname };
  useEffect(() => {
    let focusByKeyboard = false;
    const onPointer = () => (focusByKeyboard = false);
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Tab") {
        focusByKeyboard = true;
      }
      const { main, nickname: named } = now.current.view.footer;
      if (event.key !== "Enter" || !main || main.disabled || !keyIsFree(event, focusByKeyboard)) {
        return;
      }
      if (named && !now.current.nickname.trim()) {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      now.current.dispatch(main.intent);
    };
    document.addEventListener("keydown", onKey, true);
    document.addEventListener("pointerdown", onPointer, true);
    return () => {
      document.removeEventListener("keydown", onKey, true);
      document.removeEventListener("pointerdown", onPointer, true);
    };
  }, []);

  // The board is the board of the game: of what it can do, a lobby has the look at a system.
  const onBoard = (intent: Intent) => {
    if (intent.type === "inspectSystem" || intent.type === "closeInspector") {
      dispatch(intent);
    }
  };
  const copy = () => {
    const say = (text: string) => setCopied({ id: performance.now(), text });
    navigator.clipboard.writeText(link).then(
      () => say("Link copied"),
      () => say(link),
    );
  };

  const footer = (
    <Footer
      view={view}
      dispatch={dispatch}
      nickname={nickname}
      onNickname={onNickname}
      phone={phone}
    />
  );
  // Until the server said the board there is none to frame.
  const board = view.board.tiles.length ? (
    <Board view={view.board} />
  ) : (
    <section aria-label="Galaxy board" className="min-h-0 min-w-0 bg-board" />
  );
  const players = <Players view={view} dispatch={dispatch} />;
  const setup = <Setup view={view} />;
  const latest = (copied?.id ?? 0) > (toast?.id ?? 0) ? copied : toast;

  return (
    <GameProvider seats={view.seats} dispatch={onBoard}>
      <LinkProvider>
        <ShellFrame
          accent="var(--color-cyan)"
          header={
            <header className="flex min-h-12 items-center gap-3.5 border-b border-line bg-surface px-4 py-1.5 phone:gap-x-2 phone:px-3 phone:pt-[max(6px,env(safe-area-inset-top))]">
              <div className="mr-auto flex min-w-0 flex-col text-xs leading-[1.3] text-muted">
                <strong className="truncate text-sm font-semibold text-text">{view.title}</strong>
                {view.status && (
                  <span role="status">
                    <i className="mr-1.5 mb-0.5 inline-block size-1.5 rounded-full bg-accent" />
                    {view.status}
                  </span>
                )}
              </div>
              <Button tone="quiet" size="sm" onClick={copy}>
                Copy link
              </Button>
              {other && (
                <Button tone="quiet" size="sm" onClick={() => location.assign(other.href)}>
                  {other.label}
                </Button>
              )}
            </header>
          }
          board={board}
          column={
            <>
              {setup}
              {players}
              {footer}
            </>
          }
          panes={[
            { id: "map", label: "Map", content: board },
            { id: "players", label: "Players", content: players, scrolls: true },
            { id: "setup", label: "Setup", content: setup, scrolls: true },
          ]}
          pane={pane}
          onPane={(id) => setPane(id as Pane)}
          footer={footer}
        >
          <Toast message={latest} />
        </ShellFrame>
      </LinkProvider>
    </GameProvider>
  );
}
