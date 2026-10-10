import { useEffect, useRef, useState } from "react";
import type {
  ExplorationDeck,
  ExplorationDeckView,
  ListSectionView,
  PlayerRowView,
  ReferenceView,
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

/** The cards that are played in a reaction window. The setting belongs to the card. */
function OfferCards({ cards }: { cards: ReferenceView["offerCards"] }) {
  const dispatch = useDispatch();
  if (!cards.length) {
    return null;
  }
  return (
    <div>
      <h3 className="mt-3.5 text-muted">
        <Eyebrow>Reaction cards · offer</Eyebrow>
      </h3>
      {cards.map((item) => (
        <div key={item.card} className="mt-2 flex items-start justify-between gap-3">
          <span>
            <strong>{item.card}</strong>
            <span className="block text-xs text-muted">{item.text}</span>
          </span>
          <Button
            size="sm"
            tone="quiet"
            active={item.never}
            aria-pressed={item.never}
            aria-label={`Never offer ${item.card}`}
            onClick={() => dispatch({ type: "setCardOffer", card: item.card, never: !item.never })}
          >
            {item.never ? "Never offered" : "Offered"}
          </Button>
        </div>
      ))}
    </div>
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
              {card.left === 0 && "– "}
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

function playerSections(row: PlayerRowView): ListSectionView[] {
  return [
    { title: "Faction abilities", rows: row.abilities.map((text) => [text]) },
    { title: "Technologies", rows: row.technologies.map((text) => [text]) },
    { title: "Leaders", rows: [[row.leaders]] },
    {
      title: "Relics and fragments",
      rows: [
        ...row.relics.map((name) => [name]),
        ...row.fragments.map((item) => [`${plural(item.count, `${item.deck} relic fragment`)}`]),
        ...(row.relics.length || row.fragments.length ? [] : [["None"]]),
      ],
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
      ) : (
        <Sections sections={view[key]} />
      )}
      {key === "cards" && <OfferCards cards={view.offerCards} />}
    </Drawer>
  );
}
