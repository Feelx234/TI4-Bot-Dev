import React from "react";
import { unavailableFeatures } from "./capabilities.ts";
import { feedbackChipText, useFeedbackQueue } from "./useFeedbackQueue.ts";
import type { Capabilities, OfflineFeedbackQueue } from "./types.ts";

/**
 * Slim strip shown above an offline game: says the game is local, lists what is not available
 * (driven by the capability flags, so it never lies about a feature that gets added later) and
 * shows the queued-feedback state.
 */
export const OfflineBanner: React.FC<{
  capabilities: Capabilities;
  feedback: OfflineFeedbackQueue | null;
  onRetryFeedback?: () => void;
}> = ({ capabilities, feedback, onRetryFeedback }) => {
  const missing = unavailableFeatures(capabilities).filter((x) => x !== "Instant feedback delivery");
  const chip = feedbackChipText(useFeedbackQueue(feedback));
  return (
    <div className="offline-banner" role="status" data-testid="offline-banner">
      <strong className="offline-banner__tag">Offline game</strong>
      <span className="offline-banner__text">Runs on this device against rule bots.</span>
      {missing.length > 0 && (
        <span className="offline-banner__missing" data-testid="offline-missing">
          Not available offline: {missing.join(", ")}.
        </span>
      )}
      {chip && (
        <span className="offline-banner__queue" data-testid="offline-feedback-chip">
          {chip}
          {onRetryFeedback && chip.includes("could not") && (
            <button type="button" className="offline-link" onClick={onRetryFeedback}>
              Retry
            </button>
          )}
        </span>
      )}
    </div>
  );
};
