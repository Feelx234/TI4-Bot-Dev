import React from "react";
import { PlayerView } from "../protocol/types.ts";
import {
  getStrategyCardMeta,
  getSecretObjectiveMeta,
  getActionCardMeta,
  getPublicObjectiveMeta,
} from "../protocol/contentCatalog.ts";
import { CardSubject } from "./CardDetails.tsx";
import { SeatBadge, usePlayerIdentity } from "../presentation/PlayerIdentity.tsx";

export interface PlayerSheetProps {
  players: PlayerView[];
  userSeat?: string;
  revealedObjectives?: string[];
  onInspectCard?: (subject: CardSubject) => void;
}

export const PlayerSheet: React.FC<PlayerSheetProps> = ({
  players,
  userSeat,
  revealedObjectives = [],
  onInspectCard,
}) => {
  const display = usePlayerIdentity();
  return (
    <aside
      data-testid="player-sheet-panel"
      className="player-sheet-panel"
      style={{
        width: 330,
        height: "100%",
        borderLeft: "1px solid var(--color-border)",
        overflowY: "auto",
        padding: 16,
        display: "flex",
        flexDirection: "column",
        gap: 16,
      }}
    >
      <h2 style={{ fontSize: 16, fontWeight: "bold", margin: 0, color: "#94a3b8" }}>Players</h2>

      {revealedObjectives.length > 0 && (
        <section aria-label="Revealed public objectives" className="card" style={{ padding: 12 }}>
          <h3 style={{ margin: "0 0 8px", fontSize: 13 }}>Revealed Public Objectives</h3>
          {revealedObjectives.map((id) => {
            const meta = getPublicObjectiveMeta(id);
            return (
              <button
                type="button"
                key={id}
                className="button button--secondary detail-trigger"
                data-testid={`public-objective-${id}`}
                onClick={() => onInspectCard?.({ kind: "publicObjective", id })}
              >
                {meta.name} · {meta.points} VP
              </button>
            );
          })}
        </section>
      )}

      {players.map((player) => {
        const isSelf = userSeat === player.id;
        const identity = display(player.id);

        return (
          <div
            key={player.id}
            data-testid="player-card"
            data-is-self={isSelf ? "true" : "false"}
            className={`card${isSelf ? " card--selected" : ""}`}
            style={{
              padding: 14,
              borderLeft: `4px solid ${identity.color}`,
              display: "flex",
              flexDirection: "column",
              gap: 10,
            }}
          >
            {/* Header: current name + physical position + faction + VP */}
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <div>
                <strong style={{ fontSize: 15, color: "#f8fafc" }}>
                  {identity.position && <SeatBadge position={identity.position} />} {identity.label}{" "}
                  {isSelf && "(You)"}
                </strong>
                <div style={{ fontSize: 12, color: "#94a3b8" }}>{player.faction}</div>
              </div>
              <div
                data-testid="player-vp"
                style={{
                  background: "#fbbf24",
                  color: "#0f172a",
                  fontWeight: "bold",
                  fontSize: 14,
                  padding: "2px 8px",
                  borderRadius: 12,
                }}
              >
                {player.victory_points} VP
              </div>
            </div>

            {/* Economy & Tokens */}
            <div
              style={{
                display: "grid",
                gridTemplateColumns: "1fr 1fr",
                gap: 6,
                fontSize: 12,
                color: "#cbd5e1",
              }}
            >
              <div>
                TG: <strong>{player.trade_goods}</strong> | Comm:{" "}
                <strong>{player.commodities}</strong>
              </div>
              <div>
                Tokens:{" "}
                <strong>
                  {player.tactic_tokens}/{player.fleet_tokens}/{player.strategic_tokens}
                </strong>
              </div>
            </div>

            {/* Public strategy cards */}
            {player.strategy_cards.length > 0 && (
              <div style={{ fontSize: 12 }}>
                <div style={{ color: "#94a3b8", marginBottom: 4, fontWeight: 500 }}>
                  Strategy Cards:
                </div>
                <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
                  {player.strategy_cards.map((scId) => {
                    const meta = getStrategyCardMeta(scId);
                    return (
                      <button
                        type="button"
                        key={scId}
                        onClick={() => onInspectCard?.({ kind: "strategy", id: scId })}
                        data-testid={`strategy-card-badge-${scId}`}
                        data-card-id={scId}
                        style={{
                          background: "#090d16",
                          border: "1px solid #38bdf8",
                          borderRadius: 4,
                          padding: "2px 8px",
                          fontSize: 12,
                          fontWeight: 600,
                          color: "#38bdf8",
                          cursor: "pointer",
                          display: "inline-block",
                        }}
                      >
                        {meta.initiative > 0 ? `${meta.initiative}. ` : ""}
                        {meta.name}
                      </button>
                    );
                  })}
                </div>
              </div>
            )}

            {/* Public Card Counts */}
            <div style={{ display: "flex", gap: 10, fontSize: 11, color: "#94a3b8" }}>
              <span>
                Action Cards: <strong>{player.action_cards_count}</strong>
              </span>
              <span>
                Secret Obj: <strong>{player.secret_objectives_count}</strong>
              </span>
            </div>

            {player.scored_secret_objectives && player.scored_secret_objectives.length > 0 && (
              <div style={{ display: "grid", gap: 4 }}>
                <span className="text-muted">Scored Secret Objectives:</span>
                {player.scored_secret_objectives.map((id) => (
                  <button
                    type="button"
                    key={id}
                    className="button button--secondary detail-trigger"
                    data-testid={`scored-objective-${id}`}
                    onClick={() => onInspectCard?.({ kind: "secretObjective", id })}
                  >
                    {getSecretObjectiveMeta(id).name}
                  </button>
                ))}
              </div>
            )}

            {/* Private Information (Visible only to this player's seat) */}
            {isSelf && (
              <div
                data-testid="private-hand-section"
                style={{
                  marginTop: 6,
                  borderTop: "1px dashed #475569",
                  paddingTop: 10,
                  display: "flex",
                  flexDirection: "column",
                  gap: 10,
                }}
              >
                <div style={{ fontSize: 12, fontWeight: "bold", color: "#38bdf8" }}>
                  Private Hand
                </div>

                {/* Held Action Cards */}
                {player.held_action_cards && player.held_action_cards.length > 0 ? (
                  <div>
                    <div style={{ fontSize: 11, color: "#94a3b8", marginBottom: 4 }}>
                      Action Cards ({player.held_action_cards.length}):
                    </div>
                    <ul
                      style={{
                        margin: 0,
                        padding: 0,
                        listStyle: "none",
                        display: "flex",
                        flexDirection: "column",
                        gap: 6,
                      }}
                    >
                      {player.held_action_cards.map((cardId) => {
                        const meta = getActionCardMeta(cardId);

                        return (
                          <li
                            key={cardId}
                            data-private-card="true"
                            data-private-card-owner={
                              identity.position ? `position-${identity.position}` : "participant"
                            }
                            data-action-card-id={cardId}
                            data-testid={`action-card-item-${cardId}`}
                            className="card"
                            style={{
                              border: "1px solid #475569",
                              padding: "6px 10px",
                              cursor: "pointer",
                              position: "relative",
                            }}
                          >
                            <button
                              type="button"
                              className="detail-trigger"
                              onClick={() => onInspectCard?.({ kind: "action", id: cardId })}
                            >
                              <div style={{ width: "100%" }}>
                                <div
                                  style={{
                                    display: "flex",
                                    justifyContent: "space-between",
                                    alignItems: "center",
                                  }}
                                >
                                  <strong style={{ color: "#e2e8f0", fontSize: 12 }}>
                                    {meta.name}
                                  </strong>
                                  {meta.phase && (
                                    <span
                                      style={{
                                        fontSize: 10,
                                        color: "#94a3b8",
                                        background: "#1e293b",
                                        padding: "1px 5px",
                                        borderRadius: 3,
                                      }}
                                    >
                                      {meta.phase}
                                    </span>
                                  )}
                                </div>
                                <div
                                  style={{
                                    fontSize: 11,
                                    color: "#94a3b8",
                                    marginTop: 2,
                                    lineHeight: 1.3,
                                  }}
                                >
                                  {meta.description}
                                </div>
                              </div>
                            </button>
                          </li>
                        );
                      })}
                    </ul>
                  </div>
                ) : (
                  <div style={{ fontSize: 11, color: "#64748b" }}>No action cards in hand</div>
                )}

                {/* Held Secret Objectives (Human Readable with Tooltips) */}
                {player.held_secret_objectives && player.held_secret_objectives.length > 0 && (
                  <div>
                    <div style={{ fontSize: 11, color: "#94a3b8", marginBottom: 4 }}>
                      Secret Objectives ({player.held_secret_objectives.length}):
                    </div>
                    <ul
                      style={{
                        margin: 0,
                        padding: 0,
                        listStyle: "none",
                        display: "flex",
                        flexDirection: "column",
                        gap: 6,
                      }}
                    >
                      {player.held_secret_objectives.map((objId) => {
                        const meta = getSecretObjectiveMeta(objId);

                        return (
                          <li
                            key={objId}
                            data-private-card="true"
                            data-private-card-owner={
                              identity.position ? `position-${identity.position}` : "participant"
                            }
                            data-secret-obj-id={objId}
                            data-testid={`secret-objective-item-${objId}`}
                            className="card"
                            style={{
                              border: "1px solid #fbbf24",
                              padding: "6px 10px",
                              cursor: "pointer",
                              position: "relative",
                            }}
                          >
                            <button
                              type="button"
                              className="detail-trigger"
                              onClick={() =>
                                onInspectCard?.({ kind: "secretObjective", id: objId })
                              }
                            >
                              <div style={{ width: "100%" }}>
                                <div
                                  style={{
                                    display: "flex",
                                    justifyContent: "space-between",
                                    alignItems: "center",
                                  }}
                                >
                                  <strong style={{ color: "#fbbf24", fontSize: 12 }}>
                                    {meta.name}
                                  </strong>
                                  <span
                                    style={{
                                      fontSize: 10,
                                      color: "#fbbf24",
                                      background: "rgba(251, 191, 36, 0.15)",
                                      padding: "1px 5px",
                                      borderRadius: 3,
                                    }}
                                  >
                                    {meta.phase} • {meta.points} VP
                                  </span>
                                </div>
                                <div
                                  style={{
                                    fontSize: 11,
                                    color: "#cbd5e1",
                                    marginTop: 3,
                                    lineHeight: 1.3,
                                  }}
                                >
                                  {meta.description}
                                </div>
                              </div>
                            </button>
                          </li>
                        );
                      })}
                    </ul>
                  </div>
                )}
              </div>
            )}
          </div>
        );
      })}
    </aside>
  );
};
