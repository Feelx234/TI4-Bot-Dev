import React from "react";
import { GameLogEntry } from "../hooks/useGameSession.ts";
import { HistoryChange } from "../protocol/client.ts";
import { useParticipantText, usePlayerIdentity } from "../presentation/PlayerIdentity.tsx";

export interface EventLogProps {
  events: GameLogEntry[];
  isOpen: boolean;
  onToggle: () => void;
  onRestore?: (cursor: number) => void;
  onChangeHistory?: (action: HistoryChange) => void;
  cursor?: number;
  redoCount?: number;
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

/** One batch can contain invisible decisions and multiple visible facts at one cursor. */
export function foldLogEvents(events: readonly GameLogEntry[]): GameLogEntry[][] {
  const rows: GameLogEntry[][] = [];
  let activeBatch: { id: string; row: GameLogEntry[] } | null = null;
  for (const event of events) {
    if (event.event.kind !== "decision_resolved") {
      rows.push([event]);
      continue;
    }
    if (event.batch_id && activeBatch?.id === event.batch_id) {
      activeBatch.row.push(event);
    } else {
      const row = [event];
      rows.push(row);
      activeBatch = event.batch_id ? { id: event.batch_id, row } : null;
    }
  }
  return rows;
}

export const EventLog: React.FC<EventLogProps> = ({
  events,
  isOpen,
  onToggle,
  onRestore,
  onChangeHistory,
  cursor = 0,
  redoCount = 0,
  busy = false,
}) => {
  const display = usePlayerIdentity();
  const present = useParticipantText();
  const rows = foldLogEvents(events);
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
        <>
          {onChangeHistory && redoCount > 0 && (
            <div className="event-log__redo" aria-label="Redo history">
              <span>
                {redoCount} undone {redoCount === 1 ? "decision" : "decisions"}
              </span>
              <button
                type="button"
                className="button button--secondary button--sm"
                disabled={busy}
                onClick={() => onChangeHistory("redo")}
              >
                Redo one
              </button>
              <button
                type="button"
                className="button button--secondary button--sm"
                disabled={busy}
                onClick={() => onChangeHistory("redo_batch")}
              >
                Redo batch
              </button>
              <button
                type="button"
                className="button button--secondary button--sm"
                disabled={busy}
                onClick={() => onChangeHistory("redo_pipeline")}
              >
                Redo action
              </button>
            </div>
          )}
          <div
            id="event-log-list"
            role="region"
            aria-labelledby="event-log-toggle"
            data-testid="event-log-list"
            style={{
              maxHeight: 320,
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
                const first = group[0];
                const unique = new Map<number, GameLogEntry>();
                for (const entry of group.filter(
                  (entry) => entry.event.kind === "decision_resolved",
                )) {
                  if (entry.decision_count === undefined) continue;
                  const prior = unique.get(entry.decision_count);
                  unique.set(
                    entry.decision_count,
                    prior
                      ? {
                          ...prior,
                          detail: prior.detail ?? entry.detail,
                          movement: prior.movement ?? entry.movement,
                          private_detail: entry.private_detail ?? prior.private_detail,
                        }
                      : entry,
                  );
                }
                const decisions = [...unique.values()];
                const presentation = eventPresentation(ev.event, (id) => display(id).label);
                const visibility = getVisibilityLabel(ev);
                const details = decisions.map(
                  (entry) => entry.private_detail ?? entry.detail ?? "Decision resolved",
                );
                const meaningful = details.filter(
                  (detail) => detail !== "Done loading" && detail !== "Done moving",
                );
                const moves = decisions.flatMap((entry) =>
                  entry.movement ? [entry.movement] : [],
                );
                const counts = new Map<string, number>();
                for (const move of moves) {
                  const key = `${move.unit}|${move.origin}|${move.destination}`;
                  counts.set(key, (counts.get(key) ?? 0) + 1);
                }
                const heading = first.batch_id
                  ? moves.length
                    ? `${display(moves[0].actor).label} moved ${[...counts]
                        .map(([key, count]) => {
                          const [unit, origin, destination] = key.split("|");
                          return `${count} ${unit}${count === 1 ? "" : "s"} from #${origin} to #${destination}`;
                        })
                        .join(", ")}`
                    : meaningful.length
                      ? meaningful.length === 1
                        ? meaningful[0]
                        : `${meaningful.length} choices: ${meaningful[0]}`
                      : "Workflow finished"
                  : (ev.private_detail ??
                    ev.detail ??
                    (ev.movement
                      ? `${display(ev.movement.actor).label} moved ${ev.movement.unit} from #${ev.movement.origin} to #${ev.movement.destination}`
                      : presentation.text));
                return (
                  <div key={group[0].id} data-testid="event-log-entry" className="event-log__entry">
                    <span className="event-log__index">{i + 1}</span>
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
                    {first.batch_id ? (
                      <details
                        className="event-log__body"
                        style={{ color: presentation.color, wordBreak: "break-word" }}
                      >
                        <summary>{present(heading)}</summary>
                        <ul>
                          {decisions.map((entry, index) => (
                            <li key={entry.id}>
                              {present(details[index])}
                              {onRestore &&
                                entry.decision_count !== undefined &&
                                entry.decision_count <= cursor && (
                                  <button
                                    type="button"
                                    className="event-log__detail-undo"
                                    disabled={busy}
                                    onClick={() => onRestore(entry.decision_count! - 1)}
                                    aria-label={`Undo from decision ${entry.decision_count}`}
                                  >
                                    Undo from here
                                  </button>
                                )}
                            </li>
                          ))}
                        </ul>
                      </details>
                    ) : (
                      <span
                        className="event-log__body"
                        style={{ color: presentation.color, wordBreak: "break-word" }}
                      >
                        {present(heading)}
                      </span>
                    )}
                    {onRestore &&
                      decisions.length > 0 &&
                      ev.decision_count !== undefined &&
                      ev.decision_count <= cursor && (
                        <details className="event-log__actions">
                          <summary
                            aria-label={`History actions for event ${i + 1}`}
                            title="History actions"
                          >
                            ⋯
                          </summary>
                          <div className="event-log__action-list">
                            <button
                              type="button"
                              disabled={busy}
                              onClick={() => onRestore(ev.decision_count! - 1)}
                            >
                              Undo from decision {ev.decision_count}
                            </button>
                            {first.batch_start_cursor !== undefined && (
                              <button
                                type="button"
                                disabled={busy}
                                onClick={() => onRestore(first.batch_start_cursor!)}
                              >
                                Undo from batch start
                              </button>
                            )}
                            {first.action_start_cursor !== undefined &&
                              first.action_start_cursor < cursor && (
                                <button
                                  type="button"
                                  disabled={busy}
                                  onClick={() => onRestore(first.action_start_cursor!)}
                                >
                                  Undo from action start
                                </button>
                              )}
                          </div>
                        </details>
                      )}
                  </div>
                );
              })
            )}
          </div>
        </>
      )}
    </div>
  );
};
