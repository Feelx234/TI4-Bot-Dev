import type {
  ActionCardFacts,
  ActiveEffectDto,
  AgendaFacts,
  ExplorationFacts,
  GameView,
} from "../protocol/types.ts";
import { AGENDAS } from "../protocol/catalogStore.ts";
import {
  findActionCardMeta,
  findAttachmentMeta,
  findExplorationCardMeta,
  findPlanetMeta,
  findPublicObjectiveMeta,
  findStrategyCardMeta,
  humanizeId,
} from "../protocol/contentCatalog.ts";
import { agendaNameFromId } from "./agendaStrip.ts";
import { agendaOutcomeLabel, readAgendaLog, type AgendaRecordDto, type SeatName } from "./agendaToasts.ts";
import { describeActiveEffect, expiryText } from "./effectText.ts";
import { readExplorationLog } from "./explorationToasts.ts";

/**
 * The "In play" panel: what lies on the table for everybody (laws, lingering effects, public
 * objectives, the custodians token, strategy cards with their trade goods, tokens on the map) and
 * the discard piles that the rules keep face up (action cards, agendas, exploration).
 *
 * Pure and read-only. It reads a fixed list of public fields of the view and the redacted state;
 * it never reads a draw pile's content or order (only its length), a hand, or the secret
 * objective discard, so nothing hidden can reach the panel whatever the state carries.
 */

export type InPlayGroup = "play" | "discard";

export interface InPlayItem {
  id: string;
  title: string;
  /** Small chips under the title: "Law", "Round 3", "Elected: Arden". */
  meta: string[];
  /** One paragraph per entry; the panel clamps long text. */
  text: string[];
  /** Everything the search looks in, lower case. */
  search: string;
}

export interface InPlayFact {
  label: string;
  value: string;
}

export interface InPlaySection {
  id: string;
  title: string;
  group: InPlayGroup;
  /** Items (or the pile's size); null for a section that is only facts. */
  count: number | null;
  facts: InPlayFact[];
  items: InPlayItem[];
  /** Shown instead of the items while there are none. */
  empty: string;
  /** Items are newest first (discards). */
  newestFirst?: boolean;
}

export interface InPlayModel {
  sections: InPlaySection[];
  /** Laws in play. */
  laws: number;
  /** Cards in the face-up discard piles. */
  discards: number;
}

export interface InPlayInput {
  view: GameView | null | undefined;
  /** The redacted engine state the client receives; only public keys are read from it. */
  state: Record<string, unknown> | null | undefined;
  actionCards?: ActionCardFacts | null;
  exploration?: ExplorationFacts | null;
  agendas?: AgendaFacts | null;
  /** A seat's display name, or null when `id` is not a seat. */
  seatName: SeatName;
  viewerSeat?: string | null;
}

/** What the panel says it leaves out, so a player knows it is not a gap. */
export const IN_PLAY_HIDDEN_NOTE =
  "Not shown: the contents and order of draw piles (only how many cards are left), hands, the secret objective discard. Purged relics are not recorded by the game.";

const EXPLORATION_DECKS = ["CULTURAL", "INDUSTRIAL", "HAZARDOUS", "FRONTIER"] as const;

const isRecord = (value: unknown): value is Record<string, unknown> =>
  !!value && typeof value === "object" && !Array.isArray(value);
const strings = (value: unknown): string[] =>
  Array.isArray(value) ? value.filter((entry): entry is string => typeof entry === "string") : [];
const length = (value: unknown): number => (Array.isArray(value) ? value.length : 0);
const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;
const list = (names: string[]) => (names.length ? names.join(", ") : "nobody");

function item(id: string, title: string, meta: string[], text: string[]): InPlayItem {
  return {
    id,
    title,
    meta,
    text,
    search: [title, ...meta, ...text].join(" ").toLowerCase(),
  };
}

const deckLabel = (deck: string) => `${deck.charAt(0).toUpperCase()}${deck.slice(1).toLowerCase()}`;
const planetName = (id: string) => findPlanetMeta(id)?.name ?? humanizeId(id);
const systemName = (id: string) => `System ${id}`;

