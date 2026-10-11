import { type ReactNode, useEffect, useRef, useState } from "react";
import type {
  ActionCardsView,
  ComponentCardView,
  ExplorationDeck,
  ExplorationDeckView,
  ListSectionView,
  PlayerRowView,
  ReferenceView,
  SecretsView,
} from "../../model";
import { plural } from "../../model";
import { Button, Drawer, Eyebrow, Icon, cx } from "../../ui";
import { TYPE_COLOR } from "../board/PlanetTraits";
import { RichText, SeatSymbol, useDispatch, useSeat, useSeats } from "../context";

function Sections({ sections }: { sections: ListSectionView[] }) {
  return (
    <>
      {sections.map((section) => (
        <div key={section.title}>
          <h3 className="mt-3.5 text-muted">
            <Eyebrow>{section.title}</Eyebrow>
          </h3>
          <ul className="mt-1 list-disc pl-[18px]">
            {section.rows.map((row, index) => (
              <li key={index} className="mt-[3px]">
                <RichText value={row} />
              </li>
            ))}
          </ul>
        </div>
      ))}
    </>
  );
}

function Log({ log }: { log: ReferenceView["log"] }) {
  const dispatch = useDispatch();
  const seats = useSeats();
  const [open, setOpen] = useState<Record<string, boolean>>(() =>
    Object.fromEntries(
      log.filter((round) => round.current).map((round) => ["r" + round.round, true]),
    ),
  );
  const toggle = (id: string) => setOpen((now) => ({ ...now, [id]: !now[id] }));
  return (
    <>
      {log.map((round) => (
        <div key={round.round}>
          <button
            type="button"
            className="mt-3 block w-full border-b border-line py-1.5 text-left font-strong"
            aria-expanded={!!open["r" + round.round]}
            onClick={() => toggle("r" + round.round)}
          >
            {open["r" + round.round] ? "▾" : "▸"} {round.label}
          </button>
          {open["r" + round.round] &&
            round.entries.map((entry) => (
              <div
                key={entry.id}
                className={cx(
                  "border-b border-line/50 py-2",
                  entry.current && "border-l-2 border-l-cyan pl-2",
                )}
              >
                <div className="flex flex-col gap-px">
                  <span>
                    <SeatSymbol seat={entry.seat} /> <strong>{seats[entry.seat].name}</strong> ·{" "}
                    {entry.type}
                  </span>
                  <span className="text-muted">{entry.summary}</span>
                </div>
                <div className="mt-[5px] flex gap-1.5">
                  {entry.current ? (
                    <span className="text-muted">Shown in Live</span>
                  ) : (
                    <>
                      <Button
                        tone="quiet"
                        size="sm"
                        aria-expanded={!!open[entry.id]}
                        onClick={() => toggle(entry.id)}
                      >
                        Steps
                      </Button>
                      <Button
                        size="sm"
                        onClick={() => dispatch({ type: "inspectLogEntry", id: entry.id })}
                      >
                        Inspect
                      </Button>
                    </>
                  )}
                </div>
                {open[entry.id] && (
                  <ol className="mt-1 list-decimal pl-[18px]">
                    {entry.stages.map((stage) => (
                      <li key={stage.name} className="mt-[3px]">
                        <strong>{stage.name}</strong> · {stage.text}
                      </li>
                    ))}
                  </ol>
                )}
              </div>
            ))}
        </div>
      ))}
    </>
  );
}

/** A card as printed: the name, what it is, when it is used and the text. Dense, with the frame of a card. */
function PrintedCard({
  name,
  kind,
  window,
  text,
  aside,
  className,
  children,
}: {
  name: ReactNode;
  kind?: string;
  window?: string;
  text?: string;
  /** At the right of the name: the state of the card, or its control. */
  aside?: ReactNode;
  className?: string;
  children?: ReactNode;
}) {
  return (
    <div className={cx("mt-1.5 rounded border border-line px-2.5 py-1.5", className)}>
      <div className="flex items-baseline justify-between gap-3">
        <strong className="font-mid">{name}</strong>
        {aside != null && <span className="flex-none text-xs">{aside}</span>}
      </div>
      {kind && <div className="text-xs text-faint">{kind}</div>}
      {(window || text) && (
        <div className="mt-0.5 text-xs whitespace-pre-line text-muted">
          {window && <span className="text-text">{window} </span>}
          {text}
        </div>
      )}
      {children}
    </div>
  );
}

/** The heading of a group of a deck. It opens and closes the group, as a round of the log. */
function GroupHeading({
  open,
  onToggle,
  name,
  count,
}: {
  open: boolean;
  onToggle: () => void;
  name: string;
  /** How many the group has, or how many of them are unseen. */
  count: ReactNode;
}) {
  return (
    <h4>
      <button
        type="button"
        className="mt-3 flex w-full items-baseline gap-2 border-b border-line py-1.5 text-left font-strong"
        aria-expanded={open}
        onClick={onToggle}
      >
        <span aria-hidden="true">{open ? "▾" : "▸"}</span>
        {name}
        <span className="text-xs font-normal text-muted tabular-nums">{count}</span>
      </button>
    </h4>
  );
}

