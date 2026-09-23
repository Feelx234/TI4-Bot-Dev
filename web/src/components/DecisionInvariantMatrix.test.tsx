import { act } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { ChoiceRendererDispatcher } from './GameShell.tsx';
import { deriveChoiceRendererModel, type ChoiceWorkflowKind } from '../presentation/choiceModel.ts';
import type { PendingChoiceDto, PlayerView } from '../protocol/types.ts';

// These are explicitly synthetic protocol-shaped choices, not captured live sessions.
// Producer references and the boundary each fixture exercises are in the INV-03 plan ledger.
const actor = 'seat_a';
const player: PlayerView = {
  id: actor, faction: 'sol', victory_points: 0, trade_goods: 2, commodities: 0,
  tactic_tokens: 3, fleet_tokens: 3, strategic_tokens: 2, passed: false,
  strategy_cards: [], exhausted_strategy_cards: [], technologies: [], exhausted_technologies: [],
  relics: [], exhausted_relics: [], action_cards_count: 0, secret_objectives_count: 0, leaders: {},
};

const cases: Array<{
  name: string;
  choice: PendingChoiceDto;
  workflow: ChoiceWorkflowKind;
  region: string;
  action: string;
  option: string;
  prepare?: string;
}> = [
  {
    name: 'empty movement', workflow: 'tactical_movement', region: 'tactical-movement-tray',
    action: 'commit-moves-btn', option: 'done_moving',
    choice: { actor, nonce: 'empty-1', prompt: 'movement', context: { subtype: 'movement_step', target: { System: '18' } },
      options: [{ id: 'done_moving', kind: 'decline', label: 'finish movement' }] },
  },
  {
    name: 'normal movement', workflow: 'tactical_movement', region: 'tactical-movement-tray',
    prepare: 'rally-inc-24-cruiser', action: 'commit-moves-btn', option: 'move|24|0',
    choice: { actor, nonce: 'move-1', prompt: 'movement', context: { subtype: 'movement_step', target: { System: '18' } },
      options: [{ id: 'move|24|0', kind: 'move', label: 'move cruiser from 24', payload: { origin: '24', unit: 'cruiser', capacity: 0 } },
        { id: 'done_moving', kind: 'decline', label: 'finish movement' }] },
  },
  {
    name: 'resource payment', workflow: 'payment', region: 'payment-drawer',
    prepare: 'planet-card-exhaust|jord', action: 'confirm-payment-btn', option: 'exhaust|jord',
    choice: { actor, nonce: 'pay-1', prompt: 'pay 4 resources', context: { subtype: 'pay_resources', outstanding: [{ kind: 'resources', amount: 4, paid: 0 }] },
      options: [{ id: 'exhaust|jord', kind: 'pay', label: 'exhaust jord for 4 resources', payload: { worth: 4, owed: 4, kind: 'resources' } }] },
  },
  {
    name: 'unit production', workflow: 'production', region: 'production-builder-drawer',
    action: 'done-producing-btn', option: 'done_producing',
    choice: { actor, nonce: 'produce-1', prompt: 'produce in 18 (3 left)', context: { subtype: 'produce_unit', target: { System: '18' }, outstanding: [{ kind: 'production_capacity', amount: 3, paid: 0 }] },
      options: [{ id: 'build|infantry|1', kind: 'produce', label: 'produce 1x infantry for 1' }, { id: 'done_producing', kind: 'decline', label: 'produce nothing further' }] },
  },
  {
    name: 'reaction pass', workflow: 'action_card_reaction', region: 'reaction-status-bar',
    action: 'pass-reaction-btn', option: 'decline',
    choice: { actor, nonce: 'react-1', prompt: 'play an action card (after ACTION_CARD_PLAYED)',
      context: { subtype: 'play_reaction_after_ACTION_CARD_PLAYED', optional: true, source: { Reaction: 'ACTION_CARD_PLAYED' } },
      options: [{ id: 'fs1', kind: 'action_card', label: 'play Sabotage' }, { id: 'decline', kind: 'decline', label: 'pass' }] },
  },
  {
    name: 'constrained generic choice', workflow: 'generic_selection', region: 'pending-choice-dialog',
    prepare: 'select-generic', action: 'submit-choice-button', option: 'card_a',
    choice: { actor, nonce: 'generic-1', prompt: 'select two cards', context: { subtype: 'synthetic_bounded_selection', outstanding: [{ min_selection: 2, max_selection: 2 }] },
      options: [{ id: 'card_a', label: 'Card A' }, { id: 'card_b', label: 'Card B' }, { id: 'card_c', label: 'Card C' }] },
  },
];

