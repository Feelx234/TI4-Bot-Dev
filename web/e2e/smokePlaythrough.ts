import { appendFileSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import {
  expect,
  type APIRequestContext,
  type Browser,
  type Page,
} from "@playwright/test";
import {
  createStartedGame,
  gameSnapshot,
  openPlayerGame,
} from "./lobbyHelpers";
import type { BoardView } from "../src/protocol/types";
import {
  activationWeight,
  preferHitConfirm,
  preferPayment,
  preferTokenConfirm,
  steerWeight as policySteerWeight,
  strongUnselected,
  classifyControl,
  isBarControl,
  turnBarPool,
  pausedBatch,
  EXCLUDED_CONTROLS,
  isReactionSubtype,
  reactionTextProblem,
} from "./smokePolicy";

/**
 * Random UI playthrough: every pending decision is resolved by clicking randomly among the
 * controls the web UI offers to the acting seat. The server is only read to find the acting
 * seat and to detect progress; every action goes through the browser.
 */
export interface PlaythroughOptions {
  playerCount: number;
  gameSeed: number;
  clickSeed: number;
  /** Start preset for the game (e.g. "combat": fleets beside homes and Mecatol). */
  startPreset?: string;
  /** Stop successfully after this many resolved decisions. */
  maxDecisions: number;
  /** Fail when a single decision does not advance after this many clicks. */
  maxClicksPerDecision: number;
  /** Stop successfully once the game reaches this round. */
  stopAtRound?: number;
  /** `random` clicks uniformly; `steer` favours tactical play so combat, invasion and agendas occur. */
  policy?: "random" | "steer";
  log?: (line: string) => void;
  /**
   * When set, write a machine-readable trace here: `game.json` (id and seats) at the start,
   * `trace.jsonl` (one line per offered decision), and `report.json` + `final-snapshot.json`
   * at the end. Used by the nightly proctors to compile what happened in a game.
   */
  traceDir?: string;
}

export interface PlaythroughReport {
  gameId: string;
  startPreset: string | null;
  decisions: number;
  clicks: number;
  finished: boolean;
  /** Decisions resolved before each round was first seen, keyed by round. */
  roundStarts: Record<number, number>;
  finalStatus: unknown;
  subtypes: Record<string, number>;
  /** Error banners the UI showed after a click, usually server rejections of offered controls. */
  rejections: string[];
  /** Batches the server stopped at a reaction window (not failures). */
  pausedBatches: number;
  /** Action-phase decisions (the turn menu and the end-turn question) seen in the run. */
  turnMenuDecisions: number;
  /** Of those, resolved with a click on the persistent turn bar. */
  barDecisions: number;
  /** Bar clicks that submitted, by control (`turn-bar-strategic-<card>`, `turn-bar-open`, ...). */
  barControls: Record<string, number>;
  /** Turn-menu decisions that fell back to the old list because the bar could not map them. */
  turnMenuFallbacks: string[];
  /** Reaction decisions (windows, the inner card pick, Instinct Training, the L1Z1X agent) seen. */
  reactionDecisions: number;
  /** Reaction dialogs whose text showed a raw engine id or a doubled verb ("Play play"). */
  reactionTextProblems: string[];
}

// Containers that render a decision for the acting seat.
const DECISION_CONTAINERS = [
  "turn-action-bar",
  "pending-choice-dialog",
  "system-activation-bar",
  "planet-selection-bar",
  "tactical-movement-tray",
  "cargo-loading-tray",
  "invasion-landing-tray",
  "invasion-overlay",
  "payment-drawer",
  "payment-bar",
  "production-builder-drawer",
  "objectives-modal",
  "technology-modal",
  "combat-resolution-modal",
  "trade-desk-modal",
  "agenda-ballot-modal",
  "reaction-status-bar",
  "paused-plan",
];

const ERROR_BANNERS = [
  "choice-error-banner",
  "movement-error-banner",
  "activation-error",
  "planet-selection-error",
  "reaction-error-badge",
  "combat-error-banner",
  "hit-assignment-error",
  "token-error",
];

// Controls that hide the decision or rewrite history; clicking them never advances the game.
const EXCLUDED = EXCLUDED_CONTROLS;
// Controls that take back staged selections. Only used to escape a staging dead end, such as
// cargo over transport capacity, where every submit button is disabled.
interface Candidate {
  idx: number;
  desc: string;
  /** Untruncated description used for steering. */
  full: string;
  resume: boolean;
  commit: boolean;
  unstage: boolean;
  checked: boolean;
}

function mulberry32(seed: number) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export async function collectCandidates(page: Page): Promise<Candidate[]> {
  const raw = await page.evaluate(
    ({ containers, excluded }) => {
      const excludedRe = new RegExp(excluded, "i");
      document
        .querySelectorAll("[data-smoke-idx]")
        .forEach((el) => el.removeAttribute("data-smoke-idx"));
      // A board hex behind a modal is rendered but cannot receive the click. Only on-screen
      // centers are tested, because Playwright scrolls off-screen controls into view itself.
      const covered = (rect: DOMRect, el: Element) => {
        const x = rect.left + rect.width / 2;
        const y = rect.top + rect.height / 2;
        if (x < 0 || y < 0 || x >= window.innerWidth || y >= window.innerHeight)
          return false;
        const hit = document.elementFromPoint(x, y);
        return hit !== null && !el.contains(hit);
      };
      const visible = (el: Element) => {
        const rect = el.getBoundingClientRect();
        const style = getComputedStyle(el);
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          style.visibility !== "hidden" &&
          style.pointerEvents !== "none" &&
          !covered(rect, el)
        );
      };
      const enabled = (el: Element) =>
        !(el as HTMLButtonElement).disabled &&
        el.getAttribute("aria-disabled") !== "true" &&
        el.getAttribute("data-actionable") !== "false";
      const found = new Set<Element>();
      document
        .querySelectorAll(
          '[data-testid^="resume-"], [data-testid="choice-minimized-pill"] button, [data-target-candidate="true"]',
        )
        .forEach((el) => found.add(el));
      for (const id of containers) {
        document
          .querySelectorAll(`[data-testid="${id}"]`)
          .forEach((container) => {
            container
              .querySelectorAll(
                'button, [role="button"], input[type="checkbox"], input[type="radio"], [data-testid="choice-option"], [data-selectable="true"]',
              )
              .forEach((el) => {
                // A choice option label already covers its inner radio or checkbox.
                if (
                  el.closest('[data-testid="choice-option"]') !== el &&
                  el.closest('[data-testid="choice-option"]')
                )
                  return;
                found.add(el);
              });
          });
      }
      const out: {
        idx: number;
        desc: string;
        full: string;
        resume: boolean;
        checked: boolean;
      }[] = [];
      let idx = 0;
      for (const el of found) {
        if (!visible(el) || !enabled(el)) continue;
        // An unlabeled checkbox or radio is described by the label that wraps it.
        const named =
          el instanceof HTMLInputElement ? (el.closest("label") ?? el) : el;
        const testId = named.getAttribute("data-testid") ?? "";
        const label = named.getAttribute("aria-label") ?? "";
        const fullText = (named.textContent ?? "").trim().replace(/\s+/g, " ");
        const text = fullText.slice(0, 60);
        const desc = [testId, label, text].filter(Boolean).join(" | ");
        const full = [testId, label, fullText].filter(Boolean).join(" | ");
        if (excludedRe.test(testId) || excludedRe.test(label)) continue;
        if (!testId && excludedRe.test(text)) continue;
        el.setAttribute("data-smoke-idx", String(idx));
        out.push({
          idx,
          desc,
          full,
          resume:
            testId.startsWith("resume-") ||
            el.closest('[data-testid="choice-minimized-pill"]') !== null,
          checked: el instanceof HTMLInputElement && el.checked,
        });
        idx++;
      }
      return out;
    },
    { containers: DECISION_CONTAINERS, excluded: EXCLUDED.source },
  );
  return raw.map((c) => {
    return { ...c, ...classifyControl(c.desc) };
  });
}

