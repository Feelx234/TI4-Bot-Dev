import type { Page } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { playerWithHand, opponent } from "../_shared/players";
import { galleryBoard } from "../_shared/fixtures";
import { paymentBoard } from "../D-planet-selection/payment";

/** Which side of the change a run is: `before` is taken on the commit without the change. */
export const prefix = process.env.TI4_SHOT_PREFIX ?? "after";

export const SIZES = {
  desktop: { width: 1440, height: 900 },
  "390x844": { width: 390, height: 844 },
} as const;
export type SizeName = keyof typeof SIZES;

const SOURCE = { ActionCard: "plagiarize" };

/** What the neighbour owns that Plagiarize may take (non-faction); everything else is not offered. */
export const OFFERED = ["gd", "hm", "sr", "dd2"];

/** Plagiarize asks for the 5 influence first, exactly as every influence payment is asked. */
export async function openPayment(page: Page, size: SizeName) {
  await page.setViewportSize(SIZES[size]);
  const planets = [["jord", "Jord", 2], ["lodor", "Lodor", 3], ["quann", "Quann", 1], ["arinam", "Arinam", 2]] as const;
  await openMockedGame(page, {
    board: paymentBoard,
    players: [playerWithHand({ technologies: ["amd"], trade_goods: 1 }), opponent],
    choice: {
      prompt: "pay 5 influence",
      context: { subtype: "pay_influence", source: SOURCE, outstanding: [{ kind: "influence", amount: 5, paid: 0 }] },
      options: [
        ...planets.map(([id, name, worth]) => ({
          id: `exhaust|${id}`,
          label: `Exhaust ${name} for influence`,
          kind: "pay",
          payload: { worth, owed: 5, kind: "influence", planet: id, planet_name: name },
        })),
        { id: "trade_good", label: "Spend one trade good", kind: "pay", payload: { worth: 1 } },
        { id: "decline", label: "Done", kind: "decline" },
      ],
    },
  });
  await page.getByTestId("payment-drawer").waitFor();
}

/** Then it asks which of the neighbour's technologies to take: ids are `<neighbour>|<technology>`. */
export async function openPick(page: Page, size: SizeName) {
  await page.setViewportSize(SIZES[size]);
  await openMockedGame(page, {
    board: { ...galleryBoard, systems: {} },
    players: [playerWithHand({ technologies: ["amd"], trade_goods: 2 }), opponent],
    choice: {
      prompt: "Plagiarize: which technology to steal",
      context: { subtype: "plagiarize_pick_technology", source: SOURCE },
      options: OFFERED.map((id) => ({ id: `${opponent.id}|${id}`, label: `${opponent.id}'s ${id}`, kind: "technology" })),
    },
  });
  await page.waitForTimeout(900);
}