/**
 * The action cards: the hand of the viewer against the limit, as printed, and every card of the
 * deck with where its copies are, in groups by when a card is played that open with a click. "Never offered" and "Fake"
 * are settings of a card that a window offers.
 */
function ActionCards({ view }: { view: ActionCardsView }) {
  const dispatch = useDispatch();
  const over = view.limit !== null && view.held > view.limit;
  // The groups of the deck are closed until the player opens one, as the rounds of the log.
  const [open, setOpen] = useState<Record<string, boolean>>({});
  const toggle = (group: string) => setOpen((now) => ({ ...now, [group]: !now[group] }));
  return (
    <>
      <section aria-label="Your action cards">
        <h3 className="mt-3.5 flex items-baseline justify-between gap-3 text-muted">
          <Eyebrow>Your action cards</Eyebrow>
          <span
            className={cx("tabular-nums", over ? "text-red" : "text-text")}
            title="Action cards in your hand / your hand limit"
          >
            {over && "⚠ "}
            Hand {view.held} / {view.limit ?? "no limit"}
          </span>
        </h3>
        {view.hand.length === 0 && <div className="mt-1 text-xs text-muted">None</div>}
        {view.hand.map((card) => (
          <article key={card.name} aria-label={card.name}>
            <PrintedCard
              name={
                <>
                  {card.name}
                  {card.copies > 1 && (
                    <span className="text-muted tabular-nums"> ×{card.copies}</span>
                  )}
                </>
              }
              window={card.window}
              text={card.text}
              aside={
                card.never !== null && (
                  <Button
                    size="sm"
                    tone="quiet"
                    active={card.never}
                    aria-pressed={card.never}
                    aria-label={`Never offer ${card.name}`}
                    title="A window that offers only cards that are never offered is passed without a question"
                    onClick={() =>
                      dispatch({ type: "setCardOffer", card: card.name, never: !card.never })
                    }
                  >
                    {card.never ? "Never offered" : "Offered"}
                  </Button>
                )
              }
            />
          </article>
        ))}
        {view.faked.map((card) => (
          <article key={card.name} aria-label={`${card.name}, faked`}>
            <PrintedCard
              className="border-dashed"
              name={card.name}
              kind="Faked · not in your hand"
              window={card.window}
              text={card.text}
              aside={
                <Button
                  size="sm"
                  tone="quiet"
                  onClick={() => dispatch({ type: "setCardFake", card: card.name, fake: false })}
                >
                  Stop faking
                </Button>
              }
            />
          </article>
        ))}
      </section>
      <section aria-label="Action card deck">
        <h3 className="mt-5 flex flex-wrap items-baseline justify-between gap-x-3 text-muted">
          <Eyebrow>Action card deck</Eyebrow>
          {view.deck && (
            <span className="tabular-nums">
              {view.deck.left} of {plural(view.deck.total, "card")} left · {view.deck.discarded}{" "}
              discarded
            </span>
          )}
        </h3>
        {view.reference.map((group) => (
          <section key={group.group} aria-label={group.group}>
            <GroupHeading
              open={!!open[group.group]}
              onToggle={() => toggle(group.group)}
              name={group.group}
              count={group.cards.length}
            />
            {open[group.group] &&
              group.cards.map((card) => (
                <div
                  key={card.name}
                  className={cx(
                    "mt-2 border-t border-line/50 pt-1.5",
                    card.open === 0 && "text-muted",
                  )}
                >
                  <div className="flex items-baseline justify-between gap-3">
                    <strong className="font-mid">{card.name}</strong>
                    <span className="flex flex-none items-baseline gap-2 text-xs tabular-nums">
                      <span title="Not discarded and not in your hand: in the deck or in the hand of another player / in the full deck">
                        {card.open} of {card.total} unseen
                      </span>
                      {/* A component action is offered by no window: there is nothing to fake. */}
                      {!card.action && card.held === 0 && card.open > 0 && (
                        <Button
                          size="sm"
                          tone="quiet"
                          active={card.faked}
                          aria-pressed={card.faked}
                          aria-label={`Fake ${card.name}`}
                          title="Act as if you hold this card"
                          onClick={() =>
                            dispatch({ type: "setCardFake", card: card.name, fake: !card.faked })
                          }
                        >
                          {card.faked ? "Faked" : "Fake"}
                        </Button>
                      )}
                    </span>
                  </div>
                  {(card.held > 0 || card.discarded > 0) && (
                    <div className="text-xs tabular-nums">
                      {[
                        card.held > 0 && `${card.held} in your hand`,
                        card.discarded > 0 && `${card.discarded} discarded`,
                      ]
                        .filter(Boolean)
                        .join(" · ")}
                    </div>
                  )}
                  <div className="text-xs text-muted">
                    <span className={card.open === 0 ? undefined : "text-text"}>
                      {card.window}{" "}
                    </span>
                    {card.text}
                  </div>
                </div>
              ))}
          </section>
        ))}
      </section>
    </>
  );
}

