import { test } from "@playwright/test";
import { open, scenes, take, viewports } from "./scene";

// Every retreat prompt and Naalu's Foresight, desktop and phone.
for (const [name, make] of Object.entries(scenes)) {
  for (const v of viewports) {
    test(`${name}-${v.id}`, async ({ page }, testInfo) => {
      const [board, choice] = make();
      await open(page, v, board, choice);
      await take(page, testInfo, name, v);
    });
  }
}
