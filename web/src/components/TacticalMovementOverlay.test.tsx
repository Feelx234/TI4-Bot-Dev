import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { TacticalMovementOverlay } from './TacticalMovementOverlay.tsx';
import { PendingChoiceDto, PlayerView } from '../protocol/types.ts';

const mockMoveChoice: PendingChoiceDto = {
  prompt: 'movement',
  actor: 'p1',
  nonce: 'nonce_move_step',
  context: {
    subtype: 'movement_step',
    target: { System: '18' },
  },
  options: [
    {
      id: 'move|24|0',
      label: 'Cruiser',
      kind: 'move',
      payload: { origin: '24', unit: 'cruiser' },
    },
    {
      id: 'move|24|1',
      label: 'Carrier',
      kind: 'move',
      payload: { origin: '24', unit: 'carrier', capacity: 4 },
    },
    {
      id: 'move|24|2',
      label: 'Fighter',
      kind: 'move',
      payload: { origin: '24', unit: 'fighter' },
    },
    {
      id: 'done_moving',
      label: 'Finish Movement',
      kind: 'decline',
    },
  ],
};

const mockPlayer: PlayerView = {
  id: 'p1',
  faction: 'sol',
  victory_points: 0,
  trade_goods: 0,
  commodities: 0,
  tactic_tokens: 3,
  fleet_tokens: 3,
  strategic_tokens: 2,
  passed: false,
  strategy_cards: [],
  exhausted_strategy_cards: [],
  technologies: [],
  exhausted_technologies: [],
  relics: [],
  exhausted_relics: [],
  action_cards_count: 0,
  secret_objectives_count: 0,
  leaders: {},
};

describe('TacticalMovementOverlay Component', () => {
  it('does not render when isOpen is false or choice is null', () => {
    const { container: c1 } = render(
      <TacticalMovementOverlay
        choice={null}
        activeSystemId="18"
        player={mockPlayer}
        onSubmit={vi.fn()}
        isOpen={true}
        onClose={vi.fn()}
      />
    );
    expect(c1.firstChild).toBeNull();

    const { container: c2 } = render(
      <TacticalMovementOverlay
        choice={mockMoveChoice}
        activeSystemId="18"
        player={mockPlayer}
        onSubmit={vi.fn()}
        isOpen={false}
        onClose={vi.fn()}
      />
    );
    expect(c2.firstChild).toBeNull();
  });

  it('renders ship groups by origin with initial zero staged', () => {
    render(
      <TacticalMovementOverlay
        choice={mockMoveChoice}
        activeSystemId="18"
        player={mockPlayer}
        onSubmit={vi.fn()}
        isOpen={true}
        onClose={vi.fn()}
      />
    );

    expect(screen.getByTestId('tactical-movement-tray')).toBeInTheDocument();
    expect(screen.getByText('Destination System: #18')).toBeInTheDocument();
    expect(screen.getByTestId('rally-row-24-cruiser')).toBeInTheDocument();
    expect(screen.getByTestId('rally-row-24-carrier')).toBeInTheDocument();
    expect(screen.getByTestId('rally-row-24-fighter')).toBeInTheDocument();

    expect(screen.getByTestId('fleet-supply-gauge')).toHaveTextContent('0 / 3 Ships');
    expect(screen.getByTestId('cargo-capacity-gauge')).toHaveTextContent('0 / 0 Loaded');
  });

  it('updates fleet supply and cargo capacity when staging ships', () => {
    render(
      <TacticalMovementOverlay
        choice={mockMoveChoice}
        activeSystemId="18"
        player={mockPlayer}
        onSubmit={vi.fn()}
        isOpen={true}
        onClose={vi.fn()}
      />
    );

    // Stage 1 Carrier (+1 fleet, +4 capacity)
    const carrierInc = screen.getByTestId('rally-inc-24-carrier');
    fireEvent.click(carrierInc);

    expect(screen.getByTestId('fleet-supply-gauge')).toHaveTextContent('1 / 3 Ships');
    expect(screen.getByTestId('cargo-capacity-gauge')).toHaveTextContent('0 / 4 Loaded');

    // Stage 1 Fighter (+1 cargo)
    const fighterInc = screen.getByTestId('rally-inc-24-fighter');
    fireEvent.click(fighterInc);

    expect(screen.getByTestId('cargo-capacity-gauge')).toHaveTextContent('1 / 4 Loaded');
    expect(screen.getByTestId('commit-moves-btn')).toHaveTextContent('Commit Moves (2)');
  });

  it('submits done_moving when finish movement button is clicked', () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(
      <TacticalMovementOverlay
        choice={mockMoveChoice}
        activeSystemId="18"
        player={mockPlayer}
        onSubmit={onSubmit}
        isOpen={true}
        onClose={vi.fn()}
      />
    );

    const finishBtn = screen.getByTestId('finish-movement-btn');
    fireEvent.click(finishBtn);
    expect(onSubmit).toHaveBeenCalledWith('done_moving');
  });

  it('submits staged moves sequentially using semantic intent matching', async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(
      <TacticalMovementOverlay
        choice={mockMoveChoice}
        activeSystemId="18"
        player={mockPlayer}
        onSubmit={onSubmit}
        isOpen={true}
        onClose={vi.fn()}
      />
    );

    // Stage Cruiser
    fireEvent.click(screen.getByTestId('rally-inc-24-cruiser'));

    const commitBtn = screen.getByTestId('commit-moves-btn');
    await act(async () => {
      fireEvent.click(commitBtn);
    });

    // Submits the first matched option for cruiser from 24
    expect(onSubmit).toHaveBeenCalledWith('move|24|0');
  });
});
