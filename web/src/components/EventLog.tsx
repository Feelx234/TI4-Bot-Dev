import React from "react";
import { GameLogEntry } from "../hooks/useGameSession.ts";
import { useParticipantText, usePlayerIdentity } from "../presentation/PlayerIdentity.tsx";

export interface EventLogProps {
  events: GameLogEntry[];
  isOpen: boolean;
  onToggle: () => void;
  onRestore?: (eventId: string, steps: number) => void;
  cursor?: number;
  busy?: boolean;
}

function eventPresentation(
  event: GameLogEntry["event"],
  label: (id: string) => string,
): { color: string; text: string } {
  switch (event.kind) {
    case "decision_resolved":
      return { color: "#38bdf8", text: "Decision resolved" };
    case "game_initialized":
      return {
        color: "#e2e8f0",
        text: `Game initialized: round ${event.round}, ${event.phase} phase, speaker ${label(event.speaker)}`,
      };
    case "phase_transition":
      return {
        color: "#c084fc",
        text: `Phase transition: round ${event.round}, ${event.phase} phase`,
      };
    case "game_finished":
      return {
        color: "#34d399",
        text: event.winner ? `Game finished: ${label(event.winner)} wins` : "Game finished: draw",
      };
    default:
      return assertNever(event);
  }
}

function assertNever(value: never): never {
  throw new Error(`Unknown game event: ${JSON.stringify(value)}`);
}

function getVisibilityLabel(visibility: GameLogEntry): string | null {
  switch (visibility.visibility) {
    case "public":
      return null;
    case "seat":
      return "Private";
    case "referee":
      return "Referee";
  }
}

export const EventLog: React.FC<EventLogProps> = ({
  events,
  isOpen,
  onToggle,
  onRestore,
  cursor = 0,
  busy = false,
}) => {
  const display = usePlayerIdentity();
  const present = useParticipantText();
  return (
    <div data-testid="event-log-container" className="event-log">
      <button
        id="event-log-toggle"
        type="button"
        data-testid="event-log-toggle"
        onClick={onToggle}
        aria-expanded={isOpen}
        aria-controls="event-log-list"
        className="button"
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          padding: "6px 16px",
          width: "100%",
          textAlign: "left",
          background: "var(--color-surface)",
          fontWeight: 600,
          color: "var(--color-text-muted)",
          userSelect: "none",
        }}
      >
        <span style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <span>Event Log</span>
          <span
            style={{
              background: "#1e293b",
              color: "#38bdf8",
              padding: "1px 6px",
              borderRadius: 10,
              fontSize: 11,
            }}
          >
            {events.length}
          </span>
        </span>
        <span style={{ fontSize: 11 }}>{isOpen ? "▼ Hide" : "▲ Show"}</span>
      </button>

      {isOpen && (
        <div
          id="event-log-list"
          role="region"
          aria-labelledby="event-log-toggle"
          data-testid="event-log-list"
          style={{
            maxHeight: 200,
            overflowY: "auto",
            padding: "10px 16px",
            display: "flex",
            flexDirection: "column",
            gap: 6,
          }}
        >
          {events.length === 0 ? (
            <div style={{ color: "#64748b" }}>No events recorded yet.</div>
          ) : (
            events.map((ev, i) => {
              const presentation = eventPresentation(ev.event, (id) => display(id).label);
              const visibility = getVisibilityLabel(ev);
              return (
                <div
                  key={ev.id}
                  data-testid="event-log-entry"
                  style={{
                    fontFamily: "ui-monospace, monospace",
                    fontSize: 12,
                    display: "flex",
                    gap: 8,
                    alignItems: "baseline",
                    lineHeight: 1.4,
                  }}
                >
                  <span style={{ color: "#475569", fontSize: 11, flexShrink: 0 }}>[{i + 1}]</span>
                  {ev.timestamp && (
                    <span style={{ color: "#64748b", fontSize: 11, flexShrink: 0 }}>
                      {ev.timestamp}
                    </span>
                  )}
                  {ev.version !== undefined && (
                    <span
                      style={{
                        color: "#0284c7",
                        fontSize: 10,
                        background: "#0c4a6e",
                        padding: "1px 4px",
                        borderRadius: 3,
                        flexShrink: 0,
                      }}
                    >
                      v{ev.version}
                    </span>
                  )}
                  {visibility && <span style={{ color: "#f59e0b" }}>{visibility}</span>}
                  <span style={{ color: presentation.color, wordBreak: "break-word" }}>
                    {present(presentation.text)}
                  </span>
                  {onRestore && ev.decision_count !== undefined && ev.decision_count < cursor && (
                    <button
                      type="button"
                      className="button button--secondary button--sm"
                      disabled={busy}
                      onClick={() => onRestore(ev.id, cursor - ev.decision_count!)}
                      aria-label={`Undo to event ${i + 1}`}
                    >
                      Undo to here
                    </button>
                  )}
                </div>
              );
            })
          )}
        </div>
      )}
    </div>
  );
};
