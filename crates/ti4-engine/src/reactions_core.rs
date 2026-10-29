//! The adapter half of the core ability table (package `core-reactions`; docs/ENGINE_REACTIONS.md): which string
//! ability ids the core table has a rule for, the registry the core builds from it, and `follow`, which sets up
//! a core window to follow a string run that had nested activity (reaction windows, ability effects).
//!
//! * [`table`]: the core `AbilityTable` with every ported rule.
//! * [`slot_of`] / [`text_of`]: the mapping between a registered string id
//!   (`technology:hacan:inf2:GROUND_FORCE_DESTROYED:after`) and the owner-independent [`Slot`].
//! * [`Recording::unported`]: the first ability the string run found eligible that the table lacks.
//! * [`follow`]: the resolver (configured as the string one was) and the table for a nested window.

use ti4_engine_core::choice::reaction::{AbilityId, AbilityKind};
use ti4_engine_core::kernel::ability::{AbilityTable, Slot, Spec};
use ti4_engine_core::kernel::Resolver;
use ti4_engine_core::state::CoreState;
use ti4_engine_core::Seat;
use ti4_model::state::GameState;

use ti4_engine_core_factions::cb_economy::cabal::timing as cabal;

use crate::dual_window::{Recording, ResolverCfg};
use crate::kernel_sync::text::{ability_id, event_kind, event_text, relation_text};
use crate::render::Render;

/// One ported ability rule, named: the registered id of an owner is `{kind}:{owner}:{card}:{EVENT}:{rel}`; the
/// event and the relation are the spec's. The reaction-card slots have no card segment.
struct Named {
    slot: Slot,
    kind: &'static str,
    card: &'static str,
}

/// The ported rules with a card key, in table order (the per-owner registration order of `reactions::arm`).
const NAMED: &[Named] = &[
    Named { slot: ti4_engine_core_factions::cb_economy::argent::timing::AGENT, kind: "leader", card: "argentagent" },
    Named { slot: ti4_engine_core::rules::reactions::instinct_training::INSTINCT_TRAINING, kind: "technology", card: "it" },
    Named { slot: ti4_engine_core::rules::reactions::l1z1x_agent::I48S, kind: "leader", card: "l1z1xagent" },
    Named { slot: ti4_engine_core::rules::reactions::tellurian::HITS, kind: "leader", card: "titansagent" },
    Named { slot: ti4_engine_core::rules::reactions::tellurian::CANNON_HITS, kind: "leader", card: "titansagent" },
    Named { slot: ti4_engine_core::rules::reactions::tellurian::GROUND_HITS, kind: "leader", card: "titansagent" },
    Named { slot: ti4_engine_core::rules::reactions::tellurian::BARRAGE_HITS, kind: "leader", card: "titansagent" },
    Named { slot: ti4_engine_core_factions::ce_movement::arborec::timing::WARRIOR_ROLLS, kind: "unit", card: "arborec_infantry2" },
    Named { slot: ti4_engine_core_factions::cd_registry::yin::timing::MILOR_SHIPS, kind: "leader", card: "yinagent" },
    Named { slot: ti4_engine_core_factions::cd_registry::yin::timing::MILOR_GROUND, kind: "leader", card: "yinagent" },
    Named { slot: ti4_engine_core_factions::ca_combat::nekro::timing::ANTIVIRUS_SPACE, kind: "promissory", card: "antivirus" },
    Named { slot: ti4_engine_core_factions::ca_combat::nekro::timing::ANTIVIRUS_GROUND, kind: "promissory", card: "antivirus" },
    Named { slot: ti4_engine_core_factions::ca_combat::nekro::singularity::SPACE, kind: "ability", card: "technological_singularity" },
    Named { slot: ti4_engine_core_factions::ca_combat::nekro::singularity::GROUND, kind: "ability", card: "technological_singularity" },
    Named { slot: ti4_engine_core_factions::ca_combat::nekro::singularity::OPEN_SPACE, kind: "ability", card: "technological_singularity_open" },
    Named { slot: ti4_engine_core_factions::ca_combat::nekro::singularity::OPEN_GROUND, kind: "ability", card: "technological_singularity_open" },
    Named { slot: ti4_engine_core_factions::ca_combat::nekro::singularity::CLOSE_SPACE, kind: "ability", card: "technological_singularity_close" },
    Named { slot: ti4_engine_core_factions::ca_combat::nekro::singularity::CLOSE_GROUND, kind: "ability", card: "technological_singularity_close" },
    Named { slot: ti4_engine_core_factions::ce_movement::nomad::timing::CAVALRY_PLAY, kind: "promissory", card: "cavalry" },
    Named { slot: ti4_engine_core_factions::ce_movement::nomad::timing::CAVALRY_RETURN, kind: "promissory", card: "cavalry" },
    Named { slot: ti4_engine_core_factions::cc_ground::mahact::timing::LEGIONNAIRE, kind: "unit", card: "mahact_infantry" },
    Named { slot: ti4_engine_core_factions::cc_ground::mahact::timing::LEGIONNAIRE_II, kind: "unit", card: "mahact_infantry2" },
    Named { slot: ti4_engine_core_factions::cc_ground::mentak::timing::SALVAGE_TRADE_GOOD, kind: "technology", card: "so" },
    Named { slot: ti4_engine_core_factions::cc_ground::mentak::timing::HERO_START, kind: "leader", card: "mentakhero" },
    Named { slot: ti4_engine_core_factions::cc_ground::mentak::timing::HERO_SHIPS, kind: "leader", card: "mentakhero" },
    Named { slot: ti4_engine_core_factions::cc_ground::mentak::timing::HERO_END, kind: "leader", card: "mentakhero" },
    Named { slot: ti4_engine_core::rules::reactions::saar::SAAR_TRADE_GOOD, kind: "tech", card: "sar" },
    Named { slot: ti4_engine_core_factions::cc_ground::borrowed::timing::SOL_AGENT, kind: "leader", card: "solagent" },
    Named { slot: ti4_engine_core_factions::cc_ground::borrowed::timing::LETNEV_AGENT, kind: "leader", card: "letnevagent" },
    Named { slot: ti4_engine_core::rules::reactions::INFANTRY_II_ROLLS, kind: "technology", card: "inf2" },
    Named { slot: cabal::REANIMATOR, kind: "unit", card: "cabal_mech" },
    Named { slot: cabal::DEVOUR_SHIPS, kind: "ability", card: "devour" },
    Named { slot: cabal::DEVOUR_GROUND, kind: "ability", card: "devour" },
    Named { slot: cabal::TERROR_SHIPS, kind: "unit", card: "cabal_flagship" },
    Named { slot: cabal::TERROR_GROUND, kind: "unit", card: "cabal_flagship" },
    Named { slot: cabal::MARK_SPACE, kind: "ability", card: "devour_mark" },
    Named { slot: cabal::CLEAR_SPACE, kind: "ability", card: "devour_clear" },
    Named { slot: cabal::MARK_GROUND, kind: "ability", card: "devour_mark" },
    Named { slot: cabal::CLEAR_GROUND, kind: "ability", card: "devour_clear" },
];

