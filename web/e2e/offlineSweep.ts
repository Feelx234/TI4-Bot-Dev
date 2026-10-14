import { appendFileSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { BrowserContext, Page } from "@playwright/test";
import type { BoardView } from "../src/protocol/types";
import {
  DECISION_CONTAINERS,
  ERROR_BANNERS,
  activationWeights,
  collectCandidates,
  mulberry32,
  pick,
  visibleErrors,
} from "./smokePlaythrough";
import { isReactionSubtype, reactionTextProblem } from "./smokePolicy";
import { preferTechWindowUse } from "./smokePolicy";

/**
 * One full offline game (setup form to game over) driven through the real UI, for the nightly
 * offline sweep (tools/nightly-smoke/offline-run.sh). The page is the production build served as
 * static files; the engine runs as wasm in the page's worker and the other seats are rule bots,
 * so there is no server and no proctor. Every decision of the human seat is answered by clicking
 * the controls the UI offers (the smoke harness's candidate scan and weighted pick), and the
 * worker's messages are observed from the page, so the harness knows the engine's question, its
 * save blob and its end without reading the DOM.
 *
 * Nothing here throws on a UI problem: each one becomes a finding (with a screenshot) and the run
 * ends with an outcome (finished, decision-cap, time-cap, stuck, crash). The Playwright test only
 * fails when the harness itself breaks.
 */

export interface OfflineSweepOptions {
  baseUrl: string;
  gameSeed: number;
  clickSeed: number;
  players: number;
  policy: "random" | "steer";
  /** `random`, or a 1-based seat number. */
  seat: string;
  /** `random` (a random roster faction), `dealt` (the seed deals it) or a faction id. */
  faction: string;
  maxDecisions: number;
  maxSeconds: number;
  /** No worker message and no decision for this long means the game is stuck. */
  stallSeconds: number;
  viewport: "desktop" | "phone";
  shotCap: number;
  traceDir: string;
  log: (line: string) => void;
}

export type Outcome = "finished" | "decision-cap" | "time-cap" | "stuck" | "crash" | "setup-failed";

export interface OfflineSweepReport {
  outcome: Outcome;
  gameSeed: number;
  clickSeed: number;
  players: number;
  policy: string;
  viewport: string;
  humanSeat: number | null;
  humanFaction: string | null;
  factions: string[];
  decisions: number;
  clicks: number;
  finished: boolean;
  roundStarts: Record<number, number>;
  subtypes: Record<string, number>;
  rejections: string[];
  findings: string[];
  panics: string[];
  consoleErrors: string[];
  pageErrors: string[];
  failedRequests: string[];
  recoveries: number;
  /** Times the harness closed the system inspector because it covered the controls of the question. */
  overlayDismissals: number;
  seconds: number;
  engine: { build?: string; content_digest?: string; wasmSha256?: string } | null;
  finalDecisionsHash: string | null;
  finalStandings: string[];
  shots: { taken: number; cap: number; names: string[] };
  finalStatus: unknown;
  /** Wall time the human waited for the engine/bots to ask again, summed (ms) and the worst single wait. */
  engineWaitMs: number;
  worstWaitMs: number;
  heapMb: number | null;
  /** Wall time of the harness by phase, ms: scan (candidate scan), click, commit (wait for the engine to ask again), idle (nothing to do, the bots play). */
  timing: Record<string, number>;
  decisionsPerMinute: number;
}

interface AskSummary {
  seq: number;
  version: number;
  answers: number;
  round: number | null;
  phase: string | null;
  statusKind: string | null;
  nonce: string | null;
  player: string | null;
  subtype: string;
  prompt: string;
  source: unknown;
  options: { id: string; kind?: string; label?: string }[];
  board?: BoardView;
}

/** Runs in the page before any script: seeds the launcher and spies on the engine's worker. */
export function sweepInitScript(arg: { seed: number; autoLone: boolean | null }) {
  const hi = Math.floor(arg.seed / 2 ** 32);
  const lo = arg.seed % 2 ** 32;
  const real = crypto.getRandomValues.bind(crypto);
  // The launcher draws its game seed with a two-word Uint32Array; nothing else does.
  (crypto as unknown as { getRandomValues: unknown }).getRandomValues = (array: ArrayBufferView) => {
    if (array instanceof Uint32Array && array.length === 2) {
      array[0] = hi;
      array[1] = lo;
      return array;
    }
    return real(array as Uint32Array);
  };
  if (arg.autoLone !== null) {
    try {
      localStorage.setItem("player_auto_submit_lone", String(arg.autoLone));
    } catch {
      // storage unavailable
    }
  }
  const w = window as unknown as Record<string, any>;
  const Original = window.Worker;
  let seq = 0;
  const post = (name: string, payload: unknown) => {
    try {
      w.__sweepEvent(name, payload);
    } catch {
      // the binding is not there yet
    }
  };
  w.Worker = function (...args: unknown[]) {
    const worker = new (Original as any)(...args);
    worker.addEventListener("message", (event: MessageEvent) => {
      const m = event.data;
      if (!m || typeof m.t !== "string") return;
      const stamp = Date.now();
      if (m.t === "ask" || m.t === "end") {
        const messages: any[] = m.envelope?.messages ?? [];
        const update = [...messages].reverse().find((x) => x && x.type === "state_update");
        const initial = messages.find((x) => x && x.type === "initial_snapshot");
        const view = update ?? initial ?? null;
        const choice = view?.pending_choice?.choice ?? null;
        const save = m.save ?? m.envelope?.save ?? null;
        w.__sweepSave = save ?? w.__sweepSave;
        const summary: any = {
          kind: m.t,
          stamp,
          seq: m.t === "ask" ? ++seq : seq,
          version: view?.game_version ?? save?.version ?? -1,
          answers: save?.answers?.length ?? -1,
          round: view?.turn_status?.round ?? save?.round ?? null,
          phase: view?.turn_status?.phase ?? save?.phase ?? null,
          statusKind: view?.turn_status?.kind ?? null,
          nonce: view?.pending_choice?.nonce ?? null,
          player: choice?.player ?? null,
          subtype: choice?.context?.subtype ?? (choice ? `prompt:${String(choice.prompt).slice(0, 40)}` : "none"),
          prompt: choice?.prompt ?? "",
          source: choice?.context?.source ?? null,
          options: (choice?.options ?? []).map((o: any) => ({ id: o.id, kind: o.kind, label: o.label })),
          end: m.t === "end" ? (m.envelope?.end ?? null) : undefined,
        };
        summary.events = messages
          .filter((x) => x && x.type === "event" && x.entry)
          .slice(0, 200)
          .map((x) => ({ v: x.entry.version, round: x.entry.round, phase: x.entry.phase, actor: x.entry.actor, detail: String(x.entry.detail ?? x.entry.event?.kind ?? "").slice(0, 200) }));
        if (choice?.context?.subtype === "activate_system") summary.board = view?.view?.board;
        if (view) {
          summary.players = (view.view?.players ?? view.initial?.players ?? []).map((p: any) => ({
            id: p.id,
            faction: p.faction,
            vp: p.victory_points,
          }));
          summary.viewer = view.viewer ?? null;
        }
        post("ask", summary);
      } else if (m.t === "crash" || m.t === "fatal") {
        post(m.t, { stamp, message: String(m.message).slice(0, 2000) });
      } else if (m.t === "progress") {
        post("progress", { stamp });
      }
    });
    return worker;
  } as unknown as typeof Worker;
  w.Worker.prototype = Original.prototype;
}

// The page probes /api/access when it loads; there is no server in the offline sweep (vite preview
// answers 502), so any call to the server's paths is environment noise, counted but not reported.
const SERVER_PATH = /^https?:\/\/[^/]+\/(api|ws|advisor|health)(\/|$|\?)/;

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

export async function playOfflineGame(
  context: BrowserContext,
  page: Page,
  options: OfflineSweepOptions,
): Promise<OfflineSweepReport> {
  const log = options.log;
  const started = Date.now();
  const rng = mulberry32(options.clickSeed);
  mkdirSync(join(options.traceDir, "shots"), { recursive: true });
  const report: OfflineSweepReport = {
    outcome: "setup-failed",
    gameSeed: options.gameSeed,
    clickSeed: options.clickSeed,
    players: options.players,
    policy: options.policy,
    viewport: options.viewport,
    humanSeat: null,
    humanFaction: null,
    factions: [],
    decisions: 0,
    clicks: 0,
    finished: false,
    roundStarts: {},
    subtypes: {},
    rejections: [],
    findings: [],
    panics: [],
    consoleErrors: [],
    pageErrors: [],
    failedRequests: [],
    recoveries: 0,
    overlayDismissals: 0,
    seconds: 0,
    engine: null,
    finalDecisionsHash: null,
    finalStandings: [],
    shots: { taken: 0, cap: options.shotCap, names: [] },
    finalStatus: null,
    engineWaitMs: 0,
    worstWaitMs: 0,
    heapMb: null,
    timing: { scan: 0, click: 0, commit: 0, idle: 0 },
    decisionsPerMinute: 0,
  };

  // --- observers -------------------------------------------------------------------------
  let ask: AskSummary | null = null;
  let lastMessageAt = Date.now();
  let ended: { stamp: number; end: unknown; players?: { id: string; faction: string; vp: number }[] } | null = null;
  let crashed: string | null = null;
  let players: { id: string; faction: string; vp: number }[] = [];
  await page.exposeBinding("__sweepEvent", (_source, name: string, payload: any) => {
    lastMessageAt = Date.now();
    if (payload?.players?.length) players = payload.players;
    if (name === "ask") {
      if (payload.kind === "end") ended = { stamp: payload.stamp, end: payload.end, players: payload.players };
      else if (payload.statusKind === "game_over" && !payload.subtype.startsWith("prompt:") && payload.subtype === "none") {
        // a final state without a question: the end message follows
      }
      for (const event of payload.events ?? []) appendFileSync(join(options.traceDir, "events.jsonl"), `${JSON.stringify(event)}\n`);
      if (payload.kind === "ask") ask = payload as AskSummary;
    } else if (name === "crash" || name === "fatal") {
      crashed = `${name}: ${payload.message}`;
      report.panics.push(crashed);
    }
  });
  await context.addInitScript(sweepInitScript, {
    seed: options.gameSeed,
    autoLone: process.env.TI4_OFFLINE_AUTO_LONE === "0" ? false : process.env.TI4_OFFLINE_AUTO_LONE === "1" ? true : null,
  });
  page.on("pageerror", (error) => {
    const text = `${error.message}`.split("\n")[0].slice(0, 400);
    report.pageErrors.push(text);
    if (/unreachable|RuntimeError|out of memory|memory access/i.test(error.message)) report.panics.push(`pageerror: ${text}`);
    log(`  PAGE ERROR: ${text}`);
  });
  page.on("console", (msg) => {
    if (msg.type() !== "error") return;
    const url = msg.location().url ?? "";
    if (url.includes("favicon") || SERVER_PATH.test(url)) return;
    const text = `${msg.text()}${url ? ` (${url})` : ""}`.slice(0, 500);
    report.consoleErrors.push(text);
    log(`  CONSOLE ERROR: ${text}`);
  });
  page.on("requestfailed", (request) => {
    const url = request.url();
    if (url.includes("favicon") || SERVER_PATH.test(url)) return;
    const line = `${request.method()} ${url} ${request.failure()?.errorText ?? ""}`.slice(0, 300);
    report.failedRequests.push(line);
  });
  page.on("response", (response) => {
    if (response.status() >= 400 && !response.url().includes("favicon") && !SERVER_PATH.test(response.url()))
      report.failedRequests.push(`${response.request().method()} ${response.url()} ${response.status()}`.slice(0, 300));
  });

  const finding = (text: string) => {
    if (!report.findings.includes(text)) {
      report.findings.push(text);
      log(`  FINDING: ${text}`);
    }
  };
  const shot = async (key: string): Promise<string | null> => {
    if (report.shots.taken >= options.shotCap) return null;
    const name = `${key.replace(/[^\w.-]+/g, "_").slice(0, 80)}.png`;
    if (report.shots.names.includes(name)) return name;
    report.shots.taken++;
    report.shots.names.push(name);
    await page.screenshot({ path: join(options.traceDir, "shots", name) }).catch(() => undefined);
    return name;
  };
  const writeTrace = (file: string, data: unknown, append = false) => {
    const path = join(options.traceDir, file);
    if (append) appendFileSync(path, `${JSON.stringify(data)}\n`);
    else writeFileSync(path, typeof data === "string" ? data : JSON.stringify(data, null, 2));
  };
  const readSave = async (): Promise<any> => page.evaluate(() => (window as any).__sweepSave ?? null).catch(() => null);
  const flush = async (outcome: Outcome, failure?: string) => {
    report.outcome = outcome;
    report.finished = outcome === "finished";
    report.seconds = Math.round((Date.now() - started) / 1000);
    report.decisionsPerMinute = report.seconds > 0 ? Math.round((report.decisions * 600) / report.seconds) / 10 : 0;
    report.heapMb = await page
      .evaluate(() => ((performance as any).memory?.usedJSHeapSize ?? 0) / 1048576)
      .then((v) => Math.round(v))
      .catch(() => null);
    const save = await readSave();
    if (save) {
      writeTrace("save.json", save);
      report.finalDecisionsHash = save.decisions_hash ?? null;
      report.engine = { ...(save.engine ?? {}), wasmSha256: await readWasmHash(page) };
      if (!report.humanSeat && typeof save.human_seat === "number") report.humanSeat = save.human_seat + 1;
    }
    report.finalStandings = players.map((p) => `${p.faction} ${p.vp ?? "?"} VP`);
    report.factions = players.map((p) => p.faction);
    report.finalStatus = ask ? { kind: (ask as AskSummary).statusKind, round: (ask as AskSummary).round, phase: (ask as AskSummary).phase } : null;
    if (failure) writeTrace("failure.txt", `${failure}\nreport: ${JSON.stringify({ ...report, subtypes: undefined })}\n`);
    writeTrace("browser-errors.json", [
      ...report.pageErrors.map((e) => `pageerror: ${e}`),
      ...report.consoleErrors.map((e) => `console: ${e}`),
      ...report.panics,
      ...report.failedRequests.map((e) => `request: ${e}`),
    ]);
    writeTrace("report.json", report);
    return report;
  };

  // --- setup -----------------------------------------------------------------------------
  try {
    await page.goto(`${options.baseUrl}/offline`, { waitUntil: "domcontentloaded" });
    await page.getByTestId("offline-nickname").fill("Sweeper");
    await page.getByTestId("offline-player-count").selectOption(String(options.players));
    const seat = options.seat === "random" ? String(1 + Math.floor(rng() * options.players)) : options.seat;
    if (options.seat !== "dealt") await page.getByTestId("offline-seat").selectOption(seat === "0" ? "random" : seat);
    if (options.faction !== "dealt") {
      await page.getByTestId("offline-faction-button").click();
      const ids = await page
        .locator('[data-testid^="offline-faction-option-"]')
        .evaluateAll((els) => els.map((el) => el.getAttribute("data-testid")!.replace("offline-faction-option-", "")));
      const roster = ids.filter((id) => id !== "random");
      const wanted = options.faction === "random" ? roster[Math.floor(rng() * roster.length)] : options.faction;
      report.humanFaction = wanted;
      await page.getByTestId(`offline-faction-option-${wanted}`).click();
    }
    await page.getByTestId("offline-start").click();
    await page.waitForSelector('[data-testid="offline-game"]', { timeout: 90_000 });
  } catch (error) {
    await shot("setup-failed");
    const text = await page.locator("body").innerText().catch(() => "");
    return flush("setup-failed", `setup failed: ${String(error).split("\n")[0]}\nPage text: ${text.slice(0, 800)}`);
  }
  const gameId = /\/offline\/games\/([^/?#]+)/.exec(page.url())?.[1] ?? "unknown";
  writeTrace("game.json", { gameId, options: { ...options, log: undefined } });
  log(`game ${gameId} seed=${options.gameSeed} clickSeed=${options.clickSeed} players=${options.players} policy=${options.policy}`);

  // --- the game ----------------------------------------------------------------------------
  let handledSeq = 0;
  let failure: string | undefined;
  let outcome: Outcome = "decision-cap";
  let recoveries = 0;
  let layoutChecks = 0;
  let nextLayoutCheckAt = 1;
  const stallMs = options.stallSeconds * 1000;
  const deadline = started + options.maxSeconds * 1000;
  const seenContainers = new Set<string>();

  const stuck = async (what: string, current: AskSummary) => {
    const name = await shot(`stuck-d${report.decisions}-${current.subtype}`);
    const visible = await page
      .evaluate((ids) => ids.filter((id) => document.querySelector(`[data-testid="${id}"]`) !== null), DECISION_CONTAINERS)
      .catch(() => [] as string[]);
    const text = await page
      .evaluate(() => (document.querySelector('[data-testid="pending-choice-dialog"], [data-testid="turn-action-bar"]') as HTMLElement | null)?.innerText ?? "")
      .catch(() => "");
    const controls = await page
      .evaluate((ids) => {
        const out: string[] = [];
        for (const id of ids) {
          for (const container of Array.from(document.querySelectorAll(`[data-testid="${id}"]`))) {
            for (const el of Array.from(container.querySelectorAll("button, [role='button'], input")).slice(0, 40)) {
              const rect = el.getBoundingClientRect();
              const x = rect.left + rect.width / 2;
              const y = rect.top + rect.height / 2;
              const top = x >= 0 && y >= 0 && x < window.innerWidth && y < window.innerHeight ? document.elementFromPoint(x, y) : null;
              const hidden = top && !el.contains(top) && !top.contains(el) ? `${top.getAttribute("data-testid") ?? top.tagName.toLowerCase()}.${String(top.className).slice(0, 30)}` : "";
              out.push(
                `${id}>${el.getAttribute("data-testid") ?? ""}[${(el.textContent ?? "").trim().replace(/\s+/g, " ").slice(0, 24)}]` +
                  `${(el as HTMLButtonElement).disabled ? " disabled" : ""}${el.getAttribute("aria-disabled") === "true" ? " aria-disabled" : ""}` +
                  `${rect.width === 0 ? " zero-size" : ""}${hidden ? ` covered-by ${hidden}` : ""}`,
              );
            }
          }
        }
        return out;
      }, DECISION_CONTAINERS)
      .catch(() => [] as string[]);
    finding(
      `stuck: ${what} at decision #${report.decisions}, round ${current.round}, ${current.phase}, subtype ${current.subtype}, prompt "${current.prompt}"; ` +
        `options ${JSON.stringify(current.options.map((o) => o.id)).slice(0, 400)}; containers on screen ${JSON.stringify(visible)}; ` +
        `controls ${JSON.stringify(controls).slice(0, 1500)}; visible text "${text.replace(/\s+/g, " ").slice(0, 300)}"${name ? `; screenshot shots/${name}` : ""}`,
    );
  };

  const layoutCheck = async (current: AskSummary) => {
    layoutChecks++;
    const problems = await page
      .evaluate((ids) => {
        const out: string[] = [];
        const root = document.documentElement;
        if (root.scrollWidth > window.innerWidth + 6) out.push(`page scrolls sideways (${root.scrollWidth}px wide in a ${window.innerWidth}px window)`);
        for (const id of ids) {
          for (const el of Array.from(document.querySelectorAll(`[data-testid="${id}"]`))) {
            const rect = el.getBoundingClientRect();
            if (rect.width === 0 || rect.height === 0) continue;
            if (rect.right > window.innerWidth + 8 || rect.left < -8)
              out.push(`${id} sticks out horizontally (${Math.round(rect.left)}..${Math.round(rect.right)} in a ${window.innerWidth}px window)`);
          }
        }
        return out;
      }, DECISION_CONTAINERS)
      .catch(() => [] as string[]);
    for (const problem of problems) {
      const name = await shot(`layout-d${report.decisions}-${current.subtype}`);
      finding(`layout: ${problem} at decision #${report.decisions}, round ${current.round}, subtype ${current.subtype}${name ? `; screenshot shots/${name}` : ""}`);
    }
  };

  try {
    while (true) {
      if (crashed) {
        outcome = "crash";
        failure = `engine crash: ${crashed}`;
        break;
      }
      if (ended) {
        outcome = "finished";
        break;
      }
      if (report.decisions >= options.maxDecisions) {
        outcome = "decision-cap";
        break;
      }
      if (Date.now() > deadline) {
        outcome = "time-cap";
        break;
      }
      const current = ask as AskSummary | null;
      if (!current || current.seq <= handledSeq) {
        // the engine/bots are thinking, or the game is over
        if (Date.now() - lastMessageAt > stallMs) {
          const last = current ?? ({ subtype: "none", prompt: "", round: null, phase: null, options: [] } as unknown as AskSummary);
          await stuck(`the engine sent nothing for ${options.stallSeconds} s after decision #${report.decisions}`, last);
          if (recoveries < 1) {
            recoveries++;
            report.recoveries = recoveries;
            lastMessageAt = Date.now();
            log("  recovery: reload the page (resume)");
            await page.reload({ waitUntil: "domcontentloaded" }).catch(() => undefined);
            await page.waitForSelector('[data-testid="offline-game"]', { timeout: 120_000 }).catch(() => undefined);
            handledSeq = 0; // the resumed game asks again
            ask = null;
            continue;
          }
          outcome = "stuck";
          failure = `the engine sent nothing for ${options.stallSeconds} s (decision #${report.decisions})`;
          break;
        }
        report.timing.idle += 40;
        await sleep(40);
        continue;
      }

      // ---- one decision -----------------------------------------------------------------
      const waited = Date.now() - lastMessageAt;
      const subtype = current.subtype;
      const hexWeights =
        options.policy === "steer" && subtype === "activate_system" && current.board && current.player
          ? activationWeights(current.board, current.player, false)
          : new Map<string, number>();
      report.subtypes[subtype] = (report.subtypes[subtype] ?? 0) + 1;
      if (current.round !== null && !(current.round in report.roundStarts)) {
        report.roundStarts[current.round] = report.decisions;
        log(`round ${current.round} reached after ${report.decisions} decisions (${Math.round((Date.now() - started) / 1000)} s)`);
        await shot(`round-${current.round}-d${report.decisions}-${subtype}`);
      }
      writeTrace(
        "trace.jsonl",
        {
          decision: report.decisions,
          version: current.version,
          round: current.round,
          phase: current.phase,
          seat: report.humanSeat ?? 1,
          player: current.player,
          faction: players.find((p) => p.id === current.player)?.faction,
          subtype,
          source: current.source,
          prompt: current.prompt,
          options: current.options,
        },
        true,
      );
      const isReaction = isReactionSubtype(subtype);
      let reactionChecked = !isReaction;
      let clicks = 0;
      let emptyPolls = 0;
      let answered = false;
      let reloaded = false;
      const maxClicks = Number(process.env.TI4_OFFLINE_MAX_CLICKS ?? 80);
      const answeredNow = () => ((ask as AskSummary | null)?.seq ?? 0) > current.seq || !!ended || !!crashed;

      while (!answered && !answeredNow()) {
        if (Date.now() > deadline) break;
        if (clicks >= maxClicks || emptyPolls > Math.max(100, (stallMs / 1000) * 5)) {
          await stuck(clicks >= maxClicks ? `${clicks} clicks did not answer the question` : `no actionable control`, current);
          // Recovery 1: Escape and the unstage controls; recovery 2: reload (resume replays the answers).
          if (recoveries < 3) {
            recoveries++;
            report.recoveries = recoveries;
            log(`  recovery ${recoveries}: ${recoveries === 1 ? "escape" : "reload the page (resume)"}`);
            if (recoveries === 1) {
              await page.keyboard.press("Escape").catch(() => undefined);
            } else {
              await page.reload({ waitUntil: "domcontentloaded" }).catch(() => undefined);
              await page.waitForSelector('[data-testid="offline-game"]', { timeout: 120_000 }).catch(() => undefined);
              handledSeq = 0;
              ask = null;
              reloaded = true;
              break;
            }
            clicks = 0;
            emptyPolls = 0;
            continue;
          }
          outcome = "stuck";
          failure = `stuck on ${subtype} at decision #${report.decisions} after ${recoveries} recoveries`;
          break;
        }
        if (!reactionChecked) {
          const shown = await page.getByTestId("reaction-status-bar").first().textContent({ timeout: 300 }).catch(() => null);
          if (shown !== null) {
            reactionChecked = true;
            const problem = reactionTextProblem(shown);
            if (problem) {
              const name = await shot(`reaction-text-d${report.decisions}-${subtype}`);
              finding(`reaction text: ${subtype}: ${problem}${name ? `; screenshot shots/${name}` : ""}`);
            }
          }
        }
        if (report.decisions >= nextLayoutCheckAt && !answeredNow()) {
          nextLayoutCheckAt = report.decisions + 20;
          await layoutCheck(current);
        }
        const scanStart = Date.now();
        const candidates = await collectCandidates(page).catch((err: Error) => {
          if (emptyPolls % 20 === 0) log(`  candidate scan failed: ${err.message.split("\n")[0]}`);
          return [] as Awaited<ReturnType<typeof collectCandidates>>;
        });
        report.timing.scan += Date.now() - scanStart;
        if (!candidates.length) {
          emptyPolls++;
          // A system inspector (opened by tapping a hex) can cover the map and the prompt; closing it is a
          // normal player move. Counted, because on a phone it is how a player gets back to the map.
          if (emptyPolls === 14 || emptyPolls === 30) {
            // Controls of the question that are covered by something else (a toast history, a banner): a real
            // player would close or move the cover; the harness makes the cover click-through, and says so.
            const covers = await unblockCovered(page);
            if (covers.length) {
              const name = await shot(`overlap-d${report.decisions}-${subtype}`);
              for (const cover of covers)
                finding(`overlap: ${cover} at decision #${report.decisions}, round ${current.round}, subtype ${subtype}${name ? `; screenshot shots/${name}` : ""}`);
              emptyPolls = 0;
              continue;
            }
          }
          if (emptyPolls === 8) {
            const close = page.locator('[data-testid="close-inspector-button"]').first();
            if ((await close.count().catch(() => 0)) > 0 && (await close.click({ timeout: 1_000 }).then(() => true).catch(() => false))) {
              report.overlayDismissals++;
              emptyPolls = 0;
              continue;
            }
          }
          await sleep(100);
          continue;
        }
        emptyPolls = 0;
        const chosen = pick(
          preferTechWindowUse(subtype, current.prompt, candidates, 0, true),
          clicks,
          rng,
          options.policy,
          hexWeights,
        );
        log(`#${report.decisions} ${subtype} click ${chosen.desc}`);
        const clickStart = Date.now();
        await page
          .locator(`[data-smoke-idx="${chosen.idx}"]`)
          .click({ timeout: 2_000 })
          .catch((err: Error) => log(`  click failed: ${err.message.split("\n")[0]}`));
        report.timing.click += Date.now() - clickStart;
        clicks++;
        report.clicks++;
        if (chosen.commit) {
          const until = Date.now() + 3_000;
          const commitStart = Date.now();
          while (!answeredNow() && Date.now() < until) await sleep(25);
          report.timing.commit += Date.now() - commitStart;
        } else {
          await sleep(40);
        }
        if (answeredNow()) break;
        for (const error of await visibleErrors(page)) {
          const entry = `${subtype}: ${error}`;
          if (!report.rejections.includes(entry)) {
            report.rejections.push(entry);
            const name = await shot(`error-banner-d${report.decisions}-${subtype}`);
            finding(`error banner: ${entry.slice(0, 300)}${name ? `; screenshot shots/${name}` : ""}`);
          }
        }
      }
      if (outcome === "stuck") break;
      if (reloaded) continue;
      handledSeq = current.seq;
      report.decisions++;
      report.engineWaitMs += waited > 0 && waited < 600_000 ? waited : 0;
      report.worstWaitMs = Math.max(report.worstWaitMs, waited);
      if (report.decisions % 100 === 0)
        log(`  ${report.decisions} decisions, round ${current.round}, ${Math.round((Date.now() - started) / 1000)} s, console errors ${report.consoleErrors.length}`);
    }
  } catch (error) {
    failure = `harness error: ${String(error instanceof Error ? error.stack ?? error.message : error).slice(0, 1500)}`;
    outcome = "stuck";
    await shot("harness-error");
  }

  // --- the end -----------------------------------------------------------------------------
  if (outcome === "finished") {
    const banner = await page
      .getByTestId("turn-status-banner")
      .textContent({ timeout: 10_000 })
      .catch(() => "");
    if (!/Game Over/i.test(banner ?? "")) {
      const name = await shot("no-game-over-banner");
      finding(`the engine ended the game but the status banner says "${(banner ?? "").slice(0, 80)}"${name ? `; screenshot shots/${name}` : ""}`);
    } else {
      await shot("game-over");
    }
  }
  // Leftover visible error banners at the end.
  void ERROR_BANNERS;
  if (crashed && outcome !== "crash") {
    outcome = "crash";
    failure ??= `engine crash: ${crashed}`;
  }
  if (outcome === "crash" || outcome === "stuck") await shot(`final-${outcome}`);
  return flush(outcome, failure);
}

/**
 * Makes whatever covers an enabled control of the pending question click-through. Returns one line per
 * cover ("<control> is covered by <element>"), empty when nothing covers a control.
 */
async function unblockCovered(page: Page): Promise<string[]> {
  return page
    .evaluate((ids) => {
      const out: string[] = [];
      for (const id of ids) {
        for (const container of Array.from(document.querySelectorAll(`[data-testid="${id}"]`))) {
          for (const el of Array.from(container.querySelectorAll("button, [role='button']")).slice(0, 60)) {
            const button = el as HTMLButtonElement;
            if (button.disabled || el.getAttribute("aria-disabled") === "true") continue;
            const rect = el.getBoundingClientRect();
            if (rect.width === 0 || rect.height === 0) continue;
            const x = rect.left + rect.width / 2;
            const y = rect.top + rect.height / 2;
            if (x < 0 || y < 0 || x >= window.innerWidth || y >= window.innerHeight) continue;
            const top = document.elementFromPoint(x, y) as HTMLElement | null;
            if (!top || el.contains(top) || top.contains(el)) continue;
            // the whole cover (its outermost positioned ancestor below the body) stops intercepting
            let cover: HTMLElement = top;
            while (cover.parentElement && cover.parentElement !== document.body && !container.contains(cover.parentElement) && !cover.parentElement.contains(container)) cover = cover.parentElement;
            const label = (e: Element) => `${e.getAttribute("data-testid") ?? e.tagName.toLowerCase()}${e.className ? `.${String(e.className).split(" ")[0]}` : ""}`;
            out.push(`${el.getAttribute("data-testid") ?? (el.textContent ?? "").trim().slice(0, 20)} (${id}) is covered by ${label(top)}${cover !== top ? ` inside ${label(cover)}` : ""}`);
            cover.style.pointerEvents = "none";
            top.style.pointerEvents = "none";
          }
        }
      }
      return Array.from(new Set(out));
    }, DECISION_CONTAINERS)
    .catch(() => [] as string[]);
}

async function readWasmHash(page: Page): Promise<string | undefined> {
  const text = await page.getByTestId("offline-engine").textContent({ timeout: 500 }).catch(() => null);
  return text?.replace(/^engine\s+/, "") ?? undefined;
}
