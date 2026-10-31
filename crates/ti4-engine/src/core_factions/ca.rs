//! The dual of package CA (combat line: bastion, crimson, nekro, ralnel), tier Q.
//!
//! For one position: every pure hook the package registered, asked of the typed state through a kernel that holds
//! only that faction's module, next to the string function it was moved from; every seat, system, planet and unit
//! kind the position offers. Also the order audit of the commutative seams over the same sites, and the hook-level
//! `dual_hook!` of the S-step functions ([`dual_roll_shift`]).

use std::collections::BTreeSet;

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Planet, System, Unit as CoreUnit};
use ti4_engine_core::hooks::{CombatCtx, HookTable};
use ti4_engine_core::kernel::seam::{Clamp, CombatUnit as CoreCombatUnit, Q, RollKind, RollSite};
use ti4_engine_core::kernel::{Kernel, Sources as CoreSources};
use ti4_engine_core::state::{CoreState, Piece, SliceSet};
use ti4_engine_core_factions::ca_combat::{bastion, crimson, nekro, ralnel};
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlanetId, PlayerId, SystemId};
use ti4_model::state::GameState;
use ti4_model::units::Unit;

use crate::core_adapter as adapter;
use crate::core_sync::codec::Sources as CodecSources;
use crate::core_sync::{CoreView, Ctx};
use crate::dual::HookGroup;
use crate::generated::IdStr;

/// The modules whose tier Q this package has ported.
pub const PORTED: &[ti4_engine_core_factions::Id] = &[
    ti4_engine_core_factions::Id::Bastion,
    ti4_engine_core_factions::Id::Crimson,
    ti4_engine_core_factions::Id::Nekro,
    ti4_engine_core_factions::Id::Ralnel,
];

/// What was compared.
#[derive(Debug, Default, Clone)]
pub struct CaCheck {
    pub positions: u64,
    pub unmapped: u64,
    pub compared: u64,
    pub audited: u64,
    pub mismatches: Vec<String>,
}

impl CaCheck {
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
    Kernel {
        hooks: HookTable::of(modules),
        sources: CoreSources(u8::try_from(adapter::scope(sources).0).unwrap_or(u8::MAX)),
    }
}

/// The typed state of a position, whole.
fn load(gs: &GameState, galaxy: Option<&Galaxy>) -> Option<(CoreState, Ctx)> {
    load_why(gs, galaxy).ok()
}