/**
 * The secret objectives: those of the viewer, scored or not, and every objective of the deck with
 * where it is, in groups by what an objective asks for that open with a click.
 */
function Secrets({ view }: { view: SecretsView }) {
  const seats = useSeats();
  const [open, setOpen] = useState<Record<string, boolean>>({});
  const toggle = (group: string) => setOpen((now) => ({ ...now, [group]: !now[group] }));
  const have = view.held + view.scored;
  return (
    <>
      <section aria-label="Your secret objectives">
        <h3 className="mt-3.5 flex items-baseline justify-between gap-3 text-muted">
          <Eyebrow>Your secret objectives</Eyebrow>
          <span
            className="text-text tabular-nums"
            title="Secret objectives that you scored · scored and unscored together / the limit"
          >
            {view.scored} scored · {have}
            {view.limit !== null && ` / ${view.limit}`}
          </span>
        </h3>
        {view.mine.length === 0 && <div className="mt-1 text-xs text-muted">None</div>}
        {view.mine.map((card) => (
          <article key={card.name} aria-label={card.name}>
            <PrintedCard
              name={card.name}
              kind={card.phase}
              text={card.text}
              aside={card.scored ? "✓ Scored" : "Not scored"}
            />
          </article>
        ))}
      </section>
      <section aria-label="Secret objective deck">
        <h3 className="mt-5 flex flex-wrap items-baseline justify-between gap-x-3 text-muted">
          <Eyebrow>Secret objective deck</Eyebrow>
          <span
            className="tabular-nums"
            title="Not in your hand and not scored: in the deck or in the hand of another player"
          >
            {view.unseen} of {view.total} unseen
            {view.deck !== null && ` · ${view.deck} in the deck`}
          </span>
        </h3>
        {view.reference.map((group) => (
          <section key={group.group} aria-label={group.group}>
            <GroupHeading
              open={!!open[group.group]}
              onToggle={() => toggle(group.group)}
              name={group.group}
              count={`${group.unseen} of ${group.cards.length} unseen`}
            />
            {open[group.group] &&
              group.cards.map((card) => (
                <div
                  key={card.name}
                  className={cx(
                    "mt-2 border-t border-line/50 pt-1.5",
                    card.state !== "unseen" && "text-muted",
                  )}
                >
                  <div className="flex items-baseline justify-between gap-3">
                    <strong className="font-mid">{card.name}</strong>
                    <span className="flex flex-none items-baseline gap-1 text-xs">
                      {card.state === "unseen" ? (
                        "Unseen"
                      ) : card.mine ? (
                        card.state === "scored" ? (
                          "✓ Scored by you"
                        ) : (
                          "In your hand"
                        )
                      ) : (
                        <>
                          {card.state === "scored" ? "✓ Scored by" : "In the hand of"}
                          {card.by && <SeatSymbol seat={card.by} />}
                          {card.by && seats[card.by]?.faction}
                        </>
                      )}
                    </span>
                  </div>
                  <div className="text-xs text-muted">
                    <span className={card.state === "unseen" ? "text-text" : undefined}>
                      {card.phase}:{" "}
                    </span>
                    {card.text}
                  </div>
                </div>
              ))}
          </section>
        ))}
      </section>
    </>
  );
}

