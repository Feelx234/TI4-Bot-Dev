import React, { useEffect, useState } from "react";
import type { AgendaPeek } from "../presentation/agendaPeek.ts";
import "./ActionCardPickers.css";

/**
 * Insider Information: the top agendas of the deck, for the one player who looked. A corner card
 * the player can close; it comes back for a new peek and goes away when the top of the deck moves.
 */
export const AgendaPeekCard: React.FC<{ peek: AgendaPeek | null }> = ({ peek }) => {
  const [dismissed, setDismissed] = useState<string | null>(null);
  useEffect(() => {
    if (!peek) setDismissed(null);
  }, [peek]);
  if (!peek || dismissed === peek.key) return null;
  return (
    <aside className="peek" data-testid="agenda-peek" role="region" aria-label="Top of the agenda deck, only you can see this">
      <header className="peek__header">
        <div>
          <strong>Top of the agenda deck</strong>
          <div className="text-muted peek__private">Insider Information: only you can see this</div>
        </div>
        <button
          type="button"
          className="button button--secondary button--icon"
          data-testid="agenda-peek-close"
          aria-label="Close the agenda preview"
          onClick={() => setDismissed(peek.key)}
        >
          ×
        </button>
      </header>
      <ol className="peek__list">
        {peek.entries.map((entry) => (
          <li key={entry.place} className="peek__entry" data-testid="agenda-peek-entry">
            <div className="peek__name">
              <span className="peek__place" aria-label={entry.place === 1 ? "Next to be revealed" : `Number ${entry.place} in the deck`}>
                {entry.place}
              </span>
              <strong>{entry.name}</strong>
              {entry.tags.map((tag) => (
                <span key={tag} className="politics-panel__tag">
                  {tag}
                </span>
              ))}
            </div>
            {entry.text.map((line) => (
              <p key={line} className="politics-panel__text">
                {line}
              </p>
            ))}
          </li>
        ))}
      </ol>
    </aside>
  );
};
