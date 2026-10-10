// The seats and the player table.
import type { PlayerRowView, PlayerTableView, SeatId, SeatView } from "../../model";
import type { PlayerView, SessionUpdate } from "../wire";
import { DECKS } from "./exploration";
import { commodityValue, factionName, seatMark, strategyCard, titleCase } from "./names";

/** The players in seat order. */
const seated = (update: SessionUpdate): PlayerView[] => {
  const order = update.view.seating_order;
  return [...update.view.players].sort((a, b) => order.indexOf(a.id) - order.indexOf(b.id));
};

export function selectSeats(update: SessionUpdate): Record<SeatId, SeatView> {
  return Object.fromEntries(
    seated(update).map((player, index) => [
      player.id,
      {
        id: player.id,
        // A local game has no player names. The seat letter tells the seats apart.
        name: `Seat ${player.id.toUpperCase()}`,
        faction: factionName(player.faction),
        ...seatMark(index),
      },
    ]),
  );
}

/** Who the game waits for, or whose turn it is. */
export const actingSeat = (update: SessionUpdate): SeatId | null => {
  const status = update.turn_status;
  return status.kind === "waiting_for_decision"
    ? status.seat
    : status.kind === "active_turn"
      ? status.player
      : null;
};

const LEADERS: Record<string, string> = { readied: "ready" };
/** "Agent ready · Commander locked · Hero locked" */
const leaders = (player: PlayerView) =>
  ["agent", "commander", "hero"]
    .flatMap((kind) =>
      Object.entries(player.leaders)
        .filter(([id]) => id.endsWith(kind))
        .map(
          ([, status]) =>
            `${kind.charAt(0).toUpperCase()}${kind.slice(1)} ${LEADERS[status] ?? status}`,
        ),
    )
    .join(" · ");

export function selectPlayers(update: SessionUpdate): PlayerTableView {
  const { view, viewer } = update;
  const acting = actingSeat(update);
  // What each seat's planets give: ready and total, from the planets that the seat controls.
  const values = new Map<string, { resources: number; influence: number }>();
  for (const tile of view.board.map_tiles ?? []) {
    for (const planet of tile.planets ?? []) {
      values.set(planet.id, planet);
    }
  }
  const row = (player: PlayerView): PlayerRowView => {
    const resources: [number, number] = [0, 0];
    const influence: [number, number] = [0, 0];
    let planets = 0;
    for (const system of Object.values(view.board.systems)) {
      for (const planet of Object.values(system.planets)) {
        const value = values.get(planet.planet_id);
        if (planet.controlled_by !== player.id || !value) {
          continue;
        }
        planets++;
        resources[1] += value.resources;
        influence[1] += value.influence;
        if (!planet.exhausted) {
          resources[0] += value.resources;
          influence[0] += value.influence;
        }
      }
    }
    return {
      seat: player.id,
      isMe: viewer.role === "player" && viewer.seat === player.id,
      turn: acting === player.id ? "now" : null,
      speaker: view.speaker === player.id,
      passed: player.passed,
      victoryPoints: player.victory_points,
      strategyCards: player.strategy_cards
        .map((id) => ({
          ...strategyCard(id),
          used: player.exhausted_strategy_cards.includes(id),
        }))
        .sort((a, b) => a.number - b.number),
      resources,
      influence,
      tradeGoods: player.trade_goods,
      commodities: [player.commodities, commodityValue(player.faction) ?? player.commodities],
      tokens: [player.tactic_tokens, player.fleet_tokens, player.strategic_tokens],
      actionCards: player.action_cards_count,
      secrets: [player.scored_secret_objectives?.length ?? 0, player.secret_objectives_count],
      planets,
      abilities: [],
      // The ids are the short names that players use: AMD, DET, NM.
      technologies: player.technologies.map((id) => id.toUpperCase()),
      leaders: leaders(player),
      relics: player.relics.map(titleCase),
      fragments: DECKS.flatMap(([key, deck]) => {
        const count = player.relic_fragments?.[key] ?? 0;
        return count > 0 ? [{ deck, count }] : [];
      }),
    };
  };
  return {
    pick: null,
    rows: seated(update).map(row),
    freeCards: view.table.unclaimed_strategy_cards
      .map((id) => ({
        ...strategyCard(id),
        tradeGoods: view.table.strategy_card_goods[id] ?? 0,
      }))
      .sort((a, b) => a.number - b.number),
  };
}