fn is_reaction_slot(slot: Slot) -> bool {
    let base = ti4_engine_core::rules::action_cards::reaction_slot::SLOT_BASE;
    (base..base + 100).contains(&slot.0)
}

/// Equal for the copies of one printed action card (`action_cards::name_of`).
fn card_name_key(card: ti4_engine_core::generated::ActionCard) -> u32 {
    use crate::generated::IdStr;
    static KEYS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    let names = KEYS.get_or_init(|| {
        let content = ti4_content::ContentStore::embedded();
        ti4_engine_core::generated::ActionCard::ALL
            .iter()
            .map(|c| crate::action_cards::name_of(content, &ti4_model::id::ActionCardId::new(c.as_str())))
            .collect()
    });
    let mine = &names[card as usize];
    u32::try_from(names.iter().position(|n| n == mine).unwrap_or(0)).unwrap_or(0)
}

thread_local! {
    /// `(player, system, neighbours)`: the systems adjacent to `system` as `player` sees them, for the window being
    /// run (`PlayerAdjacency::neighbours`; the core is lent no galaxy).
    static REACH: std::cell::RefCell<Vec<(String, String, Vec<String>)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Run `body` with the adjacency facts of the window installed (read by [`follow`]).
pub fn with_reach<T>(entries: Vec<(String, String, Vec<String>)>, body: impl FnOnce() -> T) -> T {
    let before = REACH.with(|v| v.replace(entries));
    let out = body();
    REACH.with(|v| *v.borrow_mut() = before);
    out
}

/// The registered id of `spec` for an owner called `owner`.
fn id_text(r: &Render, spec: &Spec, owner: &str) -> String {
    let (event, rel) = (event_text(r, spec.event), relation_text(spec.relation));
    if is_reaction_slot(spec.slot) {
        return format!("reaction:{owner}:{event}:{rel}");
    }
    let n = NAMED.iter().find(|n| n.slot == spec.slot).expect("every spec is named");
    format!("{}:{owner}:{}:{event}:{rel}", n.kind, n.card)
}

/// The core table: every ported ability rule.
#[must_use]
pub fn table() -> AbilityTable {
    static TABLE: std::sync::OnceLock<AbilityTable> = std::sync::OnceLock::new();
    TABLE
        .get_or_init(|| {
            let mut t = AbilityTable::default();
            t.bars = Some(ti4_engine_core::rules::agenda::political_secret_bars);
            t.card_name = Some(card_name_key);
            // The string order of `reactions::arm`, per owner: the reaction slots, then Instinct Training, I48S,
            // Infantry II, Magen Defense, the factions.
            ti4_engine_core_factions::cc_ground::register_first(&mut t);
            ti4_engine_core::rules::action_cards::reaction_slot::register(&mut t);
            ti4_engine_core::rules::reactions::register(&mut t);
            ti4_engine_core_factions::cb_economy::register_abilities(&mut t);
            ti4_engine_core_factions::ce_movement::register_abilities(&mut t);
            // The string order of the faction modules: only abilities of one window share a bucket, and Mahact's
            // Legionnaire and Yin's agent do (GROUND_FORCE_DESTROYED), Mentak's Salvage Operations and the Nomad's
            // Cavalry (SPACE_COMBAT_ENDED).
            ti4_engine_core_factions::cc_ground::register_abilities(&mut t);
            ti4_engine_core_factions::ca_combat::register_abilities(&mut t);
            ti4_engine_core_factions::ce_movement::register_late_abilities(&mut t);
            ti4_engine_core_factions::cd_registry::register_abilities(&mut t);
            ti4_engine_core::rules::reactions::register_after_factions(&mut t);
            ti4_engine_core_factions::cards::register(&mut t);
            t
        })
        .clone()
}

fn specs() -> Vec<Spec> {
    table().specs().to_vec()
}

#[allow(dead_code)]
fn kind_text(kind: AbilityKind) -> &'static str {
    match kind {
        AbilityKind::Ability => "ability",
        AbilityKind::Leader => "leader",
        AbilityKind::Breakthrough => "breakthrough",
        AbilityKind::Technology => "technology",
        AbilityKind::Unit => "unit",
        AbilityKind::Promissory => "promissory",
        AbilityKind::Reaction => "reaction",
        AbilityKind::Other => "other",
    }
}

