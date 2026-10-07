import React from "react";
import type { PendingChoiceDto } from "../protocol/types.ts";
import { describeExploreCard } from "../presentation/exploreReward.ts";

/** Above the options of an exploration card's reward choice: the card as printed. */
export const ExploreRewardPanel: React.FC<{ choice: PendingChoiceDto }> = ({ choice }) => {
  const card = describeExploreCard(choice);
  if (!card) return null;
  return (
    <div className="politics-panel" data-testid="explore-card">
      <div className="politics-panel__title" data-testid="explore-card-name">
        {card.name}
        <span className="politics-panel__tag">{card.type} exploration card</span>
      </div>
      {card.text.map((line) => (
        <p key={line} className="politics-panel__text" data-testid="explore-card-text">
          {line}
        </p>
      ))}
    </div>
  );
};
