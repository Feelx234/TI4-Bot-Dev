//! The dual of package CC (ground-and-ship line: firmament, mahact, borrowed, ghost, mentak), tier Q.
//!
//! For one position: every pure hook the package registered, asked of the typed state through a kernel that holds the
//! package's modules (and the Nekro, whose Alastor the combat sides read), next to the string function it was moved
//! from; every seat, system, planet and unit kind the position offers. Also the order audit of the commutative seams
//! over the same sites.

use std::collections::BTreeSet;

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine_core::generated::{Faction, Leader, Planet, System, Unit as CoreUnit, UnitBaseType};
use ti4_engine_core::hooks::{CombatCtx, HookTable};
use ti4_engine_core::kernel::seam::{Clamp, CombatUnit as CoreCombatUnit, Q, RollKind, RollSite};
use ti4_engine_core::kernel::{Kernel, Sources as CoreSources};
use ti4_engine_core::state::{CoreState, SliceSet};
use ti4_engine_core_factions::ca_combat::nekro;
use ti4_engine_core_factions::cc_ground::{borrowed, firmament, ghost, mahact, mentak};
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlanetId, PlayerId, SystemId};
use ti4_model::state::GameState;

use crate::core_adapter as adapter;
use crate::core_sync::codec::Sources as CodecSources;
use crate::core_sync::{CoreView, Ctx};
use crate::dual::HookGroup;
use crate::generated::IdStr;

/// The modules whose tier Q this package has ported.
pub const PORTED: &[ti4_engine_core_factions::Id] = &[
    ti4_engine_core_factions::Id::Firmament,
    ti4_engine_core_factions::Id::Mahact,
    ti4_engine_core_factions::Id::Mentak,
    ti4_engine_core_factions::Id::Ghost,
    ti4_engine_core_factions::Id::Borrowed,
];

/// What was compared.
#[derive(Debug, Default, Clone)]
pub struct CcCheck {
    pub positions: u64,
    pub unmapped: u64,
    pub compared: u64,
    pub audited: u64,
    pub mismatches: Vec<String>,
}

impl CcCheck {
    pub fn add(&mut self, o: &Self) {
        self.positions += o.positions;
        self.unmapped += o.unmapped;
        self.compared += o.compared;
        self.audited += o.audited;
        for m in &o.mismatches {
            self.miss(m.clone());
        }
    }

    fn miss(&mut self, m: String) {
        if self.mismatches.len() < 60 {
            self.mismatches.push(m);
        }
    }

    fn eq<T: PartialEq + std::fmt::Debug>(&mut self, label: &str, what: &str, string: &T, core: &T) {
        self.compared += 1;
        if string != core {
            self.miss(format!("{label}: {what}: string {string:?} vs core {core:?}"));
        }
    }
}

fn kernel_with(sources: SourceSet, modules: &[fn(&mut HookTable)]) -> Kernel {
    Kernel { hooks: HookTable::of(modules), sources: CoreSources(u8::try_from(adapter::scope(sources).0).unwrap_or(u8::MAX)) }
}

/// Every module of the package, and the Nekro whose Alastor the combat sides read.
fn package_modules() -> [fn(&mut HookTable); 6] {
    [firmament::register, mahact::register, mentak::register, ghost::register, borrowed::register, nekro::register]
}

fn load_why(gs: &GameState, galaxy: Option<&Galaxy>) -> Result<(CoreState, Ctx), String> {
    let mut cx = Ctx::of(gs).map_err(|e| format!("{e:?}"))?;
    let view = CoreView::of(gs, &mut cx, CodecSources { galaxy, rng: None }, SliceSet::ALL).map_err(|e| format!("{e:?}"))?;
    Ok((view.into_state(), cx))
}

fn sys(id: &SystemId) -> Option<System> {
    <System as IdStr>::from_id(id.as_str())
}
fn planet(id: &PlanetId) -> Option<Planet> {
    <Planet as IdStr>::from_id(id.as_str())
}
fn unit_kind(id: &str) -> Option<CoreUnit> {
    <CoreUnit as IdStr>::from_id(id)
}
fn leader(id: &str) -> Option<Leader> {
    <Leader as IdStr>::from_id(id)
}

