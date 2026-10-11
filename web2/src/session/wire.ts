/**
 * The part of the wire protocol that web2 reads. The shapes are the serde output of the Rust
 * types named in each comment (`crates/ti4-view`, `crates/ti4-engine`). The server sends them in
 * `StateUpdateMsg`, the wasm build hands them over as `SessionUpdate`: same names, same shapes.
 */

/** `ti4_view::status::ViewerRole` */
export type ViewerRole = { role: "player"; seat: string } | { role: "spectator" };

/** `ti4_view::status::PublicTurnStatus` */
export type PublicTurnStatus =
  | { kind: "active_turn"; player: string; phase: string; round: number }
  | { kind: "waiting_for_decision"; seat: string; phase: string; round: number; stage: string }
  | { kind: "phase_transition"; phase: string; round: number }
  | { kind: "game_over"; winner?: string | null };

/** `ti4_engine::choice::ChoiceOption` */
export interface ChoiceOption {
  id: string;
  kind: string;
  label: string;
  payload: Record<string, unknown>;
  auto_resolved?: boolean;
}

/** `ti4_engine::decision_context::DecisionContext`: the fields that web2 reads. */
export interface DecisionContext {
  /** What is asked: "activate_system", "movement_step", "load_cargo", "reaction_…". */
  subtype: string;
  actor: string;
}

/** `ti4_engine::choice::Choice`. `details` is not read yet. */
export interface Choice {
  player: string;
  prompt: string;
  options: ChoiceOption[];
  context?: DecisionContext | null;
  details?: Record<string, unknown>;
}

/** `ti4_view::projection::PendingChoiceEnvelope` */
export interface PendingChoiceEnvelope {
  nonce: string;
  choice: Choice;
}

/** `ti4_view::view::PlayerView` */
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
  /** The legendary planets whose ability card is exhausted. */
  exhausted_legendary?: string[];
  /** Relic fragments by deck: CULTURAL, HAZARDOUS, INDUSTRIAL, FRONTIER. */
  relic_fragments?: Record<string, number>;
  exploration_cards?: string[];
  action_cards_count: number;
  secret_objectives_count: number;
  held_action_cards?: string[];
  held_secret_objectives?: string[];
  scored_secret_objectives?: string[];
  leaders: Record<string, string>;
}

/** `ti4_view::view::PlanetView` */
export interface PlanetView {
  planet_id: string;
  controlled_by: string | null;
  exhausted: boolean;
  attachments?: string[];
}

/** `ti4_view::view::PlacedUnitView` */
export interface PlacedUnitView {
  unit_type: string;
  owner: string;
  planet: string | null;
  damaged: boolean;
  galvanized?: boolean;
}

/** `ti4_view::view::SystemView` */
export interface SystemView {
  system_id: string;
  command_tokens: string[];
  planets: Record<string, PlanetView>;
  units: PlacedUnitView[];
}

/** `ti4_view::view::PlanetMetaView` */
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

/** `ti4_view::view::BoardTileView` */
export interface BoardTileView {
  system_id: string;
  label: string;
  q: number;
  r: number;
  hyperlane?: boolean;
  special_area?: string;
  anomalies?: string[];
  wormholes?: string[];
  egress?: boolean;
  planets?: PlanetMetaView[];
}

/** `ti4_view::view::BoardView`. `combat` and `invasion` are not read yet. */
export interface BoardView {
  systems: Record<string, SystemView>;
  active_system?: string;
  map_tiles?: BoardTileView[];
  combat?: unknown;
  invasion?: unknown;
}

/** `ti4_view::view::TableView`. `objective_progress` is not read yet. */
export interface TableView {
  revealed_objectives: string[];
  scored_objectives: Record<string, string[]>;
  objective_progress?: unknown;
  unclaimed_strategy_cards: string[];
  strategy_card_goods: Record<string, number>;
  laws: Record<string, string>;
}

/** `ti4_view::view::GameView` */
export interface GameView {
  round: number;
  phase: string;
  speaker: string;
  seating_order: string[];
  active_player?: string;
  finished: boolean;
  players: PlayerView[];
  board: BoardView;
  table: TableView;
}

