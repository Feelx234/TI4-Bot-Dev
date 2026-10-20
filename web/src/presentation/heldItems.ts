import type { BoardView, ChoiceOption, PlayerView } from "../protocol/types.ts";
import { CONTENT } from "../protocol/catalogStore.ts";
import {
  findAttachmentMeta,
  findExplorationCardMeta,
  findPlanetMeta,
  findTechnologyMeta,
  humanizeId,
} from "../protocol/contentCatalog.ts";
import { describeBreakthrough } from "./breakthroughs.ts";
import { findLeaderRecord, findRelicRecord } from "./componentInfo.ts";
import {
  findFactionRecord,
  type FactionAbilityRecord,
  type PromissoryNoteRecord,
} from "./factionInfo.ts";

/**
 * The "Held items" panel's model: everything a seat holds or has in play that is not an action
 * card or a secret objective, grouped into sections with state chips. Pure and display only.
 *
 * Sources are what the client already receives: the redacted `PlayerView`s, the board view and the
 * engine state's promissory fields (`promissory_notes`, `promissory_faceup`, `support_holders`,
 * `faction_marks`). Hidden information stays hidden here: another seat's notes in hand and marks
 * are never read, only notes lying faceup in a play area and Support for the Throne, which are
 * public (the engine's own `Observed::promissory_notes` exposes exactly the faceup subset).
 */

/** The promissory fields of the engine state that the panel reads. */
export interface HeldState {
  /** Note id (`alias:owner`) to the seat holding it. */
  notes: Readonly<Record<string, string>>;
  /** Note ids lying faceup in the holder's play area. */
  faceup: readonly string[];
  /** Support for the Throne: owner to the seat holding it. */
  support: Readonly<Record<string, string>>;
  /** Faction bookkeeping marks, `<faction>:<card>:<detail>` to value. */
  marks: Readonly<Record<string, string>>;
}

export const EMPTY_HELD_STATE: HeldState = { notes: {}, faceup: [], support: {}, marks: {} };

const isRecord = (v: unknown): v is Record<string, unknown> => !!v && typeof v === "object" && !Array.isArray(v);

function stringMap(v: unknown): Record<string, string> {
  if (!isRecord(v)) return {};
  return Object.fromEntries(Object.entries(v).filter((e): e is [string, string] => typeof e[1] === "string"));
}

export function readHeldState(state: Record<string, unknown> | null | undefined): HeldState {
  if (!state) return EMPTY_HELD_STATE;
  return {
    notes: stringMap(state.promissory_notes),
    faceup: Array.isArray(state.promissory_faceup)
      ? state.promissory_faceup.filter((x): x is string => typeof x === "string")
      : [],
    support: stringMap(state.support_holders),
    marks: stringMap(state.faction_marks),
  };
}

export type ChipTone = "ready" | "exhausted" | "purged" | "locked" | "unlocked" | "info";

export interface HeldChip {
  label: string;
  tone: ChipTone;
}

export interface HeldItem {
  key: string;
  name: string;
  /** Small kind line: "Agent", "Faction technology", "Relic fragments". */
  kind: string | null;
  /** Whose card it is, when not the seat itself. */
  owner: string | null;
  text: string | null;
  chips: HeldChip[];
  /** Read-only: the pending choice of the viewer offers this component right now. */
  usableNow: boolean;
}

export interface HeldSection {
  id:
    | "alliance"
    | "promissory"
    | "relics"
    | "techs"
    | "upgrades"
    | "leaders"
    | "faction"
    | "legendary"
    | "attachments";
  title: string;
  count: number;
  items: HeldItem[];
}

export interface HeldItemsModel {
  subject: string;
  /** True when the subject is the viewer: private items (notes in hand, marks) may be shown. */
  own: boolean;
  sections: HeldSection[];
  /** Total number of items over every section; the badge on the entry button. */
  total: number;
}

export interface HeldItemsInput {
  players: readonly PlayerView[];
  subject: string;
  /** The viewing seat; absent for a spectator. */
  viewer?: string;
  board?: BoardView | null;
  held?: HeldState;
  /** The breakthrough alias of the subject, from the state, for the faction section. */
  breakthrough?: string | null;
  /** The table plays the content that has breakthroughs (Thunder's Edge); default on. Off: no section. */
  breakthroughsOn?: boolean;
  /** Options of the pending choice the viewer is asked; marks "usable now" (read-only). */
  options?: readonly Pick<ChoiceOption, "id" | "label" | "kind">[];
  /** Seat label, e.g. "Red". */
  label?: (playerId: string) => string;
}