// Base move values; faction variants such as `sol_carrier` share the suffix. Ground forces,
// structures and fighters cannot move on their own.
function shipMove(unitType: string): number {
  if (/(carrier|dreadnought|flagship)$/.test(unitType)) return 1;
  if (/(cruiser|destroyer|warsun|war_sun)$/.test(unitType)) return 2;
  return 0;
}

/**
 * Activation targets worth steering toward: systems one of the actor's fleets can reach by base
 * move value (ignoring anomalies and wormholes), weighted up for Mecatol Rex and for systems
 * holding another player's units. The UI does not show reachability, so this reads the actor's
 * board view.
 */
function activationWeights(
  board: BoardView,
  actor: string,
): Map<string, number> {
  const tiles = new Map((board.map_tiles ?? []).map((t) => [t.system_id, t]));
  const fleets = Object.values(board.systems).flatMap((sys) => {
    const tile = tiles.get(sys.system_id);
    const move = Math.max(
      0,
      ...sys.units
        .filter((u) => u.owner === actor && !u.planet)
        .map((u) => shipMove(u.unit_type)),
    );
    return tile && move > 0 ? [{ tile, move }] : [];
  });
  const weights = new Map<string, number>();
  for (const [id, tile] of tiles) {
    const reachable = fleets.some(({ tile: f, move }) => {
      const distance =
        (Math.abs(f.q - tile.q) +
          Math.abs(f.r - tile.r) +
          Math.abs(f.q + f.r - tile.q - tile.r)) /
        2;
      // Ships already in the active system cannot move, so distance 0 does not count.
      return distance > 0 && distance <= move;
    });
    const enemies =
      board.systems[id]?.units.some((u) => u.owner !== actor) ?? false;
    const inPlace =
      id === "18" &&
      (board.systems[id]?.units.some(
        // Still in space: once landed on the planet the custodians are gone and there is nothing
        // left to do in place.
        (u) =>
          u.owner === actor &&
          !u.planet &&
          /infantry|mech|spec_ops/i.test(u.unit_type),
      ) ??
        false);
    weights.set(id, activationWeight(id, reachable, enemies, inPlace));
  }
  return weights;
}

