import {
  ClientMessage,
  InitialSnapshotMsg,
  PendingChoiceDto,
  PROTOCOL_VERSION,
  PublicTurnStatus,
  ServerMessage,
  StateUpdateMsg,
  ViewerRole,
} from './types.ts';
import { decodeInitialSnapshot, decodeServerMessage, isStaleServerMessage } from './decode.ts';

export type ConnectionStatus = 'connecting' | 'connected' | 'disconnected' | 'error';
export type SnapshotState = InitialSnapshotMsg | StateUpdateMsg;
export type GameLogEntry = import('./types.ts').GameEvent;

const MAX_EVENT_LOG_ENTRIES = 500;

export interface GameSessionState {
  status: ConnectionStatus;
  gameVersion: number;
  snapshot: SnapshotState | null;
  pendingChoice: PendingChoiceDto | null;
  turnStatus: PublicTurnStatus | null;
  lastError: string | null;
  events: GameLogEntry[];
}

export interface GameSessionClientOptions {
  gameId: string;
  viewer: ViewerRole;
  serverUrl?: string;
}

type Listener = () => void;

const initialState: GameSessionState = {
  status: 'connecting',
  gameVersion: 0,
  snapshot: null,
  pendingChoice: null,
  turnStatus: null,
  lastError: null,
  events: [],
};

/** Keeps the rendered audit log server-authored while bounding client memory use. */
export function serverEventLog(entries: readonly GameLogEntry[] | undefined): GameLogEntry[] {
  return (entries ?? []).slice(-MAX_EVENT_LOG_ENTRIES);
}

function rejectionMessage(message: Extract<ServerMessage, { type: 'action_rejected' }>): string {
  switch (message.reason.reason) {
    case 'stale_version':
      return `Rejected: Stale version (expected ${message.reason.expected}, server at ${message.reason.current})`;
    case 'stale_nonce':
      return 'Rejected: Stale decision nonce';
    case 'unauthorized_seat':
      return 'Rejected: Unauthorized seat';
    case 'unknown_option':
      return `Rejected: Unknown option '${message.reason.option_id}'`;
    default:
      return 'Action rejected';
  }
}

/** Applies only validated, non-stale protocol messages to the client projection. */
export function reduceServerMessage(state: GameSessionState, message: ServerMessage): GameSessionState {
  if (isStaleServerMessage(message, state.gameVersion)) return state;

  switch (message.type) {
    case 'initial_snapshot':
    case 'state_update':
      return {
        ...state,
        snapshot: message,
        gameVersion: message.game_version,
        turnStatus: message.turn_status,
        pendingChoice: message.pending_choice ?? null,
        events: message.type === 'initial_snapshot' ? serverEventLog(message.events) : state.events,
      };
    case 'event':
      if (state.events.some((entry) => entry.id === message.entry.id)) return state;
      return { ...state, events: serverEventLog([...state.events, message.entry]) };
    case 'pending_choice':
      return { ...state, gameVersion: message.game_version, pendingChoice: message.choice };
    case 'turn_status':
      return { ...state, gameVersion: message.game_version, turnStatus: message.status };
    case 'action_accepted':
      return { ...state, pendingChoice: null, lastError: null };
    case 'action_rejected':
      return { ...state, lastError: rejectionMessage(message) };
    case 'error':
      return { ...state, lastError: `Server Error: ${message.message}` };
    case 'game_over':
    case 'pong':
      return state;
  }
}

/** Owns every network ingress point and the lifecycle of one game-session connection. */
export class GameSessionClient {
  private state = initialState;
  private readonly listeners = new Set<Listener>();
  private socket: WebSocket | null = null;
  private stopped = false;
  private submittedNonce: string | null = null;
  private submissionResolve: (() => void) | null = null;

  constructor(private readonly options: GameSessionClientOptions) {}

  getState(): GameSessionState {
    return this.state;
  }