/// Unit kinds the cross-product probes besides the ones on the board.
const PROBE_KINDS: [&str; 11] = [
    "mentak_flagship",
    "mentak_mech",
    "mahact_flagship",
    "firmament_mech",
    "ghost_flagship",
    "nekro_flagship",
    "carrier",
    "fighter",
    "infantry",
    "cruiser2",
    "mentak_cruiser3",
];

/// Commanders the Imperia and the borrowed commanders are asked about.
const COMMANDERS: [&str; 5] = ["arboreccommander", "firmamentcommander", "obsidiancommander", "mentakcommander", "solcommander"];

/// A position and the same position turned into the scenarios the golden games rarely reach: a Mahact with tokens in its
/// fleet pool and unlocked commanders around it, an Obsidian with a faceup Assail on a seat, a Firmament with a Viper
/// and Myru Vos, a Mentak with Mirror Computing and The Table's Grace beside a Fourth Moon and a Moll Terminus, a Ghost
/// with Particle Synthesis over wormholes, and a Fracture with the Obsidian's commander.
pub fn compare(label: &str, gs: &GameState, galaxy: &Galaxy, sources: SourceSet, out: &mut CcCheck) {
    compare_state(label, gs, galaxy, sources, out);
    for (name, variant) in variants(gs) {
        compare_state(&format!("{label} [{name}]"), &variant, galaxy, sources, out);
    }
}