/** `ti4_view::tactical::MoveFact` */
export interface MoveFact {
  /** The ship cannot arrive without Gravity Drive. */
  gravity_drive: boolean;
  ionian: boolean;
  /** The systems of the way, from the system of the ship to the active system. */
  path: string[];
  /** The gravity rifts that the ship leaves: one roll for each. */
  rifts: string[];
}

/** `ti4_view::tactical::ShipFact` */
export interface ShipFact {
  origin: string;
  index: number;
  unit: string;
  damaged: boolean;
  capacity: number;
  /** The ship counts against the fleet pool. */
  fleet: boolean;
  /** The move value in this activation, without Gravity Drive and the Ionian Fuel Refinery. */
  move_value: number;
  /** The ship starts in a nebula: it moves with a value of 1. */
  nebula?: boolean;
  move?: MoveFact;
  blocked?: "command_token" | "range";
  /** What the ship can load: places in `MovementFacts.cargo`. */
  loads?: number[];
}

/** `ti4_view::tactical::CargoPool` */
export interface CargoPool {
  system: string;
  /** The planet, or null for the space area. */
  source: string | null;
  unit: string;
  damaged: boolean;
  galvanized: boolean;
  count: number;
}

/** `ti4_view::tactical::ActivationFacts` */
export interface ActivationFacts {
  kind: "activation";
  tactic_tokens: number;
  systems: {
    system: string;
    ships: number;
    origins: number;
    /** The production value of the seat in the system, when it is more than 0. */
    production?: number;
    /** The guns of the seat that roll SPACE CANNON at the activation, when there are any. */
    cannon?: number;
  }[];
}

/** `ti4_view::tactical::MovementFacts` */
export interface MovementFacts {
  kind: "movement";
  active: string;
  ships: ShipFact[];
  cargo: CargoPool[];
  /** Gravity Drive can still give one ship of this action its move. */
  gravity_drive: boolean;
  ionian: boolean;
  fleet_supply: { limit: number; charged: number };
  /** How many ships already moved in this activation. */
  moved: number;
}

/** `ti4_view::tactical::LandingPool` */
export interface LandingPool {
  unit: string;
  damaged: boolean;
  /** Where the units are, when it is not the space area of the active system. */
  from_system?: string;
  from_planet?: string;
  count: number;
}

/** `ti4_view::tactical::InvasionFacts` */
export interface InvasionFacts {
  kind: "invasion";
  active: string;
  forces: LandingPool[];
  /** Every planet of the active system. `lands`: what can land there, places in `forces`. */
  planets: { planet: string; lands?: number[] }[];
}

/** `ti4_view::tactical::BuildFact`: one kind of unit of a production, also one that the seat cannot produce now. */
export interface BuildFact {
  unit: string;
  /** The printed cost of one purchase, before a discount. */
  cost: number;
  /** How many units one purchase makes: 2 for fighters and infantry. */
  per: number;
  /** How many are left in the reinforcements. Absent when there is no limit. */
  supply?: number;
  /** The unit is a ship that counts against the fleet pool. */
  fleet?: boolean;
  /** The unit uses capacity while it is in a space area. */
  cargo?: boolean;
  /** Where the unit can be placed: "space", or a planet of the system. */
  places: string[];
  /** What the unit hits on in a combat, and with how many dice. Absent: it does not fight. */
  combat?: { hits_on: number; dice: number };
  /** The move value. 0: the unit does not move by itself. */
  move_value: number;
  /** What the unit carries. */
  capacity: number;
  /** The keyword abilities as printed: "SUSTAIN DAMAGE", "BOMBARDMENT 5+", "PRODUCTION +2". */
  abilities?: string[];
  /** The special rule that is printed on the unit. Absent: it has none. */
  rules?: string;
  /** Why the seat cannot produce the unit here now. Absent: it can. */
  unavailable?: "technology" | "supply" | "blockade" | "no_place" | "not_offered";
}

