import { useState, useEffect, useRef } from 'react';
import { PendingChoiceDto, ChoiceOptionDto } from '../protocol/types.ts';

export interface SemanticIntent {
  predicate: (option: ChoiceOptionDto) => boolean;
}

export function usePipelineRunner(
  pendingChoice: PendingChoiceDto | null,
  submitChoice: (optionId: string) => Promise<void>
) {
  const [activeQueue, setActiveQueue] = useState<SemanticIntent[]>([]);
  const [isRunning, setIsRunning] = useState(false);
  const [lastError, setLastError] = useState<string | null>(null);
  const isSubmittingRef = useRef(false);
  // Keep a stable ref to submitChoice so the effect never lists it as a
  // dependency. This prevents the effect from firing mid-run whenever the
  // parent re-creates onSubmit on every render.
  const submitRef = useRef(submitChoice);
  submitRef.current = submitChoice;

  useEffect(() => {
    if (!isRunning) return;

    if (activeQueue.length === 0) {
      setIsRunning(false);
      isSubmittingRef.current = false;
      return;
    }

    if (!pendingChoice || isSubmittingRef.current) return;

    const nextIntent = activeQueue[0];
    const matchingOption = pendingChoice.options.find(nextIntent.predicate);

    if (matchingOption) {
      isSubmittingRef.current = true;
      submitRef.current(matchingOption.id)
        .then(() => {
          setActiveQueue((prev) => {
            const next = prev.slice(1);
            if (next.length === 0) {
              setIsRunning(false);
            }
            return next;
          });
        })
        .catch((err) => {
          setIsRunning(false);
          setActiveQueue([]);
          setLastError(err instanceof Error ? err.message : String(err));
        })
        .finally(() => {
          isSubmittingRef.current = false;
        });
    } else {
      // Intervening decision occurred or intent no longer available: cleanly stop pipeline
      setIsRunning(false);
      setActiveQueue([]);
      isSubmittingRef.current = false;
    }
  }, [pendingChoice?.nonce, isRunning, activeQueue]);

  const executePipeline = (intents: SemanticIntent[]) => {
    if (intents.length === 0) {
      setIsRunning(false);
      return;
    }
    setActiveQueue(intents);
    setIsRunning(true);
    setLastError(null);
  };

  const cancelPipeline = () => {
    setActiveQueue([]);
    setIsRunning(false);
    isSubmittingRef.current = false;
  };

  return {
    executePipeline,
    cancelPipeline,
    isRunning,
    queueLength: activeQueue.length,
    lastError,
  };
}
