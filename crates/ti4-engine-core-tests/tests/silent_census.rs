//! Census tests of the silent gaps (docs/SILENT_GAPS_18.md, N-02 and N-03): the checks that turn the static scripts `tools/engine_core/silent_gaps_d.py` and
//! `silent_gaps_def.py` into guards, so that a rule that exists in the string engine and is simply absent from the core cannot hide behind an empty slot.
//!
//! * `every_hook_slot_without_a_provider_is_accounted_for`: a slot of the hook table that no module fills looks exactly like a rule that is not ported.
//!   Every empty slot of the full table must be in `EMPTY_SLOTS` with the reason it is empty, and with how the core reads it; a slot that gets a provider,
//!   or a read that appears where there was none, fails the test until the table says so.
//! * `every_decision_subtype_the_string_asks_is_rendered_or_accounted_for`: the observed corpus of the string's decision subtypes against the literals the
//!   renderers of the core produce.
//! * `the_core_names_no_technology_the_deck_does_not_hold` and `every_deck_technology_is_read_by_the_core_or_accounted_for`: the catalog census.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ti4_engine_core::generated::Technology;
use ti4_engine_core::query::tech::in_deck;
use ti4_engine_core_tests::kernel::kernel_all;

fn crates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("crates directory").to_path_buf()
}

fn repo_dir() -> PathBuf {
    crates_dir().parent().expect("repository root").to_path_buf()
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.filter_map(Result::ok).map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "generated" || n == "tests") {
                continue;
            }
            walk(&p, out);
        } else if p.extension().is_some_and(|e| e == "rs") && !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("__") || n.to_string_lossy().ends_with("_tests.rs")) {
            out.push(p);
        }
    }
}

/// `(path relative to crates/, text)` of the production code of a crate: everything above the first `#[cfg(test)]`, whole-line comments blanked.
fn production(root: &str) -> Vec<(String, String)> {
    let mut files = Vec::new();
    walk(&crates_dir().join(root).join("src"), &mut files);
    assert!(files.len() > 50, "the walk of {root} found only {} files", files.len());
    files
        .into_iter()
        .map(|f| {
            let text = std::fs::read_to_string(&f).expect("readable source");
            let kept: Vec<&str> = text.lines().take_while(|l| !l.trim_start().starts_with("#[cfg(test)]")).map(|l| if l.trim_start().starts_with("//") { "" } else { l }).collect();
            (f.strip_prefix(crates_dir()).expect("under crates").to_string_lossy().replace('\\', "/"), kept.join("\n"))
        })
        .collect()
}

// ---- the hook slots -------------------------------------------------------------------------------------------------------------

/// How the core reads a slot: `read` (a rule calls it), `bridge` (only the seam adapter calls it, `kernel/seam/adapters.rs`), `unread` (nothing calls it).
const READ: &str = "read";
const BRIDGE: &str = "bridge";
const UNREAD: &str = "unread";

