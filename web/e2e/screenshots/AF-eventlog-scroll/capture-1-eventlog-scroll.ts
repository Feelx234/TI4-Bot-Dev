/**
 * Reproduction (audit item L7): "the event log scrolls oddly on new events". REPRODUCE ONLY, no fix.
 *
 * Runs against the mocked game (no backend), so it works with the screenshots config:
 *   cd web && npx playwright test -c e2e/screenshots/playwright.config.ts e2e/screenshots/O-eventlog-scroll/
 * It writes the measurements to e2e/screenshots/O-eventlog-scroll/out/results.json and prints them,
 * and takes a few PNGs into out/. Scenarios whose expectation fails are the reproduced bad behaviour
 * (expect.soft, so every scenario runs and reports).
 *
 * Measured: scrollTop / scrollHeight / clientHeight of [data-testid=event-log-list] before and after
 * each new log entry (the mock pushes `{type:"event"}` messages, or a new `initial_snapshot` for a
 * history-generation change).
 */
import { expect, test, type Page } from "@playwright/test";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { PROTOCOL_VERSION, type GameEvent } from "../../../src/protocol/types";
import { GAME_ID, openMockedGame, type MockedGame } from "../_shared/mockGame";
import { actor } from "../_shared/fixtures";

const OUT = join(dirname(new URL(import.meta.url).pathname), "out");
const FILE = join(OUT, "results.json");
/** Merges one scenario's measurements into out/results.json (a failing soft expect restarts the worker). */
const results = new Proxy({} as Record<string, unknown>, {
  set(_t, key: string, value) {
    mkdirSync(OUT, { recursive: true });
    const all = existsSync(FILE) ? JSON.parse(readFileSync(FILE, "utf8")) : {};
    all[key] = value;
    writeFileSync(FILE, JSON.stringify(all, null, 2));
    return true;
  },
});

const decision = (n: number, round: number, phase: string, extra: Partial<GameEvent> = {}): GameEvent =>
  ({
    id: `g-${n}`,
    timestamp: "10:05",
    visibility: "public",
    event: { kind: "decision_resolved" },
    round,
    phase,
    actor,
    decision_count: n,
    detail: `${actor} did thing number ${n}`,
    ...extra,
  }) as GameEvent;
const marker = (id: string, round: number, phase: string): GameEvent =>
  ({ id, visibility: "public", event: { kind: "phase_transition", round, phase } }) as GameEvent;

/** Round 1: 40 decisions in the action phase; round 2: 30 decisions. */
function history(): GameEvent[] {
  const out: GameEvent[] = [
    { id: "init", visibility: "public", event: { kind: "game_initialized", round: 1, phase: "action", speaker: actor } } as GameEvent,
  ];
  let n = 0;
  for (let i = 0; i < 40; i++) out.push(decision(++n, 1, "action"));
  out.push(marker("pt-2", 2, "action"));
  for (let i = 0; i < 30; i++) out.push(decision(++n, 2, "action"));
  return out;
}
let counter = 1000;
const next = (round = 2, phase = "action") => decision(++counter, round, phase);
const push = (game: MockedGame, entry: GameEvent) =>
  game.send({ type: "event", protocol_version: PROTOCOL_VERSION, game_id: GAME_ID, entry });

const list = (page: Page) => page.getByTestId("event-log-list");
const metrics = (page: Page) =>
  list(page).evaluate((el) => ({
    scrollTop: Math.round(el.scrollTop),
    scrollHeight: el.scrollHeight,
    clientHeight: el.clientHeight,
    bottomGap: Math.round(el.scrollHeight - el.clientHeight - el.scrollTop),
    entries: el.querySelectorAll('[data-testid="event-log-entry"]').length,
    // Heading text without the entry-count badge (it changes with every event).
    openHeadings: [...el.querySelectorAll('button[aria-expanded="true"]')].map((b) => [...b.childNodes].filter((n) => n.nodeType === 3).map((n) => n.textContent).join("").trim()),
    // What is on top at the middle of the list's bottom edge (the turn action bar covers it on a phone).
    hitAtBottom: (() => {
      const r = el.getBoundingClientRect();
      const hit = document.elementFromPoint(r.left + r.width / 2, r.bottom - 4);
      return hit ? `${hit.tagName.toLowerCase()}.${String(hit.className).split(" ")[0]}${el.contains(hit) ? "" : " (NOT in list)"}` : "none";
    })(),
  }));
