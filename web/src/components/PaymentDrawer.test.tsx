import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { PaymentDrawer } from './PaymentDrawer.tsx';
import { PendingChoiceDto, PlayerView } from '../protocol/types.ts';

const mockPaymentChoice: PendingChoiceDto = {
  prompt: 'pay 4 more resources',
  actor: 'p1',
  nonce: 'nonce_pay_1',
  context: {
    subtype: 'pay_resources',
    outstanding: [{ kind: 'resources', amount: 4, paid: 0 }],
  },
  options: [
    {
      id: 'exhaust|jord',
      label: 'exhaust jord for 4 resources',
      kind: 'pay',
      payload: { worth: 4, owed: 4, kind: 'resources', source: 'planet' },
    },
    {
      id: 'exhaust|arinam',
      label: 'exhaust arinam for 1 resources',
      kind: 'pay',
      payload: { worth: 1, owed: 4, kind: 'resources', source: 'planet' },
    },
    {
      id: 'trade_good',
      label: 'spend a trade good',
      kind: 'pay',
      payload: { worth: 1, owed: 4, kind: 'resources' },
    },
    {
      id: 'decline',
      label: 'Cancel Payment',
      kind: 'decline',
    },
  ],
};

const mockPlayer: PlayerView = {
  id: 'p1',
  faction: 'sol',
  victory_points: 0,
  trade_goods: 3,
  commodities: 2,
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
  action_cards_count: 2,
  secret_objectives_count: 1,
  leaders: {},
};

describe('PaymentDrawer Component', () => {
  it('does not render when choice is null', () => {
    const { container } = render(
      <PaymentDrawer
        choice={null}
        player={mockPlayer}
        onSubmit={vi.fn()}
        isOpen={true}
        onClose={vi.fn()}
      />
    );
    expect(container.firstChild).toBeNull();
  });

  it('renders ready planets and trade good stepper with initial zero committed', () => {
    render(
      <PaymentDrawer
        choice={mockPaymentChoice}
        player={mockPlayer}
        onSubmit={vi.fn()}
        isOpen={true}
        onClose={vi.fn()}
      />
    );

    expect(screen.getByTestId('payment-drawer-title')).toHaveTextContent('Pay 4 Resources');
    expect(screen.getByTestId('committed-amount')).toHaveTextContent('0 Resources');
    expect(screen.getByTestId('planet-card-jord')).toBeInTheDocument();
    expect(screen.getByTestId('planet-card-arinam')).toBeInTheDocument();
    expect(screen.getByTestId('trade-goods-stepper')).toBeInTheDocument();

    const confirmBtn = screen.getByTestId('confirm-payment-btn');
    expect(confirmBtn).toBeDisabled();
  });

  it('toggles planet selection and enables confirm when debt is met', async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(
      <PaymentDrawer
        choice={mockPaymentChoice}
        player={mockPlayer}
        onSubmit={onSubmit}
        isOpen={true}
        onClose={vi.fn()}
      />
    );

    const jordCard = screen.getByTestId('planet-card-jord');
    const jordCheckbox = jordCard.querySelector('input[type="checkbox"]')!;

    // Select Jord (+4)
    fireEvent.click(jordCheckbox);

    expect(screen.getByTestId('committed-amount')).toHaveTextContent('4 Resources');
    const confirmBtn = screen.getByTestId('confirm-payment-btn');
    expect(confirmBtn).not.toBeDisabled();

    // Confirm single payment
    await act(async () => {
      fireEvent.click(confirmBtn);
    });

    expect(onSubmit).toHaveBeenCalledWith('exhaust|jord');
  });

  it('supports spending trade goods and shows retained credit on overpayment', () => {
    render(
      <PaymentDrawer
        choice={mockPaymentChoice}
        player={mockPlayer}
        onSubmit={vi.fn()}
        isOpen={true}
        onClose={vi.fn()}
      />
    );

    // Select Arinam (+1)
    const arinamCard = screen.getByTestId('planet-card-arinam');
    fireEvent.click(arinamCard.querySelector('input[type="checkbox"]')!);
    expect(screen.getByTestId('committed-amount')).toHaveTextContent('1 Resources');

    // Increment Trade Goods twice (+2)
    const tgIncBtn = screen.getByTestId('tg-increment-btn');
    fireEvent.click(tgIncBtn);
    fireEvent.click(tgIncBtn);
    expect(screen.getByTestId('tg-count')).toHaveTextContent('2');
    expect(screen.getByTestId('committed-amount')).toHaveTextContent('3 Resources');

    // Also select Jord (+4), bringing total to 7 (overpayment: 3 credit)
    const jordCard = screen.getByTestId('planet-card-jord');
    fireEvent.click(jordCard.querySelector('input[type="checkbox"]')!);
    expect(screen.getByTestId('committed-amount')).toHaveTextContent('7 Resources');
    expect(screen.getByText(/Credit Retained:/)).toBeInTheDocument();
    expect(screen.getByText(/\+3 Resources/)).toBeInTheDocument();
  });

  it('calls onSubmit with decline option when cancel is clicked', () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(
      <PaymentDrawer
        choice={mockPaymentChoice}
        player={mockPlayer}
        onSubmit={onSubmit}
        isOpen={true}
        onClose={vi.fn()}
      />
    );

    const declineBtn = screen.getByTestId('decline-payment-btn');
    expect(declineBtn).toHaveTextContent('Cancel Payment');
    fireEvent.click(declineBtn);
    expect(onSubmit).toHaveBeenCalledWith('decline');
  });
});
