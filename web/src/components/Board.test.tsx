import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Board } from './Board.tsx';
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
    render(<Board board={mockBoard} />);

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

  it('triggers onSelectSystem on click and keyboard activation', () => {
    const onSelect = vi.fn();
    render(<Board board={mockBoard} onSelectSystem={onSelect} />);

    const hex18 = screen.getByTestId('system-hex-18');
    fireEvent.click(hex18);
    expect(onSelect).toHaveBeenCalledWith(mockBoard.systems['18']);

    const hex34 = screen.getByTestId('system-hex-34');
    fireEvent.keyDown(hex34, { key: 'Enter' });
    expect(onSelect).toHaveBeenCalledWith(mockBoard.systems['34']);
  });

  it('marks actionable systems with data-actionable and highlight styling', () => {
    render(<Board board={mockBoard} actionableSystemIds={['34']} />);

    const hex18 = screen.getByTestId('system-hex-18');
    const hex34 = screen.getByTestId('system-hex-34');

    expect(hex18).toHaveAttribute('data-actionable', 'false');
    expect(hex34).toHaveAttribute('data-actionable', 'true');
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

    render(<Board board={boardWithMap} />);

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
});
