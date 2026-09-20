/**
 * TypeScript definitions for the Twilight Imperium 4 Wire Protocol DTOs.
 * Exactly mirrors Rust structs from `crates/ti4-server/src/protocol/`.
 */

export const PROTOCOL_VERSION = 1;

export type ViewerRole =
  | { role: 'player'; seat: string }
  | { role: 'spectator' };

export type PublicTurnStatus =
  | { kind: 'active_turn'; player: string; phase: string; round: number }
  | { kind: 'waiting_for_decision'; seat: string; phase: string; round: number; stage: string }
  | { kind: 'phase_transition'; phase: string; round: number }
  | { kind: 'game_over'; winner?: string | null };

export type RejectionReason =
  | { reason: 'stale_version'; expected: number; current: number }
  | { reason: 'stale_nonce' }
  | { reason: 'unauthorized_seat'; seat?: string | null }
  | { reason: 'no_pending_choice' }
  | { reason: 'unknown_option'; option_id: string }
  | { reason: 'validation_failed'; message: string };

export interface ChoiceOptionDto {
  id: string;
  label: string;
  description?: string;
}

export interface DecisionContextDto {
  kind: string;
  subtype: string;
  details?: Record<string, unknown>;
}

export interface OutstandingConstraintDto {
  min_selection?: number;
  max_selection?: number;
}

export interface PendingChoiceDto {
  prompt: string;
  actor: string;
  nonce: string;
  options: ChoiceOptionDto[];
  context?: DecisionContextDto;
  constraints?: OutstandingConstraintDto;
}

export interface PlanetView {
  planet_id: string;
  controlled_by?: string | null;
  exhausted: boolean;
  attachments?: string[];
}

export interface PlacedUnitView {
  unit_type: string;
  owner: string;
  planet?: string | null;
  damaged: boolean;
}

export interface SystemView {
  system_id: string;
  command_tokens: string[];
  planets: Record<string, PlanetView>;
  units: PlacedUnitView[];
  coordinate?: string;
  tile_type?: string;
}

export interface PlayerView {
  id: string;
  faction: string;
  victory_points: number;
  trade_goods: number;
  commodities: number;
  tactic_tokens: number;
  fleet_tokens: number;
  strategic_tokens: number;
  passed: boolean;
  strategy_cards: string[];
  exhausted_strategy_cards: string[];
  technologies: string[];
  exhausted_technologies: string[];
  relics: string[];
  exhausted_relics: string[];
  action_cards_count: number;
  secret_objectives_count: number;
  held_action_cards?: string[];
  held_secret_objectives?: string[];
  scored_secret_objectives?: string[];
  leaders: Record<string, string>;
}

export interface TableView {
  revealed_objectives: string[];
  scored_objectives: Record<string, string[]>;
  unclaimed_strategy_cards: string[];
  strategy_card_goods: Record<string, number>;
  laws: Record<string, string>;
}

export interface PlanetMetaView {
  id: string;
  label: string;
  resources: number;
  influence: number;
  traits?: string[];
  tech_specialties?: string[];
  legendary?: boolean;
  space_station?: boolean;
}

export interface BoardTileView {
  system_id: string;
  label: string;
  q: number;
  r: number;
  hyperlane?: boolean;
  special_area?: string | null;
  anomalies?: string[];
  wormholes?: string[];
  egress?: boolean;
  planets?: PlanetMetaView[];
}

export interface BoardView {
  systems: Record<string, SystemView>;
  active_system?: string | null;
  map_tiles?: BoardTileView[];
}

export interface GameView {
  round: number;
  phase: string;
  speaker: string;
  seating_order: string[];
  active_player?: string | null;
  finished: boolean;
  players: PlayerView[];
  board: BoardView;
  table: TableView;
}

export interface InitialSnapshotMsg {
  type?: 'initial_snapshot';
  protocol_version: number;
  game_id: string;
  game_version: number;
  viewer: ViewerRole;
  view: GameView;
  pending_choice?: PendingChoiceDto | null;
  turn_status: PublicTurnStatus;
}

export interface StateUpdateMsg {
  type?: 'state_update';
  protocol_version: number;
  game_id: string;
  game_version: number;
  viewer: ViewerRole;
  view: GameView;
  pending_choice?: PendingChoiceDto | null;
  turn_status: PublicTurnStatus;
}

export interface PendingChoiceMsg {
  type?: 'pending_choice';
  protocol_version: number;
  game_id: string;
  game_version: number;
  choice: PendingChoiceDto;
}

export interface TurnStatusMsg {
  type?: 'turn_status';
  protocol_version: number;
  game_id: string;
  game_version: number;
  status: PublicTurnStatus;
}

export interface ActionAcceptedMsg {
  type?: 'action_accepted';
  protocol_version: number;
  game_id: string;
  game_version: number;
  option_id: string;
}

export interface ActionRejectedMsg {
  type?: 'action_rejected';
  protocol_version: number;
  game_id: string;
  game_version: number;
  reason: RejectionReason;
}

export interface ProtocolErrorMsg {
  type?: 'error';
  protocol_version: number;
  kind: string;
  message: string;
}

export interface GameOverMsg {
  type?: 'game_over';
  protocol_version: number;
  game_id: string;
  game_version: number;
  winner?: string | null;
  final_scores: Record<string, number>;
}

export interface PongMsg {
  type?: 'pong';
  protocol_version: number;
  sequence: number;
}

export type ServerMessage =
  | ({ type: 'initial_snapshot' } & InitialSnapshotMsg)
  | ({ type: 'state_update' } & StateUpdateMsg)
  | ({ type: 'pending_choice' } & PendingChoiceMsg)
  | ({ type: 'turn_status' } & TurnStatusMsg)
  | ({ type: 'action_accepted' } & ActionAcceptedMsg)
  | ({ type: 'action_rejected' } & ActionRejectedMsg)
  | ({ type: 'error' } & ProtocolErrorMsg)
  | ({ type: 'game_over' } & GameOverMsg)
  | ({ type: 'pong' } & PongMsg);

export type ClientMessage =
  | {
      type: 'subscribe';
      protocol_version: number;
      game_id: string;
      seat_token?: string;
    }
  | {
      type: 'submit_choice';
      protocol_version: number;
      game_id: string;
      nonce: string;
      expected_version: number;
      option_id: string;
    }
  | {
      type: 'ping';
      protocol_version: number;
      sequence: number;
    };
