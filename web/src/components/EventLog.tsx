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
  const rows: GameLogEntry[][] = [];
  for (const event of events) {
    const last = rows[rows.length - 1];
    if (event.batch_id && event.event.kind === "decision_resolved" &&
        last?.[0].batch_id === event.batch_id &&
        last.at(-1)?.event.kind === "decision_resolved" &&
        event.decision_count !== undefined && last.at(-1)?.decision_count !== undefined &&
        event.decision_count === last.at(-1)!.decision_count! + 1) last.push(event);
    else rows.push([event]);
  }
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
            rows.map((group, i) => {
              const ev = group[group.length - 1];
              const presentation = eventPresentation(ev.event, (id) => display(id).label);
              const visibility = getVisibilityLabel(ev);
              const details = group.map((entry) => entry.detail ?? "Decision resolved");
              const meaningful = details.filter((detail) => detail !== "Done loading" && detail !== "Done moving");
              const moves = group.flatMap((entry) => entry.movement ? [entry.movement] : []);
              const counts = new Map<string, number>();
              for (const move of moves) {
                const key = `${move.unit}|${move.origin}|${move.destination}`;
                counts.set(key, (counts.get(key) ?? 0) + 1);
              }
               const heading = ev.batch_id
                 ? moves.length
                  ? `${display(moves[0].actor).label} moved ${[...counts].map(([key, count]) => {
                      const [unit, origin, destination] = key.split("|");
                      return `${count} ${unit}${count === 1 ? "" : "s"} from #${origin} to #${destination}`;
                    }).join(", ")}`
                   : meaningful.length
                     ? meaningful.length === 1 ? meaningful[0] : `${meaningful.length} choices: ${meaningful[0]}`
                     : "Workflow finished"
                : (ev.detail ?? (ev.movement
                    ? `${display(ev.movement.actor).label} moved ${ev.movement.unit} from #${ev.movement.origin} to #${ev.movement.destination}`
                    : presentation.text));
              return (
                <div
                   key={group[0].id}
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
                   {ev.batch_id ? (
                     <details style={{ color: presentation.color, wordBreak: "break-word" }}>
                       <summary>{present(heading)}</summary>
                       <ul>{details.map((detail, index) => <li key={group[index].id}>{present(detail)}</li>)}</ul>
                     </details>
                   ) : <span style={{ color: presentation.color, wordBreak: "break-word" }}>{present(heading)}</span>}
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