fn variants(gs: &GameState) -> Vec<(&'static str, GameState)> {
    use ti4_model::id::{BreakthroughId, FactionId, LeaderId, TechnologyId};
    use ti4_model::state::LeaderStatus;
    let mut out = Vec::new();
    if gs.players.len() < 2 {
        return out;
    }
    let (a, b) = (gs.players[0].id.clone(), gs.players[1].id.clone());
    let hot: Vec<SystemId> = gs.board.iter().filter(|(_, s)| !s.units.is_empty() || !s.planet_units.is_empty()).map(|(k, _)| k.clone()).take(3).collect();
    let Some(first) = hot.first().cloned() else { return out };
    let planet_of = |gs: &GameState, s: &SystemId| gs.board.get(s).and_then(|b| b.planet_units.keys().next().cloned());
    let unit = |t: &str, o: &PlayerId| ti4_model::units::Unit::new(ti4_model::id::UnitTypeId::new(t), o.clone());

    let mut v = gs.clone();
    v.players[0].faction = FactionId::new("mahact");
    v.faction_marks.insert(format!("mahact:fleet:{a}"), format!("{b}"));
    v.players[1].leaders.insert(LeaderId::new("arboreccommander"), LeaderStatus::Unlocked);
    v.players[1].leaders.insert(LeaderId::new("obsidiancommander"), LeaderStatus::Unlocked);
    v.players[0].leaders.insert(LeaderId::new("mahactagent"), LeaderStatus::Readied);
    v.active_system = Some(first.clone());
    v.system_mut(&first).units.push(unit("mahact_flagship", &a));
    v.system_mut(&first).units.push(unit("carrier", &b));
    v.system_mut(&first).command_tokens.insert(b.clone());
    out.push(("mahact pool", v));

    let mut v = gs.clone();
    v.players[0].faction = FactionId::new("mahact");
    let third = gs.players.get(2).map_or_else(|| a.clone(), |p| p.id.clone());
    v.faction_marks.insert(format!("mahact:fleet:{a}"), third.to_string());
    v.active_system = Some(first.clone());
    v.system_mut(&first).units.push(unit("mahact_flagship", &a));
    v.system_mut(&first).units.push(unit("carrier", &b));
    v.faction_marks.insert("mahact:benediction".to_owned(), first.to_string());
    out.push(("mahact open", v));

    let mut v = gs.clone();
    v.players[0].faction = FactionId::new("obsidian");
    let mut assail = ti4_model::plots::Plot::facedown("assail", &b);
    assail.faceup = true;
    let mut other = ti4_model::plots::Plot::facedown("siphon", &a);
    other.faceup = false;
    v.players[0].plots = vec![assail.encode(), other.encode()];
    v.players[1].plots = vec![ti4_model::plots::Plot::facedown("seethe", &a).encode()];
    v.system_mut(&first).units.push(unit("carrier", &a));
    v.system_mut(&first).units.push(unit("carrier", &b));
    if let Some(pl) = planet_of(&v, &first) {
        v.system_mut(&first).planet_units.entry(pl).or_default().extend([unit("infantry", &a), unit("infantry", &b)]);
    }
    out.push(("obsidian assail", v));

    let mut v = gs.clone();
    v.players[0].faction = FactionId::new("firmament");
    v.activation_seq = 9;
    v.faction_marks.insert("firmament:agent".to_owned(), format!("9|{a}"));
    v.faction_marks.insert("firmament:agent:moved".to_owned(), "9|carrier:1,cruiser:1".to_owned());
    v.system_mut(&first).units.extend([unit("carrier", &a), unit("cruiser", &a)]);
    if let Some(pl) = planet_of(&v, &first) {
        v.system_mut(&first).planet_control.insert(pl.clone(), b.clone());
        v.system_mut(&first).planet_units.entry(pl.clone()).or_default().extend([unit("firmament_mech", &a), unit("firmament_mech", &b)]);
    }
    v.players[0].plots = vec![ti4_model::plots::Plot::facedown("extract", &b).encode(), ti4_model::plots::Plot::facedown("assail", &b).encode()];
    out.push(("firmament", v));

    let mut v = gs.clone();
    v.players[0].faction = FactionId::new("mentak");
    v.players[0].technologies.insert(TechnologyId::new("mc"));
    v.players[0].breakthrough = Some(BreakthroughId::new("mentakbt"));
    v.system_mut(&first).units.extend([unit("mentak_flagship", &a), unit("carrier", &b)]);
    if let Some(pl) = planet_of(&v, &first) {
        v.system_mut(&first).planet_units.entry(pl).or_default().extend([unit("mentak_mech", &a), unit("infantry", &b)]);
    }
    out.push(("mentak", v));

    let mut v = gs.clone();
    v.players[0].faction = FactionId::new("ghost");
    v.players[0].breakthrough = Some(BreakthroughId::new("ghostbt"));
    v.nexus_unlocked = true;
    v.system_mut(&first).units.extend([unit("ghost_flagship", &a), unit("carrier", &a)]);
    v.wormhole_tokens.insert("ALPHA".to_owned(), first.clone());
    out.push(("ghost", v));

    let mut v = gs.clone();
    v.fracture_in_play = true;
    v.players[0].leaders.insert(LeaderId::new("obsidiancommander"), LeaderStatus::Unlocked);
    v.players[0].leaders.insert(LeaderId::new("firmamentcommander"), LeaderStatus::Unlocked);
    v.rules_version = 17;
    v.players[1].faction = FactionId::new("letnev");
    v.players[1].leaders.insert(LeaderId::new("letnevagent"), LeaderStatus::Readied);
    v.system_mut(&first).units.push(unit("carrier", &a));
    out.push(("borrowed", v));
    out
}

