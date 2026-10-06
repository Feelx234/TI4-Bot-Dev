import { test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openMockedGame } from "../_shared/mockGame";

test("explore", async ({ page }, testInfo) => {
  await openMockedGame(page, {
    choice: {
      prompt: "Choose a system to activate",
      context: { subtype: "activate_system" },
      options: [
        { id: "24", label: "Activate Mehar Xull (#24)", kind: "activate", payload: { system: "24" } },
        { id: "26", label: "Activate Lodor (#26)", kind: "activate", payload: { system: "26" } },
      ],
    },
  });
  await page.waitForTimeout(1500);
  await shot(page, testInfo, "mocked-system-activation");
});
