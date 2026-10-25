import { expect, test } from "@playwright/test";
import {
  alongTheTop,
  another,
  aroundWithDrive,
  open,
  take,
  throughRift,
  viaWormhole,
  viewports,
} from "./scene";

// 1. One way only: no selector. 2. Two ways, the default through a rift and the safe one for
// Gravity Drive, then the safe one chosen (the map follows). 3. Five ways, the cap.
for (const v of viewports) {
  test(`one-route-no-selector-${v.id}`, async ({ page }, testInfo) => {
    await open(page, [throughRift], v);
    await expect(page.getByTestId("route-picker-24-cruiser")).toHaveCount(0);
    await take(page, testInfo, "1-single-route", v);
  });

  test(`two-routes-${v.id}`, async ({ page }, testInfo) => {
    await open(page, [throughRift, aroundWithDrive], v);
    await expect(page.getByTestId("route-24-cruiser-0")).toHaveAttribute("aria-checked", "true");
    await page.getByTestId("route-picker-24-cruiser").scrollIntoViewIfNeeded();
    await take(page, testInfo, "2-default-selected", v);
    await page.getByTestId("route-24-cruiser-1").click();
    await expect(page.getByTestId("route-24-cruiser-1")).toHaveAttribute("aria-checked", "true");
    await take(page, testInfo, "3-safe-route-selected", v);
    // Minimised, the tray gives the map back: the route of the staged ship is drawn on it.
    await page.getByTestId("close-movement-tray").click();
    await expect(page.getByTestId("route-preview")).toHaveCount(1);
    await take(page, testInfo, "5-map-safe-route", v);
  });

  test(`map-shows-the-default-${v.id}`, async ({ page }, testInfo) => {
    await open(page, [throughRift, aroundWithDrive], v);
    await page.getByTestId("close-movement-tray").click();
    await expect(page.getByTestId("route-preview")).toHaveAttribute("data-risky", "true");
    await take(page, testInfo, "6-map-default-route", v);
  });

  test(`many-routes-capped-${v.id}`, async ({ page }, testInfo) => {
    await open(page, [throughRift, aroundWithDrive, alongTheTop, viaWormhole, another], v);
    await expect(page.getByRole("radio")).toHaveCount(5);
    await page.getByTestId("route-picker-24-cruiser").scrollIntoViewIfNeeded();
    await take(page, testInfo, "4-five-routes", v);
  });
}
