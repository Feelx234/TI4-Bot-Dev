import { expect, test, type Page } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { playerWithHand, opponent, actor } from "../_shared/players";
import { galleryBoard } from "../_shared/fixtures";
import { shot } from "../_shared/shot";

// Swiping away an agenda vote-cast toast ("Vote" toasts) used to leave an invisible box behind: the
// toast animated out, but the dismiss handler never removed it from the vote-cast list, so the
// button stayed in the DOM (opacity 0, pointer-events auto) and swallowed taps meant for what lies
// below. The prefix is "before" when the fix is stashed: the shot then marks the leftover element.
const prefix = process.env.TI4_SHOT_PREFIX ?? "after";

const SIZES = [
  { name: "phone", width: 390, height: 844, touch: true },
  { name: "desktop", width: 1280, height: 720, touch: false },
];

const casts = [
  { player: "other_seat", outcome: "against", votes: 4 },
  { player: actor, outcome: "for", votes: 7 },
];

async function swipe(page: Page, touch: boolean, x0: number, y: number, dx: number) {
  if (touch) {
    const cdp = await page.context().newCDPSession(page);
    const send = (type: string, x: number) =>
      cdp.send("Input.dispatchTouchEvent", { type, touchPoints: type === "touchEnd" ? [] : [{ x, y }] });
    await send("touchStart", x0);
    for (let x = x0; x <= x0 + dx; x += 20) {
      await send("touchMove", x);
      await page.waitForTimeout(16);
    }
    await send("touchEnd", x0 + dx);
  } else {
    await page.mouse.move(x0, y);
    await page.mouse.down();
    await page.mouse.move(x0 + dx / 2, y, { steps: 5 });
    await page.mouse.move(x0 + dx, y, { steps: 5 });
    await page.mouse.up();
  }
}

for (const size of SIZES) {
  test(`swiped vote toast leaves nothing behind, ${size.name}`, async ({ browser }, testInfo) => {
    const context = await browser.newContext({
      viewport: { width: size.width, height: size.height },
      hasTouch: size.touch,
      isMobile: size.touch,
      // A phone with reduced motion on: the toast does not slide out, it only fades.
      reducedMotion: size.touch ? "reduce" : "no-preference",
    });
    const page = await context.newPage();
    const game = await openMockedGame(page, {
      board: galleryBoard,
      players: [playerWithHand({}), opponent],
      phase: "agenda",
      round: 3,
      state: { agenda_casts: [] },
    });
    // The prepare-secondary chip would make the stack clip (and hide) what sits beside it; the leftover
    // is plainly in the way where there is no chip.
    await page.addStyleTag({ content: ".secondary-prep__chip { display: none !important; }" });
    await page.waitForTimeout(500);
    const { events: _events, ...update } = game.snapshot;
    game.send({ ...update, type: "state_update", game_version: 41, state: { agenda_casts: casts } });
    const toasts = page.getByTestId("corner-toast");
    await expect(toasts).toHaveCount(2);
    await page.waitForTimeout(500);
    const boxes = [(await toasts.nth(0).boundingBox())!, (await toasts.nth(1).boundingBox())!];
    // Hit-tests the middle of every toast still in the DOM that shows on screen (whatever its opacity):
    // is the page's topmost element there a toast? A leftover one is marked in red.
    const hit = () =>
      page.evaluate(() => {
        let onToast = false;
        for (const t of document.querySelectorAll<HTMLElement>("[data-testid=corner-toast].dismissing")) {
          const r = t.getBoundingClientRect();
          const x = (Math.max(r.left, 0) + Math.min(r.right, window.innerWidth)) / 2;
          const y = r.top + r.height / 2;
          if (x <= 0 || x >= window.innerWidth || Math.min(r.right, window.innerWidth) - Math.max(r.left, 0) < 4) continue;
          if (document.elementFromPoint(x, y)?.closest("[data-testid=corner-toast]") !== t) continue;
          onToast = true;
          t.style.outline = "3px dashed #ff2d55";
          t.style.background = "rgba(255,45,85,0.35)";
          t.style.setProperty("opacity", "1", "important");
        }
        return { onToast };
      });

    await swipe(page, size.touch, boxes[0].x + 40, boxes[0].y + 10, 120);
    await page.waitForTimeout(800);
    const first = await hit();
    const live = page.locator("[data-testid=corner-toast]:not(.dismissing)");
    if (await live.count()) {
      const b = (await live.first().boundingBox())!;
      await swipe(page, size.touch, b.x + 40, b.y + 10, 120);
    }
    await page.waitForTimeout(800);
    const remaining = await toasts.count();
    const last = await hit();
    console.log(`swipe ${prefix} ${size.name}: toasts left ${remaining}, hit-test ${JSON.stringify(last)} (after the first swipe ${JSON.stringify(first)})`);
    if (prefix === "after") {
      expect(remaining).toBe(0);
      expect(last.onToast).toBe(false);
      expect(first.onToast).toBe(false);
    }
    await shot(page, testInfo, `${prefix}-1-swiped-${size.name}`);
    await context.close();
  });
}
