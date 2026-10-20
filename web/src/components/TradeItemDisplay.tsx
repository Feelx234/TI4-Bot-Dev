import React, { useId, useState } from "react";
import { promissoryNoteMeta } from "../protocol/contentCatalog.ts";
import { getCardDisplayInfo } from "../presentation/cardDatabase.ts";
import { printedItemLabel } from "../presentation/tradeNames.ts";

/** A promissory note id is `alias:faction`; card text is keyed by the alias. */
function cardLabelFor(label: string): string {
  const m = label.match(/^(Promissory Note|Action Card|Secret Objective):\s*([^:]+)(?::.*)?$/);
  return m ? `${m[1]}: ${m[2].trim()}` : label;
}

/** Card text longer than this is clamped to three lines with a More toggle. */
const CLAMP_CHARS = 140;

const NOTE_LABEL = /^Promissory Note:\s*(.+)$/;

/** The id a note label names; Support for the Throne without an owner takes `issuerFaction`. */
function noteIdOf(label: string, issuerFaction: string | undefined): string | null {
  const id = label.match(NOTE_LABEL)?.[1]?.trim();
  if (!id) return null;
  const meta = promissoryNoteMeta(id);
  return meta.support && !meta.issuer && issuerFaction ? `support:${issuerFaction}` : id;
}

const ClampedText: React.FC<{ text: string; className: string }> = ({ text, className }) => {
  const [open, setOpen] = useState(false);
  const textId = useId();
  const long = text.length > CLAMP_CHARS;
  return (
    <>
      <p id={textId} className={`${className}${long && !open ? " trade-item__description--clamped" : ""}`}>
        {text}
      </p>
      {long && (
        // A span, not a button: trade items sit inside the desk's toggle buttons.
        <span
          role="button"
          tabIndex={0}
          className="trade-item__more"
          aria-expanded={open}
          aria-controls={textId}
          onClick={(e) => {
            e.stopPropagation();
            setOpen((o) => !o);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              e.stopPropagation();
              setOpen((o) => !o);
            }
          }}
        >
          {open ? "Less" : "More"}
        </span>
      )}
    </>
  );
};

/**
 * Render a trade item with its description (card text, amount, ...).
 * `kind` marks items that are cards, so a card with no known text says so instead of staying silent.
 */
export const TradeItemDisplay: React.FC<{
  label: string;
  description?: string;
  type: "offer" | "receive";
  isCard?: boolean;
  testId?: string;
  /** The faction that issued the note, for Support for the Throne staged without an owner. */
  issuerFaction?: string;
  /** Who holds the item now ("You", a seat label); shown under a promissory note. */
  holder?: string;
}> = ({ label, description, type, isCard, testId, issuerFaction, holder }) => {
  const noteId = noteIdOf(label, issuerFaction);
  const note = noteId ? promissoryNoteMeta(noteId) : null;
  const cardInfo = note ? null : getCardDisplayInfo(cardLabelFor(label));
  const shown = note ? note.title : printedItemLabel(label);
  const displayDesc = description || (note ? note.text : cardInfo?.description);

  return (
    <div className={`trade-item trade-item--${type}`} data-testid={testId}>
      <div className="trade-item__header">
        <span className="trade-item__name" title={shown !== label && !note?.concealed ? label : undefined}>
          {shown}
        </span>
        {note && <span className="trade-item__badge">Promissory Note</span>}
        {cardInfo?.type && <span className="trade-item__badge">{cardInfo.type}</span>}
      </div>
      {note && holder && (
        <div className="trade-item__meta" data-testid="trade-item-holder">
          <span className="trade-item__meta-label">Held by:</span>
          <span>{holder}</span>
        </div>
      )}
      {displayDesc ? (
        <ClampedText text={displayDesc} className="trade-item__description" />
      ) : (
        isCard && <p className="trade-item__description">Card text is not available in this view.</p>
      )}
      {cardInfo?.phase && (
        <div className="trade-item__meta">
          <span className="trade-item__meta-label">Phase:</span>
          <span>{cardInfo.phase}</span>
        </div>
      )}
    </div>
  );
};
