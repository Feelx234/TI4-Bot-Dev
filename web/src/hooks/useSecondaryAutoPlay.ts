import { useCallback, useEffect, useRef, useState } from "react";
import type { HistoryStatus, PendingChoiceDto } from "../protocol/types.ts";
import type { TokenStep } from "../presentation/commandTokens.ts";
import type { StepResolution } from "../presentation/secondaryPlan.ts";
import { readSecondaryPrepMode } from "./useSecondaryPrepMode.ts";

/** How long the toast is visible (and cancellable) before a prepared answer is sent. */
export const AUTO_PLAY_DELAY_MS = 2500;
const CLAIM_KEY = "ti4_secondary_autoplay_claim";

export interface SecondaryAutoPlayInput {
  choice: PendingChoiceDto | null;
  /** The viewer's own seat; absent for spectators, who never act. */
  viewerSeat?: string | null;
  history?: HistoryStatus;
  /** The pipeline runner or a history change is busy: leave the decision alone. */
  busy?: boolean;
  /** What the prepared plan says about this decision (already validated against its options). */
  resolution: StepResolution;
  /** Same path a click takes (`submit_choice` with nonce and expected version). */
  submitOption: (optionId: string) => Promise<void>;
  /** Same path the token panel's confirm takes. */
  submitTokens?: (steps: TokenStep[]) => Promise<void>;
}

export interface AutoPlayNotice {
  nonce: string;
  text: string;
}

/** Cross-tab claim so two tabs of one seat do not both send; the server still rejects a repeat. */
function claim(nonce: string): boolean {
  try {
    if (localStorage.getItem(CLAIM_KEY) === nonce) return false;
    localStorage.setItem(CLAIM_KEY, nonce);
  } catch {
    // No storage: rely on the server's nonce and version checks.
  }
  return true;
}

/**
 * Auto mode: sends the prepared answer for the viewer's own pending decision, once, after a short
 * visible delay in which it can be cancelled. The engine still asks and the answer is an ordinary
 * recorded submission, so nothing is spent before this decision is answered.
 *
 * The same "armed" rule as the lone-case auto-submit (undo safety): a decision is auto-answered
 * only when the game moved FORWARD to it (the history cursor grew with nothing to redo before or
 * after) since the previous decision was evaluated. A page load, a reconnect that changed nothing,
 * an undo, a redo and a restore never arm it, so a decision reached by undoing is always asked.
 * Each nonce is evaluated once; a failed submit is not retried. Multi-step plans (follow, then the
 * technology) are armed again by their own forward progress, and stop at the first step that does
 * not validate (`review`) or that the plan has nothing to say about (`none`).
 */
export function useSecondaryAutoPlay({
  choice,
  viewerSeat,
  history,
  busy,
  resolution,
  submitOption,
  submitTokens,
}: SecondaryAutoPlayInput) {
  const armed = useRef(false);
  const prev = useRef<HistoryStatus | null>(null);
  const evaluated = useRef<string | null>(null);
  const timer = useRef<number | null>(null);
  const [pending, setPending] = useState<AutoPlayNotice | null>(null);
  const submitRef = useRef(submitOption);
  submitRef.current = submitOption;
  const tokensRef = useRef(submitTokens);
  tokensRef.current = submitTokens;
  const busyRef = useRef(busy);
  busyRef.current = busy;
  const resolutionRef = useRef(resolution);
  resolutionRef.current = resolution;

  // Track history: forward progress arms, anything else disarms.
  useEffect(() => {
    if (!history) return;
    const before = prev.current;
    prev.current = history;
    if (!before) return;
    if (history.generation !== before.generation) armed.current = false;
    else if (history.redo_count > 0 || before.redo_count > 0) armed.current = false;
    else if (history.cursor > before.cursor) armed.current = true;
    else if (history.cursor < before.cursor) armed.current = false;
  }, [history]);

  const nonce = choice && viewerSeat && choice.actor === viewerSeat ? choice.nonce : null;
  const actionable = resolution.kind === "option" || resolution.kind === "tokens";
  const text = actionable ? resolution.text : null;

  useEffect(() => {
    if (!nonce || evaluated.current === nonce) return;
    evaluated.current = nonce;
    const wasArmed = armed.current;
    armed.current = false;
    if (!wasArmed || !actionable || text === null) return;
    if (readSecondaryPrepMode() !== "auto") return;
    setPending({ nonce, text });
    timer.current = window.setTimeout(() => {
      timer.current = null;
      setPending(null);
      const current = resolutionRef.current;
      if (busyRef.current || readSecondaryPrepMode() !== "auto" || !claim(nonce)) return;
      let sent: Promise<void> | undefined;
      if (current.kind === "option") sent = submitRef.current(current.optionId);
      else if (current.kind === "tokens") sent = tokensRef.current?.(current.steps);
      sent?.catch(() => {
        // The session client already records the error; the decision stays open for a click.
      });
    }, AUTO_PLAY_DELAY_MS);
    return () => {
      if (timer.current !== null) window.clearTimeout(timer.current);
      timer.current = null;
      setPending(null);
    };
  }, [nonce, actionable, text]);

  /** Stops the scheduled answer; the decision then stays open for a click. */
  const cancel = useCallback(() => {
    if (timer.current !== null) window.clearTimeout(timer.current);
    timer.current = null;
    setPending(null);
  }, []);

  return { pending, cancel };
}
