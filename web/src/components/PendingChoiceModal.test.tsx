import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { PendingChoiceModal } from './PendingChoiceModal.tsx';
import { PendingChoiceDto } from '../protocol/types.ts';

const mockChoice: PendingChoiceDto = {
  prompt: 'Select a Strategy Card',
  actor: 'p1',
  nonce: '1234567890abcdef',
  options: [
    { id: 'strat_leadership', label: 'Leadership (1)', description: 'Gain 3 command tokens' },
    { id: 'strat_diplomacy', label: 'Diplomacy (2)', description: 'Ready 2 planets' },
  ],
  context: {
    kind: 'strategy_draft',
    subtype: 'draft_pick',
  },
};

describe('PendingChoiceModal Component', () => {
  it('does not render when choice is null', () => {
    const { container } = render(
      <PendingChoiceModal choice={null} onSubmit={vi.fn()} />
    );
    expect(container.firstChild).toBeNull();
  });

  it('renders prompt and options when choice is provided', () => {
    render(<PendingChoiceModal choice={mockChoice} onSubmit={vi.fn()} />);

    expect(screen.getByTestId('pending-choice-dialog')).toBeInTheDocument();
    expect(screen.getByTestId('choice-prompt')).toHaveTextContent('Select a Strategy Card');
    expect(screen.getAllByTestId('choice-option')).toHaveLength(2);
    expect(screen.getByText('Leadership (1)')).toBeInTheDocument();
    expect(screen.getByText('Diplomacy (2)')).toBeInTheDocument();
  });

  it('does not infer metadata from an opaque option ID', () => {
    const choiceWithoutDescription: PendingChoiceDto = {
      ...mockChoice,
      options: [{ id: 'pok1leadership', label: 'Take this option' }],
    };

    render(<PendingChoiceModal choice={choiceWithoutDescription} onSubmit={vi.fn()} />);

    expect(screen.getByText('Take this option')).toBeInTheDocument();
    expect(screen.queryByText(/Gain 3 command tokens/)).toBeNull();
  });

  it('submits selected option on confirm', async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<PendingChoiceModal choice={mockChoice} onSubmit={onSubmit} />);

    // Select second option
    const secondOption = screen.getByLabelText(/Diplomacy \(2\)/i);
    fireEvent.click(secondOption);

    const submitBtn = screen.getByTestId('submit-choice-button');
    await act(async () => {
      fireEvent.click(submitBtn);
    });

    expect(onSubmit).toHaveBeenCalledWith('strat_diplomacy');
  });

  it('submits once until the server response resolves the submission', async () => {
    let resolveSubmission!: () => void;
    const onSubmit = vi.fn(() => new Promise<void>((resolve) => { resolveSubmission = resolve; }));
    render(<PendingChoiceModal choice={mockChoice} onSubmit={onSubmit} />);

    const submitButton = screen.getByTestId('submit-choice-button');
    fireEvent.click(submitButton);
    fireEvent.click(submitButton);
    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(submitButton).toBeDisabled();

    await act(async () => resolveSubmission());
    expect(submitButton).not.toBeDisabled();
  });

  it('moves focus into the dialog and minimizes on Escape', () => {
    render(<PendingChoiceModal choice={mockChoice} onSubmit={vi.fn()} />);
    const dialog = screen.getByTestId('pending-choice-dialog');
    expect(dialog).toHaveFocus();
    fireEvent.keyDown(dialog, { key: 'Escape' });
    expect(screen.getByTestId('minimized-choice-banner')).toBeInTheDocument();
  });

  it('displays error banner when lastError is set', () => {
    render(
      <PendingChoiceModal
        choice={mockChoice}
        onSubmit={vi.fn()}
        lastError="Stale version mismatch"
      />
    );

    const errorBanner = screen.getByTestId('choice-error-banner');
    expect(errorBanner).toBeInTheDocument();
    expect(errorBanner).toHaveTextContent('Stale version mismatch');
  });

  it('allows minimizing to inspect the map and reopening the dialog', () => {
    render(<PendingChoiceModal choice={mockChoice} onSubmit={vi.fn()} />);

    // Full modal is initially open
    expect(screen.getByTestId('pending-choice-dialog')).toBeInTheDocument();
    expect(screen.queryByTestId('minimized-choice-banner')).toBeNull();

    // Click minimize / inspect map button
    const minimizeBtn = screen.getByTestId('minimize-choice-button');
    fireEvent.click(minimizeBtn);

    // Full dialog is hidden, non-blocking floating banner is visible
    expect(screen.queryByTestId('pending-choice-dialog')).toBeNull();
    expect(screen.getByTestId('minimized-choice-banner')).toBeInTheDocument();
    expect(screen.getByText('Decision Required (p1):')).toBeInTheDocument();

    // Click resume button to restore dialog
    const resumeBtn = screen.getByTestId('resume-choice-button');
    fireEvent.click(resumeBtn);

    // Dialog is restored
    expect(screen.getByTestId('pending-choice-dialog')).toBeInTheDocument();
    expect(screen.queryByTestId('minimized-choice-banner')).toBeNull();
  });
});
