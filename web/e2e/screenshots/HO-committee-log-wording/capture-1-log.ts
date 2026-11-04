import { expect, test, type Page } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { opponent, playerWithHand } from "../_shared/players";
import { actor, galleryBoard } from "../_shared/fixtures";
import { shot } from "../_shared/shot";
import type { GameEvent } from "../../../src/protocol/types";

// The agenda phase of alpha game off_0b396f871a32 (feedback fb_1792737447): the table voted on
// Committee Formation, then the Xxcha quashed the second agenda and the Committee Formation holder
// was asked whether to discard it. The host logs both as "<seat> resolved <question>"; the log now
// says what was asked, and a hover title spells out the Committee Formation rule.
const BEFORE = process.env.TI4_SHOT_PREFIX === "before";
const SIZES = { desktop: { width: 1440, height: 900 }, "390x844": { width: 390, height: 844 } } as const;
const rival = opponent.id;
// The host writes seat ids and the log swaps them for the participant; the mock writes the names.
const ME = "Anna";
const RIVAL = "Bob";

const entry = (n: number, stage: string, who: string, detail: string): GameEvent =>
  ({
    id: `agenda-${n}`,
    timestamp: `10:${10 + n}`,
    version: n,
    visibility: { visibility: "public" as const },
    event: { kind: "decision_resolved" },
    round: 1,
    phase: "agenda",
    action_id: "agenda_1",
    action_type: "pass",
    actor: who,
    stage,
    decision_count: n,
    detail,
  }) as GameEvent;

const events: GameEvent[] = [
  { id: "start", timestamp: "10:00", visibility: { visibility: "public" as const }, event: { kind: "game_initialized", round: 1, phase: "agenda", speaker: actor } },
  entry(1, "voting", actor, `${ME} voted for ${RIVAL}`),
  entry(2, "voting", rival, `${RIVAL} voted for ${RIVAL}`),
  entry(3, "effects", rival, `${RIVAL} resolved quash`),
  entry(4, "effects", rival, `${RIVAL} resolved offer discard law`),
  entry(5, "effects", actor, `${ME} resolved committee elect`),
];

async function openLog(page: Page, size: keyof typeof SIZES) {
  await page.setViewportSize(SIZES[size]);
  await openMockedGame(page, {
    phase: "agenda",
    board: galleryBoard,
    players: [playerWithHand(), opponent],
    events,
    nicknames: { [actor]: ME, [rival]: RIVAL },
    history: { cursor: 5, redo_count: 0 },
  });
  await page.getByTestId("ti4-board-svg").waitFor();
  if (size === "desktop") await page.getByTestId("event-log-toggle").click();
  else await page.getByRole("button", { name: "Events", exact: true }).click();
  const list = page.getByTestId("event-log-list");
  await list.waitFor();
  for (let i = 0; i < 6; i++) {
    const closed = list.locator('button[aria-expanded="false"]');
    if (!(await closed.count())) break;
    await closed.first().dispatchEvent("click"); // phone: the turn bar overlaps the drawer
  }
  await expect(page.getByTestId("event-log-entry").first()).toBeVisible();
  // Phone: the dock buttons sit over the bottom of the drawer; the shot hides them to show every line.
  if (size !== "desktop") await page.addStyleTag({ content: ".app-shell__mobile-actions{display:none!important}" });
}

for (const size of Object.keys(SIZES) as (keyof typeof SIZES)[]) {
  test(`committee formation log ${size}`, async ({ page }, testInfo) => {
    await openLog(page, size);
    if (!BEFORE) {
      const lines = page.getByTestId("event-log-entry");
      const discard = lines.filter({ hasText: "was asked whether to discard a law" });
      await expect(discard).toHaveCount(1);
      await expect(discard).toContainText("(Committee Formation or a ministry card)");
      await expect(page.getByTestId("event-log-list")).not.toContainText("resolved offer discard law");
      await expect(page.getByTestId("event-log-list")).not.toContainText("resolved quash");
      await expect(lines.filter({ hasText: "was asked whether to use Quash on the agenda" })).toHaveCount(1);
      await expect(lines.filter({ hasText: "chose the elected player with Committee Formation (nobody votes on that agenda)" })).toHaveCount(1);
      // The rule behind the line is its hover title; plain votes have none.
      await expect(discard.locator(".event-log__body")).toHaveAttribute("title", /nobody votes on that agenda/);
      await expect(lines.filter({ hasText: "voted for" }).first().locator(".event-log__body")).not.toHaveAttribute("title");
    }
    await page.getByTestId("event-log-list").evaluate((el) => { el.scrollTop = el.scrollHeight; });
    await shot(page, testInfo, `${BEFORE ? "before-" : ""}1-log-${size}`, { of: page.locator("#event-log-drawer"), pad: 8 });
  });
}