/** `ti4_view::tactical::ProductionFacts` */
export interface ProductionFacts {
  kind: "production";
  /** The system that produces. */
  system: string;
  /** The production limit of this use, and the units that it already made. */
  capacity: { limit: number; used: number };
  units: BuildFact[];
  /** What the combined cost of this use is still reduced by. */
  discount: number;
  /** Resources that are paid and not used yet. */
  credit: number;
  free?: boolean;
  /** The ready planets of the seat that pay resources, with what each is worth. */
  planets: { planet: string; worth: number }[];
  trade_goods: number;
  trade_good_worth: number;
  fleet_supply: { limit: number; charged: number };
  /** The capacity of the ships of the seat in the system, and what uses it. */
  transport: { limit: number; used: number };
}

/** `ti4_model::state::ExplorationRecord`: one card that was drawn. */
export interface ExplorationRecord {
  round: number;
  player: string;
  deck: string;
  card: string;
  /** Null for a frontier token. */
  planet: string | null;
  /** `fragment`, `attached:<attachment>`, `resolved`, `unresolved` or `discarded`. */
  outcome: string;
}

/** `ti4_view::exploration::ExplorationFacts` */
export interface ExplorationFacts {
  /** The card ids that each deck still holds, sorted. The keys are CULTURAL, HAZARDOUS, ... */
  decks: Record<string, string[]>;
  drawn: ExplorationRecord[];
  /** The planets that are explored when a player takes them, with the decks. */
  explores: Record<string, string[]>;
}

/** `ti4_view::exploration::ExplorationCatalog`: the printed exploration cards. */
export interface ExplorationCatalog {
  cards: { id: string; name: string; deck: string; resolution: string; text: string }[];
  /** The cards that each deck begins a game with. */
  decks: Record<string, string[]>;
  attachments: {
    id: string;
    name: string;
    resources: number;
    influence: number;
    tech_specialties?: string[];
    text?: string;
  }[];
}

/** `ti4_view::components::PrintedCard` */
export interface PrintedCard {
  id: string;
  name: string;
  text: string;
  /** The card has an ACTION or exhausts. A card that always holds is not used. */
  used: boolean;
}

/** `ti4_view::components::ComponentCatalog`: the printed cards of the play areas. */
export interface ComponentCatalog {
  abilities: { id: string; name: string; window?: string; text: string }[];
  /** The ability ids of each faction, in the order of the faction sheet. */
  factions: Record<string, string[]>;
  leaders: {
    id: string;
    /** "agent", "commander" or "hero". */
    kind: string;
    name: string;
    title: string;
    window?: string;
    text: string;
    /** How the leader is unlocked. Absent for a leader that is never locked. */
    unlock?: string;
  }[];
  technologies: PrintedCard[];
  relics: PrintedCard[];
  /** The ability cards of the legendary planets, by the id of the planet. */
  legendary: PrintedCard[];
  /** The action card deck as a game begins with it. Copies have ids of their own and one name. */
  action_cards: {
    id: string;
    name: string;
    /** "Action", "Agenda", "Strategy", "Status" or "Any". */
    phase: string;
    /** When the card is played, as printed: "Action", or a timing. */
    window: string;
    /** The group of the timing, one of `action_card_groups`. */
    group: string;
    text: string;
  }[];
  /** The groups of the timings of the action cards, in the order in which they are shown. */
  action_card_groups: string[];
  /** The secret objective deck as a game begins with it. */
  secret_objectives: {
    id: string;
    name: string;
    /** The phase in which the objective is scored: "Status", "Action" or "Agenda". */
    phase: string;
    /** What the objective asks for, one of `secret_objective_groups`. */
    group: string;
    text: string;
  }[];
  /** The groups of what the secret objectives ask for, in the order in which they are shown. */
  secret_objective_groups: string[];
}

/** `ti4_view::score::PointSource`: a source of points that is not a public objective. */
export interface PointSource {
  kind:
    | "secrets"
    | "custodians"
    | "imperial"
    | "support"
    | "agenda"
    | "relic"
    | "action_card"
    | "planet"
    | "other";
  /** The printed name of the agenda, relic, card or planet. Empty for the first four kinds. */
  name: string;
  /** The points of each seat, gains and losses together. A seat with none is absent. */
  points: Record<string, number>;
  /** For `support`: whose notes the seat has. */
  from?: Record<string, string[]>;
}