/// The slot of a registered string id (`kind:owner:card:EVENT:rel`, or `reaction:owner:EVENT:rel`), if the table
/// has a rule for it.
#[must_use]
pub fn slot_of(id: &str) -> Option<Slot> {
    let parts: Vec<&str> = id.split(':').collect();
    let r = Render::new(Vec::new());
    if parts.len() == 4 && parts[0] == "reaction" {
        let w = ti4_engine_core::rules::action_cards::reaction_slot::windows();
        let at = w.iter().position(|(e, rel)| event_text(&r, *e) == parts[2] && relation_text(*rel) == parts[3])?;
        return Some(Slot(ti4_engine_core::rules::action_cards::reaction_slot::SLOT_BASE + u16::try_from(at).ok()?));
    }
    if parts.len() < 5 {
        return None;
    }
    let (kind, rest) = (parts[0], &parts[2..]);
    let (event, rel) = (rest[rest.len() - 2], rest[rest.len() - 1]);
    let card = rest[..rest.len() - 2].join(":");
    let all = specs();
    NAMED.iter().filter(|n| n.kind == kind && n.card == card).find_map(|n| {
        let spec = all.iter().find(|s| s.slot == n.slot)?;
        (event_text(&r, spec.event) == event && relation_text(spec.relation) == rel).then_some(n.slot)
    })
}

/// An ability id with the owner's faction left out (`technology:*:inf2:GROUND_FORCE_DESTROYED:after`).
fn anonymous(id: &str) -> String {
    let mut parts: Vec<&str> = id.split(':').collect();
    if parts.len() > 2 {
        parts[1] = "*";
    }
    parts.join(":")
}

impl Recording {
    /// The first ability the string run found eligible (or ran) that the core table has no rule for, anonymised
    /// (`technology:*:inf2:GROUND_FORCE_DESTROYED:after`), or the reason the nested activity cannot be followed.
    #[must_use]
    pub fn unported(&self, table: &AbilityTable) -> Option<String> {
        for id in &self.eligible {
            let known = slot_of(id).and_then(|s| table.of_slot(s));
            let ran = self.effects.iter().any(|e| e == id);
            if known.is_some_and(|spec| spec.effect_ported || !ran) {
                continue;
            }
            return Some(format!("unported ability {}", anonymous(id)));
        }
        // A card played from a window (declared by the slot, announced when the window closes): the core plays the stamps
        // of B6 declared in an ordinary window; any other card, or a play at once, is the string's.
        if let Some(slot) = self.effects.iter().find(|e| e.starts_with("reaction:")) {
            let ported = |card: &String| <ti4_engine_core::generated::ActionCard as crate::generated::IdStr>::from_id(card).is_some_and(|c| ti4_engine_core::rules::action_cards::announce::plays_in(table, c));
            if let Some(card) = self.played.iter().find(|c| !ported(c)) {
                if slot.contains(":ACTION_CARD_PLAYED:") {
                    return Some(format!("unported ability {} (a card played at once)", anonymous(slot)));
                }
                return Some(format!("unported ability {} (card {card})", anonymous(slot)));
            }
        }
        self.foreign.then(|| "nested outside an ability".to_owned())
    }
}