/// Every slot of the table that no module fills, how the core reads it, and why it is empty. A reason starts with its class: `dropped` (the faction is out of
/// scope), `reserved` (the string engine declares the slot and no faction fills it either), `native` (the rule exists in the core, in a window or a rule
/// of its own), `seam` (the module registers the seam, which the rule reads), `live` (the live kernel fills it, not a module).
const EMPTY_SLOTS: &[(&str, &str, &str)] = &[
    ("cards.borrowed_agent_used", UNREAD, "reserved: `hooks_cards::borrowed_agent_used` is a helper the copying factions call, not a slot one fills; the Ssruu copy is recorded by the leader windows"),
    ("cards.borrowed_agent_used_state", UNREAD, "reserved: as `borrowed_agent_used`"),
    ("cards.vote_bonus_with_content", READ, "reserved: the string declares the slot (`hooks_cards`), no faction fills it (only tests)"),
    ("combat.afb_excess", UNREAD, "native: Argent's Raid Formation is `rules::combat::raid_formation`, run by the combat machine"),
    ("combat.borrowed_stats", BRIDGE, "seam: Nomad's The Cavalry registers `seam.borrowed_stats` (`nomad::query::register_borrowed`), read by `rules::combat`"),
    ("combat.direct_hit_immune", BRIDGE, "native: Exotrireme II is `silent::g3::exo2`, a rule of its own; the string provider is a test"),
    ("combat.grants_sustain", READ, "seam: Nomad's Quantum Manipulator and The Cavalry register `seam.grants_sustain`, which `sustains_in_space` reads next to the slot (silent-fix 18)"),
    ("combat.may_sustain", BRIDGE, "seam: Mentak's Fourth Moon (and Crimson, dropped) register `seam.may_sustain`, read by `sustains_in_space`"),
    ("combat.produced_hits", UNREAD, "native: Mentak's Ambush, Ghost's Dimensional Splicer and Yin's Impulse Core are `rules::combat::produce`"),
    ("direct.bastion_commander_blocks_sabotage", READ, "dropped: Last Bastion"),
    ("direct.bastion_note_retreat", READ, "dropped: Last Bastion"),
    ("direct.borrowed_round_agents_abilities", UNREAD, "native: Ssruu's copy of a round agent is `Ask::Leaders(RoundAgentUnit { copied })` and the round mark"),
    ("direct.borrowed_round_agents_extra_die_for", BRIDGE, "live: the live kernel fills it (`ti4-engine/src/live/kernel.rs`), no module registers it"),
    ("direct.borrowed_round_agents_has_round_copy", UNREAD, "native: as `borrowed_round_agents_abilities`"),
    ("direct.cabal_note_replenished", BRIDGE, "native: `silent::cabal` stages COMMODITIES_REPLENISHED itself"),
    ("direct.context_silenced", BRIDGE, "reserved: \"reserved for C shards\"; no faction of the string engine fills it"),
    ("direct.crimson_can_deploy", UNREAD, "dropped: Crimson Rebellion"),
    ("direct.crimson_enforce_sundered", UNREAD, "dropped: Crimson Rebellion"),
    ("direct.crimson_place_sorrow_breach", UNREAD, "dropped: Crimson Rebellion"),
    ("direct.deepwrought_begin_coexisting", UNREAD, "dropped: Deepwrought"),
    ("direct.deepwrought_research_counts", UNREAD, "dropped: Deepwrought"),
    ("direct.deepwrought_research_open", UNREAD, "dropped: Deepwrought"),
    ("direct.deepwrought_research_settle", UNREAD, "dropped: Deepwrought"),
    ("direct.firmament_plot_options", UNREAD, "dropped: Firmament"),
    ("direct.firmament_plots_assail_space", BRIDGE, "dropped: Firmament"),
    ("direct.firmament_score_as_plot", UNREAD, "dropped: Firmament"),
    ("direct.keleres_offer_agent", UNREAD, "dropped: Keleres"),
    ("direct.keleres_reconcile", UNREAD, "dropped: Keleres"),
    ("direct.keleres_take_executive_order", UNREAD, "dropped: Keleres"),
    ("direct.keleres_units_commit_blocked", BRIDGE, "dropped: Keleres"),
    ("direct.keleres_units_pay_commit_toll", UNREAD, "dropped: Keleres"),
    ("direct.mahact_units_flagship_roll_bonus", BRIDGE, "seam: `cc_ground::mahact::query` registers the roll bonus on `seam.hit_shift`"),
    ("direct.mahact_units_offer_starlancer", UNREAD, "native: `driver/windows/tactical/hooks.rs` offers the Starlancer"),
    ("direct.nekro_units_mech_roll_bonus", BRIDGE, "seam: `ca_combat::nekro::query` registers the roll bonus on `seam.hit_shift`"),
    ("direct.nomad_agents_agent_readied", UNREAD, "native: the Nomad agents are timing abilities (`ce_movement::nomad::timing`)"),
    ("direct.ralnel_cards_skimmer_takes", UNREAD, "dropped: Ral Nel"),
    ("direct.winnu_exhaust_agent_for_production", UNREAD, "native: `rules::production::window` exhausts Berekar Berekon itself"),
    ("economy.action_card_draw_requested", UNREAD, "native: Yssaril's Scheming is `rules::action_cards::draw` and `status_bookkeeping`"),
    ("economy.action_cards_drawn", UNREAD, "native: as `action_card_draw_requested`"),
    ("economy.pds_placement_alternative", UNREAD, "native: Titans' Hecatoncheires is in `rules::production::strategy_cards`"),
    ("economy.pds_placement_alternative_performed", UNREAD, "native: as `pds_placement_alternative`"),
    ("general.combat_modifier", READ, "reserved: the string declares the slot, no faction fills it (the modifiers are seam stacks)"),
    ("general.component_actions", UNREAD, "native: the faction actions are listed by `driver/menu_faction.rs` and performed by `driver/turn/faction.rs`"),
    ("general.control_gained", READ, "dropped: Last Bastion"),
    ("general.fleet_supply", READ, "reserved: the string declares the slot, no faction fills it"),
    ("general.ground_combat_round_ended", UNREAD, "reserved: the string declares the slot, no faction fills it"),
    ("general.ignores_neighbours", READ, "reserved: the string declares the slot, no faction fills it"),
    ("general.leader_strategy_followers", UNREAD, "native: Mathis Mathinus' followers are asked by the hero window (`rules::hero_windows`)"),
    ("general.mapped_component_actions", UNREAD, "native: as `component_actions`"),
    ("general.perform_component", UNREAD, "native: as `component_actions`"),
    ("general.secondary_is_free", READ, "reserved: the string declares the slot, no faction fills it"),
    ("general.status_tokens", READ, "reserved: the string declares the slot, no faction fills it"),
    ("general.strategy_resolved", READ, "reserved: the string declares the slot, no faction fills it"),
    ("general.timing_abilities", UNREAD, "native: abilities are specs of the reaction table (`AbilityTable::specs`), not a hook"),
    ("general.trades_action_cards", READ, "reserved: the string declares the slot, no faction fills it"),
    ("general.use_leader", UNREAD, "native: leader actions run through `driver/turn/hero.rs` and the leader windows"),
    ("general.use_leader_strategy_primary", UNREAD, "native: as `use_leader`"),
    ("general.use_leader_timed", UNREAD, "native: as `use_leader`"),
    ("ground.may_sustain", READ, "seam: Mentak's Moll Terminus registers `seam.may_sustain`, which `can_sustain_here` reads next to the slot (silent-fix 18)"),
];