function steerWeight(desc: string, hexWeights: Map<string, number>): number {
  const hex = /^system-hex-(\S+) /.exec(desc);
  if (hex) return hexWeights.get(hex[1]) ?? 1;
  return policySteerWeight(desc);
}

function weightedPick(
  pool: Candidate[],
  weight: (c: Candidate) => number,
  rng: () => number,
) {
  const weights = pool.map(weight);
  let roll = rng() * weights.reduce((a, b) => a + b, 0);
  for (const [i, w] of weights.entries()) {
    roll -= w;
    if (roll < 0) return pool[i];
  }
  return pool[pool.length - 1];
}

function pick(
  candidates: Candidate[],
  clicks: number,
  rng: () => number,
  policy: "random" | "steer",
  hexWeights: Map<string, number>,
): Candidate {
  // The turn bar has no staging step: choose one of its controls by weight (or uniformly).
  const bar = turnBarPool(candidates);
  if (bar) {
    if (policy === "random") return bar[Math.floor(rng() * bar.length)];
    return weightedPick(bar, (c) => steerWeight(c.full, hexWeights), rng);
  }
  candidates = preferTokenConfirm(
    preferHitConfirm(preferPayment(candidates)),
    rng,
  );
  const resume = candidates.filter((c) => c.resume);
  if (resume.length) return resume[Math.floor(rng() * resume.length)];
  const unstage = candidates.filter((c) => c.unstage);
  const forward = candidates.filter((c) => !c.unstage);
  // Take staging back when nothing else is clickable, now and then when no submit control is
  // enabled (such as cargo over transport capacity), or once a decision has stalled. Dropping
  // cargo one unit at a time keeps the ships staged, so the plan can become committable. The
  // blocked chance stays low: a payment that needs every source (3 owed, exactly 3 available)
  // only unlocks its confirm button after several staging clicks in a row.
  const blocked = !forward.some((c) => c.commit);
  if (
    unstage.length &&
    (!forward.length ||
      (blocked && rng() < 0.1) ||
      (clicks >= 10 && rng() < 0.2))
  )
    return weightedPick(
      unstage,
      (c) => (/cargo|decrement|remove/i.test(c.desc) ? 5 : 0.5),
      rng,
    );
  const commits = forward.filter((c) => c.commit);
  const stages = forward.filter((c) => !c.commit);
  const strong =
    policy === "steer"
      ? strongUnselected(stages.map((c) => ({ ...c, desc: c.full })))
      : undefined;
  if (strong) return stages.find((c) => c.idx === strong.idx) ?? strong;
  // Stage a few selections first, then lean toward submitting as the decision drags on. Steered
  // fleet and landing trays stage longer so ships and ground forces actually move.
  const staging =
    policy === "steer" &&
    stages.some((c) => /^rally-inc-| in space/i.test(c.full));
  const commitChance = staging
    ? Math.min(0.05 + 0.08 * clicks, 0.9)
    : Math.min(0.35 + 0.15 * clicks, 0.9);
  const pool =
    commits.length && (!stages.length || rng() < commitChance)
      ? commits
      : stages;
  if (policy === "random") return pool[Math.floor(rng() * pool.length)];
  return weightedPick(pool, (c) => steerWeight(c.full, hexWeights), rng);
}