/// What a core window needs to follow nested activity: the table, and the resolver configured as the string one.
pub struct Follow {
    pub table: AbilityTable,
    pub resolver: Resolver,
}

/// The resolver and table for a window that ran with `script`; `None` for a run without nested activity (the
/// window keeps its own minimal resolver and an empty table).
///
/// # Errors
/// The reason the nested activity cannot be followed (the adapter bails with it).
pub fn follow(r: &Render, gs: &GameState, script: &Recording) -> Result<Option<Follow>, String> {
    if !script.nested {
        return Ok(None);
    }
    let mut table = table();
    if let Some(why) = script.unported(&table) {
        return Err(why);
    }
    REACH.with(|v| {
        for (who, system, neighbours) in v.borrow().iter() {
            let (Some(who), Some(system)) = (r.seat(who), <ti4_engine_core::generated::System as crate::generated::IdStr>::from_id(system)) else { continue };
            let neighbours = neighbours.iter().filter_map(|n| <ti4_engine_core::generated::System as crate::generated::IdStr>::from_id(n)).collect();
            table.facts.set_reach(who, system, neighbours);
        }
    });
    for (who, system, destinations) in &script.withdraw {
        let (Some(who), Some(system)) = (r.seat(who), <ti4_engine_core::generated::System as crate::generated::IdStr>::from_id(system)) else { continue };
        let destinations = destinations.iter().filter_map(|d| <ti4_engine_core::generated::System as crate::generated::IdStr>::from_id(d)).collect();
        table.facts.set_withdraw(who, system, destinations);
    }
    // A played card: Data Skimmer takes another player's discard onto the breakthrough (a Ral Nel seat). A rules version
    // before 3 plays at once, which the core's play does too (B6: `reaction_slot::play`).
    if let Some(slot) = script.effects.iter().find(|e| e.starts_with("reaction:")) {
        if gs.players.iter().any(|p| p.faction.as_str() == "ralnel") {
            return Err(format!("unported ability {} (Data Skimmer)", anonymous(slot)));
        }
    }
    // The use of a round agent is an event of the diplomacy promises (`LeaderUsedFor`); the core does not evaluate them.
    if gs.diplomacy.enabled {
        if let Some(id) = script.effects.iter().find(|e| e.starts_with("leader:") && (e.contains(":solagent:") || e.contains(":letnevagent:"))) {
            return Err(format!("unported ability {} (diplomacy promises on a leader's use)", anonymous(id)));
        }
    }
    let Some(cfg) = script.cfg.as_ref() else {
        return Err("nested activity without a resolver".to_owned());
    };
    table.facts.plays = play_facts(r, gs, &cfg.sources);
    let resolver = resolver_of(r, cfg).ok_or_else(|| "a resolver seat the tables do not know".to_owned())?;
    Ok(Some(Follow { table, resolver }))
}

/// The printed name of each card as the index of the first card with the same name, and whether the string side has
/// an effect for each card: constant, built once.
fn card_tables() -> &'static (Vec<u8>, Vec<bool>) {
    static T: std::sync::OnceLock<(Vec<u8>, Vec<bool>)> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        use crate::generated::IdStr;
        let content = ti4_content::ContentStore::embedded();
        let names: Vec<String> = ti4_engine_core::generated::ActionCard::ALL
            .iter()
            .map(|c| crate::action_cards::name_of(content, &ti4_model::id::ActionCardId::new(c.as_str())))
            .collect();
        let group = (0..names.len()).map(|i| u8::try_from(names.iter().position(|n| *n == names[i]).unwrap_or(i)).unwrap_or(u8::MAX)).collect();
        let effect = ti4_engine_core::generated::ActionCard::ALL
            .iter()
            .map(|c| crate::action_cards::effect_for(&ti4_model::id::ActionCardId::new(c.as_str())).is_some())
            .collect();
        (group, effect)
    })
}