/** Initiative-ordered name of a strategy card. */
const strategyName = (id: string) => findStrategyCardMeta(id)?.name ?? humanizeId(id);

function agendaDef(alias: string) {
  return AGENDAS[alias];
}

function agendaName(alias: string, record?: AgendaRecordDto): string {
  return record?.name?.trim() || agendaDef(alias)?.name || agendaNameFromId(alias);
}

/** The text of the side of a vote that is in force (For / Against), or the election text. */
function lawText(alias: string, outcome: string): string[] {
  const def = agendaDef(alias);
  if (!def) return [];
  const lower = outcome.toLowerCase();
  if (lower === "against") return [def.against || def.text].filter(Boolean);
  return [def.text].filter(Boolean);
}

function tally(record: AgendaRecordDto | undefined, seatName: SeatName): string | null {
  const entries = Object.entries(record?.votes ?? {})
    .filter(([, n]) => n > 0)
    .sort((a, b) => b[1] - a[1]);
  if (!entries.length) return null;
  return entries.map(([outcome, n]) => `${agendaOutcomeLabel(outcome, seatName)} ${n}`).join(" · ");
}

function lawsSection(laws: Record<string, string>, log: AgendaRecordDto[], seatName: SeatName): InPlaySection {
  const items = Object.entries(laws).map(([alias, outcome]) => {
    const record = [...log].reverse().find((r) => r.agenda === alias && r.enacted);
    const def = agendaDef(alias);
    const name = agendaName(alias, record);
    const lower = outcome.toLowerCase();
    const forAgainst = lower === "for" || lower === "against";
    const meta = [def?.kind ?? "Law"];
    if (forAgainst) meta.push(lower === "for" ? "Passed: For" : "Passed: Against");
    else meta.push(`Elected: ${agendaOutcomeLabel(outcome, seatName)}`);
    if (record?.round) meta.push(`Round ${record.round}`);
    const votes = tally(record, seatName);
    if (votes) meta.push(`Votes: ${votes}`);
    if (record?.speaker_decided) meta.push("Speaker decided");
    if (record?.redirected_to) meta.push(`Redirected to ${agendaOutcomeLabel(record.redirected_to, seatName)}`);
    return item(`law:${alias}`, name, meta, lawText(alias, outcome));
  });
  return {
    id: "laws",
    title: "Laws in play",
    group: "play",
    count: items.length,
    facts: [],
    items,
    empty: "No law has been passed yet.",
  };
}

function effectsSection(view: GameView, input: InPlayInput): InPlaySection {
  const effects: ActiveEffectDto[] = view.active_effects ?? [];
  const labels = {
    seat: (id: string) => input.seatName(id) ?? humanizeId(id),
    viewerSeat: input.viewerSeat ?? null,
    system: systemName,
  };
  const items: InPlayItem[] = [];
  effects.forEach((effect, index) => {
    const text = describeActiveEffect(effect, labels);
    if (!text) return;
    items.push(
      item(`effect:${index}`, text.title, [labels.seat(effect.seat), text.expiry || expiryText(effect.until)], [text.summary]),
    );
  });
  return {
    id: "effects",
    title: "Lingering card effects",
    group: "play",
    count: items.length,
    facts: [],
    items,
    empty: "No played card is still in force.",
  };
}

function ledgerTotals(state: Record<string, unknown>, reasons: RegExp, seatName: SeatName): string | null {
  const rows = Array.isArray(state.vp_ledger) ? state.vp_ledger : [];
  const totals = new Map<string, number>();
  for (const row of rows) {
    if (!Array.isArray(row) || typeof row[0] !== "string" || typeof row[1] !== "number") continue;
    if (typeof row[2] !== "string" || !reasons.test(row[2])) continue;
    totals.set(row[0], (totals.get(row[0]) ?? 0) + row[1]);
  }
  const parts = [...totals]
    .filter(([, n]) => n !== 0)
    .map(([id, n]) => `${seatName(id) ?? humanizeId(id)} ${n > 0 ? "+" : ""}${n}`);
  return parts.length ? parts.join(", ") : null;
}

