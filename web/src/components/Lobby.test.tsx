import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { CreateLobby, LobbyStatus } from './Lobby.tsx';

describe('lobby UI', () => {
  it('creates the selected roster with p1 as the fixed human host', async () => {
    const onCreated = vi.fn();
    const fetchMock = vi.fn().mockResolvedValue({ ok: true, json: async () => ({ game_id: 'new-game', creator_token: 'cap-p1', lobby: { game_id: 'new-game', phase: 'lobby', lobby_version: 0, host_seat: 'p1', roster: ['p1', 'p2', 'p3', 'p4'].map((seat) => ({ seat, controller: seat === 'p3' ? 'bot' : 'human', ready: seat === 'p3', available: seat !== 'p1' && seat !== 'p3' })), viewer: { role: 'player', seat: 'p1' }, can_start: false } }) });
    vi.stubGlobal('fetch', fetchMock); render(<CreateLobby onCreated={onCreated} onError={vi.fn()} />);
    fireEvent.change(screen.getByLabelText('Players'), { target: { value: '4' } });
    fireEvent.change(screen.getAllByRole('combobox')[3], { target: { value: 'bot' } });
    fireEvent.change(screen.getByLabelText(/Seed/), { target: { value: '0' } });
    fireEvent.click(screen.getByTestId('create-game-button'));
    await waitFor(() => expect(fetchMock).toHaveBeenCalledOnce());
    expect(JSON.parse(fetchMock.mock.calls[0][1].body)).toMatchObject({ players: ['p1', 'p2', 'p3', 'p4'], bot_seats: ['p3'], seed: 0 });
    expect(JSON.parse(fetchMock.mock.calls[0][1].body)).not.toHaveProperty('game_id');
    expect(onCreated).toHaveBeenCalledOnce();
  });

  it('uses the server viewer identity for ready and host controls', () => {
    const lobby = { game_id: 'game', phase: 'lobby' as const, lobby_version: 2, host_seat: 'p1', roster: [{ seat: 'p1', controller: 'human' as const, ready: true, available: false }, { seat: 'p2', controller: 'human' as const, ready: false, available: false }], viewer: { role: 'player' as const, seat: 'p2' }, can_start: false };
    const ready = vi.fn(); render(<LobbyStatus lobby={lobby} onClaim={vi.fn()} onReady={ready} onStart={vi.fn()} onForget={vi.fn()} />);
    fireEvent.click(screen.getByTestId('ready-button'));
    expect(ready).toHaveBeenCalledWith(true);
    expect(screen.queryByTestId('start-game-button')).toBeNull();
  });
});
