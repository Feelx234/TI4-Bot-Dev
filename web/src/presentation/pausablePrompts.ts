import type { PendingChoiceDto } from "../protocol/types.ts";

/**
 * The optional abilities a player may switch off ("pause") on their own device. The engine asks
 * every one of these questions every time; a paused ability is declined for the viewer by the
 * client (an ordinary recorded decline), see `usePausedPromptAutoDecline`.
 *
 * Adding an ability is one `prompt(...)` line below. It needs the id (storage key
 * `player_ask_<id>`), the label for toasts and lists, the owner (a faction, or the technology or
 * relic that grants it), when it asks, and how the question is recognised: the reaction window
 * lists it as an option (`option`: first id segment and one later segment of the option id) and/or
 * the ability asks its own question with a decision subtype (`subtypes`).
 *
 * The audit behind this list, with the abilities that were left out and why, is in
 * docs/ABILITY_PAUSE_TOGGLES.md.
 */
export interface PausablePrompt {
  /** Stable id; the storage key is `player_ask_<id>` (unchanged for I48S and Aetherpassage). */
  id: string;
  /** Short name: "Dirzuga Rophal", used in toasts, switches and the Prompts list. */
  label: string;
  /** What it is, in a few words: "Arborec commander". */
  source: string;
  /** When it asks, for the Prompts list: "after another player activates a system". */
  when: string;
  /** Owning factions (a faction id prefix matches, so "keleres" covers keleresa/m/x). */
  factions: readonly string[];
  /** Technologies that give the ability to any faction (printed aliases). */
  technologies: readonly string[];
  /** Relics that give the ability (printed aliases). */
  relics: readonly string[];
  /** The reaction window offers it as an option whose id starts with `kind:` and holds `token`. */
  option?: { kind: string; token: string };
  /** The ability asks its own question with one of these decision subtypes. */
  subtypes: readonly string[];
  /** Laws (agenda aliases) whose current owner has the ability. */
  laws: readonly string[];
  /**
   * The question names one of these as its content source (`context.source.Content`), for
   * abilities that share a decision subtype with others (the discard offers of the ministers).
   */
  contentSources: readonly string[];
  /**
   * In whose turns the question can come up: `own` (only while the owner is the active player,
   * e.g. their own activation or production), `others` (only while another seat is the active
   * player) or `both`. Only `both` gets the "Only in my turn" choice; for the other two it would
   * be the same as Ask or as Skipped.
   */
  turns: PromptTurns;
}

export type PromptTurns = "both" | "own" | "others";

/** Whether the "Only in my turn" choice says something for this ability. */
export function offersOwnTurnMode(entry: Pick<PausablePrompt, "turns">): boolean {
  return entry.turns === "both";
}

interface PromptSpec {
  /** Default `both`; see `PausablePrompt.turns`. */
  turns?: PromptTurns;
  source: string;
  when: string;
  factions?: readonly string[];
  technologies?: readonly string[];
  relics?: readonly string[];
  option?: readonly [kind: string, token: string];
  subtypes?: readonly string[];
  laws?: readonly string[];
  /** Used together with `subtypes`: only questions whose content source is one of these. */
  contentSources?: readonly string[];
}

const REGISTRY: PausablePrompt[] = [];

function prompt(id: string, label: string, spec: PromptSpec): void {
  REGISTRY.push({
    id,
    label,
    source: spec.source,
    when: spec.when,
    factions: spec.factions ?? [],
    technologies: spec.technologies ?? [],
    relics: spec.relics ?? [],
    option: spec.option ? { kind: spec.option[0], token: spec.option[1] } : undefined,
    subtypes: spec.subtypes ?? [],
    laws: spec.laws ?? [],
    contentSources: spec.contentSources ?? [],
    turns: spec.turns ?? "both",
  });
}

// -- The two that were first (storage keys player_ask_i48s and player_ask_aetherpassage) ------------
prompt("i48s", "I48S", {
  source: "L1Z1X agent",
  when: "after a player activates a system",
  factions: ["l1z1x"],
  option: ["leader", "l1z1xagent"],
  subtypes: ["l1z1x_agent_swap"],
});
prompt("aetherpassage", "Aetherpassage", {
  source: "Empyrean faction ability",
  when: "after a player activates a system",
  factions: ["empyrean"],
  option: ["ability", "aetherpassage"],
});

