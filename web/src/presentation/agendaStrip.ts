import type { PendingChoiceDto } from "../protocol/types.ts";

/**
 * What the agenda phase is about, for a strip that stays on screen for the whole phase. All of it is
 * display only. The agenda comes from the decision (`details.agenda`, which vote decisions carry
 * from servers that send it; the reaction trigger's agenda id), and is remembered for the rest of the
 * phase so waiting states and decisions without it (a rider's prediction) still name it.
 */

export interface AgendaInfo {
  id: string | null;
  name: string | null;
  type: string | null;
  target: string | null;
  /** The printed text, one paragraph per entry (the outcomes of a vote are in it). */
  text: string[];
}

export type AgendaKind = "for_against" | "elect";

export const AGENDAS_PER_PHASE = 2;

const SMALL_WORDS = new Set(["of", "the", "a", "an", "and", "for", "to", "in", "on"]);

/** An agenda alias as a card name: "minister_of_peace" -> "Minister of Peace". */
export function agendaNameFromId(id: string): string {
  return id
    .split("_")
    .filter(Boolean)
    .map((word, index) =>
      index > 0 && SMALL_WORDS.has(word) ? word : `${word.charAt(0).toUpperCase()}${word.slice(1)}`,
    )
    .join(" ");
}

const record = (value: unknown): Record<string, unknown> | null =>
  value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : null;
const str = (value: unknown): string | null =>
  typeof value === "string" && value.trim() ? value.trim() : null;

/** Subtypes of the decisions an agenda raises (votes, predictions, tie-break, the voter's tribute). */
const VOTE_SUBTYPES = new Set(["cast_vote", "vote_exhaust_planet", "vote_spend_trade_goods", "vote_predictive_intelligence", "recombination_tribute"]);
const AGENDA_SUBTYPES = new Set([...VOTE_SUBTYPES, "vote_tiebreak", "predict_agenda_outcome"]);

export const isAgendaDecision = (choice: Pick<PendingChoiceDto, "context"> | null | undefined): boolean => {
  const subtype = choice?.context?.subtype ?? "";
  if (AGENDA_SUBTYPES.has(subtype)) return true;
  const trigger = choice?.context?.trigger;
  return Boolean(trigger && (trigger.kind === "agenda_revealed" || trigger.kind === "votes_cast" || trigger.kind === "agenda_resolved"));
};

export const isAgendaPhase = (phase: string | null | undefined): boolean =>
  (phase ?? "").toLowerCase() === "agenda";

/** The agenda a decision names, or `null`. */
export function agendaInfoFromChoice(
  choice: Pick<PendingChoiceDto, "context" | "details"> | null | undefined,
): AgendaInfo | null {
  if (!choice) return null;
  const printed = record(choice.details?.agenda);
  if (printed) {
    const id = str(printed.id);
    const name = str(printed.name) ?? (id ? agendaNameFromId(id) : null);
    if (name || id) {
      return {
        id,
        name,
        type: str(printed.type),
        target: str(printed.target),
        text: [str(printed.text1), str(printed.text2)].filter((line): line is string => line !== null),
      };
    }
  }
  const card = record(choice.details?.agenda_card);
  if (card && str(card.name)) {
    return {
      id: null,
      name: str(card.name),
      type: null,
      target: null,
      text: [str(card.yes_outcome) && `For: ${str(card.yes_outcome)}`, str(card.no_outcome) && `Against: ${str(card.no_outcome)}`].filter(
        (line): line is string => typeof line === "string",
      ),
    };
  }
  const triggered = str(choice.context?.trigger?.agenda);
  if (triggered) return { id: triggered, name: agendaNameFromId(triggered), type: null, target: null, text: [] };
  return null;
}


/**
 * What the game state says about the agenda being decided, public to every seat and spectator the
 * moment the agenda is revealed: `agenda_in_play` (its alias, until the vote closes), the outcomes
 * on offer, what each seat cast so far, the riders' predictions and the finished votes of the log.
 * All of it is read-only display data; absent fields (older servers) read as empty.
 */
