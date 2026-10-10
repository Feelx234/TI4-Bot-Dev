import type { ActionView } from "./action";
import type { BoardView, ExplorationDeck } from "./board";
import type { Rich, SeatId, SeatView } from "./core";
import type { Intent } from "./intents";

export type Workspace = "live" | "draft" | "history";

export interface ToolbarView {
  round: number;
  phase: string;
  /** "Tactical action complete", or what the game waits for. */
  status: string;
  workspace: Workspace;
  /** False when the viewer may not draft, with the reason. */
  draftLocked: string | null;
  past: { label: string } | null;
  /** Controls of the private draft. Null outside the draft. */
  draft: { canUndo: boolean; canRedo: boolean; canApply: boolean; applyHint: string } | null;
}

/** One row of the player table. Compare `PlayerView`. */
export interface PlayerRowView {
  seat: SeatId;
  isMe: boolean;
  turn: "now" | "next" | null;
  speaker: boolean;
  passed: boolean;
  victoryPoints: number;
  /** One card for each player with five or more players, two with four or fewer. In initiative order. */
  strategyCards: { number: number; name: string; used: boolean }[];
  resources: [ready: number, total: number];
  influence: [ready: number, total: number];
  tradeGoods: number;
  commodities: [held: number, limit: number];
  tokens: [tactic: number, fleet: number, strategy: number];
  actionCards: number;
  secrets: [scored: number, held: number];
  planets: number;
  abilities: string[];
  technologies: string[];
  leaders: string;
  relics: string[];
  /** Relic fragments in the play area, by deck. Decks with none are left out. */
  fragments: { deck: ExplorationDeck; count: number }[];
}

/** The open task lets the viewer choose a player. The player table is the picker. */
export interface PlayerPickView {
  /** "Choose", for the label of a row. */
  verb: string;
  /** The seats in the choice. A reason says why a seat cannot be chosen. */
  seats: Record<SeatId, { reason: string | null; note?: string }>;
  /** One seat, or several when the task allows it. */
  chosen: SeatId[];
}

export interface PlayerTableView {
  pick: PlayerPickView | null;
  rows: PlayerRowView[];
  freeCards: { number: number; name: string; tradeGoods: number }[];
}

export interface ListSectionView {
  title: string;
  rows: Rich[];
}

export interface LogEntryView {
  id: string;
  seat: SeatId;
  type: string;
  summary: string;
  stages: { name: string; text: string }[];
  /** The action that is open in Live. */
  current: boolean;
}

/** One card of an exploration deck. Cards of one name are one row. */
export interface ExplorationCardView {
  name: string;
  /** "Instant", "Fragment", "Attach" or "Token". */
  kind: string;
  text: string;
  left: number;
  total: number;
  /** Who drew a card of this name, and where: "Round 2 · Sol · Vega Major". */
  drawn: string[];
}

/** One exploration deck: what it holds, and who has its relic fragments. */
export interface ExplorationDeckView {
  deck: ExplorationDeck;
  title: string;
  left: number;
  total: number;
  fragments: { seat: SeatId; count: number }[];
  cards: ExplorationCardView[];
}

export interface ReferenceView {
  objectives: ListSectionView[];
  technology: ListSectionView[];
  cards: ListSectionView[];
  /** The viewer's cards that are played in a reaction window, with the offer setting of each. */
  offerCards: { card: string; text: string; never: boolean }[];
  /** The exploration decks, in the order cultural, hazardous, industrial, frontier. */
  exploration: ExplorationDeckView[];
  /** `current` is the round in play: the log shows it open. */
  log: { round: number; label: string; current: boolean; entries: LogEntryView[] }[];
}

export interface ApplyView {
  items: string[];
  cost: string;
  note: string;
}

export interface ShellView {
  seats: Record<SeatId, SeatView>;
  /** Colours the frame: a private draft, the live game, or a complete action. */
  accent: "draft" | "live" | "done";
  toolbar: ToolbarView;
  players: PlayerTableView;
  board: BoardView;
  action: ActionView;
  reference: ReferenceView;
  /** What "Apply to Live" will commit. Null when there is nothing to apply. */
  apply: ApplyView | null;
  /** A list row to scroll into view, after the player chose its counterpart on the board. */
  reveal: string | null;
  toast: { id: number; text: string } | null;
  announcement: string;
}

/** The only thing the game shell needs. The mock implements it now; a websocket session will later. */
export interface GameSession {
  view: ShellView;
  dispatch: (intent: Intent) => void;
}
