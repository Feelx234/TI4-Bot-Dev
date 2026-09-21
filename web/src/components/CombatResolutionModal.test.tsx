import React, { act } from 'react';
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { CombatResolutionModal } from './CombatResolutionModal.tsx';
import { PendingChoiceDto } from '../protocol/types.ts';

describe('CombatResolutionModal', () => {
  it('renders sustain damage stage with Direct Hit warning and handles sustain submit', async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();

    const sustainChoice: PendingChoiceDto = {
      actor: 'seat_1',
      nonce: 10,
      prompt: 'Sustain damage on a ship',
      context: {
        subtype: 'sustain_damage',
      },
      options: [
        { id: 'sustain:dreadnought:1', label: 'Dreadnought (System 18)', kind: 'sustain' },
        { id: 'decline', label: 'Do not sustain damage', kind: 'decline' },
      ],
    };

    render(
      <CombatResolutionModal
        isOpen={true}
        choice={sustainChoice}
        viewerSeat="seat_1"
        onSubmit={onSubmit}
        onClose={onClose}
      />
    );

    expect(screen.getByTestId('combat-stage-title')).toHaveTextContent('Stage 1: Sustain Damage');
    expect(screen.getByText(/Caution: Opponents holding "Direct Hit"/i)).toBeInTheDocument();

    const sustainBtn = screen.getByTestId('sustain-opt-sustain:dreadnought:1');
    expect(sustainBtn).toBeInTheDocument();
    await act(async () => {
      fireEvent.click(sustainBtn);
    });

    expect(onSubmit).toHaveBeenCalledWith('sustain:dreadnought:1');
  });

  it('handles decline option in sustain damage stage', async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();

    const sustainChoice: PendingChoiceDto = {
      actor: 'seat_1',
      nonce: 11,
      prompt: 'Sustain damage',
      context: {
        subtype: 'sustain_damage',
      },
      options: [
        { id: 'sustain:dreadnought:1', label: 'Dreadnought', kind: 'sustain' },
        { id: 'decline', label: 'Decline', kind: 'decline' },
      ],
    };

    render(
      <CombatResolutionModal
        isOpen={true}
        choice={sustainChoice}
        viewerSeat="seat_1"
        onSubmit={onSubmit}
        onClose={onClose}
      />
    );

    const declineBtn = screen.getByTestId('decline-sustain-btn');
    await act(async () => {
      fireEvent.click(declineBtn);
    });
    expect(onSubmit).toHaveBeenCalledWith('decline');
  });

  it('renders casualty stage with steppers and enforces hits owed allocation', async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();

    const casualtyChoice: PendingChoiceDto = {
      actor: 'seat_1',
      nonce: 12,
      prompt: 'Assign 2 hits',
      constraints: {
        min_selection: 1,
        max_selection: 1,
        amount: 2,
      },
      context: {
        subtype: 'assign_casualty',
      },
      options: [
        { id: 'destroy|fighter|0', label: 'destroy fighter (system 18)', kind: 'casualty', payload: { unit: 'fighter' } },
        { id: 'destroy|fighter|1', label: 'destroy fighter (system 18)', kind: 'casualty', payload: { unit: 'fighter' } },
        { id: 'destroy|cruiser|0', label: 'destroy cruiser (system 18)', kind: 'casualty', payload: { unit: 'cruiser' } },
      ],
    };

    render(
      <CombatResolutionModal
        isOpen={true}
        choice={casualtyChoice}
        viewerSeat="seat_1"
        onSubmit={onSubmit}
        onClose={onClose}
      />
    );

    expect(screen.getByTestId('combat-stage-title')).toHaveTextContent('Stage 3: Allocate Casualties (2 Hits)');
    const confirmBtn = screen.getByTestId('confirm-casualties-btn');
    expect(confirmBtn).toBeDisabled();
    expect(screen.getByTestId('casualty-allocated-count')).toHaveTextContent('0 / 2 Hits');

    // Increment fighter
    fireEvent.click(screen.getByTestId('casualty-inc-fighter'));
    expect(screen.getByTestId('casualty-count-fighter')).toHaveTextContent('1');
    expect(confirmBtn).toBeDisabled();

    // Increment cruiser
    fireEvent.click(screen.getByTestId('casualty-inc-cruiser'));
    expect(screen.getByTestId('casualty-count-cruiser')).toHaveTextContent('1');
    expect(screen.getByTestId('casualty-allocated-count')).toHaveTextContent('2 / 2 Hits');
    expect(confirmBtn).not.toBeDisabled();

    // Confirm casualties triggers pipeline runner
    await act(async () => {
      fireEvent.click(confirmBtn);
    });
    expect(onSubmit).toHaveBeenCalled();
  });

  it('supports Auto-Cheapest casualty allocation prioritizing fighters over cruisers', async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();

    const casualtyChoice: PendingChoiceDto = {
      actor: 'seat_1',
      nonce: 13,
      prompt: 'Assign 2 hits',
      constraints: {
        amount: 2,
      },
      context: {
        subtype: 'assign_casualty',
      },
      options: [
        { id: 'destroy|cruiser|0', label: 'destroy cruiser', kind: 'casualty', payload: { unit: 'cruiser' } },
        { id: 'destroy|fighter|0', label: 'destroy fighter', kind: 'casualty', payload: { unit: 'fighter' } },
        { id: 'destroy|fighter|1', label: 'destroy fighter', kind: 'casualty', payload: { unit: 'fighter' } },
      ],
    };

    render(
      <CombatResolutionModal
        isOpen={true}
        choice={casualtyChoice}
        viewerSeat="seat_1"
        onSubmit={onSubmit}
        onClose={onClose}
      />
    );

    // Click auto cheapest
    act(() => {
      fireEvent.click(screen.getByTestId('auto-cheapest-btn'));
    });

    // Both fighters should be selected, 0 cruisers
    expect(screen.getByTestId('casualty-count-fighter')).toHaveTextContent('2');
    expect(screen.getByTestId('casualty-count-cruiser')).toHaveTextContent('0');
    expect(screen.getByTestId('casualty-allocated-count')).toHaveTextContent('2 / 2 Hits');

    const confirmBtn = screen.getByTestId('confirm-casualties-btn');
    expect(confirmBtn).not.toBeDisabled();
  });

  it('renders retreat stage and handles option selection', async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();

    const retreatChoice: PendingChoiceDto = {
      actor: 'seat_1',
      nonce: 14,
      prompt: 'Announce retreat',
      context: {
        subtype: 'announce_retreat',
      },
      options: [
        { id: 'announce_retreat:yes', label: 'Announce Retreat' },
        { id: 'announce_retreat:no', label: 'Decline Retreat' },
      ],
    };

    render(
      <CombatResolutionModal
        isOpen={true}
        choice={retreatChoice}
        viewerSeat="seat_1"
        onSubmit={onSubmit}
        onClose={onClose}
      />
    );

    expect(screen.getByTestId('combat-stage-title')).toHaveTextContent('Combat Movement: Announce Retreat');
    const optBtn = screen.getByTestId('retreat-opt-announce_retreat:yes');
    await act(async () => {
      fireEvent.click(optBtn);
    });
    expect(onSubmit).toHaveBeenCalledWith('announce_retreat:yes');
  });

  it('displays spectator notice when viewerSeat is not the active actor', () => {
    const onSubmit = vi.fn();
    const onClose = vi.fn();

    const sustainChoice: PendingChoiceDto = {
      actor: 'seat_2',
      nonce: 15,
      prompt: 'Sustain damage',
      context: {
        subtype: 'sustain_damage',
      },
      options: [{ id: 'sustain:carrier:1', label: 'Carrier' }],
    };

    render(
      <CombatResolutionModal
        isOpen={true}
        choice={sustainChoice}
        viewerSeat="seat_1"
        onSubmit={onSubmit}
        onClose={onClose}
      />
    );

    expect(screen.getByTestId('spectator-combat-notice')).toHaveTextContent(
      'Observing combat resolution in progress for seat seat_2...'
    );
  });

  it('displays dice feed and error alert when provided', () => {
    const onSubmit = vi.fn();
    const onClose = vi.fn();

    const choice: PendingChoiceDto = {
      actor: 'seat_1',
      nonce: 16,
      prompt: 'Sustain damage',
      context: {
        subtype: 'sustain_damage',
      },
      options: [{ id: 'decline', label: 'Decline' }],
    };

    const diceRolls = [
      { unit: 'Cruiser', roll: 8, target: 7, hit: true },
      { unit: 'Fighter', roll: 4, target: 9, hit: false },
    ];

    render(
      <CombatResolutionModal
        isOpen={true}
        choice={choice}
        viewerSeat="seat_1"
        onSubmit={onSubmit}
        onClose={onClose}
        lastError="Invalid selection"
        recentDiceRolls={diceRolls}
      />
    );

    expect(screen.getByTestId('combat-dice-feed')).toBeInTheDocument();
    expect(screen.getAllByTestId('dice-roll-badge')).toHaveLength(2);
    expect(screen.getByTestId('combat-error-banner')).toHaveTextContent('Invalid selection');
  });
});