const NOTES = () => CONTENT.promissoryNotes as unknown as Record<string, PromissoryNoteRecord>;
const ABILITIES = () => CONTENT.factionAbilities as unknown as Record<string, FactionAbilityRecord>;

const norm = (text: string) => text.toLowerCase().replace(/[^a-z0-9]/g, "");

/** `alias:owner` of a note id; `support:owner` for Support for the Throne. */
export function splitNoteId(id: string): { alias: string; owner: string } {
  const cut = id.indexOf(":");
  return cut < 0 ? { alias: id, owner: "" } : { alias: id.slice(0, cut), owner: id.slice(cut + 1) };
}

/** The printed note of an alias: a faction's own, or the `<color>_` one every faction has. */
export function findNoteRecord(alias: string, ownerFaction?: string): PromissoryNoteRecord | undefined {
  const notes = NOTES();
  if (alias === "support") return notes["<color>_sftt"];
  const generic = notes[`<color>_${alias}`];
  const faction = notes[alias];
  if (faction && (!faction.faction || !ownerFaction || norm(faction.faction) === norm(ownerFaction))) return faction;
  return generic ?? faction;
}

/** Whether a text asks the card to be exhausted to use it. */
export const exhaustsToUse = (text: string | null | undefined): boolean =>
  !!text && /\bexhaust this\b/i.test(text);

