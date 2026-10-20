import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openPublicGame, type SizeName } from "../CX-public-playtest/scene";
import { SIZES } from "../CX-public-playtest/scene";

type Mode = "ok" | "refuse";

for (const size of ["desktop", "phone"] as SizeName[]) {
  const label = size === "phone" ? "390x844" : "desktop";
  test(`feedback outbox ${label}`, async ({ page, context }, testInfo) => {
    await page.setViewportSize(SIZES[size]);
    let mode: Mode = "ok";
    const received: Array<Record<string, any>> = [];
    await page.route("**/api/feedback", async (route) => {
      const body = route.request().postDataJSON() as Record<string, any>;
      received.push(body);
      if (mode === "refuse") return route.fulfill({ status: 400, json: { error: "invalid_feedback", message: "refused" } });
      return route.fulfill({ json: { id: "fb_1760268000_9f3a1c2e", duplicate: false } });
    });
    await openPublicGame(page, size);

    // 1. The connection drops (a tunnel). The sheet says so before anything is typed.
    await context.setOffline(true);
    await page.getByTestId("feedback-button").click();
    await expect(page.getByTestId("feedback-offline-note")).toBeVisible();
    await page.getByTestId("feedback-category-bug").click();
    await page
      .getByTestId("feedback-text")
      .fill("The Pass button stayed grey after I activated the second system. Writing this in the train.");
    await expect(page.getByTestId("feedback-submit")).toHaveText("Save to send later");
    await shot(page, testInfo, `1-offline-note-${label}`);

    // 2. Submit while offline: saved on the device, a pending chip, nothing sent.
    await page.getByTestId("feedback-submit").click();
    await expect(page.getByTestId("feedback-sent")).toContainText("Saved on this device, will be sent when you are back online");
    await expect(page.getByTestId("feedback-pending-chip")).toHaveText("1 waiting to be sent");
    expect(received).toHaveLength(0);
    await shot(page, testInfo, `2-saved-offline-${label}`);

    // 3. Closed and reopened: the chip and the history are still there.
    await page.getByTestId("feedback-done").click();
    await page.getByTestId("feedback-button").click();
    await expect(page.getByTestId("feedback-pending-chip")).toHaveText("1 waiting to be sent");
    await expect(page.getByTestId("feedback-history")).toContainText("Waiting to be sent");
    await shot(page, testInfo, `3-pending-chip-${label}`);

    // 4. Back online, but the server refuses for good: Not sent, with Send now.
    mode = "refuse";
    await context.setOffline(false);
    await expect(page.getByTestId("feedback-pending-chip")).toHaveText("1 not sent");
    await expect(page.getByTestId("feedback-send-now")).toBeVisible();
    expect((await page.getByTestId("feedback-send-now").boundingBox())!.height).toBeGreaterThanOrEqual(44);
    await shot(page, testInfo, `4-failed-send-now-${label}`);

    // 5. The server accepts again: Send now delivers it once, with the write time.
    mode = "ok";
    await page.getByTestId("feedback-send-now").click();
    await expect(page.getByTestId("feedback-pending-chip")).toHaveText("All sent");
    await expect(page.getByTestId("feedback-history")).toContainText("Sent");
    const last = received[received.length - 1]!;
    expect(last.report_id).toMatch(/^r_[0-9a-f]{24}$/);
    expect(new Set(received.map((r) => r.report_id)).size).toBe(1);
    expect(typeof last.client_queued_at).toBe("string");
    expect(last.context.delivery).toMatchObject({ via: "outbox" });
    await shot(page, testInfo, `5-retry-succeeded-${label}`);
    const { width } = page.viewportSize()!;
    const box = (await page.getByTestId("feedback-sheet").boundingBox())!;
    expect(box.x + box.width).toBeLessThanOrEqual(width + 1);
  });
}