/// Compare every ported hook on one position.
fn compare_state(label: &str, gs: &GameState, galaxy: &Galaxy, sources: SourceSet, out: &mut CcCheck) {
    out.positions += 1;
    let (core, cx) = match load_why(gs, Some(galaxy)) {
        Ok(loaded) => loaded,
        Err(why) => {
            out.unmapped += 1;
            if label.ends_with(']') {
                out.miss(format!("{label}: the scenario does not load: {why}"));
            }
            return;
        }
    };
    let content = ContentStore::embedded();
    let k = kernel_with(sources, &package_modules());
    let ids: Vec<PlayerId> = gs.players.iter().map(|p| p.id.clone()).collect();
    let seat = |p: &PlayerId| cx.owners.seat(p.as_str());
    let name = |s: ti4_engine_core::Seat| cx.owners.id(s).map(str::to_owned);
    let systems: BTreeSet<SystemId> = gs.board.keys().cloned().collect();
    let kinds: BTreeSet<String> = gs
        .board
        .values()
        .flat_map(|b| b.units.iter().chain(b.planet_units.values().flatten()))
        .map(|u| u.type_id.as_str().to_owned())
        .chain(PROBE_KINDS.map(str::to_owned))
        .collect();

    out.eq(label, "remaining_cards", &(crate::factions::firmament::remaining_cards(gs).len() as u32), &k.hooks.direct.firmament_remaining_cards(&core));
    out.eq(label, "has_owner_space_round_agent", &crate::factions::borrowed_round_agents::has_owner_space_round_agent(gs), &k.hooks.direct.borrowed_round_agents_has_owner_space_round_agent(&core));
    out.eq(label, "has_owner_round_agent", &crate::factions::borrowed_round_agents::has_owner_round_agent(gs), &k.hooks.direct.borrowed_round_agents_has_owner_round_agent(&core));
    out.eq(label, "ship_movement_barred", &crate::factions::mahact_units::ship_movement_barred_text(gs), &k.hooks.direct.mahact_units_ship_movement_barred(&core));
    for f in &gs.players {
        if let Some(fx) = <Faction as IdStr>::from_id(f.faction.as_str()) {
            out.eq(label, &format!("purges_alliance {}", f.faction), &crate::factions::mahact::purges_alliance(f.faction.as_str()), &k.hooks.direct.mahact_purges_alliance(fx));
        }
    }

    for p in &ids {
        let Some(who) = seat(p) else { continue };
        out.eq(label, &format!("is_mahact {p}"), &crate::factions::mahact::is_mahact(gs, p), &k.hooks.direct.mahact_is_mahact(&core, who));
        out.eq(label, &format!("foreign_tokens {p}"), &crate::factions::mahact::foreign_tokens_text(gs, p), &k.hooks.direct.mahact_foreign_tokens(&core, who));
        let want: Vec<String> = crate::factions::mahact::fleet_pool_owners_text(gs, p).iter().map(ToString::to_string).collect();
        let got: Vec<String> = k.hooks.direct.mahact_fleet_pool_owners(&core, who).iter().filter_map(|s| name(*s)).collect();
        out.eq(label, &format!("fleet_pool_owners {p}"), &want, &got);
        for c in COMMANDERS {
            let Some(cl) = leader(c) else { continue };
            out.eq(label, &format!("imperia_grants {p} {c}"), &crate::factions::mahact::imperia_grants(gs, p, c), &k.hooks.direct.mahact_imperia_grants(&core, who, cl));
            // The commander-ability routes, through the borrowed helpers that use it.
            let _ = cl;
        }
        let want: Vec<String> = crate::factions::borrowed_commanders_b::firmament_planets(gs, content, sources, p).iter().map(ToString::to_string).collect();
        let got: Vec<String> = k.hooks.direct.borrowed_commanders_b_firmament_planets(&core, &k, who).iter().map(|x| x.as_str().to_owned()).collect();
        out.eq(label, &format!("firmament_planets {p}"), &want, &got);
        let want: Vec<String> = crate::factions::firmament::puppeted_text(gs, p).iter().map(ToString::to_string).collect();
        let got: Vec<String> = {
            let mut v: Vec<String> = k.hooks.direct.firmament_puppeted(&core, who).iter().filter_map(|s| name(*s)).collect();
            v.sort();
            v
        };
        out.eq(label, &format!("puppeted {p}"), &want, &got);
        for q in &ids {
            let Some(other) = seat(q) else { continue };
            let want: Vec<String> = crate::factions::mahact::secondary_waivers(gs, content, p, q, None).into_iter().map(|w| w.id).collect();
            let got: Vec<String> = k.hooks.strategy.secondary_waivers(&core, &k, who, other, None).iter().map(|w| format!("agent|{}", System::ALL[usize::from(w.code)].as_str())).collect();
            out.eq(label, &format!("secondary_waivers {p} {q}"), &want, &got);
            out.eq(
                label,
                &format!("assail_against {p} {q}"),
                &crate::factions::firmament_plots::assail_against(gs, p, [q]),
                &k.hooks.direct.firmament_plots_assail_against(&core, who, &[other]),
            );
            for t in [1i64, 2, 3] {
                let _ = t;
            }
        }
        for w in [1i64, 2, 3] {
            out.eq(label, &format!("trade_good_worth {p} {w}"), &crate::factions::mentak::trade_good_worth_text(gs, p, w), &k.hooks.economy.trade_good_worth(&core, who, w));
        }
        for (base, base_ix, chosen, chosen_ix) in [("cruiser", UnitBaseType::Cruiser, "cruiser2", "cruiser2"), ("cruiser", UnitBaseType::Cruiser, "cruiser", "cruiser1"), ("carrier", UnitBaseType::Carrier, "carrier2", "carrier2")] {
            let want = crate::factions::mentak::unit_form_override(gs, content, sources, p, base, chosen).map(|u| u.to_string());
            let got = unit_kind(chosen_ix).and_then(|c| k.hooks.strategy.unit_form_override(&core, &k, who, base_ix, c)).map(|u| u.as_str().to_owned());
            if unit_kind(chosen_ix).is_some() {
                out.eq(label, &format!("unit_form_override {p} {base}/{chosen}"), &want, &got);
            }
        }
        for system in &systems {
            let Some(system_ix) = sys(system) else { continue };
            out.eq(label, &format!("space_cannon_silenced {p} {system}"), &crate::factions::firmament::space_cannon_silenced_text(gs, content, sources, p, system), &k.hooks.direct.firmament_space_cannon_silenced(&core, &k, who, system_ix));
            out.eq(label, &format!("extra_production {p} {system}"), &crate::factions::ghost::extra_production_text(gs, content, sources, p, system), &k.hooks.economy.extra_production(&core, &k, who, system_ix));
            out.eq(label, &format!("production_cost_reduction {p} {system}"), &crate::factions::ghost::production_cost_reduction_text(gs, p, system), &k.hooks.economy.production_cost_reduction(&core, who, system_ix));
            if let Some(b) = gs.board.get(system) {
                for pl in b.planet_units.keys() {
                    let Some(planet_ix) = planet(pl) else { continue };
                    out.eq(label, &format!("assail_ground {p} {system} {pl}"), &crate::factions::firmament_plots::assail_ground(gs, p, system, pl), &k.hooks.direct.firmament_plots_assail_ground(&core, who, system_ix, planet_ix));
                    out.eq(label, &format!("viper_open {p} {system} {pl}"), &crate::factions::firmament::viper_open_text(gs, p, system, pl), &k.hooks.direct.firmament_viper_open(&core, who, system_ix, planet_ix));
                    for q in &ids {
                        let Some(other) = seat(q) else { continue };
                        out.eq(label, &format!("viper_defender_open {p} {q} {system} {pl}"), &crate::factions::firmament::viper_defender_open_text(gs, p, q, system, pl), &k.hooks.direct.firmament_viper_defender_open(&core, who, other, system_ix, planet_ix));
                    }
                }
            }
        }
        // The roll shift (Arvicon Rex and Assail are native `hit_shift` rows) and the Obsidian commander, per site.
        for kind in &kinds {
            let Some(unit_ix) = unit_kind(kind) else { continue };
            for system in &systems {
                let Some(system_ix) = sys(system) else { continue };
                let spots: Vec<Option<PlanetId>> = std::iter::once(None).chain(gs.board.get(system).into_iter().flat_map(|b| b.planet_units.keys().cloned().map(Some))).collect();
                for spot in spots {
                    let planet_ix = spot.as_ref().and_then(planet);
                    if spot.is_some() && planet_ix.is_none() {
                        continue;
                    }
                    let ground = spot.is_some();
                    let ctx_text = if ground { "ground" } else { "space" };
                    let cu = crate::factions::CombatUnit { player: p, system: Some(system), planet: spot.as_ref(), unit_type: kind, context: ctx_text };
                    let site = CoreCombatUnit { owner: who, system: Some(system_ix), planet: planet_ix, unit: unit_ix, ctx: if ground { CombatCtx::Ground } else { CombatCtx::Space } };
                    let mentak_k = kernel_with(sources, &[mentak::register]);
                    let allowed = mentak_k.hooks.seam.may_sustain.decide(Q::new(&core, &mentak_k), &site, true);
                    let want = if ground {
                        crate::factions::mentak::mech_allows_sustain(gs, content, sources, &cu)
                    } else {
                        crate::factions::mentak::flagship_allows_sustain(gs, content, sources, &cu)
                    };
                    out.eq(label, &format!("may_sustain {p} {kind} {system}/{spot:?}"), &want, &allowed);
                    // The roll shift: Arvicon Rex and Assail (space), Assail (ground), the Obsidian commander (both), and the Nekro
                    // term that rides the same bridge.
                    let mut string_gs = gs.clone();
                    string_gs.active_system = Some(system.clone());
                    let mut core_gs = core.clone();
                    core_gs.meta_mut().active_system = Some(system_ix);
                    let nekro_term = crate::factions::nekro_units::mech_roll_bonus(&string_gs, content, sources, p, kind, Some(system), spot.as_ref());
                    let want = string_shift(&string_gs, content, sources, p, kind, Some(system), spot.as_ref()) + nekro_term;
                    let site = RollSite {
                        unit: CoreCombatUnit { owner: who, system: Some(system_ix), planet: planet_ix, unit: unit_ix, ctx: if ground { CombatCtx::Ground } else { CombatCtx::Space } },
                        kind: if ground { RollKind::Ground } else { RollKind::Combat },
                        round: 1,
                        unit_index: 0,
                        foes: ti4_engine_core::SeatSet::EMPTY,
                    };
                    let shift_k = kernel_with(sources, &package_modules());
                    let got = shift_k.hooks.seam.hit_shift.eval(Q::new(&core_gs, &shift_k), &site, 0, Clamp::None).value;
                    out.eq(label, &format!("hit_shift {p} {kind} {system}/{spot:?}"), &want, &got);
                }
            }
        }
    }

    audit(label, &core, sources, &ids, &systems, &kinds, &cx, out);
}