export interface AgendaTable {
  /** The alias of the agenda being voted on, or `null` between agendas. */
  inPlay: string | null;
  /** The outcomes on offer (`FOR`, `AGAINST`, or the candidates). */
  choices: string[];
  casts: { player: string; outcome: string | null; votes: number }[];
  /** Imperial Rider and the like: seat to the outcome it predicted. */
  predictions: { player: string; outcome: string }[];
  /** The finished agendas of the log: the round and alias of each. */
  log: { round: number | null; agenda: string }[];
}

export const EMPTY_AGENDA_TABLE: AgendaTable = { inPlay: null, choices: [], casts: [], predictions: [], log: [] };

export function readAgendaTable(state: Record<string, unknown> | null | undefined): AgendaTable {
  if (!state) return EMPTY_AGENDA_TABLE;
  const list = (value: unknown): Record<string, unknown>[] =>
    Array.isArray(value) ? value.filter((v): v is Record<string, unknown> => !!record(v)) : [];
  const predictions = record(state.agenda_predictions);
  return {
    inPlay: str(state.agenda_in_play),
    choices: Array.isArray(state.agenda_choices) ? state.agenda_choices.filter((c): c is string => typeof c === "string") : [],
    casts: list(state.agenda_casts)
      .filter((c) => typeof c.player === "string")
      .map((c) => ({ player: c.player as string, outcome: str(c.outcome), votes: typeof c.votes === "number" ? c.votes : 0 })),
    predictions: predictions
      ? Object.entries(predictions)
          .filter((entry): entry is [string, string] => typeof entry[1] === "string")
          .map(([player, outcome]) => ({ player, outcome }))
      : [],
    log: list(state.agenda_log)
      .filter((r) => typeof r.agenda === "string")
      .map((r) => ({ round: typeof r.round === "number" ? r.round : null, agenda: r.agenda as string })),
  };
}

/** A printed agenda as the catalog holds it (`Catalog.components.agendas`). */
export interface PrintedAgenda {
  id: string;
  name: string;
  kind: string;
  elect?: string;
  text: string;
  against?: string;
}

/** The agenda in play as a card: the catalog's printed text when it knows the alias, else just the name. */
export function agendaInfoFromTable(
  table: AgendaTable,
  lookup: (alias: string) => PrintedAgenda | undefined,
): AgendaInfo | null {
  const id = table.inPlay;
  if (!id) return null;
  const def = lookup(id);
  const elect = def?.elect?.trim() || null;
  // The catalog prints the side labels itself ("For: ...", "Against: ..."); an election has one text.
  const text = [def?.text, def?.against].map((line) => line?.trim() ?? "").filter(Boolean);
  return { id, name: def?.name || agendaNameFromId(id), type: def?.kind ?? null, target: elect, text };
}

/** The agenda has just been resolved and the next one is not revealed yet (the log has it, nothing is in play). */
export function agendaJustResolved(table: AgendaTable, round: number | null | undefined, info: AgendaInfo | null): boolean {
  if (table.inPlay) return false;
  const thisRound = table.log.filter((r) => r.round === null || round == null || r.round === round);
  if (thisRound.length === 0) return false;
  // Nothing known about a current agenda (the page loaded after it resolved), or the one known is in the log.
  return !info?.id || thisRound.some((r) => r.agenda === info.id);
}

const sameAgenda = (a: AgendaInfo, b: AgendaInfo): boolean =>
  a.id !== null && b.id !== null ? a.id === b.id : a.name !== null && a.name === b.name;

/** What the decision and the game state each say about the agenda: the decision's printed card wins, the state fills the gaps. */
export function mergeAgendaInfo(fromChoice: AgendaInfo | null, fromState: AgendaInfo | null): AgendaInfo | null {
  if (fromChoice && fromState && sameAgenda(fromChoice, fromState)) return richer(fromState, fromChoice);
  return fromState ?? fromChoice;
}

const richer = (known: AgendaInfo, update: AgendaInfo): AgendaInfo => ({
  id: update.id ?? known.id,
  name: update.name ?? known.name,
  type: update.type ?? known.type,
  target: update.target ?? known.target,
  text: update.text.length ? update.text : known.text,
});

export interface AgendaMemory {
  round: number | null;
  /** The agendas seen so far in this phase, in order; the last one is the current one. */
  agendas: AgendaInfo[];
}

export const EMPTY_AGENDA_MEMORY: AgendaMemory = { round: null, agendas: [] };

