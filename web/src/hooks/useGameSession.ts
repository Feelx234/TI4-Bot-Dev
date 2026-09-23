import { useCallback, useEffect, useState } from 'react';
import { PendingChoiceDto, PublicTurnStatus, ViewerRole } from '../protocol/types.ts';
import {
  ConnectionStatus,
  GameLogEntry,
  GameSessionClient,
  GameSessionState,
  serverEventLog,
  SnapshotState,
} from '../protocol/client.ts';

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
  submitChoice: (optionId: string) => Promise<void>;
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

  return {
    ...state,
    submitChoice,
  };
}
