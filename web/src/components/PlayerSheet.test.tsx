import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { PlayerSheet } from './PlayerSheet.tsx';
import { PlayerView } from '../protocol/types.ts';

const mockPlayers: PlayerView[] = [
  {
    id: 'p1',
    faction: 'Federation of Sol',
    victory_points: 3,
    trade_goods: 2,
    commodities: 4,
    tactic_tokens: 3,
    fleet_tokens: 3,
    strategic_tokens: 2,
    passed: false,
    strategy_cards: ['pok1leadership', 'pok6warfare'],
    exhausted_strategy_cards: [],
    technologies: [],
    exhausted_technologies: [],
    relics: [],
    exhausted_relics: [],
    action_cards_count: 2,
    secret_objectives_count: 1,
    held_action_cards: ['direct_hit'],
    held_secret_objectives: ['faa'],
    leaders: {},
  },
  {
    id: 'p2',
    faction: 'Barony of Letnev',
    victory_points: 2,
    trade_goods: 1,
    commodities: 2,
    tactic_tokens: 2,
    fleet_tokens: 4,
    strategic_tokens: 1,
    passed: false,
    strategy_cards: ['pok2diplomacy'],
    exhausted_strategy_cards: [],
    technologies: [],
    exhausted_technologies: [],
    relics: [],
    exhausted_relics: [],
    action_cards_count: 3,
    secret_objectives_count: 1,
    held_action_cards: [], // Redacted by server
    held_secret_objectives: [],
    leaders: {},
  },
];

describe('PlayerSheet Component & Human Readable Metadata', () => {
  it('resolves raw IDs to human-readable names and descriptions with tooltips', () => {
    render(<PlayerSheet players={mockPlayers} userSeat="p1" />);

    // Strategy cards pok1leadership and pok6warfare mapped to readable names
    const scLeadership = screen.getByTestId('strategy-card-badge-pok1leadership');
    expect(scLeadership).toHaveTextContent('1. Leadership');
    expect(scLeadership).toHaveAttribute('title');
    expect(scLeadership.getAttribute('title')).toContain('Gain 3 command tokens');

    const scWarfare = screen.getByTestId('strategy-card-badge-pok6warfare');
    expect(scWarfare).toHaveTextContent('6. Warfare');
    expect(scWarfare).toHaveAttribute('title');

    // Secret objective "faa" mapped to Forge an Alliance with description and points
    const soFaa = screen.getByTestId('secret-objective-item-faa');
    expect(soFaa).toHaveTextContent('Forge an Alliance');
    expect(soFaa).toHaveTextContent('Control 4 cultural planets.');
    expect(soFaa).toHaveTextContent('1 VP');
    expect(soFaa).toHaveAttribute('title');
    expect(soFaa.getAttribute('title')).toContain('Forge an Alliance');
    expect(soFaa.getAttribute('title')).toContain('Control 4 cultural planets.');

    // Action card direct_hit mapped to Direct Hit
    const acDirectHit = screen.getByTestId('action-card-item-direct_hit');
    expect(acDirectHit).toHaveTextContent('Direct Hit');
  });

  it('renders private cards exclusively for the viewer seat', () => {
    // Viewer is p1
    render(<PlayerSheet players={mockPlayers} userSeat="p1" />);

    // p1 sees own private cards
    expect(screen.getByTestId('secret-objective-item-faa')).toBeInTheDocument();
    expect(screen.getByTestId('action-card-item-direct_hit')).toBeInTheDocument();

    const privateCards = screen.getAllByTestId(/player-card-/);
    expect(privateCards).toHaveLength(2);

    // Invariant: p2 must not have private-hand-section
    const p2Card = screen.getByTestId('player-card-p2');
    expect(p2Card.querySelector('[data-testid="private-hand-section"]')).toBeNull();
  });

  it('renders zero private cards when viewer is spectator', () => {
    // Viewer is spectator (no seat)
    const { container } = render(<PlayerSheet players={mockPlayers} />);

    // Invariant: no private card elements exist anywhere in DOM
    const privateElements = container.querySelectorAll('[data-private-card="true"]');
    expect(privateElements).toHaveLength(0);

    // Only public counts are rendered
    expect(screen.getAllByText(/Action Cards:/i)).toHaveLength(2);
  });

  it('renders accessible tooltip with role="tooltip" and aria-describedby when badge is focused', () => {
    vi.useFakeTimers();
    render(<PlayerSheet players={mockPlayers} userSeat="p1" />);

    const badge = screen.getByTestId('strategy-card-badge-pok1leadership');
    expect(badge).not.toHaveAttribute('aria-describedby');

    fireEvent.focus(badge);
    act(() => {
      vi.advanceTimersByTime(120);
    });

    const tooltip = screen.getByRole('tooltip');
    expect(tooltip).toBeInTheDocument();
    expect(badge).toHaveAttribute('aria-describedby', tooltip.id);
    expect(tooltip).toHaveTextContent(/Gain 3 command tokens/);

    vi.useRealTimers();
  });
});