/** One exploration deck: what is left of it, who has its fragments, and each card as printed. */
function Deck({ view, focus }: { view: ExplorationDeckView; focus: boolean }) {
  const seats = useSeats();
  const root = useRef<HTMLElement>(null);
  // The deck that the player asked for is on top of the sheet.
  useEffect(() => {
    if (focus) {
      root.current?.scrollIntoView({ block: "start" });
    }
  }, [focus]);
  return (
    <section ref={root} aria-label={`${view.title} deck`} className="scroll-mt-1 pt-3.5">
      <h3 className="flex items-baseline gap-2" style={{ color: TYPE_COLOR[view.deck] }}>
        {view.deck !== "frontier" && <Icon name={view.deck} className="size-4 self-center" />}
        <strong className="text-base font-semibold">{view.title}</strong>
        <span className="text-muted tabular-nums">
          {view.left} of {plural(view.total, "card")} left
        </span>
      </h3>
      {view.fragments.length > 0 && (
        <div className="mt-1 flex flex-wrap gap-x-3 text-xs text-muted">
          Relic fragments
          {view.fragments.map((item) => (
            <span key={item.seat} className="inline-flex items-baseline gap-1 text-text">
              <SeatSymbol seat={item.seat} />
              {seats[item.seat]?.faction} {item.count}
            </span>
          ))}
        </div>
      )}
      {view.cards.map((card) => (
        <div
          key={card.name}
          className={cx("mt-2 border-t border-line/50 pt-1.5", card.left === 0 && "text-muted")}
        >
          <div className="flex items-baseline justify-between gap-3">
            <strong className="font-mid">{card.name}</strong>
            <span className="flex-none tabular-nums" title="Left in the deck / in the full deck">
              {card.left} / {card.total}
            </span>
          </div>
          <div className="text-xs text-muted">
            {card.kind && card.kind !== "Instant" && <span>{card.kind} · </span>}
            {card.text}
          </div>
          {card.drawn.map((line, index) => (
            <div key={index} className="text-xs text-faint">
              Drawn · {line}
            </div>
          ))}
        </div>
      ))}
    </section>
  );
}

const STATES = { ready: "Ready", exhausted: "Exhausted", locked: "Locked", purged: "Purged" };

/** The cards of one kind in the play area of a player, as printed. A card that cannot be used now is dimmed. */
function ComponentCards({ title, cards }: { title: string; cards: ComponentCardView[] }) {
  return (
    <section aria-label={title}>
      <h3 className="mt-3.5 text-muted">
        <Eyebrow>{title}</Eyebrow>
      </h3>
      {cards.length === 0 && <div className="mt-1 text-xs text-muted">None</div>}
      {cards.map((card, index) => (
        <article key={index} aria-label={card.name}>
          <PrintedCard
            className={cx(card.state && card.state !== "ready" && "opacity-55")}
            name={card.name}
            kind={card.kind}
            window={card.window}
            text={card.text}
            aside={card.state && STATES[card.state]}
          >
            {card.unlock && (
              <div className="mt-0.5 text-xs">
                <span className="text-muted">Unlock: </span>
                {card.unlock}
              </div>
            )}
          </PrintedCard>
        </article>
      ))}
    </section>
  );
}

function playerSections(row: PlayerRowView): ListSectionView[] {
  return [
    { title: "Technologies", rows: row.technologies.map((text) => [text]) },
    {
      title: "Relic fragments",
      rows: row.fragments.length
        ? row.fragments.map((item) => [`${plural(item.count, `${item.deck} relic fragment`)}`])
        : [["None"]],
    },
    {
      title: "Economy",
      rows: [
        [`Resources ${row.resources.join(" / ")} · Influence ${row.influence.join(" / ")}`],
        [`Trade goods ${row.tradeGoods} · Commodities ${row.commodities.join(" / ")}`],
        [`Command tokens ${row.tokens.join(" · ")} (tactic · fleet · strategy)`],
        [
          `${row.actionCards} action cards · ${row.secrets[1]} secret objectives, ${row.secrets[0]} scored`,
        ],
      ],
    },
  ];
}

const TITLES = {
  objectives: "Objectives",
  technology: "Technology",
  cards: "Cards",
  secrets: "Secrets",
  exploration: "Exploration",
  log: "Log",
} as const;

/** Reference sheets that open over the board: objectives, technology, cards, the log, a player sheet. */
export function ReferenceDrawer({
  id,
  view,
  players,
  deck,
  onClose,
}: {
  /** The exploration deck that the sheet was opened for. */
  deck?: ExplorationDeck | null;
  id: string;
  view: ReferenceView;
  players: PlayerRowView[];
  onClose: () => void;
}) {
  const player = id.startsWith("player:")
    ? players.find((row) => row.seat === id.slice(7))
    : undefined;
  const seat = useSeat(player?.seat ?? "");
  if (player) {
    return (
      <Drawer title={`${seat.name} · ${seat.faction}`} onClose={onClose}>
        <ComponentCards title="Faction abilities" cards={player.abilities} />
        <ComponentCards title="Leaders" cards={player.leaders} />
        <ComponentCards title="Action components" cards={player.components} />
        <Sections sections={playerSections(player)} />
      </Drawer>
    );
  }
  const key = id as keyof typeof TITLES;
  return (
    <Drawer title={TITLES[key]} onClose={onClose}>
      {key === "log" ? (
        <Log log={view.log} />
      ) : key === "exploration" ? (
        view.exploration.map((item) => (
          <Deck key={item.deck} view={item} focus={item.deck === deck} />
        ))
      ) : key === "secrets" ? (
        view.secrets && <Secrets view={view.secrets} />
      ) : (
        <Sections sections={view[key]} />
      )}
      {key === "cards" && view.actionCards && <ActionCards view={view.actionCards} />}
    </Drawer>
  );
}