// -- A law's owner (the ministers ask in a shared subtype; the content source tells them apart) -------
prompt("minister_of_peace", "Minister of Peace", {
  source: "law",
  when: "after a player activates a system that holds another player's units",
  laws: ["minister_peace"],
  subtypes: ["offer_discard_law"],
  contentSources: ["minister_peace"],
});

// -- Reaction to other players' actions: the owner is asked on every occasion ------------------------
prompt("instinct_training", "Instinct Training", {
  source: "Xxcha technology",
  when: "when another player plays an action card",
  factions: ["xxcha"],
  option: ["technology", "it"],
  subtypes: ["instinct_training_cancel"],
});
prompt("nullification_field", "Nullification Field", {
  turns: "others",
  source: "Xxcha technology",
  when: "after another player activates a system with your ships",
  factions: ["xxcha"],
  technologies: ["nf"],
  subtypes: ["nullification_field_end_turn"],
});
prompt("arborec_commander", "Dirzuga Rophal", {
  turns: "others",
  source: "Arborec commander",
  when: "after another player activates a system with your producing units",
  factions: ["arborec"],
  option: ["leader", "arboreccommander"],
});
prompt("argent_agent", "Trilossa Aun Mirik", {
  source: "Argent agent",
  when: "when a player produces ground forces",
  factions: ["argent"],
  option: ["leader", "argentagent"],
});
prompt("argent_commander", "Trrakan Aun Zulok", {
  source: "Argent commander",
  when: "when your units roll for a unit ability",
  factions: ["argent"],
  option: ["leader", "argentcommander"],
});
prompt("bastion_agent", "Dame Briar", {
  source: "Bastion agent",
  when: "when a player's unit is destroyed",
  factions: ["bastion"],
  option: ["leader", "bastionagent"],
});
prompt("phoenix_standard", "Phoenix Standard", {
  source: "Bastion faction ability",
  when: "at the end of a combat",
  factions: ["bastion"],
  option: ["ability", "phoenixstandard"],
});
prompt("proxima_targeting", "Proxima Targeting VI", {
  source: "Bastion technology",
  when: "at the start of a ground combat round",
  factions: ["bastion"],
  option: ["technology", "proxima"],
});
prompt("cabal_agent", "The Stillness of Stars", {
  source: "Vuil'raith agent",
  when: "after another player replenishes commodities",
  factions: ["cabal"],
  option: ["leader", "cabalagent"],
});
prompt("crimson_exile", "Exile", {
  source: "Crimson Rebellion destroyer",
  when: "at the end of a combat in or next to its system",
  factions: ["crimson"],
  option: ["unit", "crimson_destroyer"],
});
prompt("subatomic_splicer", "Subatomic Splicer", {
  source: "Crimson Rebellion technology",
  when: "when one of your ships is destroyed",
  factions: ["crimson"],
  option: ["technology", "subatomic"],
});
prompt("null_reference", "Null Reference", {
  source: "Nekro technology",
  when: "when one of your ships is destroyed",
  factions: ["nekro"],
  option: ["technology", "nekroc4y"],
});
prompt("technological_singularity", "Technological Singularity", {
  source: "Nekro faction ability",
  when: "after an opponent's unit is destroyed",
  factions: ["nekro"],
  option: ["ability", "technological_singularity"],
});
prompt("aetherstream", "Aetherstream", {
  source: "Empyrean technology",
  when: "after you or a neighbor activates a system next to an anomaly",
  factions: ["empyrean"],
  option: ["technology", "as"],
});
prompt("empyrean_agent", "Acamar", {
  source: "Empyrean agent",
  when: "after a player moves ships into a system without planets",
  factions: ["empyrean"],
  option: ["leader", "empyreanagent"],
});
prompt("empyrean_commander", "Xuange", {
  turns: "others",
  source: "Empyrean commander",
  when: "after another player moves ships into a system with your command token",
  factions: ["empyrean"],
  option: ["leader", "empyreancommander"],
});
prompt("dynamo", "Dynamo", {
  source: "Empyrean flagship",
  when: "after any unit nearby sustains damage",
  factions: ["empyrean"],
  option: ["unit", "empyrean_flagship"],
});
prompt("watcher", "Watcher", {
  source: "Empyrean mech",
  when: "when an action card is played",
  factions: ["empyrean"],
  option: ["unit", "empyrean_mech"],
});
prompt("void_tether", "Void Tether", {
  turns: "others",
  source: "Empyrean breakthrough",
  when: "after another player activates a system",
  factions: ["empyrean"],
  option: ["breakthrough", "empyreanbt_allow"],
});
prompt("firmament_agent", "Myru Vos", {
  source: "Firmament agent",
  when: "after a player activates a system",
  factions: ["firmament"],
  option: ["leader", "firmamentagent"],
});
prompt("ghost_agent", "Emissary Taivra", {
  source: "Creuss agent",
  when: "after a player activates a system with a wormhole",
  factions: ["ghost"],
  option: ["leader", "ghostagent"],
});
prompt("icarus_drive", "Icarus Drive", {
  source: "Creuss mech",
  when: "after any player activates a system",
  factions: ["ghost"],
  option: ["unit", "ghost_mech"],
});
prompt("laws_order", "Laws Order", {
  source: "Keleres faction ability",
  when: "at the start of every turn while a law is in play",
  factions: ["keleres"],
  option: ["ability", "laws_order"],
});
prompt("pillage", "Pillage", {
  source: "Mentak faction ability",
  when: "after a neighbor gains trade goods or finishes a transaction",
  factions: ["mentak"],
  option: ["ability", "pillage"],
});
prompt("ambush", "Ambush", {
  source: "Mentak faction ability",
  when: "at the start of a space combat",
  factions: ["mentak"],
  subtypes: ["ambush_roll"],
});
prompt("muaat_commander", "Magmus", {
  source: "Muaat commander",
  when: "after you spend a strategy token",
  factions: ["muaat"],
  option: ["leader", "muaatcommander"],
});
prompt("naalu_agent", "Z'eu", {
  source: "Naalu agent",
  when: "after any player places a command token in a system",
  factions: ["naalu"],
  option: ["leader", "naaluagent-te"],
});
prompt("foresight", "Foresight", {
  turns: "others",
  source: "Naalu faction ability",
  when: "after another player's movement",
  factions: ["naalu"],
  option: ["ability", "foresight"],
});
prompt("naaz_agent", "Garv and Gunn", {
  source: "Naaz-Rokha agent",
  when: "at the end of a player's turn",
  factions: ["naaz"],
  option: ["leader", "naazagent"],
});
prompt("thunders_paradox", "Thunder's Paradox", {
  source: "Nomad breakthrough",
  when: "at the start of every turn",
  factions: ["nomad"],
  option: ["breakthrough", "nomadbt"],
});
prompt("temporal_command_suite", "Temporal Command Suite", {
  source: "Nomad technology",
  when: "after any player's agent is exhausted",
  factions: ["nomad"],
  option: ["technology", "tcs"],
});
prompt("nomad_artuno", "Artuno the Betrayer", {
  source: "Nomad agent",
  when: "when you gain trade goods",
  factions: ["nomad"],
  option: ["leader", "nomadagentartuno"],
});
prompt("nomad_mercer", "Field Marshal Mercer", {
  source: "Nomad agent",
  when: "at the end of a player's tactical action",
  factions: ["nomad"],
  option: ["leader", "nomadagentmercer"],
});
prompt("nomad_thundarian", "The Thundarian", {
  source: "Nomad agent",
  when: "after the Roll Dice step of any combat round",
  factions: ["nomad"],
  option: ["leader", "nomadagentthundarian"],
});
prompt("obsidian_agent", "Vos Hollow", {
  source: "Obsidian agent",
  when: "when a ship is destroyed",
  factions: ["obsidian"],
  option: ["leader", "obsidianagent"],
});
prompt("alarum", "Alarum", {
  source: "Ral Nel mech",
  when: "at the end of a ground combat round on its planet",
  factions: ["ralnel"],
  option: ["unit", "ralnel_mech"],
});
prompt("saar_agent", "Captain Mendosa", {
  source: "Saar agent",
  when: "when a player activates a system",
  factions: ["saar"],
  option: ["leader", "saaragent"],
});
prompt("exotrireme", "Exotrireme II", {
  source: "N'orr technology",
  when: "after every round of space combat",
  factions: ["sardakk"],
  option: ["technology", "exo2"],
});
prompt("sardakk_agent", "T'ro", {
  source: "N'orr agent",
  when: "at the end of a player's tactical action",
  factions: ["sardakk"],
  option: ["leader", "sardakkagent"],
});
prompt("titans_agent", "Tellurian", {
  source: "Titans agent",
  when: "before a hit would be assigned",
  factions: ["titans"],
  option: ["leader", "titansagent"],
});
prompt("titans_commander", "Tungstantus", {
  turns: "own",
  source: "Titans commander",
  when: "when your units use PRODUCTION",
  factions: ["titans"],
  option: ["leader", "titanscommander"],
});
prompt("hegemonic_trade_policy", "Hegemonic Trade Policy", {
  turns: "own",
  source: "Winnu technology",
  when: "when your units use PRODUCTION",
  factions: ["winnu"],
  option: ["technology", "htp"],
});
prompt("winnu_agent", "Berekar Berekon", {
  source: "Winnu agent",
  when: "when a player's units use PRODUCTION",
  factions: ["winnu"],
  option: ["leader", "winnuagent"],
});
prompt("yin_spinner", "Yin Spinner Omega", {
  turns: "own",
  source: "Yin technology",
  when: "after you produce units",
  factions: ["yin"],
  option: ["technology", "yso"],
});
prompt("yin_agent", "Brother Milor", {
  source: "Yin agent",
  when: "after a player's unit is destroyed in combat",
  factions: ["yin"],
  option: ["leader", "yinagent"],
});
prompt("yssaril_commander", "So Ata", {
  turns: "others",
  source: "Yssaril commander",
  when: "after another player activates a system with your units",
  factions: ["yssaril"],
  option: ["leader", "yssarilcommander"],
});
prompt("sol_agent", "Evelyn DeLouis", {
  source: "Sol agent",
  when: "at the start of a ground combat round",
  factions: ["sol"],
  option: ["leader", "solagent"],
});
prompt("letnev_agent", "Viscount Unlenn", {
  source: "Letnev agent",
  when: "at the start of a space combat round",
  factions: ["letnev"],
  option: ["leader", "letnevagent"],
});
prompt("sol_commander", "Claire Gibson", {
  source: "Sol commander",
  when: "at the start of a ground combat on a planet you control",
  factions: ["sol"],
  option: ["leader", "solcommander"],
});