/// How the production code of the core reads `slot` (`group.name`): outside the hook declarations, a call `hooks.group.name(`.
fn reads_of(files: &[(String, String)], slot: &str) -> &'static str {
    // (`hooks.combat.may_sustain(` and `hooks.ground.may_sustain(` are two slots of one name.)
    let (group, name) = slot.split_once('.').expect("group.name");
    // (A `direct` slot has a name of its own, and the adapters call it through a local alias of the group.)
    let calls = [format!("hooks.{slot}("), if group == "direct" { format!(".{name}(") } else { String::new() }];
    let hits: Vec<&str> = files
        .iter()
        .filter(|(path, text)| !path.contains("/kernel/hooks/") && calls.iter().any(|c| !c.is_empty() && text.contains(c.as_str())))
        .map(|(path, _)| path.as_str())
        .collect();
    if hits.is_empty() {
        UNREAD
    } else if hits.iter().all(|p| p.ends_with("kernel/seam/adapters.rs")) {
        BRIDGE
    } else {
        READ
    }
}

#[test]
fn every_hook_slot_without_a_provider_is_accounted_for() {
    let k = kernel_all();
    ti4_engine_core_tests::kernel::assert_hooks_wired(&k);
    let empty: BTreeSet<String> = k.hooks.empty_slots().into_iter().collect();
    let accounted: BTreeSet<String> = EMPTY_SLOTS.iter().map(|(slot, _, _)| (*slot).to_owned()).collect();
    assert_eq!(accounted.len(), EMPTY_SLOTS.len(), "a slot is listed twice");
    let unaccounted: Vec<&String> = empty.difference(&accounted).collect();
    assert!(unaccounted.is_empty(), "slots no module fills and the census does not know (a rule absent from the core looks like this): {unaccounted:?}");
    let stale: Vec<&String> = accounted.difference(&empty).collect();
    assert!(stale.is_empty(), "slots the census lists as empty that now have a provider: {stale:?}");
    let files = production("ti4-engine-core");
    let mut wrong = Vec::new();
    for (slot, reads, reason) in EMPTY_SLOTS {
        let class = reason.split(':').next().unwrap_or_default();
        assert!(["dropped", "reserved", "native", "seam", "live"].contains(&class) && reason.len() > class.len() + 3, "{slot}: a reason that names its class");
        let found = reads_of(&files, slot);
        if found != *reads {
            wrong.push(format!("{slot}: the table says {reads}, the source says {found}"));
        }
    }
    assert!(wrong.is_empty(), "how the core reads an empty slot changed (a new read of a slot nobody fills is a gap until proven otherwise): {wrong:#?}");
}