  subscribe(listener: Listener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  start(): void {
    this.stopped = false;
    this.setState({ ...this.state, status: 'connecting', lastError: null });
    void this.loadSnapshot();
    this.openSocket();
  }

  stop(): void {
    this.stopped = true;
    this.detachSocket();
    this.resolveSubmission();
  }

  async submitChoice(optionId: string): Promise<void> {
    const { pendingChoice, gameVersion } = this.state;
    if (!this.socket || this.socket.readyState !== WebSocket.OPEN) {
      this.setState({ ...this.state, lastError: 'Cannot submit choice: not connected to server' });
      return;
    }
    if (!pendingChoice) {
      this.setState({ ...this.state, lastError: 'No decision currently pending' });
      return;
    }
    if (this.submittedNonce === pendingChoice.nonce) return;

    const message: ClientMessage = {
      type: 'submit_choice',
      protocol_version: PROTOCOL_VERSION,
      game_id: this.options.gameId,
      nonce: pendingChoice.nonce,
      expected_version: gameVersion,
      option_id: optionId,
    };
    this.submittedNonce = pendingChoice.nonce;
    return new Promise((resolve) => {
      this.submissionResolve = resolve;
      this.socket?.send(JSON.stringify(message));
    });
  }

  private async loadSnapshot(): Promise<void> {
    try {
      const response = await fetch(this.snapshotUrl(), { headers: this.snapshotHeaders() });
      if (!response.ok) throw new Error(`Snapshot request failed (${response.status})`);
      this.ingestHttpSnapshot(await response.json());
    } catch (error) {
      if (!this.stopped) this.setState({ ...this.state, lastError: `Snapshot request failed: ${String(error)}` });
    }
  }

  private openSocket(): void {
    const socket = new WebSocket(this.webSocketUrl());
    this.socket = socket;
    socket.onopen = () => {
      if (this.stopped || this.socket !== socket) return;
      const seatToken = this.options.viewer.role === 'player' ? this.options.viewer.seatToken : undefined;
      const message: ClientMessage = {
        type: 'subscribe',
        protocol_version: PROTOCOL_VERSION,
        game_id: this.options.gameId,
        seat_token: seatToken,
      };
      socket.send(JSON.stringify(message));
      this.setState({ ...this.state, status: 'connected', lastError: null });
    };
    socket.onmessage = (event) => this.ingestWebSocket(event.data);
    socket.onerror = () => {
      if (!this.stopped && this.socket === socket) {
        this.setState({ ...this.state, status: 'error', lastError: 'WebSocket network error occurred' });
      }
    };
    socket.onclose = () => {
      if (!this.stopped && this.socket === socket) this.setState({ ...this.state, status: 'disconnected' });
    };
  }

  private ingestHttpSnapshot(value: unknown): void {
    this.apply(decodeInitialSnapshot(value, this.options.gameId) as Extract<ServerMessage, { type: 'initial_snapshot' }>);
  }

  private ingestWebSocket(value: unknown): void {
    try {
      this.apply(decodeServerMessage(typeof value === 'string' ? JSON.parse(value) : value, this.options.gameId));
    } catch (error) {
      if (!this.stopped) this.setState({ ...this.state, lastError: `Invalid server message: ${String(error)}` });
    }
  }

  private apply(message: ServerMessage): void {
    const previousChoice = this.state.pendingChoice;
    this.setState(reduceServerMessage(this.state, message));
    if (
      message.type === 'action_accepted' ||
      message.type === 'action_rejected' ||
      (this.submittedNonce !== null && previousChoice?.nonce === this.submittedNonce && this.state.pendingChoice?.nonce !== this.submittedNonce)
    ) {
      this.resolveSubmission();
    }
  }

  private resolveSubmission(): void {
    this.submittedNonce = null;
    this.submissionResolve?.();
    this.submissionResolve = null;
  }

  private detachSocket(): void {
    const socket = this.socket;
    this.socket = null;
    if (!socket) return;
    socket.onopen = null;
    socket.onmessage = null;
    socket.onerror = null;
    socket.onclose = null;
    if (socket.readyState === WebSocket.CONNECTING || socket.readyState === WebSocket.OPEN) socket.close();
  }

  private setState(next: GameSessionState): void {
    this.state = next;
    this.listeners.forEach((listener) => listener());
  }

  private webSocketUrl(): string {
    if (this.options.serverUrl) return this.options.serverUrl;
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    return `${protocol}//${window.location.host}/ws/games/${encodeURIComponent(this.options.gameId)}`;
  }

  private snapshotUrl(): string {
    if (!this.options.serverUrl) return `/api/games/${encodeURIComponent(this.options.gameId)}/snapshot`;
    const url = new URL(this.options.serverUrl, window.location.href);
    url.protocol = url.protocol === 'wss:' ? 'https:' : 'http:';
    url.pathname = `/api/games/${encodeURIComponent(this.options.gameId)}/snapshot`;
    url.search = '';
    return url.toString();
  }

  private snapshotHeaders(): HeadersInit {
    return this.options.viewer.role === 'player' && this.options.viewer.seatToken
      ? { 'x-ti4-seat-token': this.options.viewer.seatToken }
      : {};
  }
}
