import type { Browser } from "@playwright/test";
import { measureMap, type OverlayMetrics } from "./measure";
import { extras, openExtra } from "./extras";
import { minimize, openCase, SIZES, type SizeName } from "./scene";

/** One row of the audit: an overlay, how it is reached, and what it does to the map. */
export interface AuditScene {
  overlay: string;
  trigger: string;
  /** A dev-gallery decision (title) or an extras scene (id). */
  gallery?: string;
  extra?: string;
}

export const AUDIT: AuditScene[] = [
  { overlay: "System activation bar", trigger: "decision: choose a system to activate", gallery: "system activation" },
  { overlay: "Planet selection bar", trigger: "decision: pick a planet", gallery: "planet selection" },
  { overlay: "Tactical movement tray", trigger: "decision: move ships", gallery: "tactical movement" },
  { overlay: "Cargo loading tray", trigger: "decision: load cargo", gallery: "tactical cargo" },
  { overlay: "Invasion landing tray", trigger: "decision: land ground forces", extra: "invasion-landing" },
  { overlay: "Ground combat / invasion overlay", trigger: "invasion result", extra: "ground-combat-result" },
  { overlay: "Payment drawer", trigger: "decision: pay resources", gallery: "payment" },
  { overlay: "Production builder", trigger: "decision: produce units", gallery: "production" },
  { overlay: "Space combat: hits / casualties", trigger: "decision: assign hits", gallery: "combat casualty" },
  { overlay: "Space combat: retreat", trigger: "decision: retreat", gallery: "combat retreat" },
  { overlay: "Space combat result", trigger: "a finished battle", extra: "combat-result" },
  { overlay: "Agenda ballot", trigger: "decision: vote", gallery: "agenda vote planets" },
  { overlay: "Trade desk", trigger: "decision: propose a trade", gallery: "transaction propose" },
  { overlay: "Reaction window", trigger: "decision: respond to an action card", gallery: "Reaction: Sabotage" },
  { overlay: "Objective scoring", trigger: "decision: score an objective", gallery: "objective scoring" },
  { overlay: "Strategy card draft / secondary", trigger: "decision: pick a strategy card", gallery: "strategy card draft" },
  { overlay: "Generic choice dialog", trigger: "decision: any other choice", gallery: "generic selection" },
  { overlay: "Remove-unit / over capacity", trigger: "decision: remove a unit", gallery: "Remove a unit: over capacity" },
  { overlay: "Command token redistribution", trigger: "decision: redistribute tokens", gallery: "Redistribute command tokens" },
  { overlay: "Technology research", trigger: "decision: research a technology", gallery: "technology research" },
  { overlay: "Turn action bar", trigger: "your turn menu", gallery: "Turn bar: your turn, one strategy card" },
  { overlay: "Objectives modal", trigger: "header: Objectives", extra: "objectives" },
  { overlay: "Technologies modal", trigger: "header: Technologies", extra: "technologies" },
  { overlay: "Event log drawer", trigger: "Events button", extra: "event-log" },
  { overlay: "Player sheet drawer", trigger: "Players button", extra: "player-sheet" },
  { overlay: "System detail panel", trigger: "tap a system (decision open)", extra: "system-inspector" },
  { overlay: "Card detail panel", trigger: "tap a card in the player sheet", extra: "card-details" },
  { overlay: "System tooltip", trigger: "touch on a system", extra: "tile-tooltip" },
  { overlay: "Secondary prep chip", trigger: "another seat resolves a strategy card", extra: "prep-chip" },
  { overlay: "Secondary prepare banner", trigger: "chip, then the question", extra: "prep-banner" },
  { overlay: "Undo confirmation", trigger: "event log: Undo", extra: "undo-confirm" },
  { overlay: "Error toast", trigger: "a refused history change", extra: "history-error" },
  { overlay: "Corner toast", trigger: "another seat acts", extra: "corner-toast" },
  { overlay: "Waiting banner / game over", trigger: "no decision; the game ends", extra: "game-over" },
];

export interface AuditRow extends AuditScene {
  size: SizeName;
  open: OverlayMetrics;
  minimized?: OverlayMetrics;
}

/** Opens every audit scene in a fresh context at `size` and measures the map before and after minimizing. */
export async function runAudit(browser: Browser, size: SizeName, only?: (scene: AuditScene) => boolean): Promise<AuditRow[]> {
  const rows: AuditRow[] = [];
  let exclude: number[] = [];
  const fresh = async () => {
    const context = await browser.newContext({ viewport: SIZES[size], colorScheme: "dark" });
    return { context, page: await context.newPage() };
  };
  {
    // The map's own toolbar and legend, with nothing open, are not counted as covering.
    const { context, page } = await fresh();
    await openExtra(page, "waiting-other", size);
    exclude = (await measureMap(page)).chrome;
    await context.close();
  }
  for (const scene of AUDIT) {
    if (only && !only(scene)) continue;
    const { context, page } = await fresh();
    try {
      if (scene.gallery) await openCase(page, scene.gallery, size);
      else await openExtra(page, scene.extra!, size);
      const open = await measureMap(page, exclude);
      const row: AuditRow = { ...scene, size, open };
      if (open.covered > 0.15 && (await minimize(page, open.minimizeControl))) row.minimized = await measureMap(page, exclude);
      rows.push(row);
    } catch (error) {
      console.log(`AUDIT-ERR ${scene.overlay} ${size}: ${String(error).slice(0, 160)}`);
    } finally {
      await context.close();
    }
  }
  return rows;
}

export const extraIds = extras.map((e) => e.id);
