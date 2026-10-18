import { aetherpassageDeclineOption } from "../presentation/aetherpassageOffer.ts";
import { readAskAetherpassage } from "./useAetherpassageSetting.ts";
import { OFFER_DECLINE_DELAY_MS, useOfferAutoDecline, type OfferAutoDeclineInput } from "./useOfferAutoDecline.ts";

export const AETHERPASSAGE_DECLINE_DELAY_MS = OFFER_DECLINE_DELAY_MS;

const SPEC = {
  declineOption: aetherpassageDeclineOption,
  readAsk: readAskAetherpassage,
  noticePrefix: "aetherpassage",
  noticeText: "Aetherpassage skipped (switched off)",
  delayMs: AETHERPASSAGE_DECLINE_DELAY_MS,
};

/** With the Aetherpassage switch off, declines the Empyrean offer for the viewer's own seat. */
export function useAetherpassageAutoDecline(input: OfferAutoDeclineInput) {
  useOfferAutoDecline(input, SPEC);
}