// ---- the decision subtypes -----------------------------------------------------------------------------------------------------

/// The string literals of a piece of Rust source (escapes resolved for `\"` and `\\` only; a char literal never holds a quote here).
fn string_literals(text: &str) -> Vec<String> {
    let (mut out, mut cur, mut inside, mut escaped) = (Vec::new(), String::new(), false, false);
    for c in text.chars() {
        if inside {
            if escaped {
                cur.push(c);
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                out.push(std::mem::take(&mut cur));
                inside = false;
            } else {
                cur.push(c);
            }
        } else if c == '"' {
            inside = true;
        }
    }
    out
}

/// The literals and the `{}` templates of every `fn subtype` of the renderers of the core (`crates/ti4-engine-next/src/render/*.rs`, `mod.rs` aside).
fn rendered_subtypes() -> (BTreeSet<String>, Vec<Vec<String>>) {
    let dir = crates_dir().join("ti4-engine-next/src/render");
    let (mut lits, mut tpls) = (BTreeSet::new(), Vec::new());
    let mut files: Vec<_> = std::fs::read_dir(&dir).expect("the render directory").filter_map(Result::ok).map(|e| e.path()).collect();
    files.sort();
    for f in files.iter().filter(|f| f.extension().is_some_and(|e| e == "rs") && !f.ends_with("mod.rs")) {
        let text = std::fs::read_to_string(f).expect("readable source");
        let text = text.split("#[cfg(test)]").next().unwrap_or_default();
        let Some(at) = text.find("fn subtype(") else { continue };
        let body = &text[at..];
        let body = &body[..body.find("\n    }\n").map_or(body.len(), |end| end + 7)];
        for s in string_literals(body) {
            if s.contains('{') {
                tpls.push(s.split("{}").map(str::to_owned).collect());
            } else if !s.is_empty() {
                lits.insert(s);
            }
        }
    }
    assert!(lits.len() > 150, "found only {} subtype literals: the scan is broken", lits.len());
    (lits, tpls)
}

fn template_matches(parts: &[String], s: &str) -> bool {
    let (Some(first), Some(last)) = (parts.first(), parts.last()) else { return false };
    if parts.len() < 2 || !s.starts_with(first.as_str()) || !s.ends_with(last.as_str()) || s.len() < first.len() + last.len() {
        return false;
    }
    // The parts in order, each placeholder holding at least one character.
    let mut at = first.len();
    let end = s.len() - last.len();
    for part in &parts[1..parts.len() - 1] {
        let from = at + 1;
        let Some(found) = s.get(from..end).and_then(|window| window.find(part.as_str())) else { return false };
        at = from + found + part.len();
    }
    at < end || parts.len() == 2 && at < end
}