function tableSection(view: GameView, input: InPlayInput): InPlaySection {
  const { state, seatName } = { state: input.state ?? {}, seatName: input.seatName };
  const name = (id: string) => seatName(id) ?? humanizeId(id);
  const facts: InPlayFact[] = [{ label: "Speaker token", value: name(view.speaker) }];

  const removed = state.custodians_removed === true;
  const ledger = Array.isArray(state.vp_ledger) ? state.vp_ledger : [];
  const taker = ledger.find((row) => Array.isArray(row) && row[2] === "custodians" && typeof row[0] === "string");
  facts.push({
    label: "Custodians token",
    value: removed
      ? `Removed${taker ? ` by ${name((taker as unknown[])[0] as string)}` : ""}; every round has an agenda phase`
      : "On Mecatol Rex",
  });

  // Planet control is public; Mecatol's id differs between corpora ("mecatol_rex", "mecatolrex").
  for (const system of Object.values(view.board?.systems ?? {})) {
    for (const [planetId, planet] of Object.entries(system.planets ?? {})) {
      if (!/^mecatol/.test(planetId)) continue;
      facts.push({
        label: "Mecatol Rex",
        value: planet.controlled_by ? `Controlled by ${name(planet.controlled_by)}` : "Uncontrolled",
      });
    }
  }
  const imperial = ledgerTotals(state, /^(custodians|imperial_primary|imperial_rider)$/, seatName);
  if (imperial) facts.push({ label: "Points from Custodians and Imperial", value: imperial });

  const event = typeof state.galactic_event === "string" ? state.galactic_event : null;
  if (event) facts.push({ label: "Galactic event", value: humanizeId(event) });

  const decks: string[] = [];
  if (input.agendas) decks.push(`Agendas ${input.agendas.deck}`);
  if (input.actionCards) decks.push(`Action cards ${input.actionCards.deck}`);
  if (Array.isArray(state.relic_deck)) decks.push(`Relics ${length(state.relic_deck)}`);
  for (const deck of EXPLORATION_DECKS) {
    const left = input.exploration?.decks?.[deck];
    if (left) decks.push(`${deckLabel(deck)} ${left.length}`);
  }
  if (decks.length) facts.push({ label: "Cards left in draw piles", value: decks.join(" · ") });

  return {
    id: "table",
    title: "The table",
    group: "play",
    count: null,
    facts,
    items: [],
    empty: "",
  };
}

function objectivesSection(view: GameView, state: Record<string, unknown>, seatName: SeatName): InPlaySection {
  const scored = view.table.scored_objectives ?? {};
  const items = (view.table.revealed_objectives ?? []).map((id) => {
    const meta = findPublicObjectiveMeta(id);
    const by = Object.entries(scored)
      .filter(([, ids]) => (ids as string[]).includes(id))
      .map(([seat]) => seatName(seat) ?? humanizeId(seat));
    const stage = meta?.points === 2 ? "Stage II" : meta?.points === 1 ? "Stage I" : "Public";
    return item(
      `objective:${id}`,
      meta?.name ?? humanizeId(id),
      [stage, meta ? plural(meta.points, "point") : "", by.length ? `Scored by ${list(by)}` : "Not scored yet"].filter(Boolean),
      meta?.description ? [meta.description] : [],
    );
  });
  // Only how many face-down objectives are left in each stage, never which: the deck's order is hidden.
  let stage1 = 0;
  let stage2 = 0;
  for (const id of strings(state.objective_deck)) {
    const points = findPublicObjectiveMeta(id)?.points;
    if (points === 1) stage1 += 1;
    else if (points === 2) stage2 += 1;
  }
  const facts: InPlayFact[] = [];
  if (Array.isArray(state.objective_deck))
    facts.push({ label: "Not yet revealed", value: `Stage I ${stage1} · Stage II ${stage2}` });
  return {
    id: "objectives",
    title: "Public objectives",
    group: "play",
    count: items.length,
    facts,
    items,
    empty: "No public objective is revealed.",
  };
}

