import type { BoardView } from "./board";
import type { HelpView, SeatId, SeatView } from "./core";

/** What a player does in the lobby. A change applies at once: nothing is staged. */
export type LobbyIntent =
  /** Takes an open seat, as the player of this tab. */
  | { type: "join" }
  /** Takes an open seat as another player, for another tab of this browser. */
  | { type: "joinTab" }
  /** Makes this tab a player that this browser has at the table. */
  | { type: "become"; player: string }
  | { type: "addBot"; kind: string }
  | { type: "fillBots"; kind: string }
  | { type: "remove"; player: string }
  | { type: "start" }
  | { type: "inspectSystem"; system: string }
  | { type: "closeInspector" };

/** The one sign of a row, with its word (the tooltip and the accessible name). */
export interface LobbySignView {
  kind: "host" | "rule" | "random" | "away" | "open";
  word: string;
}

export interface LobbyButtonView {
  label: string;
  /** What the button does, when the label does not say it. */
  title?: string;
  intent: LobbyIntent;
}

/** One seat of the table, in the order in which the players came. The list has no numbers. */
export interface LobbyRowView {
  id: string;
  sign: LobbySignView | null;
  /** The nickname, or "Open". */
  name: string;
  /** The row of the viewer. */
  you: boolean;
  /** Another player of this browser. The row is the control: the tab becomes that player. */
  become: LobbyIntent | null;
  /** The name of the faction, or "Random". Empty for an open seat. */
  faction: string;
  /** The controls of the row: the kinds of an open seat, "Join in a new tab", "×". */
  buttons: LobbyButtonView[];
  /** Removes the seat. Shown as "×". */
  remove: LobbyButtonView | null;
}

/** One value of the setup. `editable`: the viewer may change it (no control in this phase). */
export interface LobbySetupRowView {
  id: string;
  label: string;
  value: string;
  editable: boolean;
}

export interface LobbyFooterView {
  /** Why the main button waits, or who the table waits for. */
  note: string;
  /** The main button. Enter is this button. Null: the viewer has none. */
  main: { label: string; disabled: boolean; intent: LobbyIntent } | null;
  /** A visitor gives a name before the join. */
  nickname: boolean;
}

/** The lobby of a table: the map, the players and the setup. The same screen for everyone. */
export interface LobbyView {
  /** "Lobby · 6 players" */
  title: string;
  /** "Offline · reconnecting", or empty. */
  status: string;
  board: BoardView;
  /** The seats that the board draws: the place of a seat, not yet given to a player. */
  seats: Record<SeatId, SeatView>;
  setup: LobbySetupRowView[];
  /** "3 of 6" */
  filled: string;
  /** What the kinds of a seat that the server plays do. Behind "?". */
  help: HelpView[];
  rows: LobbyRowView[];
  /** The button for the usual target, under the list. */
  fill: LobbyButtonView | null;
  footer: LobbyFooterView;
}