/// `text` without its `#[cfg(test)]` items: up to the `;` of a `use` or `mod x;`, or through the braces of a `mod` or a function. (Braces inside strings and
/// char literals are counted like any other; the production code of the string engine keeps them balanced.)
fn without_test_items(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find("#[cfg(test)]") {
        out.push_str(&rest[..at]);
        let after = &rest[at + "#[cfg(test)]".len()..];
        let (semi, open) = (after.find(';'), after.find('{'));
        rest = match (semi, open) {
            (Some(s), Some(o)) if s < o => &after[s + 1..],
            (Some(s), None) => &after[s + 1..],
            (_, Some(o)) => {
                let mut depth = 0_i32;
                let mut end = after.len();
                for (i, c) in after[o..].char_indices() {
                    match c {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                end = o + i + 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                &after[end..]
            }
            (None, None) => "",
        };
    }
    out.push_str(rest);
    out
}

/// The third argument of every `DecisionContext::new(` of the production code of the string engine when it is a plain literal: the subtypes the code
/// names at its own call site (the corpus holds the ones the games reached, this holds the ones nobody reached).
fn string_engine_literal_subtypes() -> BTreeSet<String> {
    let mut files = Vec::new();
    walk(&crates_dir().join("ti4-engine/src"), &mut files);
    let mut out = BTreeSet::new();
    for f in files {
        let rel = f.strip_prefix(crates_dir().join("ti4-engine/src")).expect("under src").to_string_lossy().replace('\\', "/");
        if rel.starts_with("render/") || rel == "decision_context.rs" || rel.ends_with("tests.rs") {
            continue;
        }
        let text = std::fs::read_to_string(&f).expect("readable source");
        let text = without_test_items(&text);
        for (at, _) in text.match_indices("DecisionContext::new(") {
            let rest = &text[at + "DecisionContext::new(".len()..];
            // The arguments up to the closing parenthesis of the call, split at the commas of depth 0 (strings skipped).
            let (mut depth, mut args, mut cur, mut quoted, mut escaped) = (0_i32, Vec::new(), String::new(), false, false);
            for c in rest.chars() {
                if quoted {
                    cur.push(c);
                    if escaped {
                        escaped = false;
                    } else if c == '\\' {
                        escaped = true;
                    } else if c == '"' {
                        quoted = false;
                    }
                    continue;
                }
                match c {
                    '"' => {
                        quoted = true;
                        cur.push(c);
                    }
                    '(' | '[' | '{' => {
                        depth += 1;
                        cur.push(c);
                    }
                    ')' | ']' | '}' if depth == 0 => {
                        args.push(std::mem::take(&mut cur));
                        break;
                    }
                    ')' | ']' | '}' => {
                        depth -= 1;
                        cur.push(c);
                    }
                    ',' if depth == 0 => args.push(std::mem::take(&mut cur)),
                    _ => cur.push(c),
                }
            }
            if let Some(third) = args.get(2) {
                let third = third.trim();
                if third.len() > 2 && third.starts_with('"') && third.ends_with('"') && !third[1..third.len() - 1].contains('"') {
                    out.insert(third[1..third.len() - 1].to_owned());
                }
            }
        }
    }
    out
}

/// The subtypes the string engine asks (the observed corpus of `tools/rulebot/d3/subtypes.tsv`, and the literals at its `DecisionContext::new` call
/// sites) that no renderer of the core produces, and why the core does not need one: `dropped` (an out-of-scope faction), `thunders-edge` (out of scope
/// by decision), `structured` (a decision the thin crate delivers as a structured window, not a choice), `nongame` (a bot-side or test-only text), `helper` (rendered, but by a constant or a helper the scan of `fn subtype` literals does not see).
const UNRENDERED: &[(&str, &str)] = &[
    ("asn_system", "dropped: Keleres (Agency Supply Network)"),
    ("assault_cannon_destroy", "helper: rendered by `rules/combat/assault_cannon.rs` through its own constant"),
    ("assign_casualty", "helper: rendered by `rules/combat/assign.rs` (the most frequent question of the corpus)"),
    ("avernus_placement", "thunders-edge: Stellar Genesis"),
    ("courageous_to_the_end_assign_casualty", "helper: interned by code and rendered with the casualty question"),
    ("crimson_payment", "dropped: Crimson Rebellion"),
    ("deepgloom_allow_scheming", "thunders-edge: Deepgloom Executable"),
    ("deepgloom_allow_stall_tactics", "thunders-edge: Deepgloom Executable"),
    ("deepwrought_agent_exhaust", "dropped: Deepwrought"),
    ("deepwrought_agent_place", "dropped: Deepwrought"),
    ("deepwrought_payment", "dropped: Deepwrought"),
    ("deepwrought_reduce_research", "dropped: Deepwrought"),
    ("diplomacy_agenda_talks", "structured: the diplomacy window is delivered as a structured window, not a choice"),
    ("diplomacy_response", "structured: as `diplomacy_agenda_talks`"),
    ("entropic_scar_gain_faction_technology", "thunders-edge: Entropic Scar"),
    ("expedition_discard_action_card", "thunders-edge: Expeditions"),
    ("expedition_discard_secret", "thunders-edge: Expeditions"),
    ("expedition_exhaust_planet", "thunders-edge: Expeditions"),
    ("expedition_place_system", "thunders-edge: Expeditions"),
    ("expedition_placement_tiebreak", "thunders-edge: Expeditions"),
    ("fracture_choose_ingress_system", "thunders-edge: the Fracture"),
    ("ingress_move", "thunders-edge: Ingress (Alraith Ix Ixth, Planesplitter)"),
    ("leader_hacanagent_branch", "helper: `render/leaders.rs` renders it through `HACAN_KIND`"),
    ("leader_jolnarhero_swap", "helper: `render/leaders.rs` renders it through `JOLNAR_KIND`"),
    ("leader_ssruu_round_agent_unit", "helper: `render/leaders.rs` renders it through `ROUND_KIND`"),
    ("leader_xxchahero_te_place", "helper: `render/leaders.rs` renders it through `HERO_PLACE_KIND`"),
    ("leader_xxchahero_te_planet", "helper: `render/leaders.rs` renders it through `HERO_PLANET_KIND`"),
    ("letnev_commander_trade_good", "helper: `render/leaders.rs` renders it through `KIND`"),
    ("mid_action_pause", "nongame: the bot-side pause text of the aftermath window"),
    ("produce_units", "nongame: a literal that is no decision subtype"),
    ("raid_formation_damage", "helper: `render/combat.rs` renders it through its own constant"),
    ("research_team_coexist", "thunders-edge: Coexistence research team"),
    ("research_team_defend_coexist", "thunders-edge: Coexistence research team"),
    ("slumberstate_allow", "thunders-edge: Slumberstate"),
    ("slumberstate_planet", "thunders-edge: Slumberstate"),
    ("specialist_compounds_choose_planet", "thunders-edge: Specialist Compounds"),
    ("tether_border", "thunders-edge: Void Tether"),
    ("thunders_paradox", "thunders-edge: Thunder's Paradox"),
];

#[test]
fn every_decision_subtype_the_string_asks_is_rendered_or_accounted_for() {
    let corpus = std::fs::read_to_string(repo_dir().join("tools/rulebot/d3/subtypes.tsv")).expect("the observed corpus");
    let mut asked: BTreeSet<String> = corpus.lines().skip(1).filter(|l| !l.is_empty() && !l.starts_with('#')).filter_map(|l| l.split('\t').next()).map(str::to_owned).collect();
    assert!(asked.len() > 300, "the corpus has {} subtypes", asked.len());
    let at_sites = string_engine_literal_subtypes();
    assert!(at_sites.len() > 100, "the scan of the call sites found {} literals", at_sites.len());
    asked.extend(at_sites);
    let (lits, tpls) = rendered_subtypes();
    let unrendered: BTreeSet<&str> = asked.iter().map(String::as_str).filter(|s| !s.starts_with('~') && !lits.contains(*s) && !tpls.iter().any(|t| template_matches(t, s))).collect();
    let accounted: BTreeSet<&str> = UNRENDERED.iter().map(|(s, _)| *s).collect();
    let missing: Vec<&&str> = unrendered.difference(&accounted).collect();
    assert!(missing.is_empty(), "subtypes the string asks and no renderer of the core produces: {missing:?}");
    let stale: Vec<&&str> = accounted.difference(&unrendered).collect();
    assert!(stale.is_empty(), "subtypes listed as unrendered that a renderer produces now: {stale:?}");
    for (s, reason) in UNRENDERED {
        let class = reason.split(':').next().unwrap_or_default();
        assert!(["dropped", "thunders-edge", "structured", "nongame", "helper"].contains(&class), "{s}: a reason that names its class");
    }
}

// ---- the technology catalog -------------------------------------------------------------------------------------------------

/// The text of both core crates (production code, comments blanked), for the mentions of `Technology::Variant`.
fn core_text() -> Vec<(String, String)> {
    let mut all = production("ti4-engine-core");
    all.extend(production("ti4-engine-core-factions"));
    all
}

/// Whether `text` names `::Variant` as a whole identifier.
fn mentions(text: &str, variant: &str) -> bool {
    let needle = format!("::{variant}");
    text.match_indices(&needle).any(|(at, _)| !text[at + needle.len()..].chars().next().is_some_and(|c| c.is_alphanumeric() || c == '_'))
}

/// The technologies outside the deck (the replaced printings: `query::tech::in_deck`) that the production code of the core still names. A seat can never
/// hold one, so a rule that reads one is dead: this is the class of the X-89 bug (the core read the base printing, the deck holds the Codex IV one).
/// `ERRORERROR` is the one in the table: Nekro's `???_ERROR_ERROR_???` (`nekroc4r`) is in no deck, in the string engine either, and both engines read it
/// the same way (`faction_dual.rs` reads `nekroc4r`), so the core keeps parity with an action nobody can take.
const NAMED_BUT_NOT_IN_THE_DECK: &[(&str, &str)] = &[("ERRORERROR", "parity: the string reads `nekroc4r` too and no seat can hold it in either engine")];

#[test]
fn the_core_names_no_technology_the_deck_does_not_hold() {
    let text = core_text();
    let mut named = BTreeSet::new();
    for t in Technology::ALL {
        let name = format!("{t:?}");
        if in_deck(t) || t.data().source == ti4_engine_core::generated::Source::ThundersEdge {
            continue;
        }
        // `query/tech.rs` is where the deck itself is defined.
        if text.iter().any(|(path, body)| !path.ends_with("ti4-engine-core/src/query/tech.rs") && mentions(body, &name) && body.contains("Technology")) {
            named.insert(name);
        }
    }
    let accounted: BTreeSet<String> = NAMED_BUT_NOT_IN_THE_DECK.iter().map(|(n, _)| (*n).to_owned()).collect();
    assert_eq!(named, accounted, "technologies outside the deck that the core names (a rule that reads one never fires), against the census");
}

/// The factions the core does not port: their technologies need no mention.
fn dropped(tag: ti4_engine_core::generated::FactionTag) -> bool {
    use ti4_engine_core::generated::FactionTag as F;
    matches!(tag, F::Bastion | F::Crimson | F::Deepwrought | F::Firmament | F::Keleres | F::Keleresa | F::Keleresm | F::Keleresx | F::Obsidian | F::Ralnel)
}

#[test]
fn every_deck_technology_is_read_by_the_core_or_accounted_for() {
    use ti4_engine_core::generated::TechType;
    let text = core_text();
    let mut silent = BTreeSet::new();
    for t in Technology::ALL {
        let d = t.data();
        // A unit upgrade is read through the data of its unit (`Unit::data`), not by name; the others are named by the rule that reads them.
        if !in_deck(t) || d.types.contains(&TechType::Unitupgrade) || d.faction.is_some_and(dropped) {
            continue;
        }
        let name = format!("{t:?}");
        if !text.iter().any(|(_, body)| mentions(body, &name)) {
            silent.insert(name);
        }
    }
    println!("SILENT {silent:?}");
    let accounted: BTreeSet<String> = NO_MENTION.iter().map(|(n, _)| (*n).to_owned()).collect();
    assert_eq!(silent, accounted, "deck technologies no rule of the core names, against the census");
}

/// Deck technologies (not unit upgrades, not of a dropped faction) that no rule of the core names, and why that is fine.
const NO_MENTION: &[(&str, &str)] = &[];