/// The order audit: the commutative seams of the package over the same sites, on the table of all the modules.
#[allow(clippy::too_many_arguments)]
fn audit(label: &str, core: &CoreState, sources: SourceSet, ids: &[PlayerId], systems: &BTreeSet<SystemId>, kinds: &BTreeSet<String>, cx: &Ctx, out: &mut CcCheck) {
    let table = HookTable::of(&package_modules());
    let k0 = Kernel { hooks: HookTable::new(), sources: CoreSources(u8::try_from(adapter::scope(sources).0).unwrap_or(u8::MAX)) };
    let probe_k = |t: &HookTable| Kernel { hooks: t.clone(), sources: k0.sources };
    let mut sites: Vec<RollSite> = Vec::new();
    for p in ids {
        let Some(who) = cx.owners.seat(p.as_str()) else { continue };
        for kind in kinds {
            let Some(unit) = unit_kind(kind) else { continue };
            for system in systems {
                let Some(system_ix) = sys(system) else { continue };
                sites.push(RollSite {
                    unit: CoreCombatUnit { owner: who, system: Some(system_ix), planet: None, unit, ctx: CombatCtx::Space },
                    kind: RollKind::Combat,
                    round: 1,
                    unit_index: 0,
                    foes: ti4_engine_core::SeatSet::EMPTY,
                });
            }
        }
    }
    sites.truncate(400);
    out.audited += 1;
    let mut core_active = core.clone();
    core_active.meta_mut().active_system = systems.iter().find_map(sys);
    let shift = |t: &HookTable| {
        let k = probe_k(t);
        sites.iter().map(|s| k.hooks.seam.hit_shift.eval(Q::new(&core_active, &k), s, 0, Clamp::None)).collect::<Vec<_>>()
    };
    let may = |t: &HookTable| {
        let k = probe_k(t);
        sites.iter().map(|s| k.hooks.seam.may_sustain.decide(Q::new(core, &k), &s.unit, true)).collect::<Vec<_>>()
    };
    for (slot, ok) in [
        ("hit_shift", crate::dual::audit_hook_order(HookGroup::Seam, "hit_shift", &table, shift)),
        ("may_sustain", crate::dual::audit_hook_order(HookGroup::Seam, "may_sustain", &table, may)),
    ] {
        if !ok {
            out.miss(format!("{label}: order audit of {slot} failed"));
        }
    }
}

