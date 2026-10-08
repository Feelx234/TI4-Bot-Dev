import { useCallback, useEffect, useState } from "react";
import { PendingChoiceDto, PublicTurnStatus, ViewerRole } from "../protocol/types.ts";
import {
  ConnectionStatus,
  GameLogEntry,
  GameSessionClient,
  GameSessionState,
  serverEventLog,
  SnapshotState,
} from "../protocol/client.ts";

export { serverEventLog };
export type { ConnectionStatus, GameLogEntry, SnapshotState };

export interface UseGameSessionOptions {
  gameId: string;
  viewer: ViewerRole;
  serverUrl?: string;
}

export interface UseGameSessionReturn {
  status: ConnectionStatus;
  gameVersion: number;
  snapshot: SnapshotState | null;
  pendingChoice: PendingChoiceDto | null;
  turnStatus: PublicTurnStatus | null;
  lastError: string | null;
  events: GameLogEntry[];
  history: import("../protocol/types.ts").HistoryStatus;
  batchResume?: import("../protocol/client.ts").BatchResume | null;
  submitChoice: (optionId: string) => Promise<void>;
  /** This seat's bluff settings as the server holds them (null until it has answered). */
  reactionIntent: import("../protocol/types.ts").ReactionIntentStateMsg | null;
  /** Declare the reaction windows this seat bluffs about (whole set; empty clears). */
  setReactionIntent: (triggers: string[]) => void;
  /** End the running bluff hold early. */
  passReactionHold: () => void;
  /** Never (or again) offer one action card, by printed name, to this seat. */
  setReactionMode: (card: string, mode: import("../protocol/types.ts").ReactionModeSetting) => void;
  changeHistory: (action: import("../protocol/client.ts").HistoryChange) => Promise<void>;
  /** The game's replay JSON for copying out (any seated player). */
  fetchReplay: () => Promise<{ text: string; filename: string }>;
  /** Turn redo: where it stands (null when none is in flight), and the commands that change it. */
  fetchTurnRedoStatus: () => Promise<import("../protocol/turnRedo.ts").TurnRedoStatus | null>;
  turnRedoCommand: (command: import("../protocol/turnRedo.ts").TurnRedoCommand) => Promise<void>;
  submitMovementBatch: (
    destination: string,
    steps: import("../protocol/client.ts").MovementStep[],
  ) => Promise<void>;
  submitBatch: (plan: import("../protocol/client.ts").BasketPlan) => Promise<void>;
  resumeBatch: () => Promise<void>;
  dismissBatchResume: () => void;
}

export function useGameSession({
  gameId,
  viewer,
  serverUrl,
}: UseGameSessionOptions): UseGameSessionReturn {
  const [client] = useState(() => new GameSessionClient({ gameId, viewer, serverUrl }));
  const [state, setState] = useState<GameSessionState>(() => client.getState());

  useEffect(() => {
    const unsubscribe = client.subscribe(() => setState(client.getState()));
    client.start();
    return () => {
      unsubscribe();
      client.stop();
    };
  }, [client]);

  const submitChoice = useCallback((optionId: string) => client.submitChoice(optionId), [client]);
  const changeHistory = useCallback(
    (action: import("../protocol/client.ts").HistoryChange) => client.changeHistory(action),
    [client],
  );

  return {
    ...state,
    submitChoice,
    reactionIntent: state.reactionIntent ?? null,
    setReactionIntent: (triggers) => client.setReactionIntent(triggers),
    passReactionHold: () => client.passReactionHold(),
    setReactionMode: (card, mode) => client.setReactionMode(card, mode),
    changeHistory,
    fetchReplay: () => client.fetchReplay(),
    fetchTurnRedoStatus: () => client.fetchTurnRedoStatus(),
    turnRedoCommand: (command) => client.turnRedoCommand(command),
    submitMovementBatch: (destination, steps) => client.submitMovementBatch(destination, steps),
    submitBatch: (plan) => client.submitBatch(plan),
    resumeBatch: () => client.resumeBatch(),
    dismissBatchResume: () => client.dismissBatchResume(),
  };
}
