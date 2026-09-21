import { act } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { WorkflowShell } from './WorkflowShell.tsx';
import { PendingChoiceDto } from '../protocol/types.ts';

const choice: PendingChoiceDto = {
  actor: 'seat_2',
  nonce: 'workflow-1',
  prompt: 'Choose',
  context: { subtype: '' },
  options: [{ id: 'decline', label: 'Decline', kind: 'decline' }],
};

describe('WorkflowShell', () => {
  it('centralizes spectator feedback, decline extraction, errors, and nonce-reset submission state', async () => {
    let resolveSubmit: (() => void) | undefined;
    const onSubmit = vi.fn(() => new Promise<void>((resolve) => { resolveSubmit = resolve; }));
    const { rerender } = render(
      <WorkflowShell
        choice={choice}
        viewerSeat="seat_1"
        onSubmit={onSubmit}
        spectatorNotice="Observing seat_2"
        spectatorNoticeTestId="spectator-notice"
        lastError="Submission failed"
        errorTestId="workflow-error"
      >
        {({ declineOption, isDirectSubmitting, submitDirect }) => (
          <button type="button" disabled={isDirectSubmitting} onClick={() => void submitDirect(declineOption!.id)}>
            {isDirectSubmitting ? 'Submitting' : declineOption?.label}
          </button>
        )}
      </WorkflowShell>,
    );

    expect(screen.getByTestId('spectator-notice')).toHaveTextContent('Observing seat_2');
    expect(screen.getByTestId('workflow-error')).toHaveTextContent('Submission failed');
    fireEvent.click(screen.getByRole('button', { name: 'Decline' }));
    expect(onSubmit).toHaveBeenCalledWith('decline');
    expect(screen.getByRole('button', { name: 'Submitting' })).toBeDisabled();

    rerender(
      <WorkflowShell
        choice={{ ...choice, nonce: 'workflow-2' }}
        viewerSeat="seat_1"
        onSubmit={onSubmit}
        spectatorNotice="Observing seat_2"
        spectatorNoticeTestId="spectator-notice"
        lastError="Submission failed"
        errorTestId="workflow-error"
      >
        {({ declineOption, isDirectSubmitting, submitDirect }) => (
          <button type="button" disabled={isDirectSubmitting} onClick={() => void submitDirect(declineOption!.id)}>
            {isDirectSubmitting ? 'Submitting' : declineOption?.label}
          </button>
        )}
      </WorkflowShell>,
    );

    expect(screen.getByRole('button', { name: 'Decline' })).not.toBeDisabled();
    await act(async () => { resolveSubmit?.(); });
  });
});
