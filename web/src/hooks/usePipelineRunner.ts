import { useState, useEffect } from 'react';
import { PendingChoiceDto, ChoiceOptionDto } from '../protocol/types.ts';

export interface SemanticIntent {
  kind: 'movement' | 'casualty' | 'payment';
  predicate: (option: ChoiceOptionDto) => boolean;
  description: string;
}

export function usePipelineRunner(
  pendingChoice: PendingChoiceDto | null,
  submitChoice: (optionId: string) => Promise<void>
) {
  const [activeQueue, setActiveQueue] = useState<SemanticIntent[]>([]);
  const [isRunning, setIsRunning] = useState(false);
  const [lastError, setLastError] = useState<string | null>(null);

  useEffect(() => {
    if (!isRunning || activeQueue.length === 0 || !pendingChoice) return;

    const nextIntent = activeQueue[0];
    const matchingOption = pendingChoice.options.find(nextIntent.predicate);

    if (matchingOption) {
      submitChoice(matchingOption.id)
        .then(() => {
          setActiveQueue((prev) => prev.slice(1));
        })
        .catch((err) => {
          setIsRunning(false);
          setActiveQueue([]);
          setLastError(err instanceof Error ? err.message : String(err));
        });
    } else {
      // Intervening decision occurred or intent no longer available: cleanly pause pipeline
      setIsRunning(false);
      setActiveQueue([]);
    }
  }, [pendingChoice?.nonce, isRunning]);

  const executePipeline = (intents: SemanticIntent[]) => {
    if (intents.length === 0) return;
    setActiveQueue(intents);
    setIsRunning(true);
    setLastError(null);
  };

  const cancelPipeline = () => {
    setActiveQueue([]);
    setIsRunning(false);
  };

  return {
    executePipeline,
    cancelPipeline,
    isRunning,
    queueLength: activeQueue.length,
    lastError,
  };
}
