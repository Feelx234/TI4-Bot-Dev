import { i48sDeclineOption } from "../presentation/i48sOffer.ts";
import { readAskI48s } from "./useI48sSetting.ts";
import { OFFER_DECLINE_DELAY_MS, useOfferAutoDecline, type OfferAutoDeclineInput } from "./useOfferAutoDecline.ts";

/** Pause before answering, so the board paints and a double render settles. */
export const I48S_DECLINE_DELAY_MS = OFFER_DECLINE_DELAY_MS;

export type I48sAutoDeclineInput = OfferAutoDeclineInput;

const SPEC = {
  declineOption: i48sDeclineOption,
  readAsk: readAskI48s,
  noticePrefix: "i48s",
  noticeText: "I48S skipped (switched off)",
  delayMs: I48S_DECLINE_DELAY_MS,
};

/** With the "ask about I48S" switch off, declines the L1Z1X agent's offer for the viewer's own seat. */
export function useI48sAutoDecline(input: I48sAutoDeclineInput) {
  useOfferAutoDecline(input, SPEC);
}