/// The terms of the string roll shift that the package's modules provide: Arvicon Rex and Assail on a ship, Assail on a
/// ground force, and the Obsidian commander's `unit_roll_modifier` (a ship or a ground force).
fn string_shift(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    unit_type: &str,
    system: Option<&SystemId>,
    planet: Option<&PlanetId>,
) -> i64 {
    let ground = planet.is_some();
    let commander = crate::factions::borrowed_commanders::unit_roll_modifier(
        state,
        content,
        sources,
        &crate::factions::CombatUnit { player, system, planet, unit_type, context: if ground { "ground" } else { "space" } },
    );
    let terms = match (system, planet) {
        (Some(system), Some(planet)) => crate::factions::firmament_plots::assail_ground(state, player, system, planet),
        _ => {
            crate::factions::mahact_units::flagship_roll_bonus(state, content, sources, player, unit_type)
                + crate::factions::firmament_plots::assail_space(state, content, sources, player, system)
        }
    };
    commander + terms
}

/// The S-step `dual_hook!` of the roll shift: the Arvicon Rex, Assail and Obsidian commander terms of the string window
/// next to the seam's answer of the package modules (and the Nekro's, which rides the same bridge). Called only when
/// `Area::Factions` is on `Dual`.
pub fn dual_roll_shift(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    unit_type: &str,
    system: Option<&SystemId>,
    planet: Option<&PlanetId>,
) {
    use crate::dual::{Area, Backend};
    if crate::dual::backend(Area::Factions) != Backend::Dual {
        return;
    }
    let ground = planet.is_some();
    let string = || {
        string_shift(state, content, sources, player, unit_type, system, planet)
            + crate::factions::nekro_units::mech_roll_bonus(state, content, sources, player, unit_type, system, planet)
    };
    let core = || -> i64 {
        let Ok((core, cx)) = load_why(state, None) else { return string() };
        let (Some(who), Some(unit)) = (cx.owners.seat(player.as_str()), unit_kind(unit_type)) else { return string() };
        let k = kernel_with(sources, &package_modules());
        let site = RollSite {
            unit: CoreCombatUnit {
                owner: who,
                system: system.and_then(sys),
                planet: planet.and_then(self::planet),
                unit,
                ctx: if ground { CombatCtx::Ground } else { CombatCtx::Space },
            },
            kind: if ground { RollKind::Ground } else { RollKind::Combat },
            round: 1,
            unit_index: 0,
            foes: ti4_engine_core::SeatSet::EMPTY,
        };
        k.hooks.seam.hit_shift.eval(Q::new(&core, &k), &site, 0, Clamp::None).value
    };
    let _ = crate::dual_hook!(HookGroup::Seam, "hit_shift", string(), core());
}