// -- Questions the ability asks by itself (no reaction window option) ------------------------------
prompt("munitions_reserves", "Munitions Reserves", {
  source: "Letnev faction ability",
  when: "in every space combat round",
  factions: ["letnev"],
  subtypes: ["munitions_reserves_reroll"],
});
prompt("letnev_commander", "Rear Admiral Farran", {
  source: "Letnev commander",
  when: "after one of your units sustains damage",
  factions: ["letnev"],
  subtypes: ["letnev_commander_trade_good"],
});
prompt("wrath_of_kenara", "Wrath of Kenara", {
  source: "Hacan flagship",
  when: "after you roll combat dice",
  factions: ["hacan"],
  subtypes: ["wrath_of_kenara_bump"],
});
prompt("self_assembly_routines", "Self-Assembly Routines", {
  turns: "own",
  source: "technology",
  when: "after your units use PRODUCTION",
  technologies: ["sar"],
  subtypes: ["self_assembly_mech"],
});
prompt("ai_development_algorithm", "AI Development Algorithm", {
  turns: "own",
  source: "technology",
  when: "when your units use PRODUCTION",
  technologies: ["aida"],
  subtypes: ["exhaust_for_production_discount"],
});
prompt("agency_supply_network", "Agency Supply Network", {
  turns: "own",
  source: "Keleres technology",
  when: "when your units use PRODUCTION",
  technologies: ["asn"],
  subtypes: ["asn_system"],
});
prompt("scanlink_drone_network", "Scanlink Drone Network", {
  turns: "own",
  source: "technology",
  when: "when you activate a system",
  technologies: ["sdn"],
  subtypes: ["scanlink_explore"],
});
prompt("spatial_conduit_cylinders", "Spatial Conduit Cylinders", {
  turns: "own",
  source: "Jol-Nar technology",
  when: "after you activate a system",
  factions: ["jolnar"],
  technologies: ["scc"],
  subtypes: ["spatial_conduit_link"],
});
prompt("bio_stims", "Bio-Stims", {
  turns: "own",
  source: "technology",
  when: "at the end of your turn",
  technologies: ["bs"],
  subtypes: ["bio_stims_ready"],
});
prompt("predictive_intelligence", "Predictive Intelligence", {
  turns: "own",
  source: "technology",
  when: "at the end of your turn and when you vote",
  technologies: ["pi"],
  subtypes: ["predictive_intelligence_redistribute", "vote_predictive_intelligence"],
});
prompt("indoctrination", "Indoctrination", {
  source: "Yin faction ability",
  when: "at the start of a ground combat against infantry",
  factions: ["yin"],
  subtypes: ["indoctrination_replace"],
});
prompt("jolnar_commander", "Agnlan Oln", {
  source: "Jol-Nar commander",
  when: "after your units roll for a unit ability (one question per die)",
  factions: ["jolnar"],
  subtypes: ["jolnar_commander_reroll"],
});
prompt("crown_of_thalnos", "The Crown of Thalnos", {
  source: "relic",
  when: "in each combat round (one question per die)",
  relics: ["thalnos"],
  subtypes: ["crown_of_thalnos_relic_reroll", "crown_of_thalnos_law_reroll"],
});
prompt("heart_of_ixth", "Heart of Ixth", {
  source: "relic",
  when: "when you roll combat dice",
  relics: ["heartofixth"],
  subtypes: ["heart_ixth_die_adjust"],
});