const settle = (page: Page) => page.waitForTimeout(250);

async function open(page: Page, viewport: { width: number; height: number }, events = history()) {
  await page.setViewportSize(viewport);
  const game = await openMockedGame(page, { events, version: 40 });
  await page.getByTestId("ti4-board-svg").waitFor();
  // Server tells the client where the game currently is: opens Round 2 > Action phase.
  const { events: _e, ...update } = game.snapshot;
  // history.cursor must already cover the log: otherwise the first live event flips the cursor and
  // every old row gains an Undo button (a mock artifact: +2px per row), which would pollute the numbers.
  game.send({ ...update, type: "state_update", current_path: { round: 2, phase: "action" }, history: { cursor: 100, redo_count: 0 } });
  if (viewport.width <= 900) await page.getByRole("button", { name: "Events", exact: true }).click();
  else await page.getByTestId("event-log-toggle").click();
  await list(page).waitFor();
  await settle(page);
  return game;
}
const picture = async (page: Page, name: string) => {
  mkdirSync(OUT, { recursive: true });
  await page.screenshot({ path: join(OUT, `${name}.png`), animations: "disabled" });
};

const DESKTOP = { width: 1440, height: 900 };
const PHONE = { width: 390, height: 844 };

for (const [vpName, vp] of [["desktop", DESKTOP], ["phone", PHONE]] as const) {
  test.describe(vpName, () => {
    test(`${vpName} (a) pinned to bottom, new event`, async ({ page }) => {
      const game = await open(page, vp);
      const initial = await metrics(page); // where a freshly opened log starts
      await list(page).evaluate((el) => (el.scrollTop = el.scrollHeight));
      const before = await metrics(page);
      push(game, next());
      await settle(page);
      const after = await metrics(page);
      results[`${vpName}-a`] = { initial, before, after };
      await picture(page, `${vpName}-a-after`);
      // Expected: still pinned (gap ~ 0).
      expect.soft(after.bottomGap, "log stays pinned to the bottom").toBeLessThanOrEqual(2);
    });

    test(`${vpName} (b) scrolled to top/middle, new event`, async ({ page }) => {
      const game = await open(page, vp);
      const r: Record<string, unknown> = {};
      for (const [label, frac] of [["top", 0], ["middle", 0.5]] as const) {
        await list(page).evaluate((el, f) => (el.scrollTop = (el.scrollHeight - el.clientHeight) * f), frac);
        await settle(page);
        const before = await metrics(page);
        push(game, next());
        await settle(page);
        const after = await metrics(page);
        r[label] = { before, after };
        expect.soft(after.scrollTop, `${label}: position preserved`).toBe(before.scrollTop);
      }
      results[`${vpName}-b`] = r;
    });

    test(`${vpName} (c) expanded older node while event arrives`, async ({ page }) => {
      const game = await open(page, vp);
      // Open Round 1 (manual) and its phase, then look at where we are.
      await list(page).getByRole("button", { name: /Round 1/ }).dispatchEvent("click"); // phone: the turn bar overlaps the drawer, so no pointer click
      const phase1 = list(page).getByRole("button", { name: /Action phase/ }).first();
      if ((await phase1.getAttribute("aria-expanded")) === "false") await phase1.dispatchEvent("click");
      await settle(page);
      await list(page).evaluate((el) => (el.scrollTop = 300));
      const before = await metrics(page);
      push(game, next());
      await settle(page);
      const after = await metrics(page);
      // Then a state_update with the same current_path (the server sends one after every decision).
      const { events: _e, ...update } = game.snapshot;
      game.send({ ...update, type: "state_update", game_version: 41, current_path: { round: 2, phase: "action" } });
      await settle(page);
      const afterUpdate = await metrics(page);
      results[`${vpName}-c`] = { before, after, afterStateUpdate: afterUpdate };
      expect.soft(after.openHeadings, "expanded nodes stay open").toEqual(before.openHeadings);
      expect.soft(after.scrollTop, "no jump").toBe(before.scrollTop);
      expect.soft(afterUpdate.openHeadings).toEqual(before.openHeadings);
    });

    test(`${vpName} (d) burst of events`, async ({ page }) => {
      const game = await open(page, vp);
      const r: Record<string, unknown> = {};
      for (const [label, pinned] of [["pinned", true], ["scrolled-up", false]] as const) {
        await list(page).evaluate((el, p) => (el.scrollTop = p ? el.scrollHeight : 120), pinned);
        await settle(page);
        const before = await metrics(page);
        for (let i = 0; i < 8; i++) push(game, next());
        await settle(page);
        const after = await metrics(page);
        r[label] = { before, after };
        if (pinned) expect.soft(after.bottomGap, "burst: pinned").toBeLessThanOrEqual(2);
        else expect.soft(after.scrollTop, "burst: position preserved").toBe(before.scrollTop);
      }
      results[`${vpName}-d`] = r;
    });

    test(`${vpName} (e) new round / phase node`, async ({ page }) => {
      const game = await open(page, vp);
      await list(page).evaluate((el) => (el.scrollTop = el.scrollHeight));
      const before = await metrics(page);
      push(game, marker("pt-3", 3, "strategy"));
      push(game, decision(++counter, 3, "strategy"));
      await settle(page);
      const afterNoPath = await metrics(page);
      // The server then moves current_path to the new round, which auto-opens it.
      const { events: _e, ...update } = game.snapshot;
      game.send({ ...update, type: "state_update", game_version: 42, current_path: { round: 3, phase: "strategy" } });
      await settle(page);
      const afterPath = await metrics(page);
      results[`${vpName}-e`] = { before, afterNoPath, afterPath };
      await picture(page, `${vpName}-e-after`);
      // Expected: a user at the bottom stays pinned to the newest content.
      expect.soft(afterPath.bottomGap, "pinned after new round opens").toBeLessThanOrEqual(2);
    });

    test(`${vpName} (f) history generation change keeps expansion and scroll`, async ({ page }) => {
      const game = await open(page, vp);
      await list(page).getByRole("button", { name: /Round 1/ }).dispatchEvent("click"); // phone: the turn bar overlaps the drawer, so no pointer click
      await settle(page);
      await list(page).evaluate((el) => (el.scrollTop = 200));
      const before = await metrics(page);
      // Reconnect / undo: the server sends a fresh initial_snapshot (new events array).
      const events = [...history(), decision(9001, 2, "action")];
      game.send({ ...game.snapshot, game_version: 50, events, current_path: { round: 2, phase: "action" } });
      await settle(page);
      const afterGeneration = await metrics(page);
      game.send({ ...game.snapshot, game_version: 51, events: history().slice(0, -5), current_path: { round: 2, phase: "action" } });
      await settle(page);
      const afterUndo = await metrics(page);
      results[`${vpName}-f`] = { before, afterGeneration, afterUndo };
      expect.soft(afterGeneration.openHeadings, "manually opened Round 1 survives").toEqual(before.openHeadings);
      expect.soft(afterGeneration.scrollTop, "scroll position survives").toBe(before.scrollTop);
    });
  });
}

