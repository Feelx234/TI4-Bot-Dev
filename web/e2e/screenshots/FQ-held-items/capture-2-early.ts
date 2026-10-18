import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { me, withPlayer } from "../_shared/players";
import { noHorizontalScroll, openHeldItems, openScene, rival, SIZES, type SizeName } from "./scene";

// An early seat: nothing gained yet. Only its leaders (locked) and faction abilities are listed.
const early = withPlayer(me, { technologies: [], exhausted_technologies: [], relics: [], leaders: { solagent: "readied", solcommander: "locked", solhero: "locked" } });
for (const size of Object.keys(SIZES) as SizeName[]) {
  test(`early seat ${size}`, async ({ page }, testInfo) => {
    await openScene(page, size, { players: [early, rival] });
    await openHeldItems(page, size);
    await expect(page.getByTestId("held-item-relic-heartofixth")).toHaveCount(0);
    await expect(page.getByTestId("held-items-count").first()).toHaveText("1");
    expect(await noHorizontalScroll(page)).toBeLessThanOrEqual(0);
    await shot(page, testInfo, `1-early-${size}`);
  });
}
