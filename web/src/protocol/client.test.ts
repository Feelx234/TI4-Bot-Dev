import { afterEach, describe, expect, it, vi } from 'vitest';
import { GameSessionClient, GameSessionState, reduceServerMessage } from './client.ts';
import { decodeCreateGameResponse, decodeJoinResponse, decodeLobby, decodeServerMessage } from './decode.ts';
import { InitialSnapshotMsg, PROTOCOL_VERSION } from './types.ts';

const snapshot: InitialSnapshotMsg = {
  type: 'initial_snapshot',
  protocol_version: PROTOCOL_VERSION,
  game_id: 'game_12345',
  game_version: 4,
  viewer: { role: 'spectator' },
  state: {},
  galaxy_layout: { version: 1, active_sources: [], placements: [] },
  view: {
    round: 1,
    phase: 'strategy',
    speaker: 'seat_a',
    seating_order: ['seat_a'],
    finished: false,
    players: [],
    board: { systems: {} },
    table: { revealed_objectives: [], scored_objectives: {}, unclaimed_strategy_cards: [], strategy_card_goods: {}, laws: {} },
  },
  turn_status: { kind: 'active_turn', player: 'seat_a', phase: 'strategy', round: 1 },
  events: [],
};

const state: GameSessionState = {
  status: 'connected',
  gameVersion: 0,
  snapshot: null,
  pendingChoice: null,
  turnStatus: null,
  lastError: null,
  events: [],
};

describe('GameSessionClient reducer', () => {
  it('uses one snapshot reducer and refuses older state-bearing messages', () => {
    const current = reduceServerMessage(state, decodeServerMessage(snapshot, 'game_12345'));
    const stale = reduceServerMessage(current, decodeServerMessage({
      ...snapshot,
      type: 'state_update',
      game_version: 3,
      view: { ...snapshot.view, round: 99 },
    }, 'game_12345'));

    expect(current.snapshot?.view.round).toBe(1);
    expect(stale).toBe(current);
  });

  it('replaces the projection with an accepted state update instead of merging stale snapshot fields', () => {
    const current = reduceServerMessage(state, decodeServerMessage(snapshot, 'game_12345'));
    const update = reduceServerMessage(current, decodeServerMessage({
      ...snapshot,
      type: 'state_update',
      game_version: 5,
      view: { ...snapshot.view, active_player: 'seat_a' },
      pending_choice: { prompt: 'Choose', actor: 'seat_a', nonce: 'nonce-5', options: [] },
    }, 'game_12345'));

    expect(update.gameVersion).toBe(5);
    expect(update.snapshot?.type).toBe('state_update');
    expect(update.pendingChoice?.nonce).toBe('nonce-5');
  });
});

describe('lobby decoding', () => {
  it('accepts the actual server-generated 256-bit player ID and session credential on create and join', () => {
    const playerId = `player_${'a'.repeat(64)}`;
    const playerSession = `session_${'b'.repeat(64)}`;
    const lobby = { game_id: 'game_12345', phase: 'lobby', lobby_version: 1, host_player_id: playerId,
      slots: [{ slot_id: 'slot_1', position: 1, occupant: playerId, ready: false, connected: false, can_take_over: false },
        { slot_id: 'slot_2', position: 2, occupant: null, ready: false, connected: false, can_take_over: false }] };
    const created = decodeCreateGameResponse({ game_id: 'game_12345', player_session: playerSession, player: { id: playerId }, lobby });
    expect(created.player_session).toBe(playerSession);
    expect(created.player.id).toBe(playerId);
    expect(decodeJoinResponse({ player_session: playerSession, player: { id: playerId }, lobby }, 'game_12345').player_session).toBe(playerSession);
    expect(() => decodeCreateGameResponse({ ...created, player_session: 'x'.repeat(129) })).toThrow(/invalid game creation response/);
  });

  it('decodes public slots without a credential or viewer identity', () => {
    const lobby = decodeLobby({
      game_id: 'game_12345', phase: 'running', lobby_version: 1, host_player_id: 'player_a',
      slots: [{ slot_id: 'slot_1', position: 1, occupant: 'player_a', ready: true, connected: false, can_take_over: true }, { slot_id: 'slot_2', position: 2, occupant: 'player_b', ready: true, connected: true, can_take_over: false }],
    }, 'game_12345');
    expect(lobby.slots[0].occupant).toBe('player_a');
    expect(JSON.stringify(lobby)).not.toContain('player_session');
  });
});

class FakeWebSocket {
  static CONNECTING = 0;
  static OPEN = 1;
  static latest: FakeWebSocket | null = null;
  readyState = FakeWebSocket.CONNECTING;
  onopen: (() => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onerror: (() => void) | null = null;
  onclose: (() => void) | null = null;
  sent: string[] = [];

  constructor(_url: string) {
    FakeWebSocket.latest = this;
  }

  send(value: string): void {
    this.sent.push(value);
  }

  close(): void {
    this.readyState = 3;
    this.onclose?.();
  }
}

describe('GameSessionClient ingress lifecycle', () => {
  it('routes the HTTP snapshot through the same validated reducer and closes its socket on stop', async () => {
    vi.stubGlobal('WebSocket', FakeWebSocket);
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, json: async () => snapshot }));
    const client = new GameSessionClient({ gameId: 'game_12345', viewer: { role: 'spectator' } });

    client.start();
    await vi.waitFor(() => expect(client.getState().gameVersion).toBe(4));
    client.stop();

    expect(client.getState().snapshot?.type).toBe('initial_snapshot');
    expect(client.getState().lastError).toBeNull();
    expect(FakeWebSocket.latest?.readyState).toBe(3);
  });

  it('subscribes as the authenticated player, pings every ten seconds, and resumes after disconnect', async () => {
    vi.useFakeTimers();
    vi.stubGlobal('WebSocket', FakeWebSocket);
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, json: async () => ({ ...snapshot, viewer: { role: 'player', seat: 'player_a' } }) }));
    const client = new GameSessionClient({ gameId: 'game_12345', viewer: { role: 'player', seat: 'player_a', playerSession: 'private' } });
    client.start();
    const socket = FakeWebSocket.latest!;
    socket.readyState = FakeWebSocket.OPEN;
    socket.onopen?.();
    expect(JSON.parse(socket.sent[0])).toMatchObject({ type: 'subscribe', protocol_version: 3, player_session: 'private' });
    vi.advanceTimersByTime(10_000);
    expect(JSON.parse(socket.sent[1])).toMatchObject({ type: 'ping', sequence: 1 });
    socket.onclose?.();
    vi.advanceTimersByTime(2_000);
    expect(FakeWebSocket.latest).not.toBe(socket);
    client.stop();
    vi.useRealTimers();
  });

  it('does not render a snapshot authenticated for a different player', async () => {
    vi.stubGlobal('WebSocket', FakeWebSocket);
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, json: async () => ({ ...snapshot, viewer: { role: 'player', seat: 'player_b' } }) }));
    const client = new GameSessionClient({ gameId: 'game_12345', viewer: { role: 'player', seat: 'player_a', playerSession: 'private' } });
    client.start();
    await vi.waitFor(() => expect(client.getState().lastError).toMatch(/viewer identity/));
    expect(client.getState().snapshot).toBeNull();
    client.stop();
  });
});

afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });
