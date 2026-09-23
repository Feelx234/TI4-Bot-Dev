import { expect, test, type Page, type WebSocketRoute } from '@playwright/test';
import { PROTOCOL_VERSION, type ClientMessage, type InitialSnapshotMsg, type LobbyDto, type StateUpdateMsg } from '../src/protocol/types';

const gameId = 'mocked-decision';
const seat = 'p1';
const session = 'mock-session';
const nonce = 'movement-7';

// Synthetic protocol-shaped decision: this test proves browser/socket behavior, not engine reachability.
const initial: InitialSnapshotMsg & { type: 'initial_snapshot' } = {
  type: 'initial_snapshot', protocol_version: PROTOCOL_VERSION, game_id: gameId, game_version: 7,
  viewer: { role: 'player', seat }, state: {},
  galaxy_layout: { version: 1, active_sources: [], placements: [] },
  view: {
    round: 1, phase: 'action', speaker: seat, seating_order: [seat, 'p2'], active_player: seat, finished: false,
    players: [], board: { systems: {} },
    table: { revealed_objectives: [], scored_objectives: {}, unclaimed_strategy_cards: [], strategy_card_goods: {}, laws: {} },
  },
  turn_status: { kind: 'waiting_for_decision', seat, phase: 'action', round: 1, stage: 'movement_step' },
  pending_choice: {
    nonce,
    choice: { player: seat, prompt: 'Move ships', context: { subtype: 'movement_step', target: { System: '42' } },
      options: [{ id: 'done_moving', kind: 'decline', label: 'Finish movement' }] },
  },
  events: [],
};

const lobby: LobbyDto = {
  game_id: gameId, phase: 'running', lobby_version: 1, host_player_id: seat,
  slots: [seat, 'p2'].map((id, index) => ({
    slot_id: `slot-${index + 1}`, position: index + 1, occupant: id,
    nickname: `Player ${index + 1}`, ready: true, connected: true, can_take_over: false,
  })),
};

async function openMockedGame(page: Page) {
  // Register both routes before navigation: the HTTP load and socket subscribe can race.
  await page.route(`**/api/games/${gameId}/lobby/join`, (route) => route.fulfill({ json: {
    player_session: session, player: { id: seat }, lobby,
  } }));
  await page.route(`**/api/games/${gameId}/lobby/heartbeat`, (route) => route.fulfill({ json: {} }));
  await page.route(`**/api/games/${gameId}/snapshot`, (route) => route.fulfill({ json: initial }));

  let connected!: (connection: { socket: WebSocketRoute; subscribe: ClientMessage }) => void;
  const socketReady = new Promise<{ socket: WebSocketRoute; subscribe: ClientMessage }>((resolve) => { connected = resolve; });
  await page.routeWebSocket(`**/ws/games/${gameId}`, (socket) => {
    socket.onMessage((data) => {
      const message = JSON.parse(String(data)) as ClientMessage;
      if (message.type === 'subscribe') {
        socket.send(JSON.stringify(initial));
        connected({ socket, subscribe: message });
      }
    });
  });
  await page.goto('/');
  await page.evaluate(([id, credential]) => sessionStorage.setItem(`ti4.player-session:${id}`, credential), [gameId, session]);
  await page.goto(`/games/${gameId}`);
  return socketReady;
}

test('rejected movement submission stays actionable and retries with a fresh server nonce', async ({ page }) => {
  const { socket, subscribe } = await openMockedGame(page);
  expect(subscribe).toEqual({ type: 'subscribe', protocol_version: PROTOCOL_VERSION,
    game_id: gameId, player_session: session });
  const submissions: Extract<ClientMessage, { type: 'submit_choice' }>[] = [];
  const sent = () => submissions.length;
  socket.onMessage((data) => {
    const message = JSON.parse(String(data)) as ClientMessage;
    if (message.type === 'submit_choice') submissions.push(message);
  });

  const tray = page.getByTestId('tactical-movement-tray');
  const finish = page.getByTestId('commit-moves-btn');
  await expect(tray).toBeVisible();
  await expect(page.getByText('No ships eligible to move into the active system.')).toBeVisible();
  await expect(finish).toBeEnabled();
  await finish.focus();
  await expect(finish).toBeFocused();
  await page.keyboard.press('Enter');
  await expect.poll(sent, { timeout: 5_000 }).toBe(1);
  expect(submissions[0]).toEqual({ type: 'submit_choice', protocol_version: PROTOCOL_VERSION,
    game_id: gameId, option_id: 'done_moving', nonce, expected_version: 7 });

  socket.send(JSON.stringify({ type: 'action_rejected', protocol_version: PROTOCOL_VERSION,
    game_id: gameId, game_version: 7, reason: { reason: 'stale_nonce' } }));
  await expect(tray.getByRole('alert')).toContainText('Stale decision nonce');
  await expect(finish).toBeEnabled();
  await expect(page.getByTestId('game-version')).toHaveText('v7');

  const fresh: StateUpdateMsg & { type: 'state_update' } = {
    ...initial, type: 'state_update', game_version: 8,
    pending_choice: { ...initial.pending_choice!, nonce: 'movement-8' },
  };
  socket.send(JSON.stringify(fresh));
  socket.send(JSON.stringify({ type: 'pending_choice', protocol_version: PROTOCOL_VERSION,
    game_id: gameId, game_version: 8, nonce: 'movement-8', choice: fresh.pending_choice!.choice,
    state: fresh.state, galaxy_layout: fresh.galaxy_layout }));
  await expect(page.getByTestId('game-version')).toHaveText('v8');
  await expect(finish).toBeEnabled();
  await finish.click(); // Real pointer hit target, after the refusal and authoritative refresh.
  await expect.poll(sent, { timeout: 5_000 }).toBe(2);
  expect(submissions[1]).toEqual({ type: 'submit_choice', protocol_version: PROTOCOL_VERSION,
    game_id: gameId, option_id: 'done_moving', nonce: 'movement-8', expected_version: 8 });

  socket.send(JSON.stringify({ type: 'action_accepted', protocol_version: PROTOCOL_VERSION,
    game_id: gameId, game_version: 8, option_id: 'done_moving' }));
  await expect(tray).toBeVisible(); // An acknowledgement alone cannot finish the workflow.
  await expect(finish).toBeDisabled();
  await expect(page.getByTestId('game-version')).toHaveText('v8');
  const next: StateUpdateMsg & { type: 'state_update' } = {
    ...fresh, game_version: 9, pending_choice: null,
    turn_status: { kind: 'active_turn', player: seat, phase: 'action', round: 1 },
  };
  socket.send(JSON.stringify(next));
  await expect(page.getByTestId('game-version')).toHaveText('v9');
  await expect(tray).toHaveCount(0);
});
