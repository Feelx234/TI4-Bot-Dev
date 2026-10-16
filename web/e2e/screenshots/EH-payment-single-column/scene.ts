import { expect, type Page } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { playerWithHand, opponent } from "../_shared/players";
import { paymentBoard } from "../D-planet-selection/payment";

/** `before` shots come from the commit before the change (the layout assertions are skipped). */
export const prefix = process.env.TI4_SHOT_PREFIX ?? "after";

export { newPage, SIZES, isPhone } from "../BS-production-phone/scene";

const NAMES = ["Jord", "Lodor", "Quann", "Mecatol Rex", "Arinam", "Meer", "Abyz", "Fria", "Thibah", "Tarmann"];

function planetOptions(kind: "resources" | "influence", owed: number, count: number) {
  return NAMES.slice(0, count).map((name, i) => {
    const id = name.toLowerCase().replace(/ /g, "_");
    return {
      id: `exhaust|${id}`,
      label: `Exhaust ${name} for ${kind}`,
      kind: "pay",
      payload: { worth: (i % 3) + 1, owed, kind, planet: id, planet_name: name },
    };
  });
}

export function paymentChoiceOf(kind: "resources" | "influence", owed: number, planets: number) {
  return {
    prompt: `pay ${owed} ${kind}`,
    context: { subtype: kind === "resources" ? "pay_resources" : "pay_influence", outstanding: [{ kind, amount: owed, paid: 0 }] },
    options: [
      ...planetOptions(kind, owed, planets),
      { id: "trade_good", label: "Spend one trade good", kind: "pay", payload: { worth: 1 } },
      { id: "decline", label: "Done", kind: "decline" },
    ],
  };
}

export async function openPaymentOf(
  page: Page,
  kind: "resources" | "influence",
  owed: number,
  planets: number,
) {
  await openMockedGame(page, { players: [playerWithHand(), opponent], board: paymentBoard, choice: paymentChoiceOf(kind, owed, planets) });
  await page.getByTestId("payment-drawer").waitFor();
}

/** No horizontal overflow: the document and everything in the payment panel fit the viewport. */
export async function assertNoHorizontalOverflow(page: Page) {
  const r = await page.evaluate(() => {
    const vw = document.documentElement.clientWidth;
    const bad: string[] = [];
    for (const el of Array.from(document.querySelectorAll('[data-testid="payment-drawer"], [data-testid="payment-drawer"] *'))) {
      const b = el.getBoundingClientRect();
      if (b.width > 0 && b.right > vw + 0.5) bad.push(`${(el as HTMLElement).dataset.testid ?? el.className ?? el.tagName} right=${Math.round(b.right)}`);
    }
    return { vw, scrollWidth: document.documentElement.scrollWidth, bad: bad.slice(0, 5) };
  });
  expect(r.scrollWidth, "document is not wider than the viewport").toBeLessThanOrEqual(r.vw);
  expect(r.bad, "no payment element reaches past the right edge").toEqual([]);
}

/** Summary, planet list and footer stack in one column: same left edge, planets below the summary. */
export async function assertSingleColumn(page: Page) {
  const r = await page.evaluate(() => {
    const q = (id: string) => document.querySelector(`[data-testid="${id}"]`)!.getBoundingClientRect();
    const tally = q("payment-tally-card");
    const card = document.querySelector('[data-testid^="planet-card-"]')!.getBoundingClientRect();
    return { tally: [tally.left, tally.bottom], card: [card.left, card.top] };
  });
  expect(Math.abs(r.tally[0] - r.card[0]), "tally and planets share a left edge").toBeLessThan(2);
  expect(r.card[1], "planets sit below the summary").toBeGreaterThanOrEqual(r.tally[1] - 1);
}

/** The confirm button is fully inside the viewport without scrolling. */
export async function assertConfirmReachable(page: Page) {
  const ok = await page.getByTestId("confirm-payment-btn").evaluate((el) => {
    const b = el.getBoundingClientRect();
    return b.left >= 0 && b.right <= window.innerWidth + 0.5 && b.top >= 0 && b.bottom <= window.innerHeight + 0.5;
  });
  expect(ok, "confirm button fully visible").toBe(true);
}
