import { useState, useEffect, useRef } from "react";
import { PendingChoiceDto, ChoiceOptionDto } from "../protocol/types.ts";

export interface SemanticIntent {
  predicate: (option: ChoiceOptionDto) => boolean;
}

export interface PipelineRunnerOptions {
  allowedSubtypes?: string[];
}

export function usePipelineRunner(
  pendingChoice: PendingChoiceDto | null,
  submitChoice: (optionId: string) => Promise<void>,
  options?: PipelineRunnerOptions,
) {
  const [activeQueue, setActiveQueue] = useState<SemanticIntent[]>([]);
  const [isRunning, setIsRunning] = useState(false);
  const [lastError, setLastError] = useState<string | null>(null);
  const [lastSubmittedNonce, setLastSubmittedNonce] = useState<string | null>(null);
  const isSubmittingRef = useRef(false);
  const runRef = useRef(0);
  const originRef = useRef<{
    actor: string;
    subtype?: string;
    allowedSubtypes?: string[];
  } | null>(null);
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

    if (isSubmittingRef.current) return;
    if (!pendingChoice) {
      setIsRunning(false);
      setActiveQueue([]);
      setLastError("Decision pipeline interrupted: no pending choice is available.");
      return;
    }
    if (pendingChoice.nonce === lastSubmittedNonce) return;

    if (originRef.current) {
      const isActorSame = pendingChoice.actor === originRef.current.actor;
      const isSubtypeAllowed = originRef.current.allowedSubtypes
        ? originRef.current.allowedSubtypes.includes(pendingChoice.context?.subtype ?? "")
        : pendingChoice.context?.subtype === originRef.current.subtype;

      if (!isActorSame || !isSubtypeAllowed) {
        setIsRunning(false);
        setActiveQueue([]);
        setLastError(
          "Decision pipeline interrupted: a different decision was offered. Previously submitted choices remain committed.",
        );
        return;
      }
    }

    const nextIntent = activeQueue[0];
    const matchingOption = pendingChoice.options.find(nextIntent.predicate);

    if (matchingOption) {
      isSubmittingRef.current = true;
      const run = runRef.current;
      const nonce = pendingChoice.nonce;
      submitRef
        .current(matchingOption.id)
        .then(() => {
          if (run !== runRef.current) return;
          setLastSubmittedNonce(nonce);
          setActiveQueue((prev) => {
            const next = prev.slice(1);
            if (next.length === 0) {
              setIsRunning(false);
            }
            return next;
          });
        })
        .catch((err) => {
          if (run !== runRef.current) return;
          setIsRunning(false);
          setActiveQueue([]);
          setLastError(err instanceof Error ? err.message : String(err));
        })
        .finally(() => {
          isSubmittingRef.current = false;
        });
    } else {
      // Intervening decision occurred or intent no longer available.
      setIsRunning(false);
      setActiveQueue([]);
      isSubmittingRef.current = false;
      setLastError("Decision pipeline interrupted: the next option is no longer available.");
    }
  }, [pendingChoice?.nonce, isRunning, activeQueue, lastSubmittedNonce]);

  const executePipeline = (
    intents: SemanticIntent[],
    execOptions?: PipelineRunnerOptions,
  ) => {
    runRef.current += 1;
    originRef.current = pendingChoice
      ? {
          actor: pendingChoice.actor,
          subtype: pendingChoice.context?.subtype,
          allowedSubtypes: execOptions?.allowedSubtypes ?? options?.allowedSubtypes,
        }
      : null;
    if (intents.length === 0) {
      setIsRunning(false);
      return;
    }
    setActiveQueue(intents);
    setIsRunning(true);
    setLastSubmittedNonce(null);
    setLastError(null);
  };

  const cancelPipeline = () => {
    runRef.current += 1;
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
