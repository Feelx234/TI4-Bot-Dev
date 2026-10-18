import { useEffect, useRef } from "react";
import type { HistoryStatus, PendingChoiceDto } from "../protocol/types.ts";
import { isDryNonce } from "../presentation/dryChoice.ts";

/** Pause before answering, so the board paints and a double render settles. */
export const OFFER_DECLINE_DELAY_MS = 400;

export interface OfferAutoDeclineInput {
  choice: PendingChoiceDto | null;
  /** The viewer's own seat; absent for spectators, who never act. */
  viewerSeat?: string | null;
  history?: HistoryStatus;
  busy?: boolean;
  submit: (optionId: string, nonce: string) => Promise<void>;
  onNotice?: (note: { id: string; text: string }) => void;
}

export interface OfferAutoDeclineSpec {
  /** The option to answer with, or null when the client may not answer this question. */
  declineOption: (choice: PendingChoiceDto | null, viewerSeat?: string | null) => string | null;
  /** Whether the viewer wants to be asked (read fresh each time). */
  readAsk: () => boolean;
  noticePrefix: string;
  noticeText: string;
  delayMs?: number;
}

/**
 * Declines an optional ability offer for the viewer's own seat while its "ask" switch is off. The
 * engine still asks and the decline is an ordinary recorded `submit_choice`. Only when the game
 * moved forward to the offer, never after an undo, redo, restore or page load, so the player can
 * always undo past it and see it again.
 */
export function useOfferAutoDecline(
  { choice, viewerSeat, history, busy, submit, onNotice }: OfferAutoDeclineInput,
  spec: OfferAutoDeclineSpec,
) {
  const armed = useRef(false);
  const prev = useRef<HistoryStatus | null>(null);
  const evaluated = useRef<string | null>(null);
  const submitRef = useRef(submit);
  submitRef.current = submit;
  const noticeRef = useRef(onNotice);
  noticeRef.current = onNotice;
  const busyRef = useRef(busy);
  busyRef.current = busy;

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

  const nonce = choice?.nonce ?? null;
  const declineId = !isDryNonce(nonce) ? spec.declineOption(choice, viewerSeat) : null;

  useEffect(() => {
    if (!nonce || evaluated.current === nonce) return;
    evaluated.current = nonce;
    const wasArmed = armed.current;
    armed.current = false;
    // Decided when the offer appears: switching off while it is open leaves this one to the player.
    if (!wasArmed || !declineId || spec.readAsk()) return;
    const timer = window.setTimeout(() => {
      if (busyRef.current || spec.readAsk()) return;
      noticeRef.current?.({ id: `${spec.noticePrefix}-${nonce}`, text: spec.noticeText });
      submitRef.current(declineId, nonce).catch(() => {
        // The decision stays open for a click.
      });
    }, spec.delayMs ?? OFFER_DECLINE_DELAY_MS);
    return () => window.clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [nonce, declineId]);
}