/** `ti4_view::score::ScoreFacts` */
export interface ScoreFacts {
  /** The points that win the game. */
  target: number;
  /** The public objectives as printed, in the order of the reveal, with the seats that scored. */
  public: { id: string; name: string; points: number; text: string; scored: string[] }[];
  /** Secrets, custodians, Imperial and Support are always there; the rest when a seat has a point. */
  sources: PointSource[];
  /** The points of a seat that no row explains. Absent in a game that the engine played. */
  unexplained?: Record<string, number>;
}

/** `ti4_view::secret_objectives::SecretObjectiveFacts` */
export interface SecretObjectiveFacts {
  /** How many cards the deck still holds. */
  deck: number;
  /** The ids of the secret objectives that each seat has scored. A seat with none is absent. */
  scored: Record<string, string[]>;
  /** How many secret objectives a seat may have, scored and unscored together. */
  hand_limit: number;
}

/** `ti4_view::action_cards::ActionCardFacts` */
export interface ActionCardFacts {
  /** How many cards the deck still holds. */
  deck: number;
  /** The ids of the discard pile. */
  discard: string[];
  /** The cards face up on the card of Garbozia. */
  salvage?: string[];
  /** Of the viewer. Absent for a spectator and for a seat with no limit. */
  hand_limit?: number;
}

/** `ti4_view::components::Catalog`: everything printed. The host asks for it once. */
export interface Catalog {
  exploration: ExplorationCatalog;
  components: ComponentCatalog;
}

/** `ti4_view::tactical::TacticalFacts` */
export type TacticalFacts = ActivationFacts | MovementFacts | InvasionFacts | ProductionFacts;

/** `ti4_view::planning::draft::DraftStep`: one entry of a draft script. */
export type DraftStep = import("./movementPlan").PlanStep | { kind: "choose"; option_id: string };

/** `ti4_view::planning::draft::DraftStop`: why a run of a draft ended. */
export type DraftStop =
  /** The script is used up: the question of the update is open. */
  | { kind: "open" }
  /** The game played the whole script, and the tactical action is over. */
  | { kind: "complete" }
  /** The game does not offer what step `step` of the script asks for. */
  | { kind: "refused"; step: number; reason: string }
  /** Something a draft cannot show: `ti4_view::planning::audit::StopReason`. */
  | { kind: "stopped"; reason: string }
  | { kind: "failed"; reason: string };

/** `ti4_view::planning::draft::DraftOutcome` */
export interface DraftOutcome {
  /** Where the run ended. It has the open question when the stop is "open". */
  update: SessionUpdate;
  /** The question of the first ship to move, before any ship moved. */
  movement?: SessionUpdate;
  /** The question of the first ground force to land: what the landings are staged against. */
  invasion?: SessionUpdate;
  /** The question of the first unit to produce: what a production is staged against. */
  production?: SessionUpdate;
  /** How many steps of the script the game took, or did not need. */
  consumed: number;
  stop: DraftStop;
}

/** `ti4_view::projection::SessionUpdate`: the fields of `StateUpdateMsg` that web2 reads. */
export interface SessionUpdate {
  viewer: ViewerRole;
  view: GameView;
  pending_choice?: PendingChoiceEnvelope;
  turn_status: PublicTurnStatus;
  /** What the engine says about the pending choice of a tactical action. The wasm host fills it. */
  tactical?: TacticalFacts;
  /** The exploration decks and the planets that are still explored. The wasm host fills it. */
  exploration?: ExplorationFacts;
  /** The action card deck, its discard pile and the hand limit. The wasm host fills it. */
  action_cards?: ActionCardFacts;
  /** The secret objective deck and what each seat has scored of it. The wasm host fills it. */
  secret_objectives?: SecretObjectiveFacts;
  /** Where the victory points of every seat come from. The wasm host fills it. */
  score?: ScoreFacts;
}
