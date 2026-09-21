import { describe, it, expect, vi } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { usePipelineRunner, SemanticIntent } from './usePipelineRunner.ts';
import { PendingChoiceDto } from '../protocol/types.ts';

describe('usePipelineRunner', () => {
  it('executes sequential intents and resets isRunning to false on completion', async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);

    const initialChoice: PendingChoiceDto = {
      actor: 'p1',
      nonce: 'n1',
      prompt: 'Pay debt',
      options: [
        { id: 'exhaust|jord', label: 'Jord' },
        { id: 'exhaust|mecatol', label: 'Mecatol Rex' },
      ],
    };

    let currentChoice: PendingChoiceDto | null = initialChoice;

    const { result, rerender } = renderHook(
      ({ choice }) => usePipelineRunner(choice, onSubmit),
      { initialProps: { choice: currentChoice } }
    );

    expect(result.current.isRunning).toBe(false);

    const intents: SemanticIntent[] = [
      {
        kind: 'payment',
        predicate: (opt) => opt.id === 'exhaust|jord',
        description: 'Exhaust Jord',
      },
      {
        kind: 'payment',
        predicate: (opt) => opt.id === 'exhaust|mecatol',
        description: 'Exhaust Mecatol',
      },
    ];

    // Start pipeline
    await act(async () => {
      result.current.executePipeline(intents);
    });

    expect(onSubmit).toHaveBeenCalledWith('exhaust|jord');

    // Simulate choice update from server after step 1
    currentChoice = {
      actor: 'p1',
      nonce: 'n2',
      prompt: 'Pay remaining debt',
      options: [{ id: 'exhaust|mecatol', label: 'Mecatol Rex' }],
    };

    await act(async () => {
      rerender({ choice: currentChoice });
    });

    expect(onSubmit).toHaveBeenCalledWith('exhaust|mecatol');

    // After all intents complete, isRunning MUST be false (no lockout)
    expect(result.current.isRunning).toBe(false);
    expect(result.current.queueLength).toBe(0);
  });

  it('stops cleanly and resets isRunning if an intent cannot be matched', async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);

    const choice: PendingChoiceDto = {
      actor: 'p1',
      nonce: 'n1',
      prompt: 'Choose',
      options: [{ id: 'opt_1', label: 'Option 1' }],
    };

    const { result } = renderHook(() => usePipelineRunner(choice, onSubmit));

    const intents: SemanticIntent[] = [
      {
        kind: 'payment',
        predicate: (opt) => opt.id === 'nonexistent',
        description: 'Missing option',
      },
    ];

    await act(async () => {
      result.current.executePipeline(intents);
    });

    expect(onSubmit).not.toHaveBeenCalled();
    expect(result.current.isRunning).toBe(false);
    expect(result.current.queueLength).toBe(0);
  });

  it('resets isRunning and captures error if submitChoice rejects', async () => {
    const onSubmit = vi.fn().mockRejectedValue(new Error('Network failure'));

    const choice: PendingChoiceDto = {
      actor: 'p1',
      nonce: 'n1',
      prompt: 'Choose',
      options: [{ id: 'opt_1', label: 'Option 1' }],
    };

    const { result } = renderHook(() => usePipelineRunner(choice, onSubmit));

    const intents: SemanticIntent[] = [
      {
        kind: 'payment',
        predicate: (opt) => opt.id === 'opt_1',
        description: 'Option 1',
      },
    ];

    await act(async () => {
      result.current.executePipeline(intents);
    });

    expect(onSubmit).toHaveBeenCalledWith('opt_1');
    expect(result.current.isRunning).toBe(false);
    expect(result.current.lastError).toBe('Network failure');
  });

  it('cancelPipeline clears active queue and resets isRunning', async () => {
    let resolveSubmission!: () => void;
    const onSubmit = vi.fn(() => new Promise<void>((resolve) => { resolveSubmission = resolve; }));

    const choice: PendingChoiceDto = {
      actor: 'p1',
      nonce: 'n1',
      prompt: 'Choose',
      options: [{ id: 'opt1', label: 'Option 1' }],
    };

    const { result } = renderHook(() => usePipelineRunner(choice, onSubmit));

    act(() => {
      result.current.executePipeline([
        { kind: 'payment', predicate: (o) => o.id === 'opt1', description: 'test' },
      ]);
    });

    expect(result.current.isRunning).toBe(true);

    act(() => {
      result.current.cancelPipeline();
    });

    expect(result.current.isRunning).toBe(false);
    expect(result.current.queueLength).toBe(0);

    // Cleanup pending promise
    await act(async () => {
      resolveSubmission();
    });
  });
});
