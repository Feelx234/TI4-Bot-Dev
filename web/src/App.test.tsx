import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import { App } from './App.tsx';

const lobby = (gameId: string, viewer?: { role: 'player'; seat: string }) => ({ game_id: gameId, phase: 'lobby', lobby_version: 1, host_seat: 'p1', roster: [{ seat: 'p1', controller: 'human', ready: false, available: !viewer }, { seat: 'p2', controller: 'human', ready: false, available: !viewer }], viewer, can_start: false });

afterEach(() => { cleanup(); sessionStorage.clear(); history.replaceState({}, '', '/'); vi.unstubAllGlobals(); });

describe('App lobby routing', () => {
  it('shows a mandatory chooser for a direct URL without a credential', async () => {
    history.replaceState({}, '', '/games/game-1');
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, json: async () => lobby('game-1') }));
    render(<App />);
    await waitFor(() => expect(screen.getByText('Choose an available seat')).toBeInTheDocument());
    expect(sessionStorage.getItem('ti4.viewer.v1:game-1')).toBeNull();
    expect(window.location.pathname).toBe('/games/game-1');
  });

  it('restores a direct game URL using its tab credential and displays server identity', async () => {
    sessionStorage.setItem('ti4.viewer.v1:game-2', 'capability-p2'); history.replaceState({}, '', '/games/game-2');
    const fetchMock = vi.fn().mockResolvedValue({ ok: true, json: async () => lobby('game-2', { role: 'player', seat: 'p2' }) }); vi.stubGlobal('fetch', fetchMock);
    render(<App />);
    await waitFor(() => expect(screen.getByTestId('ready-button')).toBeInTheDocument());
    expect(fetchMock.mock.calls[0][1].headers).toEqual({ 'x-ti4-seat-token': 'capability-p2' });
    expect(screen.queryByTestId('start-game-button')).toBeNull();
  });
});
