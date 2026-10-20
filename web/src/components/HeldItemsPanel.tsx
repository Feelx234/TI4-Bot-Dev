import React, { useEffect, useId, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { BoardView, ChoiceOption, PlayerView } from "../protocol/types.ts";
import { overlayStack } from "../primitives/index.ts";
import { usePlayerIdentity } from "../presentation/PlayerIdentity.tsx";
import { usePublicState } from "../presentation/PublicStateContext.tsx";
import { buildHeldItems, type HeldItem, type HeldItemsModel } from "../presentation/heldItems.ts";
import "./HeldItemsPanel.css";

/** Text longer than this is clamped to three lines with a Show more button. */
const CLAMP_CHARS = 150;

const ItemCard: React.FC<{ item: HeldItem }> = ({ item }) => {
  const [open, setOpen] = useState(false);
  const textId = useId();
  const long = (item.text?.length ?? 0) > CLAMP_CHARS;
  return (
    <li className="held-item" data-testid={`held-item-${item.key}`} data-usable={item.usableNow ? "true" : "false"}>
      <div className="held-item__head">
        <strong className="held-item__name">{item.name}</strong>
        <span className="held-item__chips">
          {item.usableNow && (
            <span className="held-chip held-chip--usable" title="The Components menu offers this right now">
              usable now
            </span>
          )}
          {item.chips.map((c) => (
            <span key={c.label} className={`held-chip held-chip--${c.tone}`} data-tone={c.tone}>
              {c.label}
            </span>
          ))}
        </span>
      </div>
      {(item.kind || item.owner) && (
        <div className="held-item__meta">
          {item.kind}
          {item.kind && item.owner ? " · " : ""}
          {item.owner ? `from ${item.owner}` : ""}
        </div>
      )}
      {item.text && (
        <>
          <p
            id={textId}
            className={`held-item__text${long && !open ? " held-item__text--clamped" : ""}`}
          >
            {item.text}
          </p>
          {long && (
            <button
              type="button"
              className="held-item__more"
              aria-expanded={open}
              aria-controls={textId}
              onClick={() => setOpen((o) => !o)}
            >
              {open ? "Show less" : "Show more"}
            </button>
          )}
        </>
      )}
    </li>
  );
};

export const HeldItemsBody: React.FC<{ model: HeldItemsModel }> = ({ model }) => {
  if (model.sections.length === 0) {
    return (
      <p className="held-items__empty" data-testid="held-items-empty">
        Nothing held yet: no relics, fragments, exhaustable technologies, notes or legendary abilities.
      </p>
    );
  }
  return (
    <div className="held-items__sections">
      {model.sections.map((section) => (
        <section key={section.id} className="held-section" data-testid={`held-section-${section.id}`}>
          <h3 className="held-section__title">
            {section.title} <span className="held-section__count">{section.count}</span>
          </h3>
          <ul className="held-section__list">
            {section.items.map((item) => (
              <ItemCard key={item.key} item={item} />
            ))}
          </ul>
        </section>
      ))}
    </div>
  );
};

export interface HeldItemsPanelProps {
  players: readonly PlayerView[];
  viewer?: string;
  board?: BoardView | null;
  /** The options of the pending choice, to mark components as usable now. */
  options?: readonly Pick<ChoiceOption, "id" | "label" | "kind">[];
  initialSubject?: string;
  onClose: () => void;
}

/**
 * The Held items panel: relics, promissory notes, exhaustable technologies, leaders, legendary
 * abilities and the rest a seat holds besides action cards and secret objectives. A full-height
 * sheet on phones, a wide panel docked beside the player column on desktop that can stay open.
 * Another seat is shown read-only with public information only.
 */
export const HeldItemsPanel: React.FC<HeldItemsPanelProps> = ({
  players,
  viewer,
  board,
  options,
  initialSubject,
  onClose,
}) => {
  const display = usePlayerIdentity();
  const { held, breakthroughs, thundersEdge } = usePublicState();
  const [subject, setSubject] = useState(initialSubject ?? viewer ?? players[0]?.id ?? "");
  const ref = useRef<HTMLElement | null>(null);
  const titleId = useId();

  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    ref.current?.focus();
    const unregister = overlayStack.register({ id: "held-items", modal: false, onDismiss: onClose, element: ref.current });
    return () => {
      unregister();
      previous?.focus?.();
    };
  }, [onClose]);

  const model = useMemo(
    () =>
      buildHeldItems({
        players,
        subject,
        viewer,
        board,
        held,
        breakthrough: breakthroughs[subject] ?? null,
        breakthroughsOn: thundersEdge,
        options,
        label: (id) => display(id).label,
      }),
    [players, subject, viewer, board, held, breakthroughs, thundersEdge, options, display],
  );
  const ordered = useMemo(
    () => [...players].sort((a, b) => Number(b.id === viewer) - Number(a.id === viewer)),
    [players, viewer],
  );

  return createPortal(
    <section
      ref={ref}
      role="dialog"
      aria-modal="false"
      aria-labelledby={titleId}
      tabIndex={-1}
      className="held-items"
      data-testid="held-items-panel"
      data-own={model.own ? "true" : "false"}
    >
      <header className="held-items__header">
        <h2 id={titleId} className="held-items__title">
          Held items
        </h2>
        <span className="held-items__mode" data-testid="held-items-mode">
          {model.own ? "Yours" : "Read-only, public information only"}
        </span>
        <button type="button" className="button button--secondary held-items__close" data-testid="held-items-close" onClick={onClose}>
          Close
        </button>
      </header>
      {ordered.length > 1 && (
        <div className="held-items__switcher" role="tablist" aria-label="Whose items">
          {ordered.map((p) => {
            const identity = display(p.id);
            return (
              <button
                key={p.id}
                type="button"
                role="tab"
                aria-selected={p.id === subject}
                data-testid={`held-items-seat-${players.indexOf(p)}`}
                className="held-items__seat"
                style={{ borderColor: identity.color }}
                onClick={() => setSubject(p.id)}
              >
                {identity.label}
                {p.id === viewer && !/^you\b/i.test(identity.label) ? " (you)" : ""}
              </button>
            );
          })}
        </div>
      )}
      <div className="held-items__scroll">
        <HeldItemsBody model={model} />
        <p className="held-items__foot">
          Action cards and secret objectives stay in the player sheet. Use components from the turn menu.
        </p>
      </div>
    </section>,
    document.body,
  );
};

