import { describe, expect, it, vi } from 'vitest';
import { GameSessionClient, GameSessionState, reduceServerMessage } from './client.ts';
import { decodeServerMessage } from './decode.ts';
import { InitialSnapshotMsg, PROTOCOL_VERSION } from './types.ts';

const snapshot: InitialSnapshotMsg = {
  type: 'initial_snapshot',
  protocol_version: PROTOCOL_VERSION,
  game_id: 'game_12345',
  game_version: 4,
  viewer: { role: 'spectator' },
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
    expect(FakeWebSocket.latest?.readyState).toBe(3);
  });
});