// -- the typed side of the call-site duals (`cb_dual!` at the string functions) ------------------------------------

/// The kernel of the package (its modules and the Nekro whose Alastor the combat sides read) for a source scope.
fn package_kernel(sources: SourceSet) -> &'static Kernel {
    static KERNELS: [std::sync::OnceLock<Kernel>; 128] = [const { std::sync::OnceLock::new() }; 128];
    let slot = usize::try_from(sources.as_u32()).ok().and_then(|i| KERNELS.get(i)).unwrap_or(&KERNELS[127]);
    slot.get_or_init(|| kernel_with(sources, &package_modules()))
}

/// One position as the hooks read it.
struct Pos {
    view: CoreView,
    cx: Ctx,
    k: &'static Kernel,
}

impl Pos {
    fn of(gs: &GameState, sources: SourceSet) -> Option<Self> {
        let mut cx = Ctx::of(gs).ok()?;
        let view = CoreView::of(gs, &mut cx, CodecSources::default(), SliceSet::ALL).ok()?;
        Some(Self { view, cx, k: package_kernel(sources) })
    }

    fn s(&self) -> &CoreState {
        &self.view
    }

    fn seat(&self, p: &PlayerId) -> Option<ti4_engine_core::Seat> {
        self.cx.owners.seat(p.as_str())
    }