/** The count the entry button shows for a seat. */
export function useHeldItemsCount(
  players: readonly PlayerView[],
  subject: string,
  viewer: string | undefined,
  board?: BoardView | null,
): number {
  const { held, breakthroughs, thundersEdge } = usePublicState();
  return useMemo(
    () => buildHeldItems({ players, subject, viewer, board, held, breakthrough: breakthroughs[subject] ?? null, breakthroughsOn: thundersEdge }).total,
    [players, subject, viewer, board, held, breakthroughs, thundersEdge],
  );
}

/** The entry button next to a seat's card counts: opens the panel on that seat, with a count badge. */
export const HeldItemsButton: React.FC<{
  players: readonly PlayerView[];
  subject: string;
  viewer?: string;
  board?: BoardView | null;
  expanded: boolean;
  onOpen: (subject: string) => void;
}> = ({ players, subject, viewer, board, expanded, onOpen }) => {
  const count = useHeldItemsCount(players, subject, viewer, board);
  return (
    <button
      type="button"
      className="button button--secondary held-items-button"
      data-testid={`held-items-button-${players.findIndex((p) => p.id === subject)}`}
      aria-haspopup="dialog"
      aria-expanded={expanded}
      aria-label={`Held items: ${count}`}
      onClick={() => onOpen(subject)}
    >
      Held items
      <span className="held-items-button__badge" data-testid="held-items-count">
        {count}
      </span>
    </button>
  );
};