/**
 * Folds what the decision says into the phase's memory. Outside the agenda phase it forgets
 * everything (the strip clears when the phase ends); a new round starts over; a decision that names
 * another agenda than the last one opens the next agenda.
 */
export function advanceAgendaMemory(
  memory: AgendaMemory,
  input: { phase: string | null | undefined; round: number | null | undefined; info: AgendaInfo | null; decision?: boolean },
): AgendaMemory {
  if (!isAgendaPhase(input.phase) && !input.decision) {
    return memory.agendas.length || memory.round !== null ? EMPTY_AGENDA_MEMORY : memory;
  }
  const round = input.round ?? null;
  const base = memory.round === round ? memory : { round, agendas: [] as AgendaInfo[] };
  const info = input.info;
  if (!info) return base === memory ? memory : base;
  const last = base.agendas[base.agendas.length - 1];
  if (last && sameAgenda(last, info)) {
    const merged = richer(last, info);
    if (JSON.stringify(merged) === JSON.stringify(last) && base === memory) return memory;
    return { round, agendas: [...base.agendas.slice(0, -1), merged] };
  }
  return { round, agendas: [...base.agendas, info] };
}

/** `for_against` or `elect`, from the printed target and, failing that, the outcomes on offer. */
export function agendaKind(info: AgendaInfo | null, choice: Pick<PendingChoiceDto, "options" | "context"> | null): AgendaKind | null {
  const target = info?.target?.toLowerCase() ?? "";
  if (target.startsWith("for")) return "for_against";
  if (target.startsWith("elect") || target === "player" || target === "planet") return "elect";
  const outcomes = (choice?.options ?? []).filter((o) => o.kind !== "decline" && o.id !== "decline");
  if (outcomes.length === 0) return null;
  const ids = outcomes.map((o) => o.id.toUpperCase());
  if (ids.every((id) => id === "FOR" || id === "AGAINST")) return "for_against";
  const subtype = choice?.context?.subtype ?? "";
  return subtype === "cast_vote" || subtype === "vote_tiebreak" ? "elect" : null;
}

/** "For/Against", "Elect Player", "Elect Planet" ... as short as the card prints it. */
function kindLabel(kind: AgendaKind | null, info: AgendaInfo | null): string | null {
  if (kind === "for_against") return "For/Against";
  if (kind === "elect") {
    const target = info?.target?.trim() ?? "";
    const what = /^elect\s+(\S+(?:\s+\S+)?)/i.exec(target)?.[1] ?? (/^(player|planet)$/i.test(target) ? target : null);
    const word = what?.split(/\s+/)[0];
    return word ? `Elect ${word.charAt(0).toUpperCase()}${word.slice(1)}` : "Elect";
  }
  return null;
}

export type TurnRole = "vote" | "tiebreak" | "predict" | "reaction" | "other";

export interface AgendaWhose {
  seat: string;
  label: string;
  you: boolean;
  role: TurnRole;
  /** "Your vote", "Alice's vote", "Speaker breaks the tie" ... */
  text: string;
}

export interface AgendaStrip {
  /** 1-based position among this phase's agendas, when it is known. */
  number: number | null;
  name: string | null;
  kind: AgendaKind | null;
  kindLabel: string | null;
  type: string | null;
  text: string[];
  /** The outcomes on offer (FOR / AGAINST, or the candidates). */
  outcomes: string[];
  whose: AgendaWhose | null;
  speaker: string | null;
  /** "FOR 3 · AGAINST 1", when the votes cast so far are public (the decision's counts, or the state's casts). */
  tally: string | null;
  /** Who has voted so far, in voting order, from the state's `agenda_casts`. */
  casts: AgendaStripCast[];
  /** Seats still to vote, in voting order (speaker last). Empty when unknown. */
  waiting: string[];
  /** Riders and predictions that are on the table. */
  riders: { label: string; outcome: string }[];
}

export interface AgendaStripCast {
  label: string;
  you: boolean;
  /** The outcome as shown ("For", a name); `null` for an abstention. */
  outcome: string | null;
  votes: number;
  badge?: { color: string; symbol: string } | null;
}

const roleOf = (subtype: string, choice: PendingChoiceDto): TurnRole => {
  if (VOTE_SUBTYPES.has(subtype)) return "vote";
  if (subtype === "vote_tiebreak") return "tiebreak";
  if (subtype === "predict_agenda_outcome") return "predict";
  if (choice.context?.trigger || /^(play_)?reaction_/.test(subtype)) return "reaction";
  return "other";
};