async function uiVersion(page: Page): Promise<number> {
  const text = await page
    .getByTestId("game-version")
    .innerText()
    .catch(() => "");
  return Number(text.replace(/^v/, "")) || 0;
}

async function visibleErrors(page: Page): Promise<string[]> {
  const texts: string[] = [];
  for (const id of ERROR_BANNERS) {
    for (const el of await page.getByTestId(id).all()) {
      if (await el.isVisible()) texts.push((await el.innerText()).trim());
    }
  }
  return texts.filter(Boolean);
}

export async function randomUiPlaythrough(
  browser: Browser,
  request: APIRequestContext,
  options: PlaythroughOptions,
): Promise<PlaythroughReport> {
  const log = options.log ?? (() => {});
  const rng = mulberry32(options.clickSeed);
  const { gameId, players } = await createStartedGame(
    request,
    options.playerCount,
    options.gameSeed,
    options.startPreset,
  );
  log(
    `game ${gameId} seed=${options.gameSeed} clickSeed=${options.clickSeed} preset=${options.startPreset ?? "none"}`,
  );

  const browserErrors: string[] = [];
  const pages: Page[] = [];
  // The last websocket frames each seat received, so a stuck decision can show whether the tab
  // was ever sent it, or had it cleared afterwards.
  const wsFrames: string[][] = [];
  for (const [index, player] of players.entries()) {
    const context = await browser.newContext();
    const page = await context.newPage();
    page.on("pageerror", (err) =>
      browserErrors.push(`[seat ${index + 1}] ${err.message}`),
    );
    const frames: string[] = (wsFrames[index] = []);
    page.on("websocket", (ws) => {
      ws.on("framesent", ({ payload }) => {
        try {
          const m = JSON.parse(String(payload));
          if (m.type === "ping") return;
          frames.push(
            `SENT ${m.type} nonce=${String(m.nonce ?? "").slice(0, 6)} option=${m.option_id ?? ""} v${m.expected_version ?? "?"}`,
          );
        } catch {
          frames.push("SENT unparsed frame");
        }
        if (frames.length > 40) frames.shift();
      });
      ws.on("framereceived", ({ payload }) => {
        let line: string;
        try {
          const m = JSON.parse(String(payload));
          const pending = m.pending_choice?.nonce ?? m.nonce ?? null;
          line = `${m.type} v${m.game_version ?? m.entry?.version ?? "?"}${pending ? ` nonce=${String(pending).slice(0, 6)}` : ""}${m.type === "turn_status" ? ` ${m.status?.kind}` : ""}${m.type === "state_update" ? ` pending=${m.pending_choice ? "yes" : "no"}` : ""}`;
        } catch {
          line = "unparsed frame";
        }
        frames.push(line);
        if (frames.length > 40) frames.shift();
      });
    });
    page.on("console", (msg) => {
      if (msg.type() !== "error") return;
      const url = msg.location().url ?? "";
      // The optional battle advisor (/battle, /ground_odds) is not started for e2e runs.
      if (url.includes("favicon") || url.includes("/battle") || url.includes("/ground_odds")) return;
      browserErrors.push(`[seat ${index + 1}] console: ${msg.text()}`);
    });
    // The console only reports a status code; keep the server's reason for failed API calls.
    page.on("response", async (response) => {
      if (!response.url().includes("/api/")) return;
      if (response.status() < 400) {
        // A batch the server paused at a reaction window is progress, not a rejection.
        if (response.url().includes("/batches")) {
          const paused = pausedBatch(await response.text().catch(() => ""));
          if (paused) {
            report.pausedBatches += 1;
            log(
              `  [seat ${index + 1}] batch paused after ${paused.applied} steps, ${paused.remaining} left, waiting on ${paused.waiting}`,
            );
          }
        }
        return;
      }
      const body = await response.text().catch(() => "");
      const line = `[seat ${index + 1}] ${response.request().method()} ${new URL(response.url()).pathname} ${response.status()}: ${body.slice(0, 500)}`;
      browserErrors.push(line);
      log(`  ${line}`);
    });
    await openPlayerGame(page, gameId, player.session);
    await expect(page.getByTestId("turn-status-bar")).toBeVisible();
    pages.push(page);
  }

  const report: PlaythroughReport = {
    gameId,
    startPreset: options.startPreset ?? null,
    decisions: 0,
    clicks: 0,
    finished: false,
    roundStarts: {},
    finalStatus: null,
    subtypes: {},
    rejections: [],
    pausedBatches: 0,
    turnMenuDecisions: 0,
    barDecisions: 0,
    barControls: {},
    turnMenuFallbacks: [],
    reactionDecisions: 0,
    reactionTextProblems: [],
  };

  const trace = (file: string, data: unknown, append = false) => {
    if (!options.traceDir) return;
    const path = join(options.traceDir, file);
    const text = append
      ? `${JSON.stringify(data)}\n`
      : JSON.stringify(data, null, 2);
    if (append) appendFileSync(path, text);
    else writeFileSync(path, text);
  };
  if (options.traceDir) mkdirSync(options.traceDir, { recursive: true });
  trace("game.json", {
    gameId,
    players: players.map((p) => p.id),
    options: { ...options, log: undefined },
  });
  const writeFinal = async () => {
    if (!options.traceDir) return;
    trace("report.json", report);
    const snapshot = await gameSnapshot(
      request,
      gameId,
      players[0].session,
    ).catch(() => null);
    if (snapshot) trace("final-snapshot.json", snapshot);
  };

  let traceWritten = false;
  const fail = async (
    page: Page | undefined,
    message: string,
  ): Promise<never> => {
    traceWritten = true;
    const status = await gameSnapshot(
      request,
      gameId,
      players[0].session,
    ).catch(() => null);
    const seatIndex = page ? pages.indexOf(page) : -1;
    const frames =
      seatIndex >= 0
        ? `\nws frames seat ${seatIndex + 1} (newest last):\n${wsFrames[seatIndex].join("\n")}`
        : "";
    const detail = `${message}\nreport: ${JSON.stringify({ ...report, finalStatus: status?.turn_status })}${frames}`;
    if (page)
      await page
        .screenshot({ path: `test-results/smoke-failure-${gameId}.png` })
        .catch(() => {});
    trace("failure.txt", detail);
    await writeFinal();
    throw new Error(detail);
  };

  try {
    let noChoicePolls = 0;
    while (report.decisions < options.maxDecisions) {
      // Through `fail` so the trace (failure.txt, report.json) is written for browser errors too.
      if (browserErrors.length)
        await fail(
          undefined,
          `browser errors during playthrough:\n${browserErrors.join("\n")}`,
        );
      const state = await gameSnapshot(request, gameId, players[0].session);
      const status = state.turn_status;
      report.finalStatus = status;
      if (status.kind === "game_over") {
        report.finished = true;
        break;
      }
      if (!(status.round in report.roundStarts)) {
        report.roundStarts[status.round] = report.decisions;
        log(`round ${status.round} reached after ${report.decisions} decisions`);
      }
      if (
        options.stopAtRound !== undefined &&
        status.round >= options.stopAtRound
      )
        break;
      if (status.kind !== "waiting_for_decision") {
        // Nothing to click; the server should move on by itself.
        const moved = await expect
          .poll(
            async () =>
              (await gameSnapshot(request, gameId, players[0].session))
                .game_version,
            {
              timeout: 10_000,
            },
          )
          .toBeGreaterThan(state.game_version)
          .then(() => true)
          .catch(() => false);
        if (!moved) {
          // The game may have ended (objective decks exhausted) after the snapshot above was taken.
          const latest = await gameSnapshot(request, gameId, players[0].session);
          if (latest.turn_status.kind === "game_over") continue;
          await fail(
            undefined,
            `game idle without a decision: ${JSON.stringify(status)}`,
          );
        }
        continue;
      }

      const actorIndex = players.findIndex((p) => p.id === status.seat);
      if (actorIndex < 0)
        await fail(undefined, `decision for unknown seat ${status.seat}`);
      const page = pages[actorIndex];
      const actorState = await gameSnapshot(
        request,
        gameId,
        players[actorIndex].session,
      );
      const choice = actorState.pending_choice?.choice;
      // The status was read before the offer moved on (e.g. to the secondary of a strategy card); re-poll.
      if (!choice) {
        if (++noChoicePolls > 40)
          await fail(
            undefined,
            `seat ${status.seat} is waiting but has no pending choice: ${JSON.stringify(status)}`,
          );
        await new Promise((resolve) => setTimeout(resolve, 100));
        continue;
      }
      noChoicePolls = 0;
      const subtype =
        choice?.context?.subtype ??
        `prompt:${choice?.prompt.slice(0, 40) ?? "none"}`;
      const before = actorState.game_version;
      const hexWeights =
        options.policy === "steer" && subtype === "activate_system"
          ? activationWeights(actorState.view.board, players[actorIndex].id)
          : new Map<string, number>();
      report.subtypes[subtype] = (report.subtypes[subtype] ?? 0) + 1;
      trace(
        "trace.jsonl",
        {
          decision: report.decisions,
          version: before,
          round: status.round,
          phase: status.phase,
          seat: actorIndex + 1,
          player: players[actorIndex].id,
          faction: actorState.view.players?.find(
            (p) => p.id === players[actorIndex].id,
          )?.faction,
          subtype,
          source: choice?.context?.source ?? null,
          prompt: choice?.prompt,
          options: choice?.options.map((o) => ({
            id: o.id,
            kind: o.kind,
            label: o.label,
          })),
        },
        true,
      );

      // Let the actor's tab catch up with the server before reading its controls.
      await expect
        .poll(() => uiVersion(page), { timeout: 10_000 })
        .toBeGreaterThanOrEqual(before)
        .catch(() =>
          fail(page, `seat ${actorIndex + 1} UI never reached v${before}`),
        );

      const isTurnMenu =
        choice.prompt === "action phase" || subtype === "end_turn";
      if (isTurnMenu) report.turnMenuDecisions++;
      let barControl: string | null = null;
      let progressed = false;
      let emptyPolls = 0;
      const isReaction = isReactionSubtype(subtype);
      if (isReaction) report.reactionDecisions++;
      let reactionChecked = !isReaction;
      for (let clicks = 0; clicks < options.maxClicksPerDecision;) {
        // Progress is read from the actor's tab (free) rather than the API; it follows the server
        // over the websocket. A late-landing commit is caught here before another click.
        if (clicks > 0 && (await uiVersion(page)) > before) {
          progressed = true;
          break;
        }
        if (!reactionChecked) {
          const shown = await page
            .getByTestId("reaction-status-bar")
            .first()
            .textContent({ timeout: 500 })
            .catch(() => null);
          if (shown !== null) {
            reactionChecked = true;
            const problem = reactionTextProblem(shown);
            if (problem) report.reactionTextProblems.push(`${subtype}: ${problem}`);
          }
        }
        const candidates = await collectCandidates(page);
        if (!candidates.length) {
          // The UI can take a moment to mount the workflow for a fresh offer. Late in a game the
          // UI may also be finishing its own multi-step plan (a production builder submits one
          // decision per unit and each commit replays the whole game on a debug build, seconds
          // apiece), so the offer shows no control until the plan has landed (run 1c, decision
          // #1153: 3 s was not enough). The wait is long; the version check below still ends it
          // as soon as the server moves on.
          if (++emptyPolls > 250) {
            await fail(
              page,
              `no actionable control for ${subtype} (seat ${actorIndex + 1}); options: ${JSON.stringify(choice?.options.map((o) => o.id))}`,
            );
          }
          await page.waitForTimeout(100);
          if (
            (await gameSnapshot(request, gameId, players[0].session))
              .game_version > before
          ) {
            progressed = true;
            break;
          }
          continue;
        }
        const chosen = pick(
          candidates,
          clicks,
          rng,
          options.policy ?? "random",
          hexWeights,
        );
        log(
          `#${report.decisions} ${subtype} seat${actorIndex + 1} click ${chosen.desc}`,
        );
        await page
          .locator(`[data-smoke-idx="${chosen.idx}"]`)
          .click({ timeout: 2_000 })
          .catch((err: Error) =>
            log(`  click failed: ${err.message.split("\n")[0]}`),
          );
        clicks++;
        report.clicks++;
        if (isTurnMenu && isBarControl(chosen.desc) && chosen.commit)
          barControl = chosen.desc
            .split(" | ")[0]
            .replace(/^(turn-bar-open)-.*/, "$1")
            .replace(/^(turn-bar-item)-.*/, "$1");

        if (chosen.commit) {
          // Wait for the tab to show the new version; fall back to one server check in case the
          // tab's websocket lagged.
          progressed = await expect
            .poll(() => uiVersion(page), { timeout: 3_000, intervals: [50] })
            .toBeGreaterThan(before)
            .then(() => true)
            .catch(async () =>
              // gameSnapshot retries once with a longer timeout; if that fails too the run ends with
              // failure.txt, report.json and a best-effort final snapshot (not a bare timeout).
              gameSnapshot(request, gameId, players[0].session).then(
                (latest) => latest.game_version > before,
                (err: Error) =>
                  fail(
                    page,
                    `snapshot request failed after ${subtype} (decision #${report.decisions}): ${err.message.split("\n")[0]}`,
                  ),
              ),
            );
        } else {
          // Staging clicks rarely advance the server: let React settle, then read the tab once.
          await page.waitForTimeout(40);
          progressed = (await uiVersion(page)) > before;
        }
        if (progressed) break;
        // A rejected batch is rejected again if re-sent, so stop at the first one.
        const batchRejection = browserErrors.find((e) =>
          /\/batches \d{3}:/.test(e),
        );
        if (batchRejection) {
          report.rejections.push(`${subtype}: ${batchRejection}`);
          await fail(page, `batch rejected: ${batchRejection}`);
        }
        const errors = await visibleErrors(page);
        for (const error of errors) {
          const entry = `${subtype}: ${error}`;
          if (!report.rejections.includes(entry)) {
            report.rejections.push(entry);
            log(`  rejected: ${error}`);
          }
        }
      }
      if (!progressed) {
        progressed =
          (await gameSnapshot(request, gameId, players[0].session)).game_version >
          before;
      }
      if (!progressed) {
        await fail(
          page,
          `${subtype} did not advance after ${options.maxClicksPerDecision} clicks (seat ${actorIndex + 1}); options: ${JSON.stringify(choice?.options.map((o) => o.id))}`,
        );
      }
      if (isTurnMenu) {
        if (barControl) {
          report.barDecisions++;
          report.barControls[barControl] =
            (report.barControls[barControl] ?? 0) + 1;
        } else {
          report.turnMenuFallbacks.push(
            `${subtype}: ${JSON.stringify(choice.options.map((o) => o.id))}`,
          );
        }
      }
      report.decisions++;
    }

  } catch (err) {
    // Anything not raised through `fail` (a snapshot or click error) still leaves the evidence.
    if (!traceWritten) {
      traceWritten = true;
      const message = err instanceof Error ? err.message : String(err);
      trace(
        "failure.txt",
        `unexpected error: ${message.split("\n")[0]}\nreport: ${JSON.stringify(report)}`,
      );
      await writeFinal();
    }
    throw err;
  }
  report.finalStatus = (
    await gameSnapshot(request, gameId, players[0].session)
  ).turn_status;
  trace("browser-errors.json", browserErrors);
  await writeFinal();
  expect(browserErrors, "browser errors during playthrough").toEqual([]);
  return report;
}
