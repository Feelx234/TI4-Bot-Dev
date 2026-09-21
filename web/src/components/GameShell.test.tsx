import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { GameShell } from './GameShell.tsx';
import { PendingChoiceDto } from '../protocol/types.ts';

const choice: PendingChoiceDto = {
  prompt: 'Choose a strategy card',
  actor: 'p1',
  nonce: 'choice-1',
  options: [{ id: 'leadership', label: 'Leadership' }],
};

function renderShell(pendingChoice: PendingChoiceDto | null = null) {
  return render(
    <GameShell
      header={<div>Header</div>}
      board={<div data-testid="board-content">Board</div>}
      playerSheet={<div>Player sheet</div>}
      events={[]}
      choice={pendingChoice}
      onSubmitChoice={vi.fn().mockResolvedValue(undefined)}
    />
  );
}

describe('GameShell', () => {
  it('owns player and event drawer visibility', () => {
    renderShell();

    const playerDrawer = screen.getByTestId('player-sheet-drawer');
    const eventDrawer = screen.getByLabelText('Event log');
    expect(playerDrawer).not.toHaveClass('app-shell__drawer--open');
    expect(eventDrawer).not.toHaveClass('app-shell__drawer--open');

    fireEvent.click(screen.getByTestId('player-sheet-toggle'));
    expect(playerDrawer).toHaveClass('app-shell__drawer--open');
    expect(screen.getByTestId('player-sheet-toggle')).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByTestId('player-sheet-toggle')).toHaveAttribute('aria-controls', 'player-sheet-drawer');

    fireEvent.click(screen.getByTestId('event-log-mobile-toggle'));
    expect(eventDrawer).toHaveClass('app-shell__drawer--open');
    expect(playerDrawer).not.toHaveClass('app-shell__drawer--open');
    expect(screen.getByTestId('event-log-mobile-toggle')).toHaveAttribute('aria-controls', 'event-log-drawer');
    expect(screen.getByTestId('event-log-toggle')).toHaveAttribute('aria-expanded', 'true');

    // Escape closes the open mobile drawer
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(eventDrawer).not.toHaveClass('app-shell__drawer--open');
  });

  it('renders pending choices through the shell overlay and resets minimization for a new nonce', () => {
    const { rerender } = renderShell(choice);

    expect(screen.getByTestId('pending-choice-dialog')).toHaveClass('choice-dialog');
    fireEvent.click(screen.getByTestId('minimize-choice-button'));
    expect(screen.getByTestId('minimized-choice-banner')).toHaveClass('choice-banner');

    rerender(
      <GameShell
        header={<div>Header</div>}
        board={<div>Board</div>}
        playerSheet={<div>Player sheet</div>}
        events={[]}
        choice={{ ...choice, nonce: 'choice-2' }}
        onSubmitChoice={vi.fn().mockResolvedValue(undefined)}
      />
    );

    expect(screen.getByTestId('pending-choice-dialog')).toBeInTheDocument();
  });

  it('synchronizes selectedOptionId and propagates onSelectOption when options change', () => {
    const onSelectOption = vi.fn();
    render(
      <GameShell
        header={<div>Header</div>}
        board={<div>Board</div>}
        playerSheet={<div>Player sheet</div>}
        events={[]}
        choice={{
          ...choice,
          options: [
            { id: 'opt_1', label: 'Option 1' },
            { id: 'opt_2', label: 'Option 2' },
          ],
        }}
        selectedOptionId="opt_2"
        onSelectOption={onSelectOption}
        onSubmitChoice={vi.fn().mockResolvedValue(undefined)}
      />
    );

    const radio2 = screen.getByDisplayValue('opt_2') as HTMLInputElement;
    expect(radio2.checked).toBe(true);

    const radio1 = screen.getByDisplayValue('opt_1') as HTMLInputElement;
    expect(radio1.checked).toBe(false);

    fireEvent.click(radio1);
    expect(onSelectOption).toHaveBeenCalledWith('opt_1');
  });
});
