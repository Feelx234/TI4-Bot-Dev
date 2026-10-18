import {
  findPlanetMeta,
  findPublicObjectiveMeta,
  findSecretObjectiveMeta,
  humanizeId,
} from "../protocol/contentCatalog.ts";
import { AGENDAS } from "../protocol/catalogStore.ts";
import { findStrategyCardMeta } from "../protocol/contentCatalog.ts";
import { agendaNameFromId } from "./agendaStrip.ts";

/** One entry of the engine's `agenda_log`: a finished agenda vote (public, written at resolution). */
export interface AgendaRecordDto {
  round?: number;
  agenda: string;
  name?: string;
  /** `for`, `against`, a seat, a planet, an objective ...; absent when nothing was voted on. */
  outcome?: string | null;
  votes?: Record<string, number>;
  speaker_decided?: boolean;
  law?: boolean;
  enacted?: boolean;
  discarded?: boolean;
  redirected_to?: string | null;
  /** A card revealed and discarded without a vote: `no_choice` (nothing to elect) or `replaced`. */
  discard_reason?: string | null;
}

/** One entry of the state's `agenda_casts`: what one seat cast in the vote now open (public the moment it is cast). */
export interface AgendaCastDto {
  player: string;
  /** The outcome voted for; absent for an abstention. */
  outcome?: string | null;
  /** Total votes from every source (planets, trade goods, cards, Predictive Intelligence ...). */
  votes?: number;
}

export interface AgendaToast {
  id: string;
  /** Small heading; "Agenda" when absent. */
  tag?: string;
  /** One line per fact. */
  text: string;
}

export function readAgendaLog(state: Record<string, unknown> | null | undefined): AgendaRecordDto[] {
  const raw = state?.agenda_log;
  if (!Array.isArray(raw)) return [];
  return raw.filter(
    (r): r is AgendaRecordDto =>
      !!r && typeof r === "object" && typeof (r as AgendaRecordDto).agenda === "string",
  );
}

/** A seat's display name, or `null` when `id` is not a seat. */
export type SeatName = (id: string) => string | null;

/** A seat id as the engine makes them (`player_<64 hex>`); never shown as it is. */
const RAW_PLAYER_ID = /^player_[0-9a-f]{8,}$/i;

/** "For", "Against", a player's name, a planet's name, an objective's, strategy card's or law's name; never a raw id. */
export function agendaOutcomeLabel(outcome: string, seatName: SeatName): string {
  const lower = outcome.toLowerCase();
  if (lower === "for") return "For";
  if (lower === "against") return "Against";
  return (
    seatName(outcome) ??
    findPlanetMeta(outcome)?.name ??
    findSecretObjectiveMeta(outcome)?.name ??
    findPublicObjectiveMeta(outcome)?.name ??
    findStrategyCardMeta(outcome)?.name ??
    AGENDAS[outcome]?.name ??
    (RAW_PLAYER_ID.test(outcome) ? "a player" : humanizeId(outcome))
  );
}

const isForAgainst = (outcome: string) => /^(for|against)$/i.test(outcome);

/** The toast text for one finished agenda: what won, the votes, and what happens to the card. */
export function agendaResultText(record: AgendaRecordDto, seatName: SeatName): string {
  const name = record.name?.trim() || agendaNameFromId(record.agenda);
  const outcome = record.outcome ?? null;
  if (!outcome && record.discard_reason === "no_choice")
    return `${name}\nDiscarded: nothing to elect. The next agenda is revealed.`;
  if (!outcome && record.discard_reason === "replaced")
    return `${name}\nDiscarded and replaced by the next agenda.`;
  if (!outcome) return `${name}\nResolved with no outcome.`;
  const label = agendaOutcomeLabel(outcome, seatName);
  const lines = [`${name}: ${isForAgainst(outcome) ? `${label} wins` : `${label} elected`}`];

  const tally = Object.entries(record.votes ?? {})
    .filter(([, n]) => n > 0)
    .sort((a, b) => b[1] - a[1]);
  const total = tally.reduce((sum, [, n]) => sum + n, 0);
  const counts = tally.map(([o, n]) => `${agendaOutcomeLabel(o, seatName)} ${n}`).join(" · ");
  if (record.speaker_decided) {
    lines.push(total === 0 ? "No votes cast: the speaker decided." : `Tied (${counts}): the speaker decided.`);
  } else if (counts) {
    lines.push(`Votes: ${counts}`);
  }

  if (record.discarded) lines.push("Discarded by Deadly Plot: no effect.");
  else if (record.law) lines.push(record.enacted ? "The law enters play." : "The law is discarded.");
  else lines.push("Directive resolved.");
  if (record.redirected_to && !record.discarded)
    lines.push(`Its effect goes to ${agendaOutcomeLabel(record.redirected_to, seatName)} instead.`);
  return lines.join("\n");
}

