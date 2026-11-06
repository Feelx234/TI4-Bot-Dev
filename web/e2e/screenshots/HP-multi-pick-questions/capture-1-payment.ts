import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { assertNoHorizontalScroll, newPage, openPayment, stagePayment } from "./scene";

// A payment is ONE multi-pick question: the drawer stages the whole set and checks it as you type
// with the engine's rule (covers the bill, and without the largest item the rest is short).
const states = [
  { name: "open", planets: [], goods: 0 },
  // Quann 2 + 1 good = 3 of 5.
  { name: "short", planets: ["quann"], goods: 1 },
  // Jord 2 + Lodor 3 + Quann 2 + 1 good = 8 of 5: without Lodor (the largest), 5 still covers it.
  { name: "more-than-needed", planets: ["jord", "lodor", "quann"], goods: 1 },
  // Jord 2 + Quann 2 + 1 good = 5: legal, one answer.
  { name: "legal", planets: ["jord", "quann"], goods: 1 },
] as const;

for (const size of ["desktop", "390x844"] as const) {
  for (const state of states) {
    if (size === "390x844" && (state.name === "open" || state.name === "short")) continue;
    test(`payment ${state.name} ${size}`, async ({ browser, baseURL }, testInfo) => {
      const { context, page } = await newPage(browser, size, baseURL);
      const sent: string[] = [];
      await openPayment(page, (id) => sent.push(id));
      await stagePayment(page, [...state.planets], state.goods);
      const confirm = page.getByTestId("confirm-payment-btn");
      if (state.name === "legal") await expect(confirm).toBeEnabled();
      else await expect(confirm).toBeDisabled();
      if (state.name === "short") await expect(page.getByTestId("payment-problem")).toContainText("Short by 2");
      if (state.name === "more-than-needed") {
        await expect(page.getByTestId("payment-problem")).toContainText("More than needed");
        await expect(page.getByTestId("payment-problem")).toContainText("without Lodor");
      }
      if (size === "390x844") await assertNoHorizontalScroll(page);
      await shot(page, testInfo, `1-payment-${state.name}-${size}`);
      if (state.name === "legal") {
        await confirm.click();
        await expect.poll(() => sent).toEqual(["exhaust|jord+exhaust|quann+trade_good"]);
      }
      await context.close();
    });
  }
}