function whoseText(role: TurnRole, you: boolean, label: string): string {
  switch (role) {
    case "vote":
      return you ? "Your vote" : `${label}'s vote`;
    case "tiebreak":
      return you ? "You break the tie" : `${label} breaks the tie`;
    case "predict":
      return you ? "Your prediction" : `${label} predicts`;
    case "reaction":
      return you ? "Your reaction" : `Reactions: ${label}`;
    case "other":
      return you ? "Your move" : `${label}'s move`;
  }
}

export interface DeriveInput {
  phase: string | null | undefined;
  choice: PendingChoiceDto | null | undefined;
  memory: AgendaMemory;
  viewerSeat: string | null | undefined;
  speaker?: string | null;
  /** Seat id to the name shown for it. */
  label: (seat: string) => string;
  /** True while the server waits for reactions that name no seat. */
  waitingForReactions?: boolean;
  /** The public agenda data of the game state; see [`readAgendaTable`]. */
  table?: AgendaTable;
  /** Seats in table order, to work out who votes after whom. */
  seatingOrder?: readonly string[];
  /** How an outcome (`FOR`, a seat, a planet) reads. */
  outcomeLabel?: (outcome: string) => string;
  /** A seat's colour and symbol, the badge shown beside its name. */
  seatBadge?: (seat: string) => { color: string; symbol: string } | null;
  round?: number | null;
}

/**
 * The strip for the current moment, or `null` when there is no agenda going on (the phase is not
 * Agenda and the decision is not an agenda decision).
 */
export function deriveAgendaStrip(input: DeriveInput): AgendaStrip | null {
  const choice = input.choice ?? null;
  const decision = isAgendaDecision(choice);
  if (!isAgendaPhase(input.phase) && !decision) return null;
  const table = input.table ?? EMPTY_AGENDA_TABLE;
  const current = agendaInfoFromChoice(choice) ?? input.memory.agendas[input.memory.agendas.length - 1] ?? null;
  const known = choice ? agendaInfoFromChoice(choice) : null;
  const info = known && current ? richer(current, known) : current;
  // Resolved and nothing else going on: the agenda is decided, the banner is gone until the next reveal.
  if (!decision && agendaJustResolved(table, input.round, info)) return null;
  const inRound = table.log.filter((r) => r.round === null || input.round == null || r.round === input.round).length;
  const index = info ? input.memory.agendas.findIndex((agenda) => sameAgenda(agenda, info)) : -1;
  const number = table.inPlay && table.log.length ? inRound + 1 : index >= 0 ? index + 1 : null;
  const kind = agendaKind(info, decision ? choice : null);
  const subtype = choice?.context?.subtype ?? "";
  let whose: AgendaWhose | null = null;
  if (choice && (decision || isAgendaPhase(input.phase))) {
    const role = roleOf(subtype, choice);
    const seat = role === "tiebreak" ? (input.speaker ?? choice.actor) : choice.actor;
    const you = Boolean(input.viewerSeat) && seat === input.viewerSeat;
    const label = input.label(seat);
    whose = { seat, label, you, role, text: whoseText(role, you, label) };
  } else if (input.waitingForReactions) {
    whose = { seat: "", label: "", you: false, role: "reaction", text: "Reactions" };
  }
  // Only a vote, a tie-break or a prediction offers outcomes; a reaction window offers cards.
  const offersOutcomes = decision && choice !== null && ["vote", "tiebreak", "predict"].includes(roleOf(subtype, choice));
  const outcomes = (offersOutcomes ? (choice?.options ?? []) : [])
    .filter((o) => o.kind !== "decline" && o.id !== "decline")
    .map((o) => optionName(o, input));
  const tallies =
    subtype === "cast_vote"
      ? (choice?.options ?? [])
          .filter((o) => o.kind !== "decline" && o.id !== "decline" && typeof o.payload?.current_votes === "number")
          .map((o) => `${optionName(o, input)} ${o.payload?.current_votes as number}`)
      : [];
  return {
    number,
    name: info?.name ?? null,
    kind,
    kindLabel: kindLabel(kind, info),
    type: info?.type ?? null,
    text: info?.text ?? [],
    outcomes: subtype === "vote_exhaust_planet" ? [] : outcomes,
    whose,
    speaker: input.speaker ? input.label(input.speaker) : null,
    tally: tallies.length ? tallies.join(" · ") : stateTally(table, input),
    ...castsAndRiders(table, input),
  };
}

