import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { deriveChoiceRendererModel, type ChoiceWorkflowKind } from '../presentation/choiceModel.ts';
import { DecisionGallery } from './DecisionGallery.tsx';
import { fallbackCases, galleryCases } from './decisionGalleryCases.ts';

describe('development decision gallery', () => {
  it('has a correctly classified synthetic preview for every workflow and labels fallbacks', () => {
    expect(galleryCases).toHaveLength(16);
    expect(new Set(galleryCases.map(({ workflow }) => workflow)).size).toBe(16);
    for (const item of [...galleryCases, ...fallbackCases]) {
      expect(deriveChoiceRendererModel(item.choice, item.choice.actor)?.workflow).toBe(item.workflow as ChoiceWorkflowKind);
      expect(deriveChoiceRendererModel(item.choice, 'other_seat')).toBeNull();
    }
    expect(fallbackCases.every((item) => Boolean(item.fallback))).toBe(true);
  });

  it('previews the real renderer, offered IDs, local submissions and the other-seat boundary', async () => {
    render(<DecisionGallery />);
    expect(screen.getByText('Workflow kinds (16)')).toBeInTheDocument();
    expect(screen.getByText(`Fallbacks and boundary states (${fallbackCases.length})`)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /empty movement/i }));
    expect(screen.getByTestId('tactical-movement-tray')).toBeVisible();
    fireEvent.click(screen.getByTestId('commit-moves-btn'));
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Local submission: done_moving'));
    expect(screen.getByRole('status')).toHaveTextContent('no engine transition');
    fireEvent.click(screen.getByRole('button', { name: 'Minimize decision' }));
    fireEvent.click(screen.getByLabelText('View as another seat'));
    expect(screen.queryByTestId('tactical-movement-tray')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'All decisions' }));
    fireEvent.click(screen.getByRole('button', { name: /^Unknown subtype Unknown subtype → generic modal/i }));
    expect(screen.getByTestId('pending-choice-dialog')).toBeVisible();
    expect(screen.getByRole('region', { name: 'Preview details' })).toHaveTextContent('Unknown subtype → generic modal');
  });
});
