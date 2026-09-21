import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { Lobby } from './Lobby.tsx';

describe('Lobby', () => {
  it('creates the explicit demo with seed zero and the p1 seat capability', async () => {
    const onJoin = vi.fn();
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({ game_id: 'demo-zero', seat_tokens: { p1: 'capability-p1' } }),
    });
    vi.stubGlobal('fetch', fetchMock);
    render(<Lobby onJoin={onJoin} />);

    fireEvent.click(screen.getByText('+ Create Three-Seat Demo'));
    const seed = screen.getByDisplayValue('42');
    fireEvent.change(seed, { target: { value: '0' } });
    expect(screen.queryByLabelText(/p1 \(Bot\)/i)).toBeNull();
    fireEvent.click(screen.getByText('Create & Join'));

    await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1));
    expect(JSON.parse(fetchMock.mock.calls[0][1].body)).toMatchObject({
      players: ['p1', 'p2', 'p3'],
      seed: 0,
    });
    expect(onJoin).toHaveBeenCalledWith('demo-zero', {
      role: 'player',
      seat: 'p1',
      seatToken: 'capability-p1',
    });
  });

  it('rejects invalid seeds before sending a request', () => {
    const fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);
    render(<Lobby onJoin={vi.fn()} />);

    fireEvent.click(screen.getByText('+ Create Three-Seat Demo'));
    fireEvent.change(screen.getByDisplayValue('42'), { target: { value: '-1' } });
    fireEvent.click(screen.getByText('Create & Join'));

    expect(screen.getByRole('alert')).toHaveTextContent('Seed must be a non-negative whole number.');
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
