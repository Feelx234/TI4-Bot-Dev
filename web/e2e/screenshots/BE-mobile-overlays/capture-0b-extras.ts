import { test } from "@playwright/test";
import { appendFileSync, mkdirSync } from "node:fs";
import { extras, openExtra } from "./extras";
import { minimize, type SizeName } from "./scene";
import { measureMap } from "./measure";

// Audit sweep for the overlays that are not gallery decisions. Runs only with TI4_AUDIT=1.
const size = (process.env.TI4_AUDIT_SIZE ?? "390x844") as SizeName;
const out = `e2e/screenshots/BE-mobile-overlays/out/extras-${size}.jsonl`;
test.skip(!process.env.TI4_AUDIT, "audit sweep; run with TI4_AUDIT=1");
test.setTimeout(10 * 60_000);
test("extras sweep", async ({ page }) => {
  mkdirSync("e2e/screenshots/BE-mobile-overlays/out", { recursive: true });
  for (const e of extras) {
    try {
      await openExtra(page, e.id, size);
      const open = await measureMap(page);
      const row: Record<string, unknown> = { title: e.title, id: e.id, size, open };
      if (open.covered > 0.3 && (await minimize(page, open.minimizeControl))) row.minimized = await measureMap(page);
      appendFileSync(out, JSON.stringify(row) + "\n");
    } catch (error) {
      appendFileSync(out, JSON.stringify({ title: e.title, id: e.id, size, error: String(error).slice(0, 300) }) + "\n");
    }
  }
});