/** The one line of the strip: "Agenda 1/2: Minister of Peace · For/Against · Your vote". */
export function agendaStripText(strip: AgendaStrip): string {
  const head = strip.number ? `Agenda ${strip.number}/${AGENDAS_PER_PHASE}` : "Agenda";
  const parts = [strip.name ?? (strip.kindLabel ? null : "being voted on"), strip.kindLabel, strip.whose?.text].filter(
    (part): part is string => Boolean(part),
  );
  return parts.length ? `${head}: ${parts.join(" · ")}` : head;
}

/** What the minimized decision pill says: "Voting: Minister of Peace — For/Against". */
export function agendaPillLabel(strip: AgendaStrip, choice: Pick<PendingChoiceDto, "context"> | null): string | null {
  if (!choice || !isAgendaDecision(choice)) return null;
  const role = strip.whose?.role;
  const verb =
    role === "vote" ? "Voting" : role === "tiebreak" ? "Tie-break" : role === "predict" ? "Predict" : "Agenda";
  const name = strip.name ?? "agenda";
  return strip.kindLabel ? `${verb}: ${name} — ${strip.kindLabel}` : `${verb}: ${name}`;
}

const SIDE = /^(for|against)$/i;

/** An option of a vote or prediction as a person reads it: planets and players by name, never by id. */
function optionName(o: PendingChoiceDto["options"][number], input: DeriveInput): string {
  if (typeof o.payload?.planet_name === "string") return o.payload.planet_name;
  const label = o.label.replace(/^predict\s+/i, "");
  if (SIDE.test(o.id) || SIDE.test(label)) return label;
  return input.outcomeLabel ? input.outcomeLabel(o.id) : label;
}

const outcomeText = (input: DeriveInput, outcome: string): string =>
  input.outcomeLabel ? input.outcomeLabel(outcome) : outcome;

/** The running count per outcome from the public casts: "For 7 · Against 3". */
function stateTally(table: AgendaTable, input: DeriveInput): string | null {
  if (!table.inPlay || table.casts.length === 0) return null;
  const sums = new Map<string, number>();
  for (const outcome of table.choices) sums.set(outcome, 0);
  for (const cast of table.casts) if (cast.outcome) sums.set(cast.outcome, (sums.get(cast.outcome) ?? 0) + cast.votes);
  const entries = [...sums].filter(([, n]) => n > 0 || table.choices.length <= 3);
  return entries.length ? entries.map(([o, n]) => `${outcomeText(input, o)} ${n}`).join(" · ") : null;
}

/** Seats in voting order: the seat after the speaker first, the speaker last. */
export function votingOrder(seating: readonly string[], speaker: string | null | undefined): string[] {
  const at = speaker ? seating.indexOf(speaker) : -1;
  return at < 0 ? [...seating] : [...seating.slice(at + 1), ...seating.slice(0, at + 1)];
}

function castsAndRiders(table: AgendaTable, input: DeriveInput): Pick<AgendaStrip, "casts" | "waiting" | "riders"> {
  if (!table.inPlay) return { casts: [], waiting: [], riders: [] };
  const label = (seat: string) => (input.viewerSeat && seat === input.viewerSeat ? "You" : input.label(seat));
  const done = new Set([...table.casts.map((c) => c.player), ...table.predictions.map((p) => p.player)]);
  const order = votingOrder(input.seatingOrder ?? [], input.speaker);
  return {
    casts: table.casts.map((c) => ({
      label: input.label(c.player),
      you: Boolean(input.viewerSeat) && c.player === input.viewerSeat,
      outcome: c.outcome ? outcomeText(input, c.outcome) : null,
      votes: c.votes,
      badge: input.seatBadge?.(c.player) ?? null,
    })),
    waiting: order.filter((seat) => !done.has(seat)).map(label),
    riders: table.predictions.map((p) => ({ label: label(p.player), outcome: outcomeText(input, p.outcome) })),
  };
}