fn load_why(gs: &GameState, galaxy: Option<&Galaxy>) -> Result<(CoreState, Ctx), String> {
    let mut cx = Ctx::of(gs).map_err(|e| format!("{e:?}"))?;
    // Without the string galaxy the map slice cannot be loaded (and a strict gate binary panics at the refusal): the hooks that
    // read it are handed the galaxy by their call site, the others do not need it.
    let slices = if galaxy.is_some() { SliceSet::ALL } else { SliceSet::of_without(SliceSet::ALL, ti4_engine_core::state::Slice::Map) };
    let view = CoreView::of(gs, &mut cx, CodecSources { galaxy, rng: None }, slices).map_err(|e| format!("{e:?}"))?;
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

fn piece(cx: &Ctx, u: &Unit) -> Option<Piece> {
    Some(
        Piece::new(unit_kind(u.type_id.as_str())?, cx.owners.seat(u.owner.as_str())?)
            .with_sustained(u.sustained_damage)
            .with_galvanized(u.galvanized),
    )
}

/// Where each unit stands: `(system, planet, unit)`.
fn units_of(gs: &GameState) -> Vec<(SystemId, Option<PlanetId>, Unit)> {
    let mut out = Vec::new();
    for (system, board) in &gs.board {
        out.extend(board.units.iter().map(|u| (system.clone(), None, u.clone())));
        for (planet, units) in &board.planet_units {
            out.extend(units.iter().map(|u| (system.clone(), Some(planet.clone()), u.clone())));
        }
    }
    out
}

/// Unit kinds the cross-product probes besides the ones on the board: the flagship, mech and structures of the package.
const PROBE_KINDS: [&str; 8] =
    ["bastion_flagship", "nekro_mech", "nekro_flagship", "crimson_flagship", "ralnel_flagship", "ralnel_mech", "pds", "spacedock"];

/// A position and the same position turned into the scenarios the golden games rarely reach: a Nekro with an X token
/// next to an opponent that owns the technology, a Last Bastion with galvanized units and Proxima, a Crimson with two
/// active breaches and an enemy Quietus, a Ral Nel with structures, Linkships and an Alarum.
pub fn compare(label: &str, gs: &GameState, galaxy: &Galaxy, sources: SourceSet, out: &mut CaCheck) {
    compare_state(label, gs, galaxy, sources, out);
    for (name, variant) in variants(gs) {
        compare_state(&format!("{label} [{name}]"), &variant, galaxy, sources, out);
    }
}

fn variants(gs: &GameState) -> Vec<(&'static str, GameState)> {
    use ti4_model::id::{FactionId, TechnologyId};
    let mut out = Vec::new();
    if gs.players.len() < 2 {
        return out;
    }
    let (a, b) = (gs.players[0].id.clone(), gs.players[1].id.clone());
    let hot: Vec<SystemId> = gs.board.iter().filter(|(_, s)| !s.units.is_empty() || !s.planet_units.is_empty()).map(|(k, _)| k.clone()).take(3).collect();
    let Some(first) = hot.first().cloned() else { return out };
    let planet_of = |gs: &GameState, s: &SystemId| gs.board.get(s).and_then(|b| b.planet_units.keys().next().cloned());
    let unit = |t: &str, o: &PlayerId| Unit::new(ti4_model::id::UnitTypeId::new(t), o.clone());

    let mut v = gs.clone();
    v.players[0].faction = FactionId::new("nekro");
    v.players[0].technologies.insert(TechnologyId::new("vax"));
    v.players[1].technologies.insert(TechnologyId::new("amd"));
    v.players[0].assimilated_technologies.insert("vax".to_owned(), TechnologyId::new("amd"));
    v.activation_seq = 7;
    v.system_mut(&first).units.push(unit("carrier", &b));
    v.system_mut(&first).units.push(unit("nekro_mech", &a));
    if let Some(pl) = planet_of(&v, &first) {
        v.system_mut(&first).planet_units.entry(pl.clone()).or_default().extend([unit("nekro_mech", &a), unit("infantry", &a), unit("infantry", &a), unit("infantry", &b)]);
        v.faction_marks.insert(format!("nekro:alastor|7|{first}|{a}|{pl}|infantry"), "1".to_owned());
    }
    v.faction_marks.insert(format!("nekro:propagation:{a}"), "2".to_owned());
    out.push(("nekro", v));

    let mut v = gs.clone();
    v.players[0].faction = FactionId::new("bastion");
    for p in &mut v.players[..2] {
        p.technologies.insert(TechnologyId::new("proxima"));
    }
    for board in v.board.values_mut() {
        for u in board.units.iter_mut().chain(board.planet_units.values_mut().flatten()) {
            u.galvanized = true;
        }
    }
    v.system_mut(&first).units.push(unit("bastion_flagship", &a));
    if let Some(pl) = planet_of(&v, &first) {
        v.system_mut(&first).planet_units.entry(pl).or_default().push(unit("bastion_spacedock2", &a));
    }
    if let Some(other) = hot.get(1) {
        v.faction_marks.insert(format!("bastion:retreated:{first}:{b}"), format!("{other}|carrier:0"));
        v.faction_marks.insert(format!("bastion:retreated:{first}:{a}"), format!("{other}|cruiser:1"));
        v.faction_marks.insert(format!("bastion:lost:{first}:{a}:carrier"), "2".to_owned());
    }
    out.push(("bastion", v));

    let mut v = gs.clone();
    v.players[0].faction = FactionId::new("crimson");
    for s in hot.iter().take(2) {
        v.breach_tokens.insert(s.clone());
        v.faction_marks.insert(format!("crimson:breach:active:{s}"), "1".to_owned());
    }
    v.system_mut(hot.last().unwrap_or(&first)).units.push(unit("crimson_flagship", &b));
    v.activation_seq = 5;
    v.faction_marks.insert(format!("crimson:deployed:{b}:3"), "1".to_owned());
    v.system_mut(&first).units.push(unit("carrier", &a));
    out.push(("crimson", v));

    let mut v = gs.clone();
    v.players[0].faction = FactionId::new("ralnel");
    v.system_mut(&first).units.extend([unit("pds", &a), unit("spacedock", &a), unit("ralnel_destroyer", &a), unit("ralnel_destroyer", &a), unit("ralnel_destroyer2", &a)]);
    if let Some(pl) = planet_of(&v, &first) {
        v.system_mut(&first).planet_control.insert(pl.clone(), a.clone());
        v.system_mut(&first).planet_units.entry(pl).or_default().push(unit("ralnel_mech", &a));
    }
    out.push(("ralnel", v));
    out
}

/// Compare every ported hook on one position.
fn compare_state(label: &str, gs: &GameState, galaxy: &Galaxy, sources: SourceSet, out: &mut CaCheck) {
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
    let bastion_k = kernel_with(sources, &[bastion::register]);
    let crimson_k = kernel_with(sources, &[crimson::register]);
    let nekro_k = kernel_with(sources, &[nekro::register]);
    let ralnel_k = kernel_with(sources, &[ralnel::register]);
    let ids: Vec<PlayerId> = gs.players.iter().map(|p| p.id.clone()).collect();
    let seat = |p: &PlayerId| cx.owners.seat(p.as_str());
    let placed = units_of(gs);
    // The systems worth asking about: those with units or planets with entries.
    let systems: BTreeSet<SystemId> = gs.board.keys().cloned().collect();

    // -- bastion --------------------------------------------------------------------------------------------
    for p in &ids {
        let Some(who) = seat(p) else { continue };
        out.eq(
            label,
            &format!("non_home_systems_with_planets {p}"),
            &crate::factions::bastion_units::non_home_systems_with_planets(gs, content, sources, p),
            &bastion_k.hooks.direct.bastion_units_non_home_systems_with_planets(&core, &bastion_k, who),
        );
        for q in &ids {
            let Some(other) = seat(q) else { continue };
            out.eq(
                label,
                &format!("watches_ground_round {p} {q}"),
                &crate::factions::bastion_units::watches_ground_round(gs, &[p, q]),
                &bastion_k.hooks.direct.bastion_units_watches_ground_round(&core, &[who, other]),
            );
        }
    }
    for (system, spot, u) in &placed {
        let (Some(pc), Some(system_ix)) = (piece(&cx, u), sys(system)) else { continue };
        out.eq(
            label,
            &format!("extra_die {} {}", u.type_id, u.galvanized),
            &crate::factions::bastion::extra_die(u),
            &bastion_k.hooks.direct.bastion_extra_die(&core, pc),
        );
        if let Some(pl) = spot {
            let Some(planet_ix) = planet(pl) else { continue };
            for victim in &ids {
                let Some(v) = seat(victim) else { continue };
                for hits in [0usize, 1, 3] {
                    out.eq(
                        label,
                        &format!("cancel_bombardment_hits {victim} {pl} {hits}"),
                        &crate::factions::bastion_units::cancel_bombardment_hits(gs, victim, system, pl, hits),
                        &bastion_k.hooks.direct.bastion_units_cancel_bombardment_hits(&core, v, system_ix, planet_ix, hits),
                    );
                }
                let mut string_groups = vec![2usize, 1, 3];
                crate::factions::bastion_units::cancel_in_groups(gs, victim, system, pl, &mut string_groups);
                let mut core_groups: SmallList<usize, 8> = SmallList::from_slice(&[2, 1, 3]);
                bastion_k.hooks.direct.bastion_units_cancel_in_groups(&core, v, system_ix, planet_ix, &mut core_groups);
                out.eq(label, &format!("cancel_in_groups {victim} {pl}"), &string_groups, &core_groups.to_vec());
            }
            let want = crate::factions::bastion_units::resource_bonus(gs, pl);
            out.eq(label, &format!("resource_bonus {pl}"), &want, &bastion_k.hooks.direct.bastion_units_resource_bonus(&core, planet_ix));
        }
    }
    // Every planet with an entry, for the dock bonus.
    for board in gs.board.values() {
        for pl in board.planet_units.keys().chain(board.planet_control.keys()) {
            if let Some(planet_ix) = planet(pl) {
                out.eq(
                    label,
                    &format!("resource_bonus {pl}"),
                    &crate::factions::bastion_units::resource_bonus(gs, pl),
                    &bastion_k.hooks.direct.bastion_units_resource_bonus(&core, planet_ix),
                );
            }
        }
    }

    // -- roll shifts: the Egeiro and Mordred, over every seat, unit kind, system and context ------------------------
    let kinds: BTreeSet<String> = placed.iter().map(|(_, _, u)| u.type_id.as_str().to_owned()).chain(PROBE_KINDS.map(str::to_owned)).collect();
    let both = kernel_with(sources, &[bastion::register, nekro::register]);
    for p in &ids {
        let Some(who) = seat(p) else { continue };
        for kind in &kinds {
            let Some(unit_ix) = unit_kind(kind) else { continue };
            for system in &systems {
                let Some(system_ix) = sys(system) else { continue };
                let spots: Vec<Option<PlanetId>> = std::iter::once(None)
                    .chain(gs.board.get(system).into_iter().flat_map(|b| b.planet_units.keys().cloned().map(Some)))
                    .collect();
                for spot in spots {
                    let planet_ix = spot.as_ref().and_then(planet);
                    if spot.is_some() && planet_ix.is_none() {
                        continue;
                    }
                    let ctx = if spot.is_some() { "ground" } else { "space" };
                    let want = crate::factions::bastion_units::unit_roll_modifier(
                        gs,
                        content,
                        sources,
                        &crate::factions::CombatUnit { player: p, system: Some(system), planet: spot.as_ref(), unit_type: kind, context: ctx },
                    ) + crate::factions::nekro_units::mech_roll_bonus(gs, content, sources, p, kind, Some(system), spot.as_ref());
                    let site = RollSite {
                        unit: CoreCombatUnit {
                            owner: who,
                            system: Some(system_ix),
                            planet: planet_ix,
                            unit: unit_ix,
                            ctx: if spot.is_some() { CombatCtx::Ground } else { CombatCtx::Space },
                        },
                        kind: if spot.is_some() { RollKind::Ground } else { RollKind::Combat },
                        round: 1,
                        unit_index: 0,
                        foes: ti4_engine_core::SeatSet::EMPTY,
                    };
                    let got = both.hooks.seam.hit_shift.eval(Q::new(&core, &both), &site, 0, Clamp::None).value;
                    out.eq(label, &format!("hit_shift {p} {kind} {system}/{spot:?}"), &want, &got);
                }
            }
        }
    }

    // -- crimson ---------------------------------------------------------------------------------------------------
    let want: Vec<String> = crate::factions::crimson::active_breaches(gs).iter().map(ToString::to_string).collect();
    let got: Vec<String> = crimson_k.hooks.direct.crimson_active_breaches(&core).iter().map(|s| s.as_str().to_owned()).collect();
    out.eq(label, "active_breaches", &want, &got);
    for p in &ids {
        let Some(who) = seat(p) else { continue };
        for system in &systems {
            let Some(system_ix) = sys(system) else { continue };
            out.eq(
                label,
                &format!("abilities_lost {p} {system}"),
                &crate::factions::crimson::abilities_lost(gs, p, system),
                &crimson_k.hooks.direct.crimson_abilities_lost(&core, who, system_ix),
            );
            for kind in &kinds {
                let Some(unit_ix) = unit_kind(kind) else { continue };
                let site = CoreCombatUnit { owner: who, system: Some(system_ix), planet: None, unit: unit_ix, ctx: CombatCtx::Space };
                let allowed = crimson_k.hooks.seam.may_sustain.decide(Q::new(&core, &crimson_k), &site, true);
                out.eq(label, &format!("may_sustain {p} {system} {kind}"), &!crate::factions::crimson::abilities_lost(gs, p, system), &allowed);
            }
            let landable: Vec<PlanetId> = gs.board.get(system).into_iter().flat_map(|b| b.planet_control.keys().cloned()).collect();
            let landable_ix: Vec<Planet> = landable.iter().filter_map(planet).collect();
            if landable.len() == landable_ix.len() {
                let want: Vec<String> = crate::factions::crimson::deploy_planets(gs, content, sources, p, system, &landable).iter().map(ToString::to_string).collect();
                let got: Vec<String> = crimson_k.hooks.direct.crimson_deploy_planets(&core, &crimson_k, who, system_ix, &landable_ix).iter().map(|x| x.as_str().to_owned()).collect();
                out.eq(label, &format!("deploy_planets {p} {system}"), &want, &got);
            }
        }
    }

    // -- nekro ---------------------------------------------------------------------------------------------------------
    let want: Vec<String> = crate::factions::nekro::pending_propagation(gs).iter().map(ToString::to_string).collect();
    let got: Vec<String> = nekro_k.hooks.direct.nekro_pending_propagation(&core).iter().filter_map(|s| cx.owners.id(*s).map(str::to_owned)).collect();
    out.eq(label, "pending_propagation", &want, &got);
    for p in &ids {
        let Some(who) = seat(p) else { continue };
        for system in &systems {
            let Some(system_ix) = sys(system) else { continue };
            let want: Vec<(String, usize)> = crate::factions::nekro_units::alastor_participants(gs, p, system).into_iter().map(|(pl, i)| (pl.to_string(), i)).collect();
            let got: Vec<(String, usize)> = nekro_k.hooks.direct.nekro_units_alastor_participants(&core, who, system_ix).iter().map(|(pl, i)| (pl.as_str().to_owned(), usize::from(*i))).collect();
            out.eq(label, &format!("alastor_participants {p} {system}"), &want, &got);
            for kind in &kinds {
                let Some(unit_ix) = unit_kind(kind) else { continue };
                out.eq(
                    label,
                    &format!("alastor_covers {p} {system} {kind}"),
                    &crate::factions::nekro_units::alastor_covers(gs, system, p, kind),
                    &nekro_k.hooks.direct.nekro_units_alastor_covers(&core, system_ix, who, unit_ix),
                );
            }
        }
    }

    // -- ralnel --------------------------------------------------------------------------------------------------------
    out.eq(
        label,
        "listens_for_tactical_end",
        &crate::factions::ralnel::listens_for_tactical_end(gs, content, sources),
        &ralnel_k.hooks.direct.ralnel_listens_for_tactical_end(&core, &ralnel_k),
    );
    for (system, spot, u) in &placed {
        let (Some(pc), Some(system_ix)) = (piece(&cx, u), sys(system)) else { continue };
        if spot.is_none() {
            out.eq(
                label,
                &format!("silenced_in_space {} {}", u.owner, u.type_id),
                &crate::factions::ralnel::silenced_in_space(gs, content, sources, u),
                &ralnel_k.hooks.direct.ralnel_silenced_in_space(&core, &ralnel_k, pc),
            );
        }
        let _ = system_ix;
    }
    for system in &systems {
        let Some(system_ix) = sys(system) else { continue };
        let want: Vec<Piece> = crate::factions::ralnel::linkship_guns(gs, content, sources, system, &|_| true).iter().filter_map(|u| piece(&cx, u)).collect();
        let got = ralnel_k.hooks.direct.ralnel_linkship_guns(&core, &ralnel_k, system_ix).to_vec();
        out.eq(label, &format!("linkship_guns {system}"), &want, &got);
        if let Some(b) = gs.board.get(system) {
            for pl in b.planet_units.keys() {
                if let Some(planet_ix) = planet(pl) {
                    out.eq(label, &format!("alarum_on {system} {pl}"), &crate::factions::ralnel::alarum_on(gs, system, pl), &ralnel_k.hooks.direct.ralnel_alarum_on(&core, system_ix, planet_ix));
                }
            }
        }
    }
    for p in &ids {
        let Some(who) = seat(p) else { continue };
        for kind in &kinds {
            let Some(unit_ix) = unit_kind(kind) else { continue };
            out.eq(
                label,
                &format!("flagship_among {p} {kind}"),
                &crate::factions::ralnel::flagship_among(gs, p, std::slice::from_ref(kind)),
                &ralnel_k.hooks.direct.ralnel_flagship_among(&core, who, &[unit_ix]),
            );
        }
    }

    audit(label, &core, sources, &ids, &systems, &kinds, &cx, out);
    observers(label, gs, &core, &cx, sources, out);
}

/// The marks the tier O hooks write, as the string state holds them.
fn watched(gs: &GameState) -> Vec<(String, String)> {
    gs.faction_marks
        .iter()
        .filter(|(k, _)| k.starts_with("bastion:lost:") || k.starts_with("bastion:retreated:") || k.starts_with("nekro:alastor") || k.starts_with("crimson:deployed:"))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Tier O: each bookkeeping function applied to a copy of the string state and the typed state, the marks of the
/// typed one stored back and compared.
fn observers(label: &str, gs: &GameState, core: &CoreState, cx: &Ctx, sources: SourceSet, out: &mut CaCheck) {
    use ti4_engine_core::kernel::{Cx, FirstOption};
    let k = kernel_with(sources, &[bastion::register, crimson::register, nekro::register, ralnel::register]);
    let mut string = gs.clone();
    let mut typed = core.clone();
    let check = |what: &str, string: &GameState, typed: &CoreState, out: &mut CaCheck| {
        let mut back = string.clone();
        back.faction_marks.retain(|key, _| !(key.starts_with("bastion:lost:") || key.starts_with("bastion:retreated:") || key.starts_with("nekro:alastor") || key.starts_with("crimson:deployed:")));
        if crate::core_sync::Codec::store_slice(cx, typed, &mut back, ti4_engine_core::state::Slice::Marks).is_err() {
            out.unmapped += 1;
            return;
        }
        out.eq(label, what, &watched(string), &watched(&back));
    };
    let mut asker = FirstOption;
    for (system, spot, u) in units_of(gs) {
        let (Some(pc), Some(system_ix)) = (piece(cx, &u), sys(&system)) else { continue };
        if spot.is_none() {
            crate::factions::bastion::note_ship_lost(&mut string, &system, &u);
            k.hooks.direct.bastion_note_ship_lost(&mut Cx::new(&mut typed, &k, &mut asker), system_ix, pc);
        }
    }
    check("bastion note_ship_lost", &string, &typed, out);
    for (system, _, u) in units_of(gs) {
        let (Some(owner), Some(system_ix), Some(unit_ix)) = (cx.owners.seat(u.owner.as_str()), sys(&system), unit_kind(u.type_id.as_str())) else { continue };
        let want = crate::factions::bastion::take_ship_lost(&mut string, &system, &u.owner, &u.type_id);
        let got = k.hooks.direct.bastion_take_ship_lost(&mut Cx::new(&mut typed, &k, &mut asker), system_ix, owner, unit_ix);
        out.eq(label, "bastion take_ship_lost", &want, &got);
    }
    check("bastion take_ship_lost marks", &string, &typed, out);
    for system in gs.board.keys() {
        let Some(system_ix) = sys(system) else { continue };
        crate::factions::bastion::clear_retreated(&mut string, system);
        k.hooks.direct.bastion_clear_retreated(&mut Cx::new(&mut typed, &k, &mut asker), system_ix);
    }
    check("bastion clear_retreated", &string, &typed, out);

    for (system, spot, u) in units_of(gs) {
        let (Some(spot_id), Some(pl), Some(owner), Some(system_ix), Some(unit_ix)) =
            (spot.as_ref(), spot.as_ref().and_then(planet), cx.owners.seat(u.owner.as_str()), sys(&system), unit_kind(u.type_id.as_str())) else { continue };
        crate::factions::nekro_units::alastor_decrement(&mut string, &system, &u.owner, spot_id, u.type_id.as_str());
        k.hooks.direct.nekro_units_alastor_decrement(&mut Cx::new(&mut typed, &k, &mut asker), system_ix, owner, pl, unit_ix);
    }
    check("nekro alastor_decrement", &string, &typed, out);
    let first = gs.board.keys().next().cloned();
    if let Some(system) = first {
        if let Some(system_ix) = sys(&system) {
            let who = gs.players.first().map(|p| p.id.clone());
            let seat = who.as_ref().and_then(|p| cx.owners.seat(p.as_str()));
            crate::factions::nekro_units::alastor_clear(&mut string, &system, who.as_ref());
            k.hooks.direct.nekro_units_alastor_clear(&mut Cx::new(&mut typed, &k, &mut asker), system_ix, seat);
            check("nekro alastor_clear (one)", &string, &typed, out);
            crate::factions::nekro_units::alastor_clear(&mut string, &system, None);
            k.hooks.direct.nekro_units_alastor_clear(&mut Cx::new(&mut typed, &k, &mut asker), system_ix, None);
            check("nekro alastor_clear (all)", &string, &typed, out);
        }
    }
    for p in &gs.players {
        let Some(seat) = cx.owners.seat(p.id.as_str()) else { continue };
        crate::factions::crimson::note_deployed(&mut string, &p.id);
        k.hooks.direct.crimson_note_deployed(&mut Cx::new(&mut typed, &k, &mut asker), seat);
    }
    check("crimson note_deployed", &string, &typed, out);
}

/// The order audit: the commutative seams of the package over the same sites, on the table of all four modules.
#[allow(clippy::too_many_arguments)]
fn audit(label: &str, core: &CoreState, sources: SourceSet, ids: &[PlayerId], systems: &BTreeSet<SystemId>, kinds: &BTreeSet<String>, cx: &Ctx, out: &mut CaCheck) {
    let table = HookTable::of(&[bastion::register, crimson::register, nekro::register, ralnel::register]);
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
    let shift = |t: &HookTable| {
        let k = probe_k(t);
        sites.iter().map(|s| k.hooks.seam.hit_shift.eval(Q::new(core, &k), s, 0, Clamp::None)).collect::<Vec<_>>()
    };
    let may = |t: &HookTable| {
        let k = probe_k(t);
        sites.iter().map(|s| k.hooks.seam.may_sustain.decide(Q::new(core, &k), &s.unit, true)).collect::<Vec<_>>()
    };
    for (slot, ok) in [("hit_shift", crate::dual::audit_hook_order(HookGroup::Seam, "hit_shift", &table, shift)), ("may_sustain", crate::dual::audit_hook_order(HookGroup::Seam, "may_sustain", &table, may))] {
        if !ok {
            out.miss(format!("{label}: order audit of {slot} failed"));
        }
    }
}

/// The S-step `dual_hook!` of the roll shift: the Egeiro and Mordred terms of the string window next to the seam's
/// answer of the package modules. Called only when `Area::Factions` is on `Dual`.
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
        crate::factions::bastion_units::unit_roll_modifier(
            state,
            content,
            sources,
            &crate::factions::CombatUnit { player, system, planet, unit_type, context: if ground { "ground" } else { "space" } },
        ) + crate::factions::nekro_units::mech_roll_bonus(state, content, sources, player, unit_type, system, planet)
    };
    let core = || -> i64 {
        let Some((core, cx)) = load(state, None) else { return string() };
        let (Some(who), Some(unit)) = (cx.owners.seat(player.as_str()), unit_kind(unit_type)) else { return string() };
        let k = kernel_with(sources, &[bastion::register, nekro::register]);
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

#[cfg(test)]
mod strict_tests {
    use super::*;
    use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};

    /// Found by the end-of-D1 gate run (`dual_run ... all=dual`, strict): the roll shift of a unit loaded the whole position
    /// without a galaxy, so the map slice refused and a gate binary panicked (without strict mode the typed side silently fell
    /// back to the string answer and compared nothing).
    #[test]
    fn the_roll_shift_dual_loads_a_view_without_the_map_and_compares_in_strict_mode() {
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let gs = crate::fixtures::seated_game(&[("a", "nekro"), ("b", "sol")], ti4_model::content_types::POK);
        let (content, sources) = (ContentStore::embedded(), ti4_model::content_types::POK);
        with_mask(AreaMask::STRING.with(Area::Factions, Backend::Dual), || {
            crate::core_sync::unmapped::strict_on_this_thread(|| {
                dual_roll_shift(&gs, content, sources, &PlayerId::new("a"), "cruiser", Some(&SystemId::new("18")), None);
            });
        });
        let r = report();
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        assert!(r.areas.get(&Area::Factions).is_some_and(|c| c.calls >= 1), "the hook was compared: {:?}", r.areas);
        let _ = take_report();
    }
}
