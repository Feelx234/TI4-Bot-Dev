import type { BoardView, ChoiceOption, PendingChoiceDto } from "../protocol/types.ts";
import { isDeclineOption } from "./choiceModel.ts";
import { describeSystem, summarizeUnits, systemNamedLabel, systemTag } from "./systemFacts.ts";

/**
 * A retreat-like move whose destination is chosen among adjacent systems: the combat retreat
 * ("retreat to which system"), Skilled Retreat, Naalu's Foresight and the Ral Nel commander.
 * The ships and the system they leave go into the heading; the destinations are picked on the
 * map (the from system stands out), with the list as the fallback.
 */
export type RetreatPickKind = "combat" | "skilled" | "foresight" | "ralnel";

export interface RetreatDestination {
  optionId: string;
  systemId: string;
  /** "Lodor (System 26)". */
  name: string;
  /** What the engine's offer tells about the system: hazards, own ships and planets there. */
  notes: string[];
}

export interface RetreatPick {
  kind: RetreatPickKind;
  /** The system the ships leave; null when the decision does not say. */
  fromId: string | null;
  fromName: string | null;
  /** "Retreat 2 cruisers from Mecatol Rex (System 18)". */
  title: string;
  /** "2 cruisers, 8 fighters": the viewer's units in the space area of the from system. */
  fleet: string;
  fleetCount: number;
  destinations: RetreatDestination[];
  /** The legality the engine applied, in one sentence. */
  rule: string;
  declineOptionId: string | null;
}

const KIND_BY_SUBTYPE: Record<string, RetreatPickKind> = {
  retreat_to: "combat",
  skilled_retreat_choose_system: "skilled",
  foresight_destination: "foresight",
  ralnel_retreat_destination: "ralnel",
};

const RULES: Record<RetreatPickKind, string> = {
  combat:
    "Adjacent systems that hold your units or a planet you control and no other player's ships, with no asteroid field (without Antimass Deflectors) and no supernova.",
  skilled: "Adjacent systems the engine accepts for the withdrawal.",
  foresight:
    "Adjacent systems with no other player's ships and none of your command tokens; costs a strategy token.",
  ralnel: "Adjacent systems the engine accepts for the retreat; up to 2 ships go.",
};

/** The system an option of a retreat pick stands for. */
export function retreatOptionSystem(opt: ChoiceOption): string | null {
  if (isDeclineOption(opt)) return null;
  const p = opt.payload?.system;
  if (typeof p === "string" && p) return p;
  if (typeof p === "number") return String(p);
  // Foresight offers "system|<id>", the Ral Nel commander "to|<id>".
  const prefixed = /^(?:system|to)\|(.+)$/.exec(opt.id);
  return prefixed ? prefixed[1] : opt.id;
}

export function isRetreatPickChoice(choice: PendingChoiceDto | null | undefined): boolean {
  return Boolean(choice && KIND_BY_SUBTYPE[choice.context?.subtype ?? ""]);
}

function fromSystem(choice: PendingChoiceDto): string | null {
  const target = choice.context?.target;
  if (target && "System" in target && typeof target.System === "string") return target.System;
  // Older Foresight questions carried no target: "move your ships from 18 to 26".
  for (const opt of choice.options) {
    const m = /\bfrom (\S+) to\b/.exec(opt.label);
    if (m) return m[1];
  }
  return null;
}

function notesFor(systemId: string, seat: string, board?: BoardView | null): string[] {
  const notes: string[] = [];
  const tile = board?.map_tiles?.find((t) => t.system_id === systemId);
  for (const anomaly of tile?.anomalies ?? []) notes.push(anomaly.replaceAll("_", " "));
  const facts = describeSystem(systemId, board);
  const own = (board?.systems?.[systemId]?.units ?? []).filter((u) => u.owner === seat && !u.planet);
  if (own.length > 0) notes.push(`your ships there: ${summarizeUnits(own)}`);
  const held = facts.planets.filter((p) => p.controlledBy === seat).map((p) => p.name);
  if (held.length > 0) notes.push(`you control ${held.join(", ")}`);
  return notes;
}

/** Null when the decision is not a retreat-like destination pick. */
export function describeRetreatPick(
  choice: PendingChoiceDto | null | undefined,
  board?: BoardView | null,
): RetreatPick | null {
  if (!choice) return null;
  const kind = KIND_BY_SUBTYPE[choice.context?.subtype ?? ""];
  if (!kind) return null;
  const fromId = fromSystem(choice);
  const fromName = fromId ? systemNamedLabel(fromId, board) : null;
  const units = fromId
    ? (board?.systems?.[fromId]?.units ?? []).filter((u) => u.owner === choice.actor && !u.planet)
    : [];
  const fleet = summarizeUnits(units);
  const kinds = new Set(units.map((u) => u.unit_type)).size;
  // A long mixed fleet reads as a count in the heading; the full list stays under it.
  const shipsInTitle =
    units.length === 0 ? "your ships" : kinds <= 2 ? fleet : `your fleet (${units.length} units)`;
  const where = fromName ? ` from ${fromName}` : "";
  const title =
    kind === "foresight"
      ? `Foresight: move ${shipsInTitle}${where}`
      : kind === "skilled"
        ? `Skilled Retreat: withdraw ${shipsInTitle}${where}`
        : kind === "ralnel"
          ? `Ral Nel commander: retreat up to 2 ships${where}`
          : `Retreat ${shipsInTitle}${where}`;
  const destinations: RetreatDestination[] = [];
  let declineOptionId: string | null = null;
  for (const opt of choice.options) {
    if (isDeclineOption(opt)) {
      declineOptionId = opt.id;
      continue;
    }
    const systemId = retreatOptionSystem(opt);
    if (!systemId) continue;
    destinations.push({
      optionId: opt.id,
      systemId,
      name: systemNamedLabel(systemId, board),
      notes: notesFor(systemId, choice.actor, board),
    });
  }
  return {
    kind,
    fromId,
    fromName,
    title,
    fleet,
    fleetCount: units.length,
    destinations,
    rule: RULES[kind],
    declineOptionId,
  };
}

/** "#26 [Lodor]": the short form for chips. */
export const retreatChipLabel = (systemId: string, board?: BoardView | null) => systemTag(systemId, board);

/**
 * The question "announce a retreat?" (stay or retreat; the destination comes next): its heading
 * names the fleet and the system. Null for any other decision.
 */
export function describeRetreatAnnouncement(
  choice: PendingChoiceDto | null | undefined,
  board?: BoardView | null,
): { title: string; fleet: string } | null {
  if (!choice || choice.context?.subtype !== "announce_retreat") return null;
  const fromId = fromSystem(choice);
  const units = fromId
    ? (board?.systems?.[fromId]?.units ?? []).filter((u) => u.owner === choice.actor && !u.planet)
    : [];
  return {
    title: `Announce a retreat${fromId ? ` from ${systemNamedLabel(fromId, board)}` : ""}?`,
    fleet: summarizeUnits(units),
  };
}
