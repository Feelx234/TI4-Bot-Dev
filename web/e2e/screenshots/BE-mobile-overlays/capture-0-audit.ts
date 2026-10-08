import { test } from "@playwright/test";
import { appendFileSync, mkdirSync } from "node:fs";
import { allCases, audit, type SizeName } from "./scene";

// The audit sweep: every dev-gallery decision on a phone, written as JSON lines to out/audit-<size>.jsonl.
// Not a screenshot: it only runs with TI4_AUDIT=1 (TI4_AUDIT_SIZE, TI4_AUDIT_ONLY="title|title").
const size = (process.env.TI4_AUDIT_SIZE ?? "390x844") as SizeName;
const only = process.env.TI4_AUDIT_ONLY?.split("|");
const out = `e2e/screenshots/BE-mobile-overlays/out/audit-${size}.jsonl`;
test.skip(!process.env.TI4_AUDIT, "audit sweep; run with TI4_AUDIT=1");
test.setTimeout(30 * 60_000);
test("audit sweep", async ({ page }) => {
  mkdirSync("e2e/screenshots/BE-mobile-overlays/out", { recursive: true });
  for (const c of allCases) {
    if (only && !only.includes(c.title)) continue;
    try {
      appendFileSync(out, JSON.stringify(await audit(page, c.title, size)) + "\n");
    } catch (error) {
      appendFileSync(out, JSON.stringify({ title: c.title, size, error: String(error).slice(0, 200) }) + "\n");
    }
  }
});
