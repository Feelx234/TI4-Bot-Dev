import { test } from "@playwright/test";
import { extras, openExtra } from "./extras";
import { allCases, minimize, openCase, type SizeName } from "./scene";
import { measureMap } from "./measure";

// Scratch: screenshots of chosen extras (TI4_LOOK=id,id) and gallery cases (TI4_LOOK_CASES="title|title")
// into $TI4_LOOK_DIR. Runs only when one of them is set.
const size = (process.env.TI4_AUDIT_SIZE ?? "390x844") as SizeName;
const ids = (process.env.TI4_LOOK ?? "").split(",").filter(Boolean);
const cases = (process.env.TI4_LOOK_CASES ?? "").split(";").filter(Boolean);
test.skip(ids.length === 0 && cases.length === 0, "scratch");
test.setTimeout(5 * 60_000);
test("look", async ({ page }) => {
  for (const e of extras.filter((x) => ids.includes(x.id))) {
    try {
      await openExtra(page, e.id, size);
      await page.screenshot({ path: `${process.env.TI4_LOOK_DIR}/${e.id}-${size}.png` });
      const m = await measureMap(page);
      if (await minimize(page, m.minimizeControl))
        await page.screenshot({ path: `${process.env.TI4_LOOK_DIR}/${e.id}-${size}-min.png` });
    } catch (error) {
      console.log("LOOK-ERR", e.id, String(error).slice(0, 200));
      await page.screenshot({ path: `${process.env.TI4_LOOK_DIR}/${e.id}-${size}-err.png` });
    }
  }
  for (const title of cases) {
    try {
      await openCase(page, allCases.find((c) => c.title.startsWith(title))!.title, size);
      const slug = title.replace(/\W+/g, "-");
      await page.screenshot({ path: `${process.env.TI4_LOOK_DIR}/c-${slug}-${size}.png` });
      const m = await measureMap(page);
      if (await minimize(page, m.minimizeControl))
        await page.screenshot({ path: `${process.env.TI4_LOOK_DIR}/c-${slug}-${size}-min.png` });
    } catch (error) {
      console.log("LOOK-ERR", title, String(error).slice(0, 200));
    }
  }
});