/// What the play of an action card inside a window reads from outside the core (`None`: a seat or a unit form the
/// tables do not know; the play then marks the run unknown and the window bails).
fn play_facts(r: &Render, gs: &GameState, sources: &ti4_model::content_types::SourceSet) -> Option<ti4_engine_core::kernel::ability::PlayFacts> {
    use ti4_engine_core::kernel::ability::{PlayFacts, SeatPlay};
    let (name_group, has_effect) = card_tables().clone();
    let mut seats = Vec::new();
    for i in 0..r.seats.len() {
        let who = Seat(u8::try_from(i).ok()?);
        let player = ti4_model::id::PlayerId::new(r.seat_text(who));
        let infantry = crate::action_cards_view::placed(gs, &player, sources, "infantry")?;
        let cruiser = crate::action_cards_view::placed(gs, &player, sources, "cruiser")?;
        seats.push(SeatPlay {
            who,
            mahact_held: gs.tokens_held_by_mahact(&player),
            infantry: (infantry.unit, infantry.on_card),
            cruiser: (cruiser.unit, cruiser.on_card),
            fighter_on_card: crate::action_cards_view::fighter_on_card(gs, &player, sources),
        });
    }
    Some(PlayFacts {
        scope: crate::core_adapter::scope(sources.clone()).0,
        blanked: crate::laws::blanked(gs),
        action_card: r.code("action_card"),
        supply_base: r.code("forward_supply_base"),
        note_order: |a, b| crate::render::cards::note_text(a).cmp(&crate::render::cards::note_text(b)),
        seats,
        name_group,
        has_effect,
        skimmer: crate::factions::ralnel_cards::skimmer_holder(gs).is_some(),
    })
}

fn phase_of(p: ti4_model::state::Phase) -> ti4_engine_core::state::Phase {
    crate::render::generic::core_phase(p)
}

/// A core resolver configured as the string one was (no registry).
#[must_use]
pub fn resolver_of(r: &Render, cfg: &ResolverCfg) -> Option<Resolver> {
    use ti4_engine_core::kernel::resolver::UsedScope;
    let seats = |v: &[String]| -> Option<Vec<Seat>> { v.iter().map(|s| r.seat(s)).collect() };
    let mut core = Resolver::new(&seats(&cfg.initiative)?, cfg.active.as_deref().and_then(|s| r.seat(s)));
    core.set_phase(phase_of(cfg.phase));
    core.set_seating_order(&seats(&cfg.seating)?);
    core.set_speaker(cfg.speaker.as_deref().and_then(|s| r.seat(s)));
    core.set_counters(cfg.round, cfg.turn);
    core.set_emission_stack(&cfg.stack.iter().map(|(kind, id)| (event_kind(r, kind), *id)).collect::<Vec<_>>());
    core.set_maximum_depth(cfg.max_depth);
    for (id, scope) in &cfg.used {
        let Some((kind, rest)) = scope.split_once('(') else { continue };
        let n: u64 = rest.trim_end_matches(')').parse().unwrap_or(0);
        let kind = match kind {
            "Trigger" => UsedScope::Trigger,
            "Turn" => UsedScope::Turn,
            _ => UsedScope::Round,
        };
        core.spend(ability_id(r, id), kind, n);
    }
    Some(core)
}

