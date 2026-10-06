import { test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openMockedGame } from "../_shared/mockGame";
import { playerWithHand, opponent } from "../_shared/players";

// Paying: planets to exhaust are chosen on the map and appear in the payment drawer, which shows
// what they are worth against what is owed.
test("planet selection: exhaust planets chosen on the map", async ({ page }, testInfo) => {
  await openMockedGame(page, {
    players: [playerWithHand(), opponent],
    choice: {
      prompt: "pay 5 resources to produce",
      context: { subtype: "pay_resources", outstanding: [{ kind: "resources", amount: 5, paid: 0 }] },
      options: [
        { id: "exhaust|jord", label: "Exhaust Jord", kind: "pay", payload: { worth: 4, owed: 5, kind: "resources", planet: "jord", planet_name: "Jord" } },
        { id: "exhaust|lodor", label: "Exhaust Lodor", kind: "pay", payload: { worth: 3, owed: 5, kind: "resources", planet: "lodor", planet_name: "Lodor" } },
        { id: "exhaust|quann", label: "Exhaust Quann", kind: "pay", payload: { worth: 1, owed: 5, kind: "resources", planet: "quann", planet_name: "Quann" } },
        { id: "trade_good", label: "Spend one trade good", kind: "pay", payload: { worth: 1 } },
        { id: "decline", label: "Done", kind: "decline" },
      ],
    },
  });
  const drawer = page.getByRole("dialog");
  await drawer.waitFor();
  await shot(page, testInfo, "5a-payment-drawer");
  // Minimise the drawer to reach the map, then pick the planets to exhaust there.
  await drawer.getByRole("button", { name: /minimi[sz]e/i }).click();
  await page.getByTestId("resume-decision-btn").waitFor();
  await shot(page, testInfo, "5b-pick-on-map");
  await page.getByTestId("planet-jord").click();
  await shot(page, testInfo, "5c-picked-on-map");
  await page.getByTestId("resume-decision-btn").click();
  await drawer.waitFor();
  await shot(page, testInfo, "5d-staged-in-drawer");
});
