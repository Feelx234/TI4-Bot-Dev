import React, { useId, useMemo, useState } from "react";
import { Drawer } from "../primitives/index.ts";
import {
  IN_PLAY_HIDDEN_NOTE,
  filterInPlay,
  type InPlayGroup,
  type InPlayItem,
  type InPlayModel,
  type InPlaySection,
} from "../presentation/inPlay.ts";
import "./InPlayPanel.css";

/** Text longer than this is clamped to three lines with a "More" toggle. */
const CLAMP_CHARS = 150;

type Filter = InPlayGroup | "all";

const ItemCard: React.FC<{ item: InPlayItem }> = ({ item }) => {
  const [open, setOpen] = useState(false);
  const textId = useId();
  const body = item.text.join(" ");
  const long = body.length > CLAMP_CHARS || item.text.length > 1;
  return (
    <li className="in-play__item" data-testid="in-play-item">
      <div className="in-play__item-title">{item.title}</div>
      {item.meta.length > 0 && (
        <ul className="in-play__chips" aria-label="Details">
          {item.meta.map((chip) => (
            <li key={chip} className="in-play__chip">
              {chip}
            </li>
          ))}
        </ul>
      )}
      {body && (
        <div id={textId} className={`in-play__text${open ? "" : " in-play__text--clamped"}`} data-clamped={!open && long}>
          {item.text.map((paragraph, index) => (
            <p key={index}>{paragraph}</p>
          ))}
        </div>
      )}
      {long && (
        <button
          type="button"
          className="in-play__more"
          aria-expanded={open}
          aria-controls={textId}
          onClick={() => setOpen(!open)}
        >
          {open ? "Less" : "More"}
        </button>
      )}
    </li>
  );
};

const Section: React.FC<{ section: InPlaySection }> = ({ section }) => (
  <section className="in-play__section" data-testid={`in-play-section-${section.id}`} aria-labelledby={`in-play-h-${section.id}`}>
    <h3 className="in-play__section-head" id={`in-play-h-${section.id}`}>
      <span>{section.title}</span>
      {section.count !== null && (
        <span className="in-play__count" data-testid={`in-play-count-${section.id}`}>
          {section.count}
        </span>
      )}
    </h3>
    {section.newestFirst && section.items.length > 1 && <p className="in-play__hint">Newest first</p>}
    {section.facts.length > 0 && (
      <dl className="in-play__facts">
        {section.facts.map((fact) => (
          <React.Fragment key={fact.label}>
            <dt>{fact.label}</dt>
            <dd>{fact.value}</dd>
          </React.Fragment>
        ))}
      </dl>
    )}
    {section.items.length > 0 ? (
      <ul className="in-play__items">
        {section.items.map((item) => (
          <ItemCard key={item.id} item={item} />
        ))}
      </ul>
    ) : (
      section.empty && <p className="in-play__empty">{section.empty}</p>
    )}
  </section>
);

/** The body of the panel: filter chips, search, and the sections in groups. */
export const InPlayBody: React.FC<{ model: InPlayModel; onClose: () => void }> = ({ model, onClose }) => {
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const sections = useMemo(() => filterInPlay(model, { query, group: filter }), [model, query, filter]);
  const play = sections.filter((s) => s.group === "play");
  const discard = sections.filter((s) => s.group === "discard");
  const chips: Array<[Filter, string]> = [
    ["all", "All"],
    ["play", "In play"],
    ["discard", `Discard piles (${model.discards})`],
  ];
  return (
    <div className="in-play" data-testid="in-play-panel">
      <div className="in-play__head">
        <h2 className="in-play__title">In play</h2>
        <span className="in-play__summary">
          {model.laws} {model.laws === 1 ? "law" : "laws"} · {model.discards} discarded
        </span>
        <button type="button" className="button button--secondary in-play__close" data-testid="in-play-close" onClick={onClose}>
          Close
        </button>
      </div>
      <div className="in-play__tools">
        <div role="group" aria-label="Show" className="in-play__filters">
          {chips.map(([value, label]) => (
            <button
              key={value}
              type="button"
              className={`in-play__filter${filter === value ? " is-active" : ""}`}
              data-testid={`in-play-filter-${value}`}
              aria-pressed={filter === value}
              onClick={() => setFilter(value)}
            >
              {label}
            </button>
          ))}
        </div>
        <input
          type="search"
          className="in-play__search"
          data-testid="in-play-search"
          placeholder="Search laws, cards, players"
          aria-label="Search what is in play"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
        />
      </div>
      <div className="in-play__scroll" data-testid="in-play-scroll">
        {sections.length === 0 && <p className="in-play__empty">Nothing matches.</p>}
        <div className="in-play__groups">
          {play.length > 0 && (
            <div className="in-play__group in-play__group--play">
              {play.map((section) => (
                <Section key={section.id} section={section} />
              ))}
            </div>
          )}
          {discard.length > 0 && (
            <div className="in-play__group in-play__group--discard">
              {discard.map((section) => (
                <Section key={section.id} section={section} />
              ))}
            </div>
          )}
        </div>
        <p className="in-play__note" data-testid="in-play-note">
          {IN_PLAY_HIDDEN_NOTE}
        </p>
      </div>
    </div>
  );
};

/**
 * Phone: a full-height sheet. Desktop: a wide side panel that does not block the game, so it can
 * stay open while play goes on (Escape or Close closes it).
 */
export const InPlayPanel: React.FC<{ open: boolean; wide: boolean; model: InPlayModel; onClose: () => void }> = ({
  open,
  wide,
  model,
  onClose,
}) => {
  // The side panel starts under the game header, so the header's buttons (this one included) stay usable.
  const top = useMemo(() => {
    if (!open || !wide) return 0;
    const header = document.querySelector(".game-header");
    return header ? Math.max(0, Math.round(header.getBoundingClientRect().bottom)) : 0;
  }, [open, wide]);
  if (!open) return null;
  if (!wide)
    return (
      <Drawer open onClose={onClose} position="bottom" title="In play" data-testid="in-play-sheet" className="in-play-sheet">
        <InPlayBody model={model} onClose={onClose} />
      </Drawer>
    );
  return (
    <aside
      className="in-play-side"
      data-testid="in-play-side"
      aria-label="In play"
      style={{ "--in-play-top": `${top}px` } as React.CSSProperties}
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.stopPropagation();
          onClose();
        }
      }}
    >
      <InPlayBody model={model} onClose={onClose} />
    </aside>
  );
};
