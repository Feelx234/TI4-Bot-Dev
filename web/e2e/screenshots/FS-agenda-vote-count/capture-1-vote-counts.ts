import { expect, test } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { playerWithHand, opponent, actor } from "../_shared/players";
import { galleryBoard } from "../_shared/fixtures";
import { shot } from "../_shared/shot";

const SIZES = { desktop: { width: 1440, height: 900 }, "390x844": { width: 390, height: 844 } } as const;

type Cast = { player: string; outcome: string | null; votes: number };
const casts = (list: Cast[]) => ({ agenda_casts: list });

// The engine's public `agenda_casts` (display only, written as each seat's votes are counted):
// the outcome and the total from every source. Each case sends the casts one state update at a
// time, as the server does while the table votes in speaker order.
const CASES: Record<string, { steps: Cast[][]; expect: string[]; history: string[] }> = {
  "own-vote": {
    steps: [[{ player: actor, outcome: "for", votes: 7 }]],
    expect: ["You voted For: 7 votes"],
    history: ["You voted For: 7 votes"],
  },
  "other-vote": {
    steps: [[{ player: "other_seat", outcome: "against", votes: 4 }]],
    expect: ["voted Against: 4 votes"],
    history: ["voted Against: 4 votes"],
  },
  abstain: {
    steps: [[{ player: "other_seat", outcome: null, votes: 0 }]],
    expect: ["abstained"],
    history: ["abstained"],
  },
  // 21 = planets + trade goods + Predictive Intelligence's 3 + a commander bonus, as one total;
  // the second cast also shows what the vote stands at.
  "large-vote": {
    steps: [
      [{ player: "other_seat", outcome: "against", votes: 9 }],
      [
        { player: "other_seat", outcome: "against", votes: 9 },
        { player: actor, outcome: "for", votes: 21 },
      ],
    ],
    expect: ["voted Against: 9 votes", "You voted For: 21 votes", "Running total: For 21 · Against 9"],
    history: ["voted Against: 9 votes", "You voted For: 21 votes"],
  },
};

for (const [size, viewport] of Object.entries(SIZES)) {
  for (const [name, spec] of Object.entries(CASES)) {
    test(`agenda vote count ${name} ${size}`, async ({ page }, testInfo) => {
      await page.setViewportSize(viewport);
      const game = await openMockedGame(page, {
        board: galleryBoard,
        players: [playerWithHand({}), opponent],
        phase: "agenda",
        round: 3,
        state: casts([]),
      });
      await page.waitForTimeout(500);
      const { events: _events, ...update } = game.snapshot;
      let version = 41;
      for (const step of spec.steps) {
        game.send({ ...update, type: "state_update", game_version: version++, state: casts(step) });
        await page.waitForTimeout(300);
      }
      const toasts = page.locator('[data-testid="corner-toast"][data-toast-kind="agenda"]');
      await toasts.first().waitFor();
      const all = await toasts.allInnerTexts();
      for (const part of spec.expect) expect(all.join("\n")).toContain(part);
      // One toast per cast, never repeated by the identical state arriving again.
      expect(await toasts.count()).toBe(spec.steps[spec.steps.length - 1].length);
      game.send({ ...update, type: "state_update", game_version: version++, state: casts(spec.steps[spec.steps.length - 1]) });
      await page.waitForTimeout(300);
      expect(await toasts.count()).toBe(spec.steps[spec.steps.length - 1].length);
      await page.waitForTimeout(700);
      for (const t of await toasts.all()) {
        const box = (await t.boundingBox())!;
        expect(box.x + box.width).toBeLessThanOrEqual(viewport.width);
      }
      await shot(page, testInfo, `${name}-${size}`);

      // The toast history keeps the same wording.
      const desktopToggle = page.getByTestId("toast-history-toggle");
      await (size === "desktop" ? desktopToggle : page.getByTestId("toast-history-mobile-toggle")).click();
      const items = page.getByTestId("toast-history-item");
      await expect(items).toHaveCount(spec.history.length);
      const listed = (await items.allInnerTexts()).join("\n");
      for (const part of spec.history) expect(listed).toContain(part);
      if (name === "large-vote") await shot(page, testInfo, `history-${size}`);
    });
  }
}
