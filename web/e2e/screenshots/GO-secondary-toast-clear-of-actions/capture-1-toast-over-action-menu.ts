import { test, expect, type Page, type TestInfo } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openMockedGame } from "../_shared/mockGame";
import { actor, opponent, playerWithHand } from "../_shared/players";
import { playedEvent, push, save, secondaryQuestion } from "../AH-secondary-prep-real-ui/prep";

// Auto mode: after the prepared secondary went out, the seat's own action menu opens at once (its turn
// came next). The "Auto-played your prepared secondary" toast used to sit at a fixed 16 px from the
// bottom, over the action menu's buttons (nightly 2026-10-20, run s1-08). It now docks above the bar
// (--bottom-bar-offset). The prefix is "before" for the old fixed offset.
const prefix = process.env.TI4_SHOT_PREFIX ?? "after";

const SIZES = [
  { name: "desktop", width: 1280, height: 720 },
  { name: "phone", width: 390, height: 844 },
];

const actionMenu = {
  prompt: "action phase",
  subtype: "action_menu",
  options: [
    { id: "tactical", label: "take a tactical action", kind: "action" },
    { id: "strategic|pok8imperial", label: "take the strategic action of 8. Imperial", kind: "action" },
    { id: "pass", label: "pass", kind: "action" },
  ],
  details: {
    kind: "turn_menu",
    closing: false,
    tokens: { tactic: 3, fleet: 3, strategy: 2 },
    strategy_cards: [{ card: "pok8imperial", used: false, option: "strategic|pok8imperial" }],
    partners: [],
  },
};

async function scene(page: Page, testInfo: TestInfo, size: (typeof SIZES)[number]) {
  await page.setViewportSize({ width: size.width, height: size.height });
  let answered = false;
  const game = await openMockedGame(page, {
    players: [
      playerWithHand({ strategy_cards: ["pok8imperial"], strategic_tokens: 2 }),
      { ...opponent, strategy_cards: ["pok7technology", "pok1leadership"] },
    ],
    events: [playedEvent("Technology")],
    view: { active_player: opponent.id },
    turnStatus: { kind: "waiting_for_decision", seat: opponent.id, phase: "action", round: 2, stage: "Waiting for player" },
    onClientMessage: (message, reply) => {
      if (message.type !== "submit_choice" || answered) return;
      answered = true;
      // The server accepts the answer; the game moves on to the viewer's own action menu.
      const { events: _events, ...base } = game.snapshot;
      reply({ type: "action_accepted", protocol_version: base.protocol_version, game_id: base.game_id, game_version: 42, option_id: message.option_id } as never);
      reply({
        ...base,
        type: "state_update",
        game_version: 43,
        history: { cursor: 42, redo_count: 0, generation: 0 },
        turn_status: { kind: "waiting_for_decision", seat: actor, phase: "action", round: 2, stage: "action_menu" },
        pending_choice: {
          nonce: "prep-nonce-43",
          choice: {
            player: actor,
            prompt: actionMenu.prompt,
            context: { subtype: actionMenu.subtype },
            options: actionMenu.options,
            details: actionMenu.details,
          },
        },
      } as never);
    },
  });
  push(game, { history: { cursor: 40, redo_count: 0, generation: 0 }, version: 41 });
  await page.waitForTimeout(150);
  await page.getByTestId("secondary-prep-mode-auto").evaluate((el) => (el as HTMLElement).click());
  await page.getByTestId("secondary-prep-chip").click();
  await page.getByTestId("secondary-yes-btn").click();
  await save(page);
  if (prefix === "before")
    await page.addStyleTag({ content: ".secondary-prep:not(.secondary-prep--preparing) { bottom: 16px !important; }" });
  push(game, {
    version: 42,
    history: { cursor: 41, redo_count: 0, generation: 0 },
    choice: secondaryQuestion("pok7technology", "spend a strategy token and 4 resources to research", "spend"),
  });
  const toast = page.getByTestId("secondary-autoplayed-toast");
  await toast.waitFor({ timeout: 10_000 });
  await page.getByTestId("turn-action-bar").waitFor();
  await page.waitForTimeout(300);
  void testInfo;
  return toast;
}

for (const size of SIZES) {
  test(`auto-played toast clear of the action menu, ${size.name}`, async ({ page }, testInfo) => {
    const toast = await scene(page, testInfo, size);
    const bar = (await page.getByTestId("turn-action-bar").boundingBox())!;
    const box = (await toast.boundingBox())!;
    console.log(`toast ${prefix} ${size.name}: toast bottom ${Math.round(box.y + box.height)}, bar top ${Math.round(bar.y)}`);
    if (prefix === "after") expect(box.y + box.height).toBeLessThanOrEqual(bar.y + 0.5);
    await shot(page, testInfo, `${prefix}-1-toast-${size.name}`);
  });
}
