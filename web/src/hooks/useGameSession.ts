import { useState, useEffect, useRef, useCallback } from 'react';
import {
  ClientMessage,
  ServerMessage,
  InitialSnapshotMsg,
  StateUpdateMsg,
  PendingChoiceDto,
  PublicTurnStatus,
  PROTOCOL_VERSION,
  ViewerRole,
} from '../protocol/types.ts';
import { formatActionDescription } from '../protocol/contentCatalog.ts';

export type ConnectionStatus = 'connecting' | 'connected' | 'disconnected' | 'error';
export type SnapshotState = InitialSnapshotMsg | StateUpdateMsg;

export interface GameLogEntry {
  id: string;
  timestamp: string;
  version?: number;
  text: string;
  category?: 'system' | 'action' | 'decision' | 'status' | 'phase' | 'error';
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
  fetchSnapshot: () => Promise<InitialSnapshotMsg | null>;
  reconnect: () => void;
}

function formatTimestamp(): string {
  const d = new Date();
  return d.toTimeString().split(' ')[0];
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
  const viewerRef = useRef<ViewerRole>(viewer);
  viewerRef.current = viewer;

  const viewerKey = viewer.role === 'player' ? `player:${viewer.seat}` : 'spectator';

  const defaultWsUrl = useCallback(() => {
    if (serverUrl) return serverUrl;
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    return `${protocol}//${window.location.host}/ws/games/${gameId}`;
  }, [serverUrl, gameId]);

  const addLog = useCallback(
    (text: string, category: GameLogEntry['category'] = 'system', version?: number) => {
      const entry: GameLogEntry = {
        id: `${Date.now()}-${Math.random().toString(36).substring(2, 7)}`,
        timestamp: formatTimestamp(),
        version,
        text,
        category,
      };
      setEvents((prev) => [...prev, entry]);
    },
    []
  );

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
      const seatToken = currentViewer.role === 'player' ? currentViewer.seat : undefined;
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
        const msg = JSON.parse(ev.data) as ServerMessage;
        switch (msg.type) {
          case 'initial_snapshot': {
            setSnapshot(msg);
            setGameVersion(msg.game_version);
            setTurnStatus(msg.turn_status);
            setPendingChoice(msg.pending_choice ?? null);
            if (msg.events && msg.events.length > 0) {
              setEvents(
                msg.events.map((e) => ({
                  id: e.id,
                  timestamp: e.timestamp,
                  version: e.version,
                  text: e.text,
                  category: e.category,
                }))
              );
            } else {
              addLog(
                `Game initialized (Round ${msg.view.round}, ${msg.view.phase.toUpperCase()} Phase, Speaker: ${msg.view.speaker})`,
                'system',
                msg.game_version
              );
              if (msg.pending_choice) {
                addLog(
                  `Decision required for ${msg.pending_choice.actor}: ${msg.pending_choice.prompt}`,
                  'decision',
                  msg.game_version
                );
              }
            }
            break;
          }
          case 'event': {
            setEvents((prev) => {
              if (prev.some((e) => e.id === msg.entry.id)) return prev;
              return [
                ...prev,
                {
                  id: msg.entry.id,
                  timestamp: msg.entry.timestamp,
                  version: msg.entry.version,
                  text: msg.entry.text,
                  category: msg.entry.category,
                },
              ];
            });
            break;
          }
          case 'state_update': {
            setSnapshot((prev) => {
              if (
                prev &&
                (prev.view.phase !== msg.view.phase || prev.view.round !== msg.view.round)
              ) {
                addLog(
                  `Phase transition: ${msg.view.phase.toUpperCase()} Phase (Round ${msg.view.round})`,
                  'phase',
                  msg.game_version
                );
              }
              return prev ? { ...prev, ...msg } : msg;
            });
            setGameVersion(msg.game_version);
            setTurnStatus(msg.turn_status);
            setPendingChoice(msg.pending_choice ?? null);
            // Redundant "State updated to version X" spam intentionally omitted!
            break;
          }
          case 'pending_choice': {
            setPendingChoice(msg.choice);
            setGameVersion(msg.game_version);
            addLog(
              `Decision required for ${msg.choice.actor}: ${msg.choice.prompt}`,
              'decision',
              msg.game_version
            );
            break;
          }
          case 'turn_status': {
            setTurnStatus(msg.status);
            setGameVersion(msg.game_version);
            if (msg.status.kind === 'active_turn') {
              addLog(
                `Active Turn: ${msg.status.player} (Round ${msg.status.round}, ${msg.status.phase})`,
                'status',
                msg.game_version
              );
            } else if (msg.status.kind === 'phase_transition') {
              addLog(
                `Phase Transition: ${msg.status.phase} Phase (Round ${msg.status.round})`,
                'phase',
                msg.game_version
              );
            }
            break;
          }
          case 'action_accepted': {
            setPendingChoice(null);
            setLastError(null);
            const formattedAction = formatActionDescription(msg.option_id);
            addLog(`Action accepted: ${formattedAction}`, 'action', msg.game_version);
            break;
          }
          case 'action_rejected': {
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
            addLog(reasonStr, 'error', msg.game_version);
            break;
          }
          case 'error': {
            setLastError(`Server Error: ${msg.message}`);
            addLog(`Server Error: ${msg.message}`, 'error');
            break;
          }
          case 'game_over': {
            addLog(
              `Game Over! Winner: ${msg.winner ?? 'Draw'}`,
              'status',
              msg.game_version
            );
            break;
          }
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
  }, [gameId, viewerKey, defaultWsUrl, addLog]);

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

      const msg: ClientMessage = {
        type: 'submit_choice',
        protocol_version: PROTOCOL_VERSION,
        game_id: gameId,
        nonce: pendingChoice.nonce,
        expected_version: gameVersion,
        option_id: optionId,
      };

      wsRef.current.send(JSON.stringify(msg));
    },
    [gameId, pendingChoice, gameVersion]
  );

  const fetchSnapshot = useCallback(async (): Promise<InitialSnapshotMsg | null> => {
    try {
      const protocol = window.location.protocol;
      const host = window.location.host;
      const seatParam = viewer.role === 'player' ? `?seat=${viewer.seat}` : '?seat=spectator';
      const res = await fetch(`${protocol}//${host}/api/games/${gameId}/snapshot${seatParam}`);
      if (!res.ok) return null;
      const data: InitialSnapshotMsg = await res.json();
      setSnapshot(data);
      setGameVersion(data.game_version);
      setTurnStatus(data.turn_status);
      setPendingChoice(data.pending_choice ?? null);
      if (data.events && data.events.length > 0) {
        setEvents(
          data.events.map((e) => ({
            id: e.id,
            timestamp: e.timestamp,
            version: e.version,
            text: e.text,
            category: e.category,
          }))
        );
      }
      return data;
    } catch (e) {
      console.error('Failed to fetch snapshot via HTTP:', e);
      return null;
    }
  }, [gameId, viewer]);

  return {
    status,
    gameVersion,
    snapshot,
    pendingChoice,
    turnStatus,
    lastError,
    events,
    submitChoice,
    fetchSnapshot,
    reconnect: connect,
  };
}
