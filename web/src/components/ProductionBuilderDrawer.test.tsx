import React, { act } from 'react';
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ProductionBuilderDrawer } from './ProductionBuilderDrawer.tsx';
import { PendingChoiceDto } from '../protocol/types.ts';

describe('ProductionBuilderDrawer', () => {
  it('renders produce_unit mode with capacity meter and handles unit click and done producing', async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();

    const produceChoice: PendingChoiceDto = {
      actor: 'seat_1',
      nonce: 50,
      prompt: 'produce a unit',
      constraints: {
        amount: 5,
        paid: 2,
      },
      context: {
        subtype: 'produce_unit',
        target: { System: '18' },
      },
      options: [
        { id: 'produce|fighter', label: 'Fighter (0.5 cost)', kind: 'produce' },
        { id: 'produce|carrier', label: 'Carrier (3 cost)', kind: 'produce' },
        { id: 'decline', label: 'Done Producing', kind: 'decline' },
      ],
    };

    render(
      <ProductionBuilderDrawer
        isOpen={true}
        choice={produceChoice}
        viewerSeat="seat_1"
        onSubmit={onSubmit}
        onClose={onClose}
      />
    );

    expect(screen.getByTestId('production-drawer-title')).toHaveTextContent('Unit Production Builder');
    expect(screen.getByTestId('production-capacity-counter')).toHaveTextContent('2 / 5 Units (3 Left)');

    // Produce a fighter
    const fighterBtn = screen.getByTestId('produce-unit-btn-produce|fighter');
    await act(async () => {
      fireEvent.click(fighterBtn);
    });
    expect(onSubmit).toHaveBeenCalledWith('produce|fighter');

    // Click Done Producing
    const doneBtn = screen.getByTestId('done-producing-btn');
    await act(async () => {
      fireEvent.click(doneBtn);
    });
    expect(onSubmit).toHaveBeenCalledWith('decline');
  });

  it('renders place_unit mode and handles spot selection', async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();

    const placeChoice: PendingChoiceDto = {
      actor: 'seat_1',
      nonce: 51,
      prompt: 'place the fighter',
      context: {
        subtype: 'place_unit',
        target: { System: '18' },
      },
      options: [
        { id: 'place|space', label: 'Place in space area', kind: 'place' },
        { id: 'place|jord', label: 'Place on Jord', kind: 'place' },
      ],
    };

    render(
      <ProductionBuilderDrawer
        isOpen={true}
        choice={placeChoice}
        viewerSeat="seat_1"
        onSubmit={onSubmit}
        onClose={onClose}
      />
    );

    expect(screen.getByTestId('production-drawer-title')).toHaveTextContent('Place Produced Unit');

    const spaceBtn = screen.getByTestId('place-spot-btn-place|space');
    await act(async () => {
      fireEvent.click(spaceBtn);
    });
    expect(onSubmit).toHaveBeenCalledWith('place|space');
  });

  it('renders spectator notice when viewerSeat is not active actor', () => {
    const onSubmit = vi.fn();
    const onClose = vi.fn();

    const choice: PendingChoiceDto = {
      actor: 'seat_2',
      nonce: 52,
      prompt: 'produce a unit',
      context: {
        subtype: 'produce_unit',
      },
      options: [{ id: 'decline', label: 'Done' }],
    };

    render(
      <ProductionBuilderDrawer
        isOpen={true}
        choice={choice}
        viewerSeat="seat_1"
        onSubmit={onSubmit}
        onClose={onClose}
      />
    );

    expect(screen.getByTestId('spectator-production-notice')).toHaveTextContent(
      'Observing unit production in progress for seat seat_2...'
    );
  });

  it('displays error banner when lastError is set', () => {
    const onSubmit = vi.fn();
    const onClose = vi.fn();

    const choice: PendingChoiceDto = {
      actor: 'seat_1',
      nonce: 53,
      prompt: 'produce a unit',
      context: {
        subtype: 'produce_unit',
      },
      options: [{ id: 'decline', label: 'Done' }],
    };

    render(
      <ProductionBuilderDrawer
        isOpen={true}
        choice={choice}
        viewerSeat="seat_1"
        onSubmit={onSubmit}
        onClose={onClose}
        lastError="Insufficient production capacity"
      />
    );

    expect(screen.getByTestId('production-error-banner')).toHaveTextContent('Insufficient production capacity');
  });
});