impl Follow {
    /// Register the ported abilities of every seat in the resolver, in the string order. Call once, with the
    /// core state of the window, before the first `run`.
    pub fn arm(&mut self, r: &Render, gs: &GameState, core: &CoreState) {
        let order: Vec<Seat> = (0..r.seats.len()).filter_map(|i| u8::try_from(i).ok().map(Seat)).collect();
        let owner_name = |who: Seat| crate::promissory::faction_name(gs, &ti4_model::id::PlayerId::new(r.seat_text(who)));
        let mut name = |spec: &Spec, who: Seat| -> AbilityId { ability_id(r, &id_text(r, spec, &owner_name(who))) };
        self.table.codes.ground_combat = r.code("ground_combat");
        self.table.codes.harrow = r.code("harrow");
        self.table.codes.self_assembly = r.code("self_assembly_routines");
        self.table.codes.action_card = r.code("action_card");
        self.table.codes.supply_base = r.code("forward_supply_base");
        self.table.codes.mentak = r.code("mentak");
        self.table.codes.instinct_training = r.code("instinct_training");
        self.table.codes.direct_hit = r.code("action_card:direct_hit");
        self.table.codes.cavalry = r.code("cavalry");
        self.table.codes.technological_singularity = r.code("technological_singularity");
        self.table.codes.mahact_infantry = r.code("mahact_infantry");
        self.table.codes.mahact_infantry2 = r.code("mahact_infantry2");
        self.table.codes.retreat_capacity = r.code("retreat_capacity");
        self.table.codes.reinforcements = r.code("reinforcements");
        self.table.facts.staging = crate::supply::staging_enabled(gs);
        let defs = self.table.registry(core, &order, &mut name);
        self.resolver.register(defs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dual::{report, take_report};

    fn rec(eligible: &[&str], effects: &[&str]) -> Recording {
        let mut r = Recording::default();
        r.nested = true;
        r.eligible = eligible.iter().map(|s| (*s).to_owned()).collect();
        r.effects = effects.iter().map(|s| (*s).to_owned()).collect();
        r
    }

    #[test]
    fn every_ported_id_maps_to_its_slot_and_back() {
        let t = table();
        for n in NAMED {
            let spec = t.of_slot(n.slot).expect("a named slot is in the table");
            let id = id_text(&Render::new(Vec::new()), spec, "hacan");
            assert_eq!(slot_of(&id), Some(n.slot), "{id}");
        }
        let w = ti4_engine_core::rules::action_cards::reaction_slot::windows();
        for (i, _) in w.iter().enumerate() {
            let slot = Slot(ti4_engine_core::rules::action_cards::reaction_slot::SLOT_BASE + u16::try_from(i).unwrap());
            let id = id_text(&Render::new(Vec::new()), t.of_slot(slot).unwrap(), "sol");
            assert_eq!(slot_of(&id), Some(slot), "{id}");
        }
        assert!(slot_of("leader:sol:solagent:GROUND_COMBAT_ROUND_STARTED:after").is_some());
        assert_eq!(slot_of("leader:yssaril:a:solagent:b:GROUND_COMBAT_ROUND_STARTED:after"), None, "a Ssruu copy is another ability");
        assert_eq!(slot_of("technology:hacan:inf2:SHIP_DESTROYED:after"), None, "the event is part of the identity");
    }

    /// Technological Singularity offers an opponent's technologies in the order of the string's `BTreeSet<TechnologyId>`
    /// (the ids' text order); the core walks a bit set in enum order.
    #[test]
    fn the_enum_order_of_the_technologies_is_the_text_order_of_their_ids() {
        let ids = &crate::generated::technologies::TECHNOLOGY_IDS;
        assert!(ids.windows(2).all(|w| w[0] < w[1]), "the core's technology order is the string's");
    }

    #[test]
    fn bail_an_eligible_ability_the_table_lacks_is_named() {
        let t = table();
        let copy = "leader:yssaril:a:solagent:b:GROUND_COMBAT_ROUND_STARTED:after";
        let why = rec(&[copy], &[]).unported(&t);
        assert_eq!(why.as_deref(), Some("unported ability leader:*:a:solagent:b:GROUND_COMBAT_ROUND_STARTED:after"));
        assert_eq!(rec(&["technology:hacan:inf2:GROUND_FORCE_DESTROYED:after"], &["technology:hacan:inf2:GROUND_FORCE_DESTROYED:after"]).unported(&t), None);
    }

    /// Every nested-bail family B12 left in the string is named by the window that meets it (`unported ability ...`), none is
    /// followed silently; the ids are the string's registrations (`docs/ENGINE_REACTIONS.md` 10 lists why each stays).
    #[test]
    fn bail_every_family_that_stays_in_the_string_is_named() {
        let t = table();
        for id in [
            "unit:empyrean:empyrean_flagship:SUSTAIN_DAMAGE_USED:after",
            "unit:empyrean:empyrean_flagship:GROUND_FORCE_SUSTAINED:after",
            "unit:empyrean:empyrean_mech:ACTION_CARD_PLAYED:when",
            "technology:mentak:so:SPACE_COMBAT_ENDED:produce",
            "leader:sardakk:sardakkhero:GROUND_COMMITMENT_FINISHED:after",
            "unit:sardakk:sardakk_mech:GROUND_FORCE_SUSTAINED:after",
            "leader:yssaril:a:yssarilagent:yinagent:SHIP_DESTROYED:after",
            "leader:yssaril:a:yssarilagent:yinagent:GROUND_FORCE_DESTROYED:after",
            "leader:yssaril:a:yssarilagent:solagent:GROUND_COMBAT_ROUND_STARTED:after",
            "leader:yssaril:a:yssarilagent:letnevagent:COMBAT_ROUND_STARTED:after",
            "leader:yssaril:a:yssarilagent:argentagent:GROUND_FORCES_BEING_PRODUCED:when",
        ] {
            assert_eq!(slot_of(id), None, "{id} is not in the table");
            let why = rec(&[id], &[]).unported(&t).unwrap_or_default();
            assert!(why.starts_with("unported ability "), "{id}: {why:?}");
        }
    }

    #[test]
    fn bail_a_declined_reaction_slot_is_followed_and_a_played_unported_card_is_not() {
        let t = table();
        let id = "reaction:hacan:HITS_TO_ASSIGN:when";
        assert!(slot_of(id).is_some());
        assert_eq!(rec(&[id], &[]).unported(&t), None, "offered and declined");
        let mut played = rec(&[id], &[id]);
        played.played = vec!["sh1".to_owned()];
        assert_eq!(played.unported(&t), None, "a Shields Holding is played by the core");
        played.played = vec!["fire_team".to_owned()];
        assert_eq!(played.unported(&t).as_deref(), Some("unported ability reaction:*:HITS_TO_ASSIGN:when (card fire_team)"), "a card the core does not play");
    }

    #[test]
    fn bail_a_played_card_the_core_does_not_play_is_named() {
        let t = table();
        let slot = "reaction:hacan:COMBAT_ROUND_STARTED:after";
        let mut r = rec(&[slot], &[slot]);
        r.played = vec!["mb1".to_owned()];
        assert_eq!(r.unported(&t), None, "a Morale Boost is a stamp the core plays");
        r.played = vec!["mb1".to_owned(), "reveal_prototype".to_owned()];
        assert_eq!(r.unported(&t).as_deref(), Some("unported ability reaction:*:COMBAT_ROUND_STARTED:after (card reveal_prototype)"));
        r.played = vec!["s_retreat1".to_owned(), "intercept".to_owned(), "dh2".to_owned(), "parley".to_owned()];
        assert_eq!(r.unported(&t), None, "the cards of the card module are played by the core");
    }

    #[test]
    fn bail_a_card_played_at_once_is_named() {
        let slot = "reaction:hacan:ACTION_CARD_PLAYED:when";
        let mut r = rec(&[slot], &[slot]);
        r.played = vec!["sabo1".to_owned()];
        assert_eq!(r.unported(&table()), None, "a Sabotage is played at once by the core");
        r.played = vec!["reveal_prototype".to_owned()];
        assert_eq!(r.unported(&table()).as_deref(), Some("unported ability reaction:*:ACTION_CARD_PLAYED:when (a card played at once)"));
    }

    #[test]
    fn bail_a_play_with_data_skimmer_is_named_and_one_below_rules_version_3_is_followed() {
        let slot = "reaction:hacan:COMBAT_ROUND_STARTED:after";
        let mut script = rec(&[slot], &[slot]);
        script.played = vec!["mb1".to_owned()];
        let render = Render::new(vec!["a".to_owned(), "b".to_owned()]);
        let mut gs = crate::fixtures::seated_game(&[("a", "hacan"), ("b", "sol")], ti4_model::content_types::POK);
        assert!(follow(&render, &gs, &script).is_err() || script.cfg.is_none(), "no resolver recorded: the only reason");
        gs.rules_version = 2;
        // B6 plays at once below version 3 (`reaction_slot::play`), so the census does not name it any more.
        assert_ne!(follow(&render, &gs, &script).err().as_deref(), Some("unported ability reaction:*:COMBAT_ROUND_STARTED:after (a card played at once)"));
        let mut gs = crate::fixtures::seated_game(&[("a", "hacan"), ("b", "ralnel")], ti4_model::content_types::POK);
        gs.rules_version = ti4_model::state::RULES_SABOTAGE_TIMING;
        assert_eq!(follow(&render, &gs, &script).err().as_deref(), Some("unported ability reaction:*:COMBAT_ROUND_STARTED:after (Data Skimmer)"));
    }

    #[test]
    fn bail_nested_activity_outside_every_ability() {
        let mut r = rec(&[], &[]);
        r.foreign = true;
        assert_eq!(r.unported(&table()).as_deref(), Some("nested outside an ability"));
    }

    #[test]
    fn bail_nested_activity_without_a_resolver_is_counted_under_the_label() {
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let r = rec(&[], &[]);
        assert!(crate::dual_window::nested_check(&r).is_none());
        let counted = report().windows.keys().any(|k| k.contains("nested (nested activity without a resolver)"));
        let _ = take_report();
        assert!(counted);
    }

    /// Brother Milor places the form `placed_unit_id` names (the upgrade, the faction's own unit, the plain one): the core
    /// answer, built from the unit tables without the content store, equals the string one for every faction.
    #[test]
    fn the_core_placed_form_equals_the_string_placed_unit_id() {
        use crate::core_sync::{with_core, Footprint, Sources};
        use crate::generated::IdStr;
        use ti4_engine_core::generated::{Faction as CoreFaction, UnitBaseType};
        use ti4_engine_core::state::Slice;
        let content = ti4_content::ContentStore::embedded();
        let sources = ti4_model::content_types::DEFAULT;
        let mut checked = 0;
        for faction in CoreFaction::ALL {
            let name = <CoreFaction as IdStr>::as_str(faction);
            if name == "neutral" {
                continue;
            }
            for techs in [&[][..], &["ff2", "inf2"][..], &["hcf2", "lw2", "cl2", "so2"][..]] {
                let mut gs = crate::fixtures::seated_game(&[("a", name), ("b", "hacan")], sources);
                gs.promissory_notes.clear();
                gs.promissory_faceup.clear();
                for t in techs {
                    crate::technology::grant(&mut gs, &ti4_model::id::PlayerId::new("a"), &ti4_model::id::TechnologyId::new(*t));
                }
                let (mut cx, r) = crate::dual_window::tables(&gs).unwrap();
                let fp = Footprint::reads(&[Slice::Meta, Slice::Seats, Slice::Players]);
                let before = gs.clone();
                let mut got = Vec::new();
                with_core(&mut gs, &mut cx, Sources::default(), fp, |core, _| {
                    for base in [UnitBaseType::Fighter, UnitBaseType::Infantry, UnitBaseType::Carrier, UnitBaseType::Destroyer, UnitBaseType::Dreadnought, UnitBaseType::Warsun] {
                        let form = ti4_engine_core_factions::cd_registry::yin::timing::placed_form(core, ti4_engine_core::tables::Scope(crate::core_adapter::scope(sources).0), r.seat("a").unwrap(), base);
                        got.push((base, form.map(<ti4_engine_core::generated::Unit as IdStr>::as_str)));
                    }
                })
                .unwrap();
                for (base, form) in got {
                    let want = crate::action_cards::placed_unit_id_text(&before, content, sources, &ti4_model::id::PlayerId::new("a"), <UnitBaseType as IdStr>::as_str(base));
                    assert_eq!(form, want.as_ref().map(ti4_model::id::UnitTypeId::as_str), "{name} {base:?} {techs:?}");
                    checked += 1;
                }
            }
        }
        assert!(checked > 100);
    }

    #[test]
    fn the_native_registry_equals_the_string_one_for_the_ported_abilities() {
        use crate::core_sync::{with_core, Footprint, Sources};
        use ti4_engine_core::state::Slice;
        let content = ti4_content::ContentStore::embedded();
        let _ = content;
        for factions in [[("a", "sol"), ("b", "hacan")], [("a", "cabal"), ("b", "sol")], [("a", "hacan"), ("b", "cabal")], [("a", "nekro"), ("b", "hacan")], [("a", "hacan"), ("b", "nekro")]] {
            let mut gs = crate::fixtures::seated_game(&factions, ti4_model::content_types::POK);
            gs.promissory_notes.clear();
            gs.promissory_faceup.clear();
            let resolver = crate::fixtures::armed_resolver(&gs);
            let view = resolver.kernel_view();
            let want: Vec<(String, String, String, bool, bool, bool, bool)> = view
                .abilities
                .iter()
                .filter(|a| slot_of(&a.id).is_some())
                .map(|a| (a.id.clone(), a.owner.clone(), a.event.clone(), a.when, a.optional, a.repeatable, a.has_condition))
                .collect();
            let (mut cx, r) = crate::dual_window::tables(&gs).unwrap();
            let before = gs.clone();
            let mut fol = Follow { table: table(), resolver: Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))) };
            let mut got = Vec::new();
            let fp = Footprint::reads(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Cards]);
            with_core(&mut gs, &mut cx, Sources::default(), fp, |core, _| {
                let order: Vec<Seat> = (0..r.seats.len()).filter_map(|i| u8::try_from(i).ok().map(Seat)).collect();
                fol.arm(&r, &before, core);
                let defs = fol.table.registry(core, &order, &mut |spec, who| {
                    ability_id(&r, &id_text(&r, spec, &crate::promissory::faction_name(&before, &ti4_model::id::PlayerId::new(r.seat_text(who)))))
                });
                for d in defs {
                    got.push((
                        crate::kernel_sync::text::ability_text(&r, d.id),
                        r.seat_text(d.owner).to_owned(),
                        event_text(&r, d.event),
                        d.relation == ti4_engine_core::choice::reaction::Relation::When,
                        d.optional,
                        d.repeatable,
                        d.has_condition,
                    ));
                }
            })
            .unwrap();
            // The string registers per seat in seat order; the order that matters is each owner's.
            for owner in ["a", "b"] {
                // Between windows the order is not observable (a bucket is one window): compare bucket by bucket,
                // each bucket in registration order (the sort is stable).
                let mut w: Vec<_> = want.iter().filter(|x| x.1 == owner).cloned().collect();
                let mut g: Vec<_> = got.iter().filter(|x| x.1 == owner).cloned().collect();
                w.sort_by(|a, b| (a.2.as_str(), a.3).cmp(&(b.2.as_str(), b.3)));
                g.sort_by(|a, b| (a.2.as_str(), a.3).cmp(&(b.2.as_str(), b.3)));
                let missing: Vec<_> = w.iter().filter(|x| !g.contains(x)).collect();
                let extra: Vec<_> = g.iter().filter(|x| !w.contains(x)).collect();
                assert!(missing.is_empty() && extra.is_empty(), "{factions:?} owner {owner}: the string registers {missing:?} and the core {extra:?}");
                if let Some(at) = g.iter().zip(&w).position(|(a, b)| a != b) {
                    panic!("{factions:?} owner {owner}: the order of the registry differs at {at}: the core has {:?}, the string {:?}", &g[at.saturating_sub(1)..=(at + 1).min(g.len() - 1)], &w[at.saturating_sub(1)..=(at + 1).min(w.len() - 1)]);
                }
            }
        }
    }
}
