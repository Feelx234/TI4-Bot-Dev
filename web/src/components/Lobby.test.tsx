import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { CreateLobby, LobbyStatus } from './Lobby.tsx';
import { LobbyDto } from '../protocol/types.ts';

const lobby: LobbyDto = { game_id: 'game', phase: 'lobby', lobby_version: 2, host_player_id: 'player_a', slots: [
  { slot_id: 'slot_1', position: 1, occupant: 'player_a', nickname: 'Same name', ready: true, connected: true, can_take_over: false },
  { slot_id: 'slot_2', position: 2, occupant: 'player_b', nickname: 'Same name', ready: false, connected: false, can_take_over: true },
  { slot_id: 'slot_3', position: 3, occupant: null, nickname: null, ready: false, connected: false, can_take_over: false },
] };
const props = { onReady: vi.fn(), onStart: vi.fn(), onLeave: vi.fn(), onJoin: vi.fn(), onWatch: vi.fn(), onTakeover: vi.fn(), onReorder: vi.fn() };
afterEach(() => { localStorage.clear(); vi.unstubAllGlobals(); });

describe('lobby UI', () => {
  it('creates empty positions without choosing bot or player identities', async () => {
    const onCreated = vi.fn();
    const fetchMock = vi.fn().mockResolvedValue({ ok: true, json: async () => ({ game_id: 'game', player_session: 'secret', player: { id: 'player_a' }, lobby }) });
    vi.stubGlobal('fetch', fetchMock); render(<CreateLobby onCreated={onCreated} onError={vi.fn()} />);
    fireEvent.change(screen.getByLabelText('Nickname'), { target: { value: 'Host 🪐' } });
    fireEvent.change(screen.getByLabelText('Players'), { target: { value: '3' } });
    fireEvent.click(screen.getByTestId('create-game-button'));
    await waitFor(() => expect(onCreated).toHaveBeenCalledOnce());
    expect(JSON.parse(fetchMock.mock.calls[0][1].body)).toEqual({ player_count: 3, nickname: 'Host 🪐' });
    expect(localStorage.getItem('ti4.nickname')).toBe('Host 🪐');
  });

  it('prefills the saved preference and refuses invalid creation before any request', async () => {
    localStorage.setItem('ti4.nickname', 'Returning visitor');
    const fetchMock = vi.fn();
    const onError = vi.fn();
    vi.stubGlobal('fetch', fetchMock);
    render(<CreateLobby onCreated={vi.fn()} onError={onError} />);
    expect(screen.getByLabelText('Nickname')).toHaveValue('Returning visitor');
    fireEvent.change(screen.getByLabelText('Nickname'), { target: { value: ' bad ' } });
    fireEvent.click(screen.getByText('Create lobby'));
    expect(onError).toHaveBeenCalledWith(expect.stringContaining('Nickname'));
    expect(fetchMock).not.toHaveBeenCalled();
    expect(localStorage.getItem('ti4.nickname')).toBe('Returning visitor');
  });

  it('shows watch, first-open join, and eligible explicit rejoin without admitting on render', () => {
    render(<LobbyStatus lobby={lobby} playerId={null} {...props} />);
    fireEvent.click(screen.getByText('Watch'));
    fireEvent.click(screen.getByText('Join game'));
    fireEvent.change(screen.getByLabelText('Nickname'), { target: { value: 'Replacement' } });
    fireEvent.click(screen.getByText(/Rejoin as Same name/));
    expect(props.onWatch).toHaveBeenCalledOnce();
    expect(props.onJoin).toHaveBeenCalledWith('');
    expect(props.onTakeover).toHaveBeenCalledWith('player_b', 'Replacement');
    expect(screen.getByText(/Position 3: Open/)).toBeInTheDocument();
    expect(screen.getByText(/Position 1: Same name \(Host\)/)).toBeInTheDocument();
    expect(screen.getByTestId('lobby-container').textContent).not.toContain('player_a');
    expect(screen.getByTestId('lobby-container').textContent).not.toContain('player_b');
    expect(screen.getByTestId('lobby-container').textContent).not.toContain('Game: game');
  });

  it('permits only the stable host identity to reorder every slot, including open slots', () => {
    const { rerender } = render(<LobbyStatus lobby={lobby} playerId="player_a" {...props} />);
    fireEvent.click(screen.getByLabelText('Move position 2 down'));
    expect(props.onReorder).toHaveBeenCalledWith(['slot_1', 'slot_3', 'slot_2']);
    expect(screen.getByTestId('start-game-button')).toBeDisabled();
    expect(screen.queryByText('Leave lobby')).toBeNull();
    rerender(<LobbyStatus lobby={lobby} playerId="player_b" {...props} />);
    expect(screen.queryByLabelText('Move position 2 down')).toBeNull();
    fireEvent.click(screen.getByTestId('ready-button'));
    fireEvent.click(screen.getByText('Leave lobby'));
    expect(props.onReady).toHaveBeenCalledWith(true);
    expect(props.onLeave).toHaveBeenCalledOnce();
    const moved = { ...lobby, slots: [
      { ...lobby.slots[1], position: 1 }, { ...lobby.slots[0], position: 2 }, lobby.slots[2],
    ] };
    rerender(<LobbyStatus lobby={moved} playerId="player_b" {...props} />);
    expect(screen.getByText(/Position 1: Same name/)).toBeInTheDocument();
    expect(screen.getByText(/Position 2: Same name \(Host\)/)).toBeInTheDocument();
  });
});
