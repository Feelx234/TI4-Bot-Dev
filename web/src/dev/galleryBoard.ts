import type { BoardView, LobbyDto, PlayerView } from "../protocol/types.ts";
import { actor } from "./decisionGalleryCases.ts";

export const gallerySeating = [actor, "other_seat"];
export const galleryLobby: LobbyDto = {
  game_id: "gallery-placeholder",
  phase: "running",
  lobby_version: 1,
  host_player_id: actor,
  slots: gallerySeating.map((id, position) => ({
    slot_id: `gallery-${position}`,
    position,
    occupant: id,
    nickname: position === 0 ? "Alex" : "Blair",
    ready: true,
    connected: true,
    can_take_over: false,
  })),
};
const player = (id: string, faction: string, trade_goods: number): PlayerView => ({
  id,
  faction,
  victory_points: 2,
  trade_goods,
  commodities: 0,
  tactic_tokens: 3,
  fleet_tokens: 3,
  strategic_tokens: 2,
  passed: false,
  strategy_cards: [],
  exhausted_strategy_cards: [],
  technologies: [],
  exhausted_technologies: [],
  relics: [],
  exhausted_relics: [],
  action_cards_count: 0,
  secret_objectives_count: 0,
  leaders: {},
});
export const galleryPlayers = [player(actor, "sol", 2), player("other_seat", "hacan", 1)];
export const galleryBoard: BoardView = {
  active_system: "18",
  map_tiles: [
    {
      system_id: "18",
      label: "Mecatol Rex",
      q: 0,
      r: 0,
      planets: [
        { id: "jord", label: "Jord", resources: 2, influence: 2 },
        { id: "exhausted", label: "Exhausted World", resources: 1, influence: 3 },
      ],
    },
    { system_id: "24", label: "Neighbor", q: 1, r: 0, planets: [] },
  ],
  systems: {
    "18": {
      system_id: "18",
      command_tokens: [],
      planets: {
        jord: { planet_id: "jord", controlled_by: actor, exhausted: false },
        exhausted: { planet_id: "exhausted", controlled_by: actor, exhausted: true },
      },
      units: [
        { unit_type: "fighter", owner: actor, damaged: false },
        { unit_type: "fighter", owner: actor, damaged: false },
        { unit_type: "dreadnought", owner: actor, damaged: true },
        { unit_type: "dreadnought", owner: actor, damaged: false },
        { unit_type: "infantry", owner: actor, damaged: false },
        { unit_type: "infantry", owner: actor, planet: "jord", damaged: false },
      ],
    },
    "24": {
      system_id: "24",
      command_tokens: [],
      planets: {},
      units: [{ unit_type: "cruiser", owner: actor, damaged: false }],
    },
  },
};
