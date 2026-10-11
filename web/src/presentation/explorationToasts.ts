import { findAttachmentMeta, findExplorationCardMeta, findPlanetMeta, humanizeId } from "../protocol/contentCatalog.ts";
import type { ActionToast } from "./actionToasts.ts";

/** One entry of the engine's `exploration_log` (public: exploration cards are turned face up). */
export interface ExplorationRecordDto {
  round?: number;
  player: string;
  deck: string;
  card: string;
  planet?: string | null;
  outcome?: string;
  system?: string | null;
  result?: string;
}

/** Cards that cost an infantry: declining leaves nothing gained. */
const SACRIFICE_CARDS = /^(cm|exp|vfs)\d$/;

export function readExplorationLog(state: Record<string, unknown> | null | undefined): ExplorationRecordDto[] {
  const raw = state?.exploration_log;
  if (!Array.isArray(raw)) return [];
  return raw.filter(
    (r): r is ExplorationRecordDto =>
      !!r && typeof r === "object" && typeof (r as ExplorationRecordDto).player === "string",
  );
}

/** "attached Mining World" / "took a cultural relic fragment" / "gained 1 trade good" ... */
export function explorationResultText(record: ExplorationRecordDto): string {
  const outcome = record.outcome ?? "";
  const result = (record.result ?? "").trim();
  if (outcome.startsWith("attached:")) {
    const id = outcome.slice("attached:".length);
    return `attached ${findAttachmentMeta(id)?.name ?? humanizeId(id)}`;
  }
  if (result) return result;
  if (SACRIFICE_CARDS.test(record.card)) return "no infantry removed, nothing gained";
  return "nothing happened";
}

/** The one-line summary of a finished exploration, without the actor's name. */
export function explorationSummary(record: ExplorationRecordDto): string {
  const card = findExplorationCardMeta(record.card)?.name ?? humanizeId(record.card);
  const trait = (record.deck || "").toLowerCase();
  const where = record.planet
    ? `${findPlanetMeta(record.planet)?.name ?? humanizeId(record.planet)} (${trait})`
    : record.system
      ? `frontier token in system ${record.system}`
      : "a frontier token";
  return `explored ${where}: ${card} - ${explorationResultText(record)}`;
}

/**
 * Toasts for exploration records the viewer has not been told about. `from` is how many records
 * were already known; records still without an outcome (a card waiting for a decision) are not
 * reported yet and do not advance the cursor. Returns the toasts and the new cursor.
 */
export function explorationToasts(
  log: readonly ExplorationRecordDto[],
  from: number,
): { toasts: ActionToast[]; next: number } {
  const toasts: ActionToast[] = [];
  let next = Math.min(from, log.length);
  for (let i = next; i < log.length; i++) {
    const record = log[i];
    if (!record.outcome) break;
    toasts.push({
      id: `explore:${i}:${record.player}:${record.card}`,
      actor: record.player,
      category: "explore",
      parts: [explorationSummary(record)],
      count: 1,
    });
    next = i + 1;
  }
  return { toasts, next };
}