/**
 * Toasts for agenda records the viewer has not been told about; `from` is how many were already
 * known. Every seat gets them, the voters included: the outcome is news to the whole table.
 */
export function agendaToasts(
  log: readonly AgendaRecordDto[],
  from: number,
  seatName: SeatName,
): { toasts: AgendaToast[]; next: number } {
  const toasts: AgendaToast[] = [];
  for (let i = Math.min(from, log.length); i < log.length; i++) {
    const record = log[i];
    toasts.push({ id: `agenda:${i}:${record.round ?? 0}:${record.agenda}`, text: agendaResultText(record, seatName) });
  }
  return { toasts, next: log.length };
}

export function readAgendaCasts(state: Record<string, unknown> | null | undefined): AgendaCastDto[] {
  const raw = state?.agenda_casts;
  if (!Array.isArray(raw)) return [];
  return raw.filter(
    (r): r is AgendaCastDto => !!r && typeof r === "object" && typeof (r as AgendaCastDto).player === "string",
  );
}

const votesText = (n: number) => `${n} ${n === 1 ? "vote" : "votes"}`;

/**
 * What one seat cast, as the table reads it: "Alice voted For: 7 votes", "You voted for Carol: 12
 * votes", "Bob abstained". `before` are the casts already made in this vote; with two or more
 * outcomes standing it adds the running total (every cast is public, so is the sum so far).
 */
export function agendaCastText(
  cast: AgendaCastDto,
  before: readonly AgendaCastDto[],
  viewerSeat: string | null | undefined,
  seatName: SeatName,
): string {
  const own = !!viewerSeat && cast.player === viewerSeat;
  const who = own ? "You" : (seatName(cast.player) ?? humanizeId(cast.player));
  const votes = cast.votes ?? 0;
  if (!cast.outcome || votes <= 0) return `${who} abstained`;
  const label = agendaOutcomeLabel(cast.outcome, seatName);
  const lines = [`${who} voted ${isForAgainst(cast.outcome) ? label : `for ${label}`}: ${votesText(votes)}`];
  const totals = new Map<string, number>();
  for (const c of [...before, cast]) {
    if (c.outcome && (c.votes ?? 0) > 0) totals.set(c.outcome, (totals.get(c.outcome) ?? 0) + (c.votes ?? 0));
  }
  if (totals.size >= 2 || (totals.size === 1 && (totals.get(cast.outcome) ?? 0) > votes)) {
    const counts = [...totals.entries()]
      .sort((a, b) => b[1] - a[1])
      .map(([o, n]) => `${agendaOutcomeLabel(o, seatName)} ${n}`)
      .join(" \u00b7 ");
    lines.push(`Running total: ${counts}`);
  }
  return lines.join("\n");
}

/**
 * Toasts for casts the viewer has not been told about; `from` is how many of this vote's casts were
 * already known and `epoch` tells votes apart (ids stay unique across agendas and history).
 */
export function agendaCastToasts(
  casts: readonly AgendaCastDto[],
  from: number,
  epoch: number,
  viewerSeat: string | null | undefined,
  seatName: SeatName,
): { toasts: AgendaToast[]; next: number } {
  const toasts: AgendaToast[] = [];
  for (let i = Math.min(from, casts.length); i < casts.length; i++) {
    toasts.push({
      id: `agenda-cast:${epoch}:${i}:${casts[i].player}`,
      tag: "Vote",
      text: agendaCastText(casts[i], casts.slice(0, i), viewerSeat, seatName),
    });
  }
  return { toasts, next: casts.length };
}
