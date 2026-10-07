import React from "react";
import { getCardDisplayInfo } from "../presentation/cardDatabase.ts";

/** A promissory note id is `alias:faction`; card text is keyed by the alias. */
function cardLabelFor(label: string): string {
  const m = label.match(/^(Promissory Note|Action Card|Secret Objective):\s*([^:]+)(?::.*)?$/);
  return m ? `${m[1]}: ${m[2].trim()}` : label;
}

/** The printed card name when the card is known, keeping a non-generic owner (`ceasefire:sol` → "Ceasefire (sol)"). */
function displayName(label: string, known: string | undefined): string {
  const m = label.match(/^(Promissory Note|Action Card|Secret Objective):\s*[^:]+(?::(.*))?$/);
  if (!m || !known) return label;
  const owner = m[2]?.split(" for ")[0].trim();
  return owner && owner !== "generic" ? `${known} (${owner})` : known;
}

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
}> = ({ label, description, type, isCard, testId }) => {
  const cardInfo = getCardDisplayInfo(cardLabelFor(label));
  const displayDesc = description || cardInfo?.description;

  return (
    <div className={`trade-item trade-item--${type}`} data-testid={testId}>
      <div className="trade-item__header">
        <span className="trade-item__name">{displayName(label, cardInfo?.name)}</span>
        {cardInfo?.type && <span className="trade-item__badge">{cardInfo.type}</span>}
      </div>
      {displayDesc ? (
        <p className="trade-item__description">{displayDesc}</p>
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
