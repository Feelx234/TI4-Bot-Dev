import { afterEach, describe, expect, it, vi } from 'vitest';
import { GameSessionClient, GameSessionState, reduceServerMessage } from './client.ts';
import { decodeCreateGameResponse, decodeJoinResponse, decodeLobby, decodeServerMessage } from './decode.ts';
import { InitialSnapshotMsg, PROTOCOL_VERSION } from './types.ts';
import { validNickname } from './nickname.ts';

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
  it('mirrors the server byte bound and rejects whitespace, controls, and format characters', () => {
    for (const name of ['Z', 'A'.repeat(64), '🪐'.repeat(16), 'Ana María', 'Same']) expect(validNickname(name)).toBe(true);
    for (const name of ['', ' ', ' x', 'x ', '🪐'.repeat(17), 'x\n', 'x\u202e', 'x\u200b', 'x\u{e0001}']) expect(validNickname(name)).toBe(false);
  });
  it('accepts the actual server-generated 256-bit player ID and session credential on create and join', () => {
    const playerId = `player_${'a'.repeat(64)}`;
    const playerSession = `session_${'b'.repeat(64)}`;
    const lobby = { game_id: 'game_12345', phase: 'lobby', lobby_version: 1, host_player_id: playerId,
      slots: [{ slot_id: 'slot_1', position: 1, occupant: playerId, nickname: 'Host', ready: false, connected: false, can_take_over: false },
        { slot_id: 'slot_2', position: 2, occupant: null, nickname: null, ready: false, connected: false, can_take_over: false }] };
    const created = decodeCreateGameResponse({ game_id: 'game_12345', player_session: playerSession, player: { id: playerId }, lobby });
    expect(created.player_session).toBe(playerSession);
    expect(created.player.id).toBe(playerId);
    expect(decodeJoinResponse({ player_session: playerSession, player: { id: playerId }, lobby }, 'game_12345').player_session).toBe(playerSession);
    expect(() => decodeCreateGameResponse({ ...created, player_session: 'x'.repeat(129) })).toThrow(/invalid game creation response/);
  });

  it('decodes public slots without a credential or viewer identity', () => {
    const lobby = decodeLobby({
      game_id: 'game_12345', phase: 'running', lobby_version: 1, host_player_id: 'player_a',
       slots: [{ slot_id: 'slot_1', position: 1, occupant: 'player_a', nickname: 'Same', ready: true, connected: false, can_take_over: true }, { slot_id: 'slot_2', position: 2, occupant: 'player_b', nickname: 'Same', ready: true, connected: true, can_take_over: false }],
    }, 'game_12345');
    expect(lobby.slots[0].occupant).toBe('player_a');
    expect(lobby.slots.map((slot) => slot.nickname)).toEqual(['Same', 'Same']);
    expect(JSON.stringify(lobby)).not.toContain('player_session');
    expect(() => decodeLobby({ ...lobby, slots: [{ ...lobby.slots[0], nickname: 'x\u202e' }, lobby.slots[1]] }, 'game_12345')).toThrow(/invalid lobby slot/);
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
  async function connectedPlayer() {
    vi.stubGlobal('WebSocket', FakeWebSocket);
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, json: async () => ({
      ...snapshot, viewer: { role: 'player', seat: 'player_a' },
      pending_choice: { nonce: 'nonce-4', choice: { player: 'player_a', prompt: 'Choose', options: [{ id: 'opt-4', label: 'Choose' }] } },
    }) }));
    const client = new GameSessionClient({ gameId: 'game_12345', viewer: { role: 'player', seat: 'player_a', playerSession: 'private' } });
    client.start();
    const socket = FakeWebSocket.latest!;
    socket.readyState = FakeWebSocket.OPEN;
    socket.onopen?.();
    await vi.waitFor(() => expect(client.getState().pendingChoice?.nonce).toBe('nonce-4'));
    const send = (message: object) => socket.onmessage?.({ data: JSON.stringify({ protocol_version: PROTOCOL_VERSION, game_id: 'game_12345', ...message }) } as MessageEvent);
    return { client, socket, send };
  }

  it.each([{ reason: 'stale_nonce' }, { reason: 'stale_version', expected: 4, current: 5 }])(
    'rejects a refused submission ($reason) and allows retry', async (reason) => {
      const { client, socket, send } = await connectedPlayer();
      const submitted = client.submitChoice('opt-4');
      const refused = expect(submitted).rejects.toThrow(/Rejected:/);
      expect(JSON.parse(socket.sent.at(-1)!)).toMatchObject({ type: 'submit_choice', option_id: 'opt-4', nonce: 'nonce-4', expected_version: 4 });
      send({ type: 'action_rejected', game_version: 4, reason });
      await refused;
      expect(client.getState().lastError).toMatch(/Rejected:/);
      const retried = client.submitChoice('opt-4');
      expect(socket.sent.filter((message) => JSON.parse(message).type === 'submit_choice')).toHaveLength(2);
      const stopped = expect(retried).rejects.toThrow(/disconnected|stopped/i);
      client.stop();
      await stopped;
    },
  );

  it.each(['ack-first', 'update-first'])('waits for acceptance and a newer authoritative state (%s)', async (order) => {
    const { client, send } = await connectedPlayer();
    const submitted = client.submitChoice('opt-4');
    let settled = false;
    void submitted.then(() => { settled = true; });
    const accepted = () => send({ type: 'action_accepted', game_version: 4, option_id: 'opt-4' });
    const update = () => send({ ...snapshot, type: 'state_update', game_version: 5,
      viewer: { role: 'player', seat: 'player_a' },
      pending_choice: { nonce: 'nonce-5', choice: { player: 'player_a', prompt: 'Next', options: [{ id: 'opt-5', label: 'Next' }] } },
    });
    if (order === 'ack-first') accepted(); else update();
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(settled).toBe(false);
    if (order === 'ack-first') update(); else accepted();
    await submitted;
    expect(client.getState().pendingChoice?.nonce).toBe('nonce-5');
    client.stop();
  });

  it('rejects an in-flight submission on disconnect and permits a fresh submission after reconnect', async () => {
    const { client, socket } = await connectedPlayer();
    vi.useFakeTimers();
    const submitted = client.submitChoice('opt-4');
    const failed = expect(submitted).rejects.toThrow(/disconnected/i);
    socket.onclose?.();
    await failed;
    await expect(client.submitChoice('opt-4')).rejects.toThrow(/not connected/i);
    await vi.advanceTimersByTimeAsync(2_000);
    const reconnected = FakeWebSocket.latest!;
    expect(reconnected).not.toBe(socket);
    reconnected.readyState = FakeWebSocket.OPEN;
    reconnected.onopen?.();
    reconnected.onmessage?.({ data: JSON.stringify({ ...snapshot, viewer: { role: 'player', seat: 'player_a' },
      pending_choice: { nonce: 'nonce-4', choice: { player: 'player_a', prompt: 'Choose', options: [{ id: 'opt-4', label: 'Choose' }] } },
    }) } as MessageEvent);
    const retried = client.submitChoice('opt-4');
    expect(JSON.parse(reconnected.sent.at(-1)!)).toMatchObject({ type: 'submit_choice', nonce: 'nonce-4' });
    reconnected.onmessage?.({ data: JSON.stringify({ type: 'action_accepted', protocol_version: PROTOCOL_VERSION,
      game_id: 'game_12345', game_version: 4, option_id: 'opt-4' }) } as MessageEvent);
    reconnected.onmessage?.({ data: JSON.stringify({ ...snapshot, type: 'state_update', game_version: 5,
      viewer: { role: 'player', seat: 'player_a' }, pending_choice: null }) } as MessageEvent);
    await retried;
    client.stop();
  });

  it('does not accept a different option or an unchanged choice as progress', async () => {
    const { client, send } = await connectedPlayer();
    const submitted = client.submitChoice('opt-4');
    let settled = false;
    void submitted.then(() => { settled = true; }, () => { settled = true; });
    send({ type: 'action_accepted', game_version: 4, option_id: 'other' });
    send({ ...snapshot, type: 'state_update', game_version: 5, viewer: { role: 'player', seat: 'player_a' },
      pending_choice: { nonce: 'nonce-4', choice: { player: 'player_a', prompt: 'Choose', options: [{ id: 'opt-4', label: 'Choose' }] } },
    });
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(settled).toBe(false);
    send({ type: 'action_accepted', game_version: 4, option_id: 'opt-4' });
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(settled).toBe(false);
    const failed = expect(submitted).rejects.toThrow(/Rejected: Stale decision nonce/);
    send({ type: 'action_rejected', game_version: 4, reason: { reason: 'stale_nonce' } });
    await failed;
    expect(client.getState().lastError).toBe('Rejected: Stale decision nonce');
    client.stop();
  });

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
