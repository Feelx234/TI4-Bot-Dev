import type { Page } from "@playwright/test";
import type { ChoiceOption } from "../../../src/protocol/types";
import { openMockedGame } from "../_shared/mockGame";
import { playerWithHand, opponent } from "../_shared/players";
import { proposeChoice, tradePlayers, answerChoice, PARTNER } from "../AG-trade-staging-desk/_trade";

export const SIZES = { desktop: { width: 1280, height: 900 }, "390x844": { width: 390, height: 844 } } as const;
export type SizeName = keyof typeof SIZES;
/** `TI4_SHOT_PREFIX=before` names the shots taken on the commit without the fix. */
export const prefix = process.env.TI4_SHOT_PREFIX ? `${process.env.TI4_SHOT_PREFIX}-` : "after-";
export const pass = { id: "decline", label: "Pass", kind: "decline" };

/** The reaction offer the engine makes for a promissory-note ability (the label is the raw id). */
export async function openNoteReaction(page: Page, event: string, id: string) {
  await openMockedGame(page, {
    players: [playerWithHand(), opponent],
    choice: {
      prompt: `after ${event}`,
      context: {
        subtype: `reaction_after_${event}`,
        optional: true,
        source: { Reaction: event },
        trigger: { kind: "generic", event_type: event, event_id: 21, relation: "after", actor: opponent.id },
      },
      options: [{ id, label: `Play ${id}`, kind: "ability" }, pass] as never,
    },
  });
  await page.getByTestId("reaction-status-bar").waitFor().catch(() => page.waitForTimeout(800));
  await page.waitForTimeout(500);
}

/** Mentak commander / Visionaria: "Give X 1 promissory note from your hand", the labels are raw note ids. */
export async function openNotePick(page: Page) {
  const notes = ["antivirus:nekro", "support:jolnar", "ra:jolnar", "cf:generic"];
  await openMockedGame(page, {
    players: [playerWithHand(), opponent],
    choice: {
      prompt: "Give Mentak 1 promissory note from your hand",
      context: { subtype: "commander_give_note", optional: false },
      options: notes.map((n) => ({ id: n, label: n, kind: "give_note" })) as ChoiceOption[],
      nonce: "note-pick",
    } as never,
  });
  await page.waitForTimeout(700);
}

/** The trade desk with the Support exchange and note asks the engine lists for these two seats. */
export const supportPropose = {
  ...proposeChoice,
  options: [
    { id: "ss", label: "exchange Support for the Throne notes", kind: "offer", payload: { kind: "mutual_support", net: 2, their_net: 2 } },
    { id: "pnsupport:hacan:4", label: "sell support:hacan for 4 trade goods", kind: "offer", payload: { note: "support:hacan", price: 4, net: 0 } },
    { id: "npsupport:jolnar:3", label: "pay 3 trade goods for the note support:jolnar", kind: "offer", payload: { received_promissory: "support:jolnar", net: 0 } },
    ...proposeChoice.options.filter((o) => o.id === "decline"),
  ] as ChoiceOption[],
};

export const supportAnswer = {
  ...answerChoice,
  prompt: "Jolnar gives support:jolnar for support:hacan -- accept?",
};
export { tradePlayers, PARTNER };