/** "the <color> player" in a printed note is its issuer: name the seat instead. */
function issuerText(text: string | undefined, issuer: string | null): string | null {
  if (!text) return null;
  if (!issuer) return text;
  return text.replace(/\bthe <color> player(?:'s)?/gi, (m) => (m.endsWith("'s") ? `${issuer}'s` : issuer)).replaceAll("<color>", issuer);
}

const FRAGMENT_TRAITS = ["cultural", "industrial", "hazardous", "frontier"];

class Usable {
  private readonly ids: string[] = [];
  private readonly legendary = new Set<string>();
  private readonly labels: string[] = [];
  constructor(options: HeldItemsInput["options"]) {
    for (const option of options ?? []) {
      this.ids.push(option.id);
      this.labels.push(norm(option.label));
      if (option.kind === "legendary") this.legendary.add(option.id);
    }
  }
  tech = (id: string) => this.ids.includes(`component|tech|${id}`);
  leader = (id: string) => this.ids.some((o) => o.startsWith("component|leader|") && o.split("|").pop() === id);
  relic = (id: string) => this.ids.includes(`relic|${id}`);
  legendaryPlanet = (planet: string) => this.legendary.has(planet);
  playArea = (card: string) => this.ids.some((o) => o.startsWith(`play_area:${card}`));
  purge = (trait: string) => this.ids.some((o) => o.toLowerCase() === `purge|${trait}`);
  ability = (key: string) => this.ids.includes(`faction|${key}`);
  /** A note has no option id of its own the client knows; its printed name in an option label stands in. */
  note = (name: string) => {
    const n = norm(name);
    return n.length > 3 && this.labels.some((l) => l.includes(n));
  };
}

const chip = (label: string, tone: ChipTone): HeldChip => ({ label, tone });
const byKey = ([a]: [string, unknown], [b]: [string, unknown]) => (a < b ? -1 : a > b ? 1 : 0);

const playerOf = (players: readonly PlayerView[], ref: string): PlayerView | undefined =>
  players.find((p) => p.id === ref) ??
  players.find((p) => norm(p.faction) === norm(ref)) ??
  players.find((p) => norm(findFactionRecord(p.faction)?.name ?? "") === norm(ref));

export function buildHeldItems(input: HeldItemsInput): HeldItemsModel {
  const { players, subject, viewer, board } = input;
  const held = input.held ?? EMPTY_HELD_STATE;
  const label = input.label ?? ((id: string) => id);
  const usable = new Usable(input.options);
  const own = !!viewer && viewer === subject;
  const player = players.find((p) => p.id === subject);
  if (!player) return { subject, own, sections: [], total: 0 };
  const faction = findFactionRecord(player.faction);
  const sections: HeldSection[] = [];
  const add = (id: HeldSection["id"], title: string, items: HeldItem[]) => {
    if (items.length) sections.push({ id, title, count: items.length, items });
  };

  // --- Promissory notes -------------------------------------------------------------------
  const faceup = new Set(held.faceup);
  const noteItems: HeldItem[] = [];
  const homeItems: HeldItem[] = [];
  const alliance: HeldItem[] = [];
  const noteItem = (key: string, alias: string, ownerRef: string, chips: HeldChip[], extra: Partial<HeldItem> = {}): HeldItem => {
    const ownerPlayer = playerOf(players, ownerRef);
    const record = findNoteRecord(alias, ownerPlayer?.faction);
    const name = record?.name ?? humanizeId(alias);
    return {
      key,
      name,
      kind: "Promissory note",
      owner: ownerPlayer ? (ownerPlayer.id === subject ? null : label(ownerPlayer.id)) : null,
      // The printed text names its issuer as <color>; say whose note it is instead.
      text: issuerText(record?.text, ownerPlayer ? label(ownerPlayer.id) : null),
      chips,
      usableNow: own && usable.note(name),
      ...extra,
    };
  };
  for (const [id, holder] of Object.entries(held.notes).sort(byKey)) {
    if (holder !== subject) continue;
    const { alias, owner } = splitNoteId(id);
    const isFaceup = faceup.has(id);
    // Public: faceup in a play area. Private: in hand, only in the viewer's own view.
    if (!isFaceup && !own) continue;
    const ownerPlayer = playerOf(players, owner);
    if (alias === "an" && isFaceup) {
      alliance.push(
        noteItem(`alliance-${id}`, alias, owner, [chip("faceup", "ready"), chip("alliance", "unlocked")], {
          text:
            ownerPlayer && ownerPlayer.id !== subject
              ? `Allied with ${label(ownerPlayer.id)}: holds their Alliance faceup, which gives access to their commander.`
              : null,
        }),
      );
    } else if (isFaceup) {
      noteItems.push(noteItem(`note-${id}`, alias, owner, [chip("faceup in play area", "ready")]));
    } else if (ownerPlayer?.id === subject) {
      homeItems.push(noteItem(`note-${id}`, alias, owner, [chip("own copy, at home", "info")]));
    } else {
      noteItems.push(noteItem(`note-${id}`, alias, owner, [chip("in hand", "info")]));
    }
  }
  // Support for the Throne has its own map; it is public and held faceup by the receiver.
  for (const [ownerRef, holderRef] of Object.entries(held.support).sort(byKey)) {
    const owner = playerOf(players, ownerRef);
    const holder = playerOf(players, holderRef);
    if (!owner || !holder || owner.id === holder.id) continue;
    if (holder.id === subject) {
      noteItems.push(
        noteItem(`support-${owner.id}`, "support", owner.id, [chip("received, +1 VP", "unlocked"), chip("faceup", "ready")]),
      );
    } else if (owner.id === subject) {
      noteItems.push(
        noteItem(`support-given-${owner.id}`, "support", owner.id, [chip(`given to ${label(holder.id)}`, "info"), chip("faceup", "ready")]),
      );
    }
  }
  // Own notes lying faceup in someone else's play area (Ceasefire, Trade Convoys, Alliance given).
  for (const [id, holder] of Object.entries(held.notes).sort(byKey)) {
    if (holder === subject || !faceup.has(id)) continue;
    const { alias, owner } = splitNoteId(id);
    if (playerOf(players, owner)?.id !== subject) continue;
    const holderPlayer = playerOf(players, holder);
    const chips = [chip(`faceup with ${holderPlayer ? label(holderPlayer.id) : holder}`, "info")];
    if (alias === "an") alliance.push(noteItem(`out-${id}`, alias, owner, [...chips, chip("alliance", "unlocked")]));
    else noteItems.push(noteItem(`out-${id}`, alias, owner, chips));
  }
  add("alliance", "Alliance", alliance);
  add("promissory", "Promissory notes", [...noteItems, ...homeItems]);

  // --- Relics, fragments, faceup exploration cards ------------------------------------------
  const relicItems: HeldItem[] = [];
  for (const id of player.relics) {
    const record = findRelicRecord(id);
    const text = record?.text ?? null;
    const exhausted = player.exhausted_relics.includes(id);
    const exhaustable = exhausted || exhaustsToUse(text);
    relicItems.push({
      key: `relic-${id}`,
      name: record?.name ?? humanizeId(id),
      kind: "Relic",
      owner: null,
      text,
      chips: exhaustable ? [chip(exhausted ? "exhausted" : "ready", exhausted ? "exhausted" : "ready")] : [chip("in play", "info")],
      usableNow: own && usable.relic(id),
    });
  }
  for (const card of player.exploration_cards ?? []) {
    const meta = findExplorationCardMeta(card);
    relicItems.push({
      key: `explore-${card}`,
      name: meta?.name ?? humanizeId(card),
      kind: "Exploration card in play area",
      owner: null,
      text: meta?.description ?? null,
      chips: [chip("faceup", "info")],
      usableNow: own && usable.playArea(card),
    });
  }
  const fragments = player.relic_fragments ?? {};
  const traits = [...new Set([...FRAGMENT_TRAITS, ...Object.keys(fragments).map((t) => t.toLowerCase())])];
  for (const trait of traits) {
    const n = Object.entries(fragments).reduce((sum, [t, c]) => (t.toLowerCase() === trait ? sum + c : sum), 0);
    if (n <= 0) continue;
    relicItems.push({
      key: `fragments-${trait}`,
      name: `${humanizeId(trait)} relic fragments`,
      kind: "Relic fragments",
      owner: null,
      text: "ACTION: Purge 3 relic fragments of one type (unknown fragments count as any type) to gain 1 relic.",
      chips: [chip(`${n} of 3`, n >= 3 ? "unlocked" : "info")],
      usableNow: own && usable.purge(trait),
    });
  }
  add("relics", "Relics and fragments", relicItems);

  // --- Technologies -------------------------------------------------------------------------
  const exhaustedTechs = new Set(player.exhausted_technologies);
  const techItems: HeldItem[] = [];
  const upgradeItems: HeldItem[] = [];
  for (const id of player.technologies) {
    const meta = findTechnologyMeta(id);
    const text = meta?.description ?? null;
    const isUpgrade = !!meta?.types?.includes("UNITUPGRADE");
    const isFaction = !!meta?.faction;
    const exhausted = exhaustedTechs.has(id);
    const exhaustable = exhausted || exhaustsToUse(text);
    const chips: HeldChip[] = [];
    if (exhaustable) chips.push(chip(exhausted ? "exhausted" : "ready", exhausted ? "exhausted" : "ready"));
    if (isFaction) chips.push(chip("faction", "info"));
    const item: HeldItem = {
      key: `tech-${id}`,
      name: meta?.name ?? humanizeId(id),
      kind: isUpgrade ? "Unit upgrade" : isFaction ? "Faction technology" : "Technology",
      owner: null,
      text,
      chips,
      usableNow: own && usable.tech(id),
    };
    if (exhaustable) techItems.push(item);
    else if (isUpgrade || isFaction) upgradeItems.push(item);
  }
  add("techs", "Technologies that exhaust", techItems);
  add("upgrades", "Unit upgrades and faction technologies", upgradeItems);

  // --- Leaders ---------------------------------------------------------------------------------
  const rank: Record<string, number> = { agent: 0, commander: 1, hero: 2 };
  const leaderTone = { readied: "ready", exhausted: "exhausted", purged: "purged", locked: "locked", unlocked: "unlocked" } as const;
  const leaderItems: HeldItem[] = Object.entries(player.leaders ?? {})
    .map(([id, status]) => ({ id, status, record: findLeaderRecord(id) }))
    .sort((a, b) => (rank[a.record?.type ?? ""] ?? 9) - (rank[b.record?.type ?? ""] ?? 9))
    .map(({ id, status, record }) => ({
      key: `leader-${id}`,
      name: record?.name ?? humanizeId(id),
      kind: humanizeId(record?.type ?? "leader"),
      owner: null,
      text: status === "locked" && record?.unlockCondition ? `Unlock: ${record.unlockCondition}` : (record?.abilityText ?? null),
      chips: [chip(status, leaderTone[status] ?? "info")],
      usableNow: own && usable.leader(id),
    }));
  add("leaders", "Leaders", leaderItems);

  // --- Faction abilities, breakthrough, marks -------------------------------------------------
  const factionItems: HeldItem[] = [];
  for (const id of faction?.abilities ?? []) {
    const ability = ABILITIES()[id];
    if (!ability) continue;
    factionItems.push({
      key: `ability-${id}`,
      name: ability.name,
      kind: "Faction ability",
      owner: null,
      text: ability.windowEffect ?? ability.permanentEffect ?? null,
      chips: [chip(ability.window ? "when triggered" : "always on", "info")],
      usableNow: own && usable.ability(id),
    });
  }
  const breakthrough =
    input.breakthroughsOn === false ? null : describeBreakthrough(player.faction, input.breakthrough ?? null);
  if (breakthrough) {
    factionItems.push({
      key: "breakthrough",
      name: breakthrough.name,
      kind: "Breakthrough",
      owner: null,
      text: breakthrough.text,
      chips: [chip(breakthrough.unlocked ? "unlocked" : "locked", breakthrough.unlocked ? "unlocked" : "locked")],
      usableNow: false,
    });
  }
  if (own) {
    const prefixes = new Set([norm(player.faction), norm(faction?.id ?? ""), norm(faction?.name ?? "")]);
    prefixes.delete("");
    for (const [key, value] of Object.entries(held.marks).sort(byKey)) {
      if (!prefixes.has(norm(key.split(":")[0]))) continue;
      factionItems.push({
        key: `mark-${key}`,
        name: humanizeId(key.split(":").slice(1).join(" ")),
        kind: "Faction marker",
        owner: null,
        text: value,
        chips: [chip("tracked", "info")],
        usableNow: false,
      });
    }
  }
  add("faction", "Faction", factionItems);

  // --- Legendary planets and attachments on controlled planets ----------------------------
  const legendItems: HeldItem[] = [];
  const attachItems: HeldItem[] = [];
  const exhaustedLegendary = new Set(player.exhausted_legendary ?? []);
  for (const system of Object.values(board?.systems ?? {})) {
    for (const [pid, planet] of Object.entries(system.planets ?? {})) {
      if (planet.controlled_by !== subject) continue;
      const meta = findPlanetMeta(pid);
      if (meta?.legendaryAbilityName) {
        const spent = exhaustedLegendary.has(pid);
        legendItems.push({
          key: `legendary-${pid}`,
          name: meta.legendaryAbilityName,
          kind: `Legendary planet: ${meta.name}`,
          owner: null,
          text: meta.legendaryAbilityText ?? null,
          chips: [
            chip(spent ? "ability exhausted" : "ability ready", spent ? "exhausted" : "ready"),
            ...(planet.exhausted ? [chip("planet exhausted", "exhausted")] : []),
          ],
          usableNow: own && usable.legendaryPlanet(pid),
        });
      }
      for (const att of planet.attachments ?? []) {
        const am = findAttachmentMeta(att);
        const signed = (n: number, what: string) => (n ? `${n > 0 ? "+" : ""}${n} ${what}` : "");
        attachItems.push({
          key: `attach-${pid}-${att}`,
          name: am?.name ?? humanizeId(att),
          kind: `On ${meta?.name ?? humanizeId(pid)}`,
          owner: null,
          text: am ? [signed(am.resourcesModifier, "resources"), signed(am.influenceModifier, "influence")].filter(Boolean).join(", ") || null : null,
          chips: [chip("attached", "info")],
          usableNow: false,
        });
      }
    }
  }
  add("legendary", "Legendary planet abilities", legendItems);
  add("attachments", "Attachments on planets", attachItems);

  // The badge leaves out what every seat has from the start (its own notes at home, its faction
  // abilities) and what is still locked, so that it counts what was gained.
  const counted = (item: HeldItem) =>
    item.kind !== "Faction ability" && !item.chips.some((c) => c.label.startsWith("own copy") || c.tone === "locked");
  return { subject, own, sections, total: sections.reduce((n, s) => n + s.items.filter(counted).length, 0) };
}
