import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Board, getPlayerColor } from './Board.tsx';
import { BoardView } from '../protocol/types.ts';

const mockBoard: BoardView = {
  systems: {
    '18': {
      system_id: '18',
      coordinate: '<0, 0, 0>',
      tile_type: 'normal',
      planets: {
        mecatol_rex: {
          planet_id: 'mecatol_rex',
          controlled_by: null,
          exhausted: false,
          attachments: [],
        },
      },
      units: [
        { unit_type: 'cruiser', owner: 'p1', damaged: false },
        { unit_type: 'infantry', owner: 'p1', damaged: false },
      ],
      command_tokens: ['p1'],
    },
    '34': {
      system_id: '34',
      coordinate: '<1, 0, -1>',
      tile_type: 'normal',
      planets: {
        abyz: {
          planet_id: 'abyz',
          controlled_by: 'p2',
          exhausted: true,
          attachments: [],
        },
        fria: {
          planet_id: 'fria',
          controlled_by: 'p2',
          exhausted: false,
          attachments: [],
        },
      },
      units: [],
      command_tokens: [],
    },
  },
};

describe('Board Component', () => {
  it('renders all systems in SVG with correct coordinates and planet labels', () => {
    render(<Board board={mockBoard} seatingOrder={['p1', 'p2']} />);

    expect(screen.getByTestId('ti4-board-svg')).toBeInTheDocument();
    expect(screen.getByTestId('system-hex-18')).toBeInTheDocument();
    expect(screen.getByTestId('system-hex-34')).toBeInTheDocument();

    // Mecatol Rex text
    expect(screen.getByText('Mecatol Rex')).toBeInTheDocument();
    expect(screen.getByText('#34')).toBeInTheDocument();

    // Planet abbreviations
    expect(screen.getByText('MEC')).toBeInTheDocument();
    expect(screen.getByText('ABY')).toBeInTheDocument();
    expect(screen.getByText('FRI')).toBeInTheDocument();

    // Units label
    expect(screen.getByText('2 units')).toBeInTheDocument();
  });

  it('keeps non-interactive systems out of the keyboard tab order', () => {
    render(<Board board={mockBoard} seatingOrder={['p1', 'p2']} />);

    const hex18 = screen.getByTestId('system-hex-18');
    const hex34 = screen.getByTestId('system-hex-34');
    expect(hex18).not.toHaveAttribute('role');
    expect(hex18).not.toHaveAttribute('tabindex');
    expect(hex34).not.toHaveAttribute('role');
  });

  it('assigns colors by projected seating order, not seat name', () => {
    expect(getPlayerColor('unusual-seat', ['unusual-seat', 'another-seat'])).toBe('#ef4444');
    expect(getPlayerColor('another-seat', ['unusual-seat', 'another-seat'])).toBe('#38bdf8');
    expect(getPlayerColor('absent-seat', ['unusual-seat'])).toBe('#94a3b8');
  });

  it('renders static map_tiles with anomalies, wormholes, and zoom controls', () => {
    const boardWithMap: BoardView = {
      systems: {
        '18': {
          system_id: '18',
          command_tokens: [],
          planets: {},
          units: [{ unit_type: 'carrier', owner: 'p1', damaged: false }],
        },
      },
      map_tiles: [
        {
          system_id: '18',
          label: 'Mecatol Rex',
          q: 0,
          r: 0,
          planets: [{ id: 'mecatol_rex', label: 'Mecatol Rex', resources: 1, influence: 6 }],
        },
        {
          system_id: '67',
          label: 'Cormund',
          q: 1,
          r: -1,
          anomalies: ['gravity rift'],
          wormholes: ['alpha'],
          planets: [],
        },
      ],
    };

    render(<Board board={boardWithMap} seatingOrder={['p1']} />);

    expect(screen.getByTestId('system-hex-18')).toBeInTheDocument();
    expect(screen.getByTestId('system-hex-67')).toBeInTheDocument();
    expect(screen.getByText('Mecatol Rex')).toBeInTheDocument();
    expect(screen.getByText('GRAVITY RIFT')).toBeInTheDocument();
    expect(screen.getByText('α')).toBeInTheDocument();
    expect(screen.getByText('1/6')).toBeInTheDocument();

    // Zoom buttons
    expect(screen.getByTitle('Zoom In')).toBeInTheDocument();
    expect(screen.getByTitle('Zoom Out')).toBeInTheDocument();
    expect(screen.getByTitle('Reset Pan & Zoom')).toBeInTheDocument();
  });

  it('highlights candidate targets on the board for the deciding actor and handles clicks', () => {
    const onSelectTarget = vi.fn();
    const onSelectSystem = vi.fn();
    const pendingChoice = {
      nonce: 'nonce_test',
      actor: 'p1',
      prompt: 'Activate a system',
      options: [
        {
          id: 'opt_activate_18',
          kind: 'activate',
          label: 'Activate Mecatol Rex',
          payload: { system: '18' },
        },
      ],
    };

    render(
      <Board
        board={mockBoard}
        seatingOrder={['p1', 'p2']}
        pendingChoice={pendingChoice}
        viewerSeat="p1"
        onSelectTarget={onSelectTarget}
        onSelectSystem={onSelectSystem}
      />
    );

    const hex18 = screen.getByTestId('system-hex-18');
    const hex34 = screen.getByTestId('system-hex-34');

    expect(hex18).toHaveAttribute('data-target-candidate', 'true');
    expect(hex18).toHaveAttribute('role', 'button');
    expect(hex18).toHaveAttribute('tabindex', '0');

    expect(hex34).not.toHaveAttribute('data-target-candidate');

    // Click candidate target hex
    fireEvent.click(hex18);
    expect(onSelectTarget).toHaveBeenCalledWith('18');
    expect(onSelectSystem).toHaveBeenCalledWith('18');
  });

  it('redacts target candidate highlighting when viewer is not the actor', () => {
    const pendingChoice = {
      nonce: 'nonce_test',
      actor: 'p1',
      prompt: 'Activate a system',
      options: [
        {
          id: 'opt_activate_18',
          kind: 'activate',
          label: 'Activate Mecatol Rex',
          payload: { system: '18' },
        },
      ],
    };

    render(
      <Board
        board={mockBoard}
        seatingOrder={['p1', 'p2']}
        pendingChoice={pendingChoice}
        viewerSeat="p2"
      />
    );

    const hex18 = screen.getByTestId('system-hex-18');
    expect(hex18).not.toHaveAttribute('data-target-candidate');
    expect(hex18).not.toHaveAttribute('role');
  });

  it('displays SystemInspector when a system is selected', () => {
    render(<Board board={mockBoard} seatingOrder={['p1', 'p2']} selectedSystemId="18" />);

    expect(screen.getByTestId('system-inspector')).toBeInTheDocument();
    expect(screen.getByTestId('inspector-system-title')).toHaveTextContent('Mecatol Rex');
  });

  it('assigns accessible button semantics and keyboard activation to candidate target planets', () => {
    const onSelectTarget = vi.fn();
    const pendingChoice = {
      nonce: 'nonce_planet',
      actor: 'p1',
      prompt: 'Commit ground forces to planet',
      options: [
        {
          id: 'opt_land_mecatol',
          kind: 'commit_ground_forces',
          label: 'Land on Mecatol Rex',
          payload: { system: '18', planet: 'mecatol_rex' },
        },
      ],
    };

    render(
      <Board
        board={mockBoard}
        seatingOrder={['p1', 'p2']}
        pendingChoice={pendingChoice}
        viewerSeat="p1"
        onSelectTarget={onSelectTarget}
      />
    );

    const planetMecatol = screen.getByTestId('planet-mecatol_rex');
    const planetAbyz = screen.getByTestId('planet-abyz');

    expect(planetMecatol).toHaveAttribute('data-target-candidate', 'true');
    expect(planetMecatol).toHaveAttribute('role', 'button');
    expect(planetMecatol).toHaveAttribute('tabindex', '0');
    expect(planetMecatol).toHaveAttribute('aria-label', 'Target planet mecatol_rex');

    expect(planetAbyz).not.toHaveAttribute('data-target-candidate');
    expect(planetAbyz).not.toHaveAttribute('role');

    // Keyboard activation via Enter
    fireEvent.keyDown(planetMecatol, { key: 'Enter' });
    expect(onSelectTarget).toHaveBeenCalledWith('18', 'mecatol_rex');

    // Keyboard activation via Space
    fireEvent.keyDown(planetMecatol, { key: ' ' });
    expect(onSelectTarget).toHaveBeenCalledWith('18', 'mecatol_rex');
    expect(onSelectTarget).toHaveBeenCalledTimes(2);
  });
});