function strategySection(view: GameView, seatName: SeatName): InPlaySection {
  const name = (id: string) => seatName(id) ?? humanizeId(id);
  const goods = view.table.strategy_card_goods ?? {};
  const rows: Array<{ id: string; initiative: number; item: InPlayItem }> = [];
  for (const id of view.table.unclaimed_strategy_cards ?? []) {
    const bonus = goods[id] ?? 0;
    rows.push({
      id,
      initiative: findStrategyCardMeta(id)?.initiative ?? 99,
      item: item(`strategy:${id}`, strategyName(id), ["In the common area", bonus ? plural(bonus, "trade good") : "No trade goods"], []),
    });
  }
  for (const player of view.players ?? []) {
    for (const id of player.strategy_cards ?? []) {
      const used = (player.exhausted_strategy_cards ?? []).includes(id);
      rows.push({
        id,
        initiative: findStrategyCardMeta(id)?.initiative ?? 99,
        item: item(`strategy:${id}`, strategyName(id), [`Picked by ${name(player.id)}`, used ? "Used" : "Ready"], []),
      });
    }
  }
  rows.sort((a, b) => a.initiative - b.initiative);
  const meta = (id: string) => findStrategyCardMeta(id);
  const items = rows.map((row) => {
    const card = meta(row.id);
    const initiative = card ? [`Initiative ${card.initiative}`] : [];
    return { ...row.item, meta: [...initiative, ...row.item.meta], search: `${row.item.search} initiative ${card?.initiative ?? ""}` };
  });
  return {
    id: "strategy",
    title: "Strategy cards",
    group: "play",
    count: view.table.unclaimed_strategy_cards?.length ?? 0,
    facts: [],
    items,
    empty: "No strategy card is on the table.",
  };
}

function mapSection(view: GameView, state: Record<string, unknown>, seatName: SeatName): InPlaySection {
  const name = (id: string) => seatName(id) ?? humanizeId(id);
  const facts: InPlayFact[] = [];
  const frontier = strings(state.frontier_tokens);
  if (Array.isArray(state.frontier_tokens))
    facts.push({
      label: "Frontier tokens",
      value: frontier.length ? `${frontier.length} (${frontier.map((s) => systemName(s)).join(", ")})` : "none left",
    });
  if (typeof state.thunders_edge_system === "string")
    facts.push({ label: "Thunder's Edge", value: `Planet in ${systemName(state.thunders_edge_system)}` });
  if (isRecord(state.expedition_slices) && Object.keys(state.expedition_slices).length)
    facts.push({
      label: "Expeditions",
      value: Object.entries(state.expedition_slices)
        .map(([slice, seat]) => `${humanizeId(slice)}: ${name(String(seat))}`)
        .join(" · "),
    });
  if (state.fracture_in_play === true) facts.push({ label: "The Fracture", value: "In play" });
  const ingress = strings(Array.isArray(state.ingress_tokens) ? state.ingress_tokens : []);
  if (ingress.length) facts.push({ label: "Ingress tokens", value: ingress.map(systemName).join(", ") });
  const breach = strings(Array.isArray(state.breach_tokens) ? state.breach_tokens : []);
  if (breach.length) facts.push({ label: "Crimson breaches", value: breach.map(systemName).join(", ") });
  if (isRecord(state.wormhole_tokens) && Object.keys(state.wormhole_tokens).length)
    facts.push({
      label: "Wormhole tokens",
      value: Object.entries(state.wormhole_tokens)
        .map(([kind, system]) => `${humanizeId(kind)} in ${systemName(String(system))}`)
        .join(" · "),
    });
  if (Array.isArray(state.ion_storm) && state.ion_storm.length === 2)
    facts.push({ label: "Ion storm", value: `${systemName(String(state.ion_storm[0]))}, ${humanizeId(String(state.ion_storm[1]))} side` });

  // Attachments lie on planets for everybody to see.
  const items: InPlayItem[] = [];
  for (const system of Object.values(view.board?.systems ?? {})) {
    for (const [planetId, planet] of Object.entries(system.planets ?? {})) {
      for (const attachment of planet.attachments ?? []) {
        const meta = findAttachmentMeta(attachment);
        const bits = meta
          ? [meta.resourcesModifier ? `${meta.resourcesModifier > 0 ? "+" : ""}${meta.resourcesModifier} resources` : "", meta.influenceModifier ? `${meta.influenceModifier > 0 ? "+" : ""}${meta.influenceModifier} influence` : ""].filter(Boolean)
          : [];
        items.push(item(`attachment:${planetId}:${attachment}`, meta?.name ?? humanizeId(attachment), [`On ${planetName(planetId)}`, ...bits], []));
      }
    }
  }
  return {
    id: "map",
    title: "Tokens and attachments on the map",
    group: "play",
    count: items.length || null,
    facts,
    items,
    empty: facts.length ? "" : "Nothing is placed on the map yet.",
  };
}

