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
use crate::kernel_sync::text::{ability_id, event_text, relation_text};
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
    Named { slot: ti4_engine_core_factions::cc_ground::mentak::timing::SALVAGE_TRADE_GOOD, kind: "technology", card: "so" },
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
            // The string order of `reactions::arm`, per owner: the reaction slots, then Instinct Training, I48S,
            // Infantry II, Magen Defense, the factions.
            ti4_engine_core_factions::cc_ground::register_first(&mut t);
            ti4_engine_core::rules::action_cards::reaction_slot::register(&mut t);
            ti4_engine_core::rules::reactions::register(&mut t);
            ti4_engine_core_factions::cb_economy::register_abilities(&mut t);
            ti4_engine_core_factions::ce_movement::register_abilities(&mut t);
            ti4_engine_core_factions::cd_registry::register_abilities(&mut t);
            ti4_engine_core_factions::cc_ground::register_abilities(&mut t);
            ti4_engine_core::rules::reactions::register_after_factions(&mut t);
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
        // After the abilities, so that a card played from a window (announced when the window closes, outside the
        // effect that declared it) is reported as the slot that played it.
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
    // The use of a round agent is an event of the diplomacy promises (`LeaderUsedFor`); the core does not evaluate them.
    if gs.diplomacy.enabled {
        if let Some(id) = script.effects.iter().find(|e| e.starts_with("leader:") && (e.contains(":solagent:") || e.contains(":letnevagent:"))) {
            return Err(format!("unported ability {} (diplomacy promises on a leader's use)", anonymous(id)));
        }
    }
    let Some(cfg) = script.cfg.as_ref() else {
        return Err("nested activity without a resolver".to_owned());
    };
    let resolver = resolver_of(r, cfg).ok_or_else(|| "a resolver seat the tables do not know".to_owned())?;
    Ok(Some(Follow { table, resolver }))
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
        self.table.codes.mentak = r.code("mentak");
        self.table.codes.instinct_training = r.code("instinct_training");
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

    #[test]
    fn bail_an_eligible_ability_the_table_lacks_is_named() {
        let t = table();
        let copy = "leader:yssaril:a:solagent:b:GROUND_COMBAT_ROUND_STARTED:after";
        let why = rec(&[copy], &[]).unported(&t);
        assert_eq!(why.as_deref(), Some("unported ability leader:*:a:solagent:b:GROUND_COMBAT_ROUND_STARTED:after"));
        assert_eq!(rec(&["technology:hacan:inf2:GROUND_FORCE_DESTROYED:after"], &["technology:hacan:inf2:GROUND_FORCE_DESTROYED:after"]).unported(&t), None);
    }

    #[test]
    fn bail_a_declined_reaction_slot_is_followed_and_a_played_one_is_not() {
        let t = table();
        let id = "reaction:hacan:HITS_TO_ASSIGN:when";
        assert!(slot_of(id).is_some());
        assert_eq!(rec(&[id], &[]).unported(&t), None, "offered and declined");
        assert_eq!(rec(&[id], &[id]).unported(&t).as_deref(), Some("unported ability reaction:*:HITS_TO_ASSIGN:when"), "a card was played");
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
                    for base in [UnitBaseType::Fighter, UnitBaseType::Infantry] {
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
        for factions in [[("a", "sol"), ("b", "hacan")], [("a", "cabal"), ("b", "sol")], [("a", "hacan"), ("b", "cabal")]] {
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
                assert_eq!(g, w, "{factions:?} owner {owner}");
            }
        }
    }
}