export const PAUSABLE_PROMPTS: readonly PausablePrompt[] = REGISTRY;

const BY_ID = new Map(REGISTRY.map((entry) => [entry.id, entry]));

export function pausablePrompt(id: string): PausablePrompt | undefined {
  return BY_ID.get(id);
}

/** The storage key of an ability's switch. */
export function askStorageKey(id: string): string {
  return `player_ask_${id}`;
}

/** Whether a reaction-window option id belongs to this ability (its own card or a Ssruu copy). */
export function optionMatches(entry: PausablePrompt, optionId: string): boolean {
  if (!entry.option) return false;
  const parts = optionId.split(":");
  return parts[0] === entry.option.kind && parts.slice(1).includes(entry.option.token);
}

/** The ability a reaction-window option offers, if it is one of the registered abilities. */
export function promptForOption(optionId: string): PausablePrompt | undefined {
  return REGISTRY.find((entry) => optionMatches(entry, optionId));
}

/**
 * The abilities that ask this question by its decision subtype. A subtype several abilities share
 * is told apart by the question's content source (`context.source.Content`); without a source
 * only the abilities that need none match.
 */
export function promptsForSubtype(subtype: string | undefined, contentSource?: string | null): PausablePrompt[] {
  if (!subtype) return [];
  return REGISTRY.filter(
    (entry) =>
      entry.subtypes.includes(subtype) &&
      (entry.contentSources.length === 0 || (contentSource != null && entry.contentSources.includes(contentSource))),
  );
}