/** Action card discard pile: newest first, equal cards grouped. */
function actionDiscardSection(state: Record<string, unknown>, facts: ActionCardFacts | null | undefined): InPlaySection {
  // The state keeps the pile in the order cards left play; the facts are sorted by id.
  const ordered = Array.isArray(state.discarded_action_cards) ? strings(state.discarded_action_cards) : (facts?.discard ?? []);
  const newest = [...ordered].reverse();
  const groups = new Map<string, { id: string; copies: number }>();
  for (const id of newest) {
    const name = findActionCardMeta(id)?.name ?? humanizeId(id);
    const group = groups.get(name);
    if (group) group.copies += 1;
    else groups.set(name, { id, copies: 1 });
  }
  const items: InPlayItem[] = [...groups].map(([name, group]) => {
    const meta = findActionCardMeta(group.id);
    const meta2 = [group.copies > 1 ? `x${group.copies}` : "", meta?.window ?? meta?.phase ?? ""].filter(Boolean);
    return item(`discard-action:${group.id}`, name, meta2, meta?.description ? [meta.description] : []);
  });
  const salvage = strings(facts?.salvage ?? state.salvage_yard);
  for (const id of salvage) {
    const meta = findActionCardMeta(id);
    items.push(item(`salvage:${id}`, meta?.name ?? humanizeId(id), ["On Garbozia"], meta?.description ? [meta.description] : []));
  }
  return {
    id: "discard-action",
    title: "Action card discard",
    group: "discard",
    count: ordered.length + salvage.length,
    facts: [],
    items,
    empty: "No action card has been discarded.",
    newestFirst: true,
  };
}

/** Agenda discard pile: every resolved agenda that is not a law in play now, newest first. */
function agendaDiscardSection(laws: Record<string, string>, log: AgendaRecordDto[], seatName: SeatName): InPlaySection {
  const records = log.map((record, index) => ({ record, index }));
  // A law in play is the latest enactment of its card; it is on the table, not in the pile.
  const inPlayIndex = new Set<number>();
  for (const alias of Object.keys(laws)) {
    const latest = records.filter(({ record }) => record.agenda === alias && record.law && record.enacted).pop();
    if (latest && latest.record.outcome === laws[alias]) inPlayIndex.add(latest.index);
  }
  const pile = records.filter(({ index }) => !inPlayIndex.has(index));
  const items = pile
    .map(({ record, index }) => {
      const def = agendaDef(record.agenda);
      const outcome = record.outcome ?? null;
      let status: string;
      if (!outcome && record.discard_reason === "no_choice") status = "Discarded: nothing to elect";
      else if (!outcome && record.discard_reason === "replaced") status = "Discarded and replaced";
      else if (record.discarded) status = "Discarded, no effect";
      else if (!outcome) status = "Resolved, no outcome";
      else if (record.law && record.enacted) status = "Law repealed";
      else if (record.law) status = "Not enacted";
      else status = `Resolved: ${agendaOutcomeLabel(outcome, seatName)}`;
      const meta = [def?.kind ?? (record.law ? "Law" : "Directive"), status];
      if (record.round) meta.push(`Round ${record.round}`);
      if (outcome && !/^(for|against)$/i.test(outcome) && !(record.law && !record.enacted)) meta.push(`Elected: ${agendaOutcomeLabel(outcome, seatName)}`);
      else if (outcome && record.law && !record.enacted) meta.push(`Result: ${agendaOutcomeLabel(outcome, seatName)}`);
      const votes = tally(record, seatName);
      if (votes) meta.push(`Votes: ${votes}`);
      if (record.speaker_decided) meta.push("Speaker decided");
      const text = outcome ? lawText(record.agenda, outcome) : def?.text ? [def.text] : [];
      return { index, item: item(`discard-agenda:${index}`, agendaName(record.agenda, record), meta, text) };
    })
    .sort((a, b) => b.index - a.index)
    .map((entry) => entry.item);
  return {
    id: "discard-agenda",
    title: "Agenda discard",
    group: "discard",
    count: items.length,
    facts: [],
    items,
    empty: "No agenda has been discarded.",
    newestFirst: true,
  };
}

