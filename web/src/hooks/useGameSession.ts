import { useState, useEffect, useRef, useCallback } from 'react';
import {
  ClientMessage,
  InitialSnapshotMsg,
  StateUpdateMsg,
  PendingChoiceDto,
  PublicTurnStatus,
  PROTOCOL_VERSION,
  ViewerRole,
} from '../protocol/types.ts';
import { decodeServerMessage, isStaleServerMessage } from '../protocol/decode.ts';

export type ConnectionStatus = 'connecting' | 'connected' | 'disconnected' | 'error';
export type SnapshotState = InitialSnapshotMsg | StateUpdateMsg;

export type GameLogEntry = import('../protocol/types.ts').GameEventDto;

const MAX_EVENT_LOG_ENTRIES = 500;

/** Keeps the rendered audit log server-authored while bounding client memory use. */
export function serverEventLog(entries: readonly GameLogEntry[] | undefined): GameLogEntry[] {
  return (entries ?? []).slice(-MAX_EVENT_LOG_ENTRIES);
}

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
  const [status, setStatus] = useState<ConnectionStatus>('connecting');
  const [gameVersion, setGameVersion] = useState<number>(0);
  const [snapshot, setSnapshot] = useState<SnapshotState | null>(null);
  const [pendingChoice, setPendingChoice] = useState<PendingChoiceDto | null>(null);
  const [turnStatus, setTurnStatus] = useState<PublicTurnStatus | null>(null);
  const [lastError, setLastError] = useState<string | null>(null);
  const [events, setEvents] = useState<GameLogEntry[]>([]);

  const wsRef = useRef<WebSocket | null>(null);
  const gameVersionRef = useRef(0);
  const submittedNonceRef = useRef<string | null>(null);
  const submissionResolveRef = useRef<(() => void) | null>(null);
  const viewerRef = useRef<ViewerRole>(viewer);
  viewerRef.current = viewer;
  gameVersionRef.current = gameVersion;

  const resolveSubmission = useCallback(() => {
    submittedNonceRef.current = null;
    submissionResolveRef.current?.();
    submissionResolveRef.current = null;
  }, []);

  const viewerKey = viewer.role === 'player' ? `player:${viewer.seat}` : 'spectator';

  const defaultWsUrl = useCallback(() => {
    if (serverUrl) return serverUrl;
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    return `${protocol}//${window.location.host}/ws/games/${gameId}`;
  }, [serverUrl, gameId]);

  const connect = useCallback(() => {
    if (wsRef.current) {
      const oldWs = wsRef.current;
      oldWs.onopen = null;
      oldWs.onmessage = null;
      oldWs.onerror = null;
      oldWs.onclose = null;
      if (oldWs.readyState === WebSocket.CONNECTING || oldWs.readyState === WebSocket.OPEN) {
        oldWs.close();
      }
      wsRef.current = null;
    }

    const url = defaultWsUrl();
    setStatus('connecting');
    setLastError(null);

    let closedIntentionally = false;
    const ws = new WebSocket(url);
    wsRef.current = ws;

    ws.onopen = () => {
      if (closedIntentionally) return;
      setStatus('connected');
      setLastError(null);

      // Subscribe immediately with viewer role
      const currentViewer = viewerRef.current;
      const seatToken = currentViewer.role === 'player' ? currentViewer.seatToken : undefined;
      const subMsg: ClientMessage = {
        type: 'subscribe',
        protocol_version: PROTOCOL_VERSION,
        game_id: gameId,
        seat_token: seatToken,
      };
      ws.send(JSON.stringify(subMsg));
    };

    ws.onmessage = (ev) => {
      if (closedIntentionally) return;
      try {
        const msg = decodeServerMessage(JSON.parse(ev.data), gameId);
        if (isStaleServerMessage(msg, gameVersionRef.current)) return;
        switch (msg.type) {
          case 'initial_snapshot': {
            setSnapshot(msg);
            setGameVersion(msg.game_version);
            setTurnStatus(msg.turn_status);
            setPendingChoice(msg.pending_choice ?? null);
            if (submittedNonceRef.current && msg.pending_choice?.nonce !== submittedNonceRef.current) resolveSubmission();
            setEvents(serverEventLog(msg.events));
            break;
          }
          case 'event': {
            setEvents((prev) => {
              if (prev.some((e) => e.id === msg.entry.id)) return prev;
              return serverEventLog([...prev, msg.entry]);
            });
            break;
          }
          case 'state_update': {
            setSnapshot((prev) => (prev ? { ...prev, ...msg } : msg));
            setGameVersion(msg.game_version);
            setTurnStatus(msg.turn_status);
            setPendingChoice(msg.pending_choice ?? null);
            if (submittedNonceRef.current && msg.pending_choice?.nonce !== submittedNonceRef.current) resolveSubmission();
            // Redundant "State updated to version X" spam intentionally omitted!
            break;
          }
          case 'pending_choice': {
            setPendingChoice(msg.choice);
            if (submittedNonceRef.current && msg.choice.nonce !== submittedNonceRef.current) resolveSubmission();
            setGameVersion(msg.game_version);
            break;
          }
          case 'turn_status': {
            setTurnStatus(msg.status);
            setGameVersion(msg.game_version);
            break;
          }
          case 'action_accepted': {
            resolveSubmission();
            setPendingChoice(null);
            setLastError(null);
            break;
          }
          case 'action_rejected': {
            resolveSubmission();
            let reasonStr = 'Action rejected';
            if (msg.reason.reason === 'stale_version') {
              reasonStr = `Rejected: Stale version (expected ${msg.reason.expected}, server at ${msg.reason.current})`;
            } else if (msg.reason.reason === 'stale_nonce') {
              reasonStr = 'Rejected: Stale decision nonce';
            } else if (msg.reason.reason === 'unauthorized_seat') {
              reasonStr = 'Rejected: Unauthorized seat';
            } else if (msg.reason.reason === 'unknown_option') {
              reasonStr = `Rejected: Unknown option '${msg.reason.option_id}'`;
            }
            setLastError(reasonStr);
            break;
          }
          case 'error': {
            setLastError(`Server Error: ${msg.message}`);
            break;
          }
          case 'game_over':
            break;
          case 'pong':
            break;
        }
      } catch (err) {
        console.error('Failed to parse WebSocket message:', err);
      }
    };

    ws.onerror = () => {
      if (closedIntentionally) return;
      setStatus('error');
      setLastError('WebSocket network error occurred');
    };

    ws.onclose = () => {
      if (closedIntentionally) return;
      setStatus('disconnected');
    };

    return () => {
      closedIntentionally = true;
      ws.onopen = null;
      ws.onmessage = null;
      ws.onerror = null;
      ws.onclose = null;
      if (ws.readyState === WebSocket.CONNECTING || ws.readyState === WebSocket.OPEN) {
        ws.close();
      }
    };
  }, [gameId, viewerKey, defaultWsUrl, resolveSubmission]);

  useEffect(() => {
    const cleanup = connect();
    return () => {
      cleanup?.();
      if (wsRef.current) {
        wsRef.current = null;
      }
    };
  }, [connect]);

  const submitChoice = useCallback(
    async (optionId: string) => {
      if (!wsRef.current || wsRef.current.readyState !== WebSocket.OPEN) {
        setLastError('Cannot submit choice: not connected to server');
        return;
      }
      if (!pendingChoice) {
        setLastError('No decision currently pending');
        return;
      }
      if (submittedNonceRef.current === pendingChoice.nonce) return;

      const msg: ClientMessage = {
        type: 'submit_choice',
        protocol_version: PROTOCOL_VERSION,
        game_id: gameId,
        nonce: pendingChoice.nonce,
        expected_version: gameVersion,
        option_id: optionId,
      };

      submittedNonceRef.current = pendingChoice.nonce;
      return new Promise<void>((resolve) => {
        submissionResolveRef.current = resolve;
        wsRef.current?.send(JSON.stringify(msg));
      });
    },
    [gameId, pendingChoice, gameVersion]
  );

  return {
    status,
    gameVersion,
    snapshot,
    pendingChoice,
    turnStatus,
    lastError,
    events,
    submitChoice,
  };
}
