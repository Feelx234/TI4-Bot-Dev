import type { MovementPlan, MovementStep } from "./movementPlan";
import type { DraftOutcome, DraftStep, SessionUpdate } from "./wire";

/** A plan that did not reach its end. Compare `BatchInterruption` and `BatchFailure` of the server. */
export interface PlanStopped {
  /** The active system of the plan. */
  destination: string;
  /** How many steps are in the game. 0 when the plan was refused: the game is as before. */
  applied: number;
  /** The steps that are not in the game. */
  remaining: MovementStep[];
  /** True when the game asked something else first; false when it refused a step. */
  interrupted: boolean;
  reason: string;
}

/** What a transport tells its listeners: the game as the viewer is shown it, or why it stopped. */
export type TransportEvent =
  | {
      kind: "update";
      update: SessionUpdate;
      /** The last answer can be taken back. */ canUndo: boolean;
      /** The plan that was sent before this update stopped here. */
      plan?: PlanStopped;
      /** The steps of the movement that the undo before this update took back, in order. */
      undone?: MovementStep[];
      /** The game waits before a decision of this seat, which is not played here: `step` goes on. */
      stepping?: { seat: string };
    }
  /** The game is played again up to where it was. The last update is out of date until the next. */
  | { kind: "replaying"; done: number; total: number }
  /** The game cannot go on. The text names the cause. */
  | { kind: "error"; message: string };

/**
 * Where the game runs. The wasm engine in this page implements it now; the websocket to the
 * server will later, with `StateUpdateMsg` as the update and `SubmitChoice` as the answer.
 */
export interface Transport {
  /** A new listener is told the latest event at once, if there is one. */
  subscribe(listener: (event: TransportEvent) => void): () => void;
  /** Answers the pending choice that has this nonce. Any other nonce is ignored. */
  submitChoice(nonce: string, optionId: string): void;
  /**
   * Answers the pending choice and the choices after it with the steps of a movement, as one
   * request. A step that the game refuses takes the whole plan back. A question of the game that
   * is not of the movement stops the plan there. Either is told with the next update.
   */
  submitPlan(nonce: string, plan: MovementPlan): void;
  /**
   * Plays a draft script as the next turn of this side, on a copy of the game, and tells where
   * it ended. The game is not changed. Null while the game does not wait for this side.
   */
  runDraft(script: DraftStep[]): DraftOutcome | null;
  /**
   * Sends a draft to the game: the pending choice and the choices after it are answered with
   * the options of these ids, then with the steps of the movement, as one request with the
   * rules of `submitPlan`.
   */
  applyDraft(nonce: string, draft: { choices: string[]; plan: MovementPlan | null }): void;
  /**
   * Whether the game waits before each decision of a seat that is not played here. The seats
   * decide the same either way.
   */
  setStepping(on: boolean): void;
  /** Lets the seat decide that the game waits for (`stepping` of the last update). */
  step(): void;
  /**
   * Plays this seat here from now on, in place of the seat that the game is shown as: that seat
   * decides at random from its open decision on. Does nothing while the game does not wait for
   * this side, or when the seat is played here already.
   */
  switchSeat(seat: string): void;
  /**
   * Takes back the last answer of this side; a movement is taken back whole, and the next update
   * tells its steps. Does nothing when there is none. An answer from before the last
   * `switchSeat` stays.
   */
  undo(): void;
  /** Ends the game on this side. */
  close(): void;
}