/** The content alias a question names as its source (`{ Content: "minister_peace" }`), if any. */
export function contentSourceOf(choice: Pick<PendingChoiceDto, "context"> | null | undefined): string | null {
  const source = choice?.context?.source as Record<string, unknown> | undefined;
  const value = source?.Content;
  return typeof value === "string" ? value : null;
}

/** Every registered ability the question is about (its window options and its own subtype). */
export function promptsOfChoice(choice: PendingChoiceDto | null | undefined): PausablePrompt[] {
  if (!choice) return [];
  const found: PausablePrompt[] = [];
  const add = (entry: PausablePrompt | undefined) => {
    if (entry && !found.includes(entry)) found.push(entry);
  };
  for (const option of choice.options) {
    if (option.kind === "decline") continue;
    add(promptForOption(option.id));
  }
  promptsForSubtype(choice.context?.subtype, contentSourceOf(choice)).forEach(add);
  return found;
}

/** What the viewer's seat holds that can grant an ability. */
export interface PromptHolder {
  faction?: string | null;
  technologies?: readonly string[] | null;
  relics?: readonly string[] | null;
  /** Laws (agenda aliases) the seat owns right now. */
  laws?: readonly string[] | null;
}

/** The aliases of the laws a seat owns, from the table's `laws` map (alias to the elected seat). */
export function lawsOwnedBy(seat: string | null | undefined, laws: Readonly<Record<string, string>> | null | undefined): string[] {
  if (!seat || !laws) return [];
  return Object.entries(laws)
    .filter(([, owner]) => owner === seat)
    .map(([alias]) => alias);
}

/** Whether the seat has the ability (by faction, technology or relic). */
export function holdsPrompt(entry: PausablePrompt, holder: PromptHolder | null | undefined): boolean {
  if (!holder) return false;
  const faction = holder.faction ?? "";
  if (faction && entry.factions.some((f) => faction === f || faction.startsWith(f))) return true;
  if (holder.technologies?.some((t) => entry.technologies.includes(t))) return true;
  if (holder.relics?.some((r) => entry.relics.includes(r))) return true;
  if (holder.laws?.some((l) => entry.laws.includes(l))) return true;
  return false;
}

/** The abilities the seat has, in the order of the registry. */
export function promptsHeldBy(holder: PromptHolder | null | undefined): PausablePrompt[] {
  return REGISTRY.filter((entry) => holdsPrompt(entry, holder));
}
