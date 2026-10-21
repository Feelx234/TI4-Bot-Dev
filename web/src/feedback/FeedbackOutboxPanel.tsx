import React, { useState } from "react";
import { ago } from "./transport.ts";
import { failedCount, type FeedbackEntry, pendingCount, sendAllNow, useFeedbackEntries } from "./entries.ts";

const STATE_TEXT: Record<FeedbackEntry["state"], string> = {
  queued: "Waiting to be sent",
  sending: "Sending",
  sent: "Sent",
  failed: "Not sent",
};

/** Pending chip, "Send now" and the last few reports with their state. Nothing when there is nothing to say. */
export const FeedbackOutboxPanel: React.FC = () => {
  const entries = useFeedbackEntries();
  const [busy, setBusy] = useState(false);
  const pending = pendingCount(entries);
  const failed = failedCount(entries);
  const shown = entries.slice(0, 5);
  if (shown.length === 0) return null;
  const sendNow = async () => {
    setBusy(true);
    try {
      await sendAllNow();
    } finally {
      setBusy(false);
    }
  };
  return (
    <section className="feedback-outbox" data-testid="feedback-outbox" aria-label="Your reports on this device">
      <div className="feedback-outbox__head">
        {pending + failed > 0 ? (
          <span
            className={`feedback-outbox__chip${failed ? " feedback-outbox__chip--failed" : ""}`}
            data-testid="feedback-pending-chip"
            role="status"
          >
            {failed > 0 ? `${failed} not sent` : `${pending} waiting to be sent`}
          </span>
        ) : (
          <span className="feedback-outbox__chip feedback-outbox__chip--ok" data-testid="feedback-pending-chip" role="status">
            All sent
          </span>
        )}
        {pending + failed > 0 && (
          <button
            type="button"
            className="button button--secondary feedback-outbox__send"
            data-testid="feedback-send-now"
            disabled={busy}
            onClick={() => void sendNow()}
          >
            {busy ? "Sending…" : "Send now"}
          </button>
        )}
      </div>
      <ul className="feedback-outbox__list" data-testid="feedback-history">
        {shown.map((entry) => (
          <li key={entry.key} className={`feedback-outbox__item feedback-outbox__item--${entry.state}`} data-state={entry.state}>
            <span className="feedback-outbox__state">{STATE_TEXT[entry.state]}</span>
            <span className="feedback-outbox__preview">{entry.preview}</span>
            <span className="feedback-outbox__meta">
              {ago(entry.queuedAt)}
              {entry.attempts > 0 ? `, ${entry.attempts} ${entry.attempts === 1 ? "try" : "tries"}` : ""}
              {entry.state !== "sent" && entry.lastError ? `, ${entry.lastError}` : ""}
            </span>
          </li>
        ))}
      </ul>
    </section>
  );
};