function dispatcher(choice: PendingChoiceDto, onSubmit: (id: string) => Promise<void>, viewerSeat = actor) {
  return <ChoiceRendererDispatcher choice={choice} viewerSeat={viewerSeat} players={{ [actor]: player }}
    onSubmit={onSubmit} isMinimized={false} onMinimizedChange={vi.fn()} />;
}

describe('INV-03 decision invariant matrix', () => {
  for (const fixture of cases) {
    it(`${fixture.name}: classifies, exposes a keyboard action, and submits only an offered ID`, async () => {
      const { choice, option } = fixture;
      const model = deriveChoiceRendererModel(choice, actor);
      expect(model?.workflow).toBe(fixture.workflow);
      expect(choice.options.map((o) => o.id)).toContain(option);
      expect(deriveChoiceRendererModel(choice, 'seat_b')).toBeNull();

      const onSubmit = vi.fn().mockResolvedValue(undefined);
      const { unmount } = render(dispatcher(choice, onSubmit));
      expect(screen.getByTestId(fixture.region)).toBeVisible();
      if (fixture.prepare === 'select-generic') {
        expect(screen.getByTestId('submit-choice-button')).toBeDisabled();
        fireEvent.click(screen.getByRole('checkbox', { name: 'Card A' }));
        fireEvent.click(screen.getByRole('checkbox', { name: 'Card B' }));
        expect(screen.getByRole('checkbox', { name: 'Card C' })).toBeDisabled();
      } else if (fixture.prepare?.startsWith('planet-card-')) {
        fireEvent.click(screen.getByTestId(fixture.prepare).querySelector('input')!);
      } else if (fixture.prepare) {
        fireEvent.click(screen.getByTestId(fixture.prepare));
      }
      const button = screen.getByTestId(fixture.action);
      expect(button).toBeVisible();
      expect(button).toBeEnabled();
      expect(button.tagName).toBe('BUTTON');
      await act(async () => { fireEvent.click(button); });
      await waitFor(() => expect(onSubmit).toHaveBeenNthCalledWith(1, option));
      expect(onSubmit.mock.calls.every(([id]) => choice.options.some((o) => o.id === id))).toBe(true);
      unmount();

      const otherSubmit = vi.fn().mockResolvedValue(undefined);
      const other = render(dispatcher(choice, otherSubmit, 'seat_b'));
      expect(screen.queryByTestId(fixture.region)).not.toBeInTheDocument();
      expect(screen.queryByTestId('pending-choice-dialog')).not.toBeInTheDocument();
      expect(otherSubmit).not.toHaveBeenCalled();
      other.unmount();
    });
  }

  it('normal movement waits for the acknowledged choice to change nonce before finishing', async () => {
    const first = cases[1].choice;
    const finish: PendingChoiceDto = { ...first, nonce: 'move-2', options: [first.options[1]] };
    let acknowledge!: () => void;
    const onSubmit = vi.fn().mockImplementationOnce(() => new Promise<void>((resolve) => { acknowledge = resolve; }))
      .mockResolvedValue(undefined);
    const { rerender } = render(dispatcher(first, onSubmit));
    fireEvent.click(screen.getByTestId('rally-inc-24-cruiser'));
    fireEvent.click(screen.getByTestId('commit-moves-btn'));
    await waitFor(() => expect(onSubmit).toHaveBeenCalledExactlyOnceWith('move|24|0'));
    await act(async () => { acknowledge(); });
    expect(onSubmit).toHaveBeenCalledTimes(1);
    rerender(dispatcher(finish, onSubmit));
    await waitFor(() => expect(onSubmit).toHaveBeenNthCalledWith(2, 'done_moving'));
    expect(finish.options.map((o) => o.id)).toContain(onSubmit.mock.calls[1][0]);
  });

  it('does not expose an actor model supplied to the dispatcher to another seat', () => {
    const choice = cases[0].choice;
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<ChoiceRendererDispatcher choice={choice} model={deriveChoiceRendererModel(choice, actor)}
      viewerSeat="seat_b" onSubmit={onSubmit} isMinimized onMinimizedChange={vi.fn()} />);
    expect(screen.queryByTestId('choice-minimized-pill')).not.toBeInTheDocument();
    expect(screen.queryByTestId('pending-choice-dialog')).not.toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();
  });
});