/** Exploration discard piles (the four decks): cards that were drawn and resolved, newest first. */
function explorationDiscardSection(state: Record<string, unknown>, seatName: SeatName): InPlaySection {
  const log = readExplorationLog(state);
  const kept = (outcome?: string) => outcome === "fragment" || outcome?.startsWith("attached") === true;
  const entries = log.map((record, index) => ({ record, index })).filter(({ record }) => !kept(record.outcome));
  const perDeck = new Map<string, number>();
  const items = entries
    .map(({ record, index }) => {
      const deck = String(record.deck).toUpperCase();
      perDeck.set(deck, (perDeck.get(deck) ?? 0) + 1);
      const meta = findExplorationCardMeta(record.card);
      const where = record.planet ? planetName(record.planet) : record.system ? systemName(record.system) : "";
      const bits = [
        deckLabel(deck),
        record.round ? `Round ${record.round}` : "",
        `Drawn by ${seatName(record.player) ?? humanizeId(record.player)}`,
        where,
      ].filter(Boolean);
      const text = [meta?.description ?? "", record.result ?? ""].filter(Boolean);
      return { index, item: item(`discard-exploration:${index}`, meta?.name ?? humanizeId(record.card), bits, text) };
    })
    .sort((a, b) => b.index - a.index)
    .map((entry) => entry.item);
  const facts = EXPLORATION_DECKS.filter((deck) => perDeck.has(deck)).map((deck) => ({
    label: deckLabel(deck),
    value: plural(perDeck.get(deck) ?? 0, "card"),
  }));
  return {
    id: "discard-exploration",
    title: "Exploration discard",
    group: "discard",
    count: items.length,
    facts,
    items,
    empty: "No exploration card has been discarded.",
    newestFirst: true,
  };
}

/** The panel's model. Sections keep a fixed order; empty ones stay so the layout does not jump. */
export function buildInPlayModel(input: InPlayInput): InPlayModel {
  const state = input.state ?? {};
  const view = input.view;
  if (!view) return { sections: [], laws: 0, discards: 0 };
  const laws = view.table?.laws ?? {};
  const log = readAgendaLog(state);
  const sections: InPlaySection[] = [
    lawsSection(laws, log, input.seatName),
    effectsSection(view, input),
    tableSection(view, input),
    objectivesSection(view, state, input.seatName),
    strategySection(view, input.seatName),
    mapSection(view, state, input.seatName),
    actionDiscardSection(state, input.actionCards),
    agendaDiscardSection(laws, log, input.seatName),
    explorationDiscardSection(state, input.seatName),
  ];
  return {
    sections,
    laws: Object.keys(laws).length,
    discards: sections.filter((s) => s.group === "discard").reduce((sum, s) => sum + (s.count ?? 0), 0),
  };
}

/** Filters a model by a free-text query and a group chip; sections without a match drop out. */
export function filterInPlay(
  model: InPlayModel,
  options: { query: string; group: InPlayGroup | "all" },
): InPlaySection[] {
  const query = options.query.trim().toLowerCase();
  return model.sections
    .filter((section) => options.group === "all" || section.group === options.group)
    .map((section) => {
      if (!query) return section;
      const title = section.title.toLowerCase().includes(query);
      const facts = section.facts.some((f) => `${f.label} ${f.value}`.toLowerCase().includes(query));
      const items = title ? section.items : section.items.filter((i) => i.search.includes(query));
      return title || facts || items.length ? { ...section, items } : null;
    })
    .filter((section): section is InPlaySection => section !== null);
}