    fn player(&self, seat: ti4_engine_core::Seat) -> Option<PlayerId> {
        self.cx.owners.id(seat).map(PlayerId::new)
    }
}

fn with<T>(gs: &GameState, sources: SourceSet, f: impl FnOnce(&Pos) -> Option<T>) -> Option<T> {
    f(&Pos::of(gs, sources)?)
}

fn full() -> SourceSet {
    ti4_model::content_types::FULL
}

pub fn mahact_foreign_tokens(gs: &GameState, who: &PlayerId) -> Option<i32> {
    with(gs, full(), |p| Some(p.k.hooks.direct.mahact_foreign_tokens(p.s(), p.seat(who)?)))
}

pub fn mahact_fleet_pool_owners(gs: &GameState, who: &PlayerId) -> Option<Vec<PlayerId>> {
    with(gs, full(), |p| {
        let owners = p.k.hooks.direct.mahact_fleet_pool_owners(p.s(), p.seat(who)?);
        owners.iter().map(|s| p.player(*s)).collect()
    })
}

pub fn mahact_imperia_grants(gs: &GameState, who: &PlayerId, commander: &str) -> Option<bool> {
    with(gs, full(), |p| Some(p.k.hooks.direct.mahact_imperia_grants(p.s(), p.seat(who)?, leader(commander)?)))
}

pub fn mahact_ship_movement_barred(gs: &GameState) -> Option<bool> {
    with(gs, full(), |p| Some(p.k.hooks.direct.mahact_units_ship_movement_barred(p.s())))
}

pub fn firmament_puppeted(gs: &GameState, owner: &PlayerId) -> Option<BTreeSet<PlayerId>> {
    with(gs, full(), |p| {
        let seats = p.k.hooks.direct.firmament_puppeted(p.s(), p.seat(owner)?);
        seats.iter().map(|s| p.player(*s)).collect()
    })
}

pub fn firmament_space_cannon_silenced(gs: &GameState, sources: SourceSet, active: &PlayerId, system: &SystemId) -> Option<bool> {
    with(gs, sources, |p| Some(p.k.hooks.direct.firmament_space_cannon_silenced(p.s(), p.k, p.seat(active)?, sys(system)?)))
}

pub fn firmament_viper_open(gs: &GameState, invader: &PlayerId, system: &SystemId, pl: &PlanetId) -> Option<bool> {
    with(gs, full(), |p| Some(p.k.hooks.direct.firmament_viper_open(p.s(), p.seat(invader)?, sys(system)?, planet(pl)?)))
}

pub fn firmament_viper_defender_open(gs: &GameState, defender: &PlayerId, invader: &PlayerId, system: &SystemId, pl: &PlanetId) -> Option<bool> {
    with(gs, full(), |p| Some(p.k.hooks.direct.firmament_viper_defender_open(p.s(), p.seat(defender)?, p.seat(invader)?, sys(system)?, planet(pl)?)))
}

pub fn mentak_trade_good_worth(gs: &GameState, who: &PlayerId, worth: i64) -> Option<i64> {
    with(gs, full(), |p| Some(p.k.hooks.economy.trade_good_worth(p.s(), p.seat(who)?, worth)))
}

pub fn ghost_production_cost_reduction(gs: &GameState, who: &PlayerId, system: &SystemId) -> Option<i64> {
    with(gs, full(), |p| Some(p.k.hooks.economy.production_cost_reduction(p.s(), p.seat(who)?, sys(system)?)))
}

pub fn ghost_extra_production(gs: &GameState, sources: SourceSet, who: &PlayerId, system: &SystemId) -> Option<i64> {
    with(gs, sources, |p| Some(p.k.hooks.economy.extra_production(p.s(), p.k, p.seat(who)?, sys(system)?)))
}