// (h) collapsed vs expanded dock: the toggle hides the list entirely; reopening starts fresh.
test("desktop (h) collapsed then reopened log, events while collapsed", async ({ page }) => {
  const game = await open(page, DESKTOP);
  await list(page).evaluate((el) => (el.scrollTop = 150));
  const before = await metrics(page);
  await page.getByTestId("event-log-toggle").click(); // hide: the list unmounts
  push(game, next());
  push(game, next());
  await settle(page);
  await page.getByTestId("event-log-toggle").click(); // show
  await list(page).waitFor();
  await settle(page);
  const reopened = await metrics(page);
  results["desktop-h"] = { before, reopened };
  expect.soft(reopened.scrollTop, "scroll position survives hide/show").toBe(before.scrollTop);
});

// Layout shift: another player's event makes a toast; it must not move or resize the log box.
test("desktop (toast) other player's event does not move the log box", async ({ page }) => {
  const game = await open(page, DESKTOP);
  const box = () => page.getByTestId("event-log-container").boundingBox();
  const before = await box();
  push(game, { ...next(), actor: "other_seat", detail: "other_seat played Sabotage" } as GameEvent);
  await page.waitForTimeout(400);
  const after = await box();
  results["desktop-toast"] = { before, after };
  expect.soft(after?.y).toBe(before?.y);
  expect.soft(after?.height).toBe(before?.height);
});
