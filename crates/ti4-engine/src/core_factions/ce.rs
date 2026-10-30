//! Package CE (movement-and-leaders line): the typed side of the hook duals. Each function answers one string faction
//! function of muaat, saar, arborec, nomad, obsidian or empyrean with the same function of
//! `ti4_engine_core_factions::ce_movement` over a typed view of the position, in the string function's own types.
//! `None` means "the typed core cannot express this position" (an unknown id, more than 16 owners): the caller keeps the
//! string answer. The call sites are `cb_dual!` (`dual_hook!` at the string function, nothing without the typed core).

use ti4_engine_core::Seat;
use ti4_engine_core::generated::{Planet, System, Unit, UnitBaseType};
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::state::{CoreState, Slice, SliceSet};
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlanetId, PlayerId, SystemId};
use ti4_model::state::GameState;

use crate::core_sync::codec::Sources;
use crate::core_sync::{CoreView, Ctx};
use crate::generated::IdStr;

/// The modules whose tiers Q and O this package has ported (arborec, nomad, obsidian: the leaders and asks are tier W).
pub const PORTED: &[ti4_engine_core_factions::Id] = &[
    ti4_engine_core_factions::Id::Arborec,
    ti4_engine_core_factions::Id::Empyrean,
    ti4_engine_core_factions::Id::Muaat,
    ti4_engine_core_factions::Id::Nomad,
    ti4_engine_core_factions::Id::Obsidian,
    ti4_engine_core_factions::Id::Saar,
];

/// The slices the CE hooks read.
const READS: SliceSet = SliceSet::of(&[
    Slice::Meta,
    Slice::Seats,
    Slice::Players,
    Slice::Board,
    Slice::Cards,
    Slice::Marks,
]);

/// One position as the CE hooks read it.
pub struct Pos {
    view: CoreView,
    cx: Ctx,
    k: &'static Kernel,
}

impl Pos {
    fn of(gs: &GameState, sources: SourceSet) -> Option<Self> {
        let mut cx = Ctx::of(gs).ok()?;
        let view = CoreView::of(gs, &mut cx, Sources::default(), READS).ok()?;
        Some(Self { view, cx, k: crate::core_adapter::kernel_for(sources) })
    }

    fn s(&self) -> &CoreState {
        &self.view
    }

    fn seat(&self, p: &PlayerId) -> Option<Seat> {
        self.cx.owners.seat(p.as_str())
    }
}

/// Run `f` over the typed view of `gs`.
fn with<T>(gs: &GameState, sources: SourceSet, f: impl FnOnce(&Pos) -> Option<T>) -> Option<T> {
    f(&Pos::of(gs, sources)?)
}

fn full() -> SourceSet {
    ti4_model::content_types::FULL
}

fn system(id: &SystemId) -> Option<System> {
    <System as IdStr>::from_id(id.as_str())
}

fn planet(id: &PlanetId) -> Option<Planet> {
    <Planet as IdStr>::from_id(id.as_str())
}

fn base_type(base: &str) -> Option<UnitBaseType> {
    Some(match base {
        "carrier" => UnitBaseType::Carrier,
        "cruiser" => UnitBaseType::Cruiser,
        "destroyer" => UnitBaseType::Destroyer,
        "dreadnought" => UnitBaseType::Dreadnought,
        "fighter" => UnitBaseType::Fighter,
        "flagship" => UnitBaseType::Flagship,
        "infantry" => UnitBaseType::Infantry,
        "mech" => UnitBaseType::Mech,
        "pds" => UnitBaseType::Pds,
        "spacedock" => UnitBaseType::Spacedock,
        "warsun" => UnitBaseType::Warsun,
        _ => return None,
    })
}

use ti4_engine_core_factions::ce_movement::{arborec, empyrean, muaat, nomad, obsidian, saar};

// -- the neutral answers that need no typed view --------------------------------------------------------------
//
// A hook answers the neutral value for every seat that is not its faction's (or when the card or mark it reads is not in
// the game). In `Core` and `Dual` each call would otherwise build a typed view to find that out, on the hottest paths of the
// engine (production, objectives, hit assignment). These guards read the string state for exactly the condition the typed
// function starts with: when one holds, the typed answer is the neutral one by construction.

fn is_faction(gs: &GameState, who: &PlayerId, faction: &str) -> bool {
    gs.player(who).is_some_and(|p| p.faction.as_str() == faction)
}

fn any_faction(gs: &GameState, faction: &str) -> bool {
    gs.players.iter().any(|p| p.faction.as_str() == faction)
}

/// Whether some faction mark starts with `prefix`.
fn has_mark_prefix(gs: &GameState, prefix: &str) -> bool {
    gs.faction_marks.range(prefix.to_owned()..).next().is_some_and(|(k, _)| k.starts_with(prefix))
}

// -- muaat, saar, arborec ---------------------------------------------------------------------------------

pub fn muaat_extra_production(gs: &GameState, sources: SourceSet, who: &PlayerId, sys: &SystemId) -> Option<i64> {
    if !is_faction(gs, who, "muaat") {
        return Some(0);
    }
    sampled_audit(gs, sources);
    with(gs, sources, |p| muaat_extra_production_at(p, who, sys))
}

fn muaat_extra_production_at(p: &Pos, who: &PlayerId, sys: &SystemId) -> Option<i64> {
    Some(muaat::query::extra_production(p.s(), p.k, p.seat(who)?, system(sys)?))
}

pub fn saar_nomadic(gs: &GameState, who: &PlayerId) -> Option<bool> {
    if !is_faction(gs, who, "saar") {
        return Some(false);
    }
    with(gs, full(), |p| saar_nomadic_at(p, who))
}

fn saar_nomadic_at(p: &Pos, who: &PlayerId) -> Option<bool> {
    Some(saar::query::nomadic(p.s(), p.seat(who)?))
}

pub fn saar_commander_docks(
    gs: &GameState,
    sources: SourceSet,
    who: &PlayerId,
    producing: &SystemId,
    base: &str,
) -> Option<Vec<(SystemId, Option<PlanetId>)>> {
    with(gs, sources, |p| saar_commander_docks_at(p, who, producing, base))
}

fn saar_commander_docks_at(p: &Pos, who: &PlayerId, producing: &SystemId, base: &str) -> Option<Vec<(SystemId, Option<PlanetId>)>> {
    let found = saar::query::commander_docks(p.s(), p.k, p.seat(who)?, system(producing)?, base_type(base)?);
    Some(found.as_slice().iter().map(|(s, pl)| (SystemId::new(s.as_str()), pl.map(|pl| PlanetId::new(pl.as_str())))).collect())
}

pub fn arborec_cannot_produce(gs: &GameState, who: &PlayerId, unit_base: &str, producer_base: &str) -> Option<bool> {
    if !is_faction(gs, who, "arborec") {
        return Some(false);
    }
    sampled_audit(gs, full());
    with(gs, full(), |p| arborec_cannot_produce_at(p, who, unit_base, producer_base))
}

fn arborec_cannot_produce_at(p: &Pos, who: &PlayerId, unit_base: &str, producer_base: &str) -> Option<bool> {
    Some(arborec::query::cannot_produce(p.s(), p.seat(who)?, base_type(unit_base)?, base_type(producer_base)?))
}

// -- nomad ------------------------------------------------------------------------------------------------

pub fn nomad_transaction_reach(gs: &GameState, a: &PlayerId, b: &PlayerId) -> Option<bool> {
    if !has_mark_prefix(gs, "nomad:tcs:negotiating:") {
        return Some(false);
    }
    with(gs, full(), |p| nomad_transaction_reach_at(p, a, b))
}

fn nomad_transaction_reach_at(p: &Pos, a: &PlayerId, b: &PlayerId) -> Option<bool> {
    Some(nomad::query::transaction_reach(p.s(), p.k, p.seat(a)?, p.seat(b)?))
}

pub fn nomad_watches_agent_exhaustion(gs: &GameState) -> Option<bool> {
    if !any_faction(gs, "nomad") {
        return Some(false);
    }
    with(gs, full(), |p| Some(nomad::query::watches_agent_exhaustion(p.s())))
}

pub fn nomad_watches_roll_step(gs: &GameState) -> Option<bool> {
    if !gs.players.iter().any(|p| p.leaders.contains_key(ti4_model::id::text(&"nomadagentthundarian"))) {
        return Some(false);
    }
    with(gs, full(), |p| Some(nomad::query::watches_roll_step(p.s())))
}

/// `grants_sustain` over the string `CombatUnit`: the space context only (the core site has no other).
pub fn nomad_grants_sustain(gs: &GameState, sources: SourceSet, unit: &crate::factions::CombatUnit<'_>) -> Option<bool> {
    if !matches!(unit.context, "space" | "ground") || (unit.unit_type != "nomad_mech" && !has_mark_prefix(gs, "nomad:cavalry:")) {
        return Some(false);
    }
    with(gs, sources, |p| nomad_grants_sustain_at(p, unit))
}

fn nomad_grants_sustain_at(p: &Pos, unit: &crate::factions::CombatUnit<'_>) -> Option<bool> {
    let ctx = match unit.context {
        "space" => ti4_engine_core::hooks::CombatCtx::Space,
        "ground" => ti4_engine_core::hooks::CombatCtx::Ground,
        _ => return Some(false),
    };
    let site = ti4_engine_core::hooks::CombatUnit {
        owner: p.seat(unit.player)?,
        system: match unit.system { Some(x) => Some(system(x)?), None => None },
        planet: match unit.planet { Some(x) => Some(planet(x)?), None => None },
        unit: <Unit as IdStr>::from_id(unit.unit_type)?,
        ctx,
    };
    Some(nomad::query::grants_sustain(p.s(), p.k, site))
}

/// `(borrowing ship type, combat value, barrage (value, dice))` in the string's order and types.
pub type Borrowed = Option<(String, Option<i64>, Option<(i64, i64)>)>;

pub fn nomad_borrowed_stats(gs: &GameState, sources: SourceSet, who: &PlayerId, sys: &SystemId) -> Option<Borrowed> {
    if !gs.faction_marks.contains_key(&format!("nomad:cavalry:{who}")) {
        return Some(None);
    }
    with(gs, sources, |p| nomad_borrowed_stats_at(p, who, sys))
}

fn nomad_borrowed_stats_at(p: &Pos, who: &PlayerId, sys: &SystemId) -> Option<Borrowed> {
    let found = nomad::query::borrowed_stats(p.s(), p.k, p.seat(who)?, system(sys)?);
    Some(found.map(|b| (b.unit.as_str().to_owned(), b.hits_on.map(i64::from), b.barrage.map(|(hits_on, dice)| (i64::from(hits_on), i64::from(dice))))))
}

// -- obsidian ---------------------------------------------------------------------------------------------

fn card(card: Option<ti4_content::StrategyCardKind>) -> Option<ti4_engine_core::generated::StrategyCardKind> {
    use ti4_engine_core::generated::StrategyCardKind as K;
    card.map(|c| match c {
        ti4_content::StrategyCardKind::Leadership => K::Leadership,
        ti4_content::StrategyCardKind::Diplomacy => K::Diplomacy,
        ti4_content::StrategyCardKind::Politics => K::Politics,
        ti4_content::StrategyCardKind::Construction => K::Construction,
        ti4_content::StrategyCardKind::Trade => K::Trade,
        ti4_content::StrategyCardKind::Warfare => K::Warfare,
        ti4_content::StrategyCardKind::Technology => K::Technology,
        ti4_content::StrategyCardKind::Imperial => K::Imperial,
    })
}

/// The `(id, label)` of the secondary waivers the typed core offers.
pub fn obsidian_secondary_waivers(
    gs: &GameState,
    follower: &PlayerId,
    primary: &PlayerId,
    kind: Option<ti4_content::StrategyCardKind>,
) -> Option<Vec<crate::factions::hooks_strategy::SecondaryWaiver>> {
    if !is_faction(gs, follower, "obsidian") {
        return Some(Vec::new());
    }
    with(gs, full(), |p| obsidian_secondary_waivers_at(p, follower, primary, kind))
}

fn obsidian_secondary_waivers_at(
    p: &Pos,
    follower: &PlayerId,
    primary: &PlayerId,
    kind: Option<ti4_content::StrategyCardKind>,
) -> Option<Vec<crate::factions::hooks_strategy::SecondaryWaiver>> {
    let found = obsidian::query::secondary_waivers(p.s(), p.k, p.seat(follower)?, p.seat(primary)?, card(kind));
    Some(
        found
            .as_slice()
            .iter()
            .map(|w| {
                debug_assert_eq!(w.code, obsidian::query::ENERVATE);
                crate::factions::hooks_strategy::SecondaryWaiver {
                    id: "enervate".to_owned(),
                    label: format!("resolve {primary}'s secondary without a command token (Enervate)"),
                }
            })
            .collect(),
    )
}

pub fn obsidian_substitutes_primary(gs: &GameState, who: &PlayerId, kind: Option<ti4_content::StrategyCardKind>) -> Option<bool> {
    if !gs.faction_marks.contains_key("firmament:enervate") {
        return Some(false);
    }
    with(gs, full(), |p| obsidian_substitutes_primary_at(p, who, kind))
}

fn obsidian_substitutes_primary_at(p: &Pos, who: &PlayerId, kind: Option<ti4_content::StrategyCardKind>) -> Option<bool> {
    Some(obsidian::query::substitutes_primary(p.s(), p.k, p.seat(who)?, card(kind)))
}

pub fn obsidian_listens_for_tactical_end(gs: &GameState) -> Option<bool> {
    if !gs.promissory_notes.contains_key(&crate::promissory::note_id("malevolency", "obsidian")) {
        return Some(false);
    }
    with(gs, full(), |p| Some(obsidian::query::listens_for_tactical_end(p.s())))
}

// -- the order audit --------------------------------------------------------------------------------------

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use ti4_engine_core::kernel::HookTable;

/// Every module of every package registered in the string `MODULES` order (what the kernel will hold).
fn full_table() -> &'static HookTable {
    static TABLE: OnceLock<HookTable> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = HookTable::new();
        ti4_engine_core_factions::register_all(&mut t);
        t
    })
}

/// One position in this many call sites of the sampled wrappers is audited.
const AUDIT_EVERY: u64 = 1024;

/// The order audit of the commutative seams CE filled (`order::<slot>` in the report), sampled. Only runs in `Dual`.
pub fn sampled_audit(gs: &GameState, sources: SourceSet) {
    static CALLS: AtomicU64 = AtomicU64::new(0);
    if !matches!(crate::dual::backend(crate::dual::Area::Factions), crate::dual::Backend::Dual)
        || CALLS.fetch_add(1, Ordering::Relaxed) % AUDIT_EVERY != 0
    {
        return;
    }
    audit(gs, sources);
}

/// [`sampled_audit`] without the sampling.
pub fn audit(gs: &GameState, sources: SourceSet) {
    use crate::dual::{HookGroup, audit_hook_order};
    let Some(pos) = Pos::of(gs, sources) else { return };
    let s = pos.s();
    let sources = pos.k.sources;
    let table = full_table();
    let kernel = |t: &HookTable| Kernel { hooks: t.clone(), sources };
    let seats: Vec<Seat> = s.players().iter().map(|p| p.seat).collect();
    let systems: Vec<System> = s.board().systems().filter(|(_, c)| !c.units.is_empty()).map(|(sys, _)| sys).collect();
    let none = ti4_engine_core::ids::system_at(0);

    audit_hook_order(HookGroup::Seam, "extra_production", table, |t| {
        use ti4_engine_core::kernel::seam::{ProdSite, Q};
        let k = kernel(t);
        let q = Q::new(s, &k);
        let mut out = Vec::new();
        for w in &seats {
            for sys in &systems {
                out.push(t.seam.extra_production.eval(q, &ProdSite { who: *w, system: *sys, planet: None }, 0, ti4_engine_core::kernel::seam::Clamp::None).value);
            }
        }
        out
    });
    audit_hook_order(HookGroup::Seam, "production_destinations", table, |t| {
        use ti4_engine_core::kernel::seam::{DestSite, Q};
        let k = kernel(t);
        let q = Q::new(s, &k);
        let mut out = Vec::new();
        for w in &seats {
            for sys in &systems {
                for base in [UnitBaseType::Infantry, UnitBaseType::Fighter, UnitBaseType::Mech] {
                    out.push(t.seam.production_destinations.collect(q, &DestSite { who: *w, system: *sys, base }).as_slice().to_vec());
                }
            }
        }
        out
    });
    audit_hook_order(HookGroup::Seam, "cannot_produce", table, |t| {
        use ti4_engine_core::kernel::seam::{ProduceSite, Q};
        let k = kernel(t);
        let q = Q::new(s, &k);
        let mut out = Vec::new();
        for w in &seats {
            for unit in [UnitBaseType::Infantry, UnitBaseType::Fighter, UnitBaseType::Mech] {
                for producer in [UnitBaseType::Spacedock, UnitBaseType::Pds] {
                    out.push(t.seam.cannot_produce.decide(q, &ProduceSite { who: *w, unit, producer }, false));
                }
            }
        }
        out
    });
    audit_hook_order(HookGroup::Seam, "grants_sustain", table, |t| {
        use ti4_engine_core::hooks::{CombatCtx, CombatUnit};
        use ti4_engine_core::kernel::seam::Q;
        let k = kernel(t);
        let q = Q::new(s, &k);
        let mut out = Vec::new();
        for w in &seats {
            for sys in systems.iter().chain(std::iter::once(&none)) {
                for unit in [Unit::QuantumManipulator, Unit::CruiserI, Unit::DestroyerI, Unit::CarrierI] {
                    let site = CombatUnit { owner: *w, system: Some(*sys), planet: None, unit, ctx: CombatCtx::Space };
                    out.push(t.seam.grants_sustain.decide(q, &site, false));
                }
            }
        }
        out
    });
}

// -- observers (state-changing, no ask) ---------------------------------------------------------------------

/// The Empyrean's promissory notes as text: `(note id, holder)`.
fn empyrean_notes(gs: &GameState) -> Vec<(String, Option<PlayerId>)> {
    gs.promissory_notes
        .iter()
        .filter(|(note, _)| crate::promissory::owner_of(note).as_deref() == Some(crate::factions::empyrean::FACTION))
        .map(|(note, holder)| (note.to_string(), Some(holder.clone())))
        .collect()
}

/// `empyrean::deal_notes` in `Dual`: `string` runs on the real state; the typed port runs on the typed view taken before
/// it; both then answer the Empyrean notes and their holders.
pub fn dual_empyrean_deal_notes(gs: &mut GameState, sources: SourceSet, who: &PlayerId, string: impl FnOnce(&mut GameState)) {
    use ti4_engine_core::kernel::{Cx, FirstOption};
    let Some(before) = Pos::of(gs, sources) else { return string(gs) };
    let Some(seat) = before.seat(who) else { return string(gs) };
    crate::dual::run_dual_hook(
        crate::dual::HookGroup::Direct,
        "empyrean::deal_notes",
        || {
            string(gs);
            empyrean_notes(gs)
        },
        || {
            let mut state = before.s().clone();
            let mut ask = FirstOption;
            empyrean::observe::deal_notes(&mut Cx::new(&mut state, before.k, &mut ask), seat);
            let mut found: Vec<(String, Option<PlayerId>)> = state
                .cards()
                .promissory
                .iter()
                .filter(|(key, _)| key.faction == ti4_engine_core::generated::Faction::TheEmpyrean)
                .filter_map(|(key, holder)| {
                    let owner = before.cx.owners.id(*holder)?;
                    Some((crate::promissory::note_id(key.note.as_str(), "empyrean").to_string(), Some(PlayerId::new(owner))))
                })
                .collect();
            found.sort();
            found
        },
    );
}

// -- the cross-product -------------------------------------------------------------------------------------

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;

/// What was compared.
#[derive(Debug, Default, Clone)]
pub struct CeCheck {
    pub positions: u64,
    pub unmapped: u64,
    pub compared: u64,
    pub mismatches: Vec<String>,
}

impl CeCheck {
    pub fn add(&mut self, o: &Self) {
        self.positions += o.positions;
        self.unmapped += o.unmapped;
        self.compared += o.compared;
        for m in &o.mismatches {
            self.miss(m.clone());
        }
    }

    fn miss(&mut self, m: String) {
        if self.mismatches.len() < 60 {
            self.mismatches.push(m);
        }
    }

    fn eq<T: PartialEq + std::fmt::Debug>(&mut self, label: &str, what: &str, string: &T, core: &Option<T>) {
        match core {
            None => self.unmapped += 1,
            Some(core) => {
                self.compared += 1;
                if string != core {
                    self.miss(format!("{label}: {what}: string {string:?} vs core {core:?}"));
                }
            }
        }
    }
}

fn module(alias: &str) -> &'static crate::factions::FactionModule {
    crate::factions::MODULES.iter().copied().find(|m| m.alias == alias).expect("a faction module")
}

/// A position and the same position turned into the scenarios the golden games rarely reach: a Muaat with the reactor and
/// a unit in a supernova, a Saar with its commander and docks (one blockaded), a Nomad with Temporal Command Suite, a
/// negotiated transaction, a mech and The Cavalry on a damaged ship, an Obsidian with Enervate and Malevolency, an Arborec
/// and an Empyrean.
pub fn compare(label: &str, gs: &GameState, galaxy: &Galaxy, sources: SourceSet, out: &mut CeCheck) {
    // The cards slice has no typed form for the generic promissory notes of an opening; no CE hook reads them.
    let mut gs = gs.clone();
    gs.promissory_notes.retain(|note, _| !note.ends_with(":generic"));
    gs.promissory_faceup.retain(|note| !note.ends_with(":generic"));
    let gs = &gs;
    compare_state(label, gs, galaxy, sources, out);
    for (name, variant) in variants(gs) {
        compare_state(&format!("{label} [{name}]"), &variant, galaxy, sources, out);
    }
}

fn variants(gs: &GameState) -> Vec<(&'static str, GameState)> {
    use ti4_model::id::{FactionId, LeaderId, TechnologyId};
    use ti4_model::state::LeaderStatus;
    let mut out = Vec::new();
    if gs.players.len() < 2 {
        return out;
    }
    let (a, b) = (gs.players[0].id.clone(), gs.players[1].id.clone());
    let hot: Vec<SystemId> = gs.board.iter().filter(|(_, s)| !s.units.is_empty() || !s.planet_units.is_empty()).map(|(k, _)| k.clone()).take(3).collect();
    let Some(first) = hot.first().cloned() else { return out };
    let planet_of = |gs: &GameState, s: &SystemId| gs.board.get(s).and_then(|x| x.planet_units.keys().next().cloned());
    let unit = |t: &str, o: &PlayerId| ti4_model::units::Unit::new(ti4_model::id::UnitTypeId::new(t), o.clone());
    let leader = |v: &mut GameState, who: usize, id: &str, status: LeaderStatus| {
        v.players[who].leaders.insert(LeaderId::new(id), status);
    };

    let mut v = gs.clone();
    v.players[0].faction = FactionId::new("muaat");
    v.players[0].technologies.insert(TechnologyId::new("mr"));
    if let Some(nova) = System::ALL.iter().find(|s| s.data().anomalies.supernova) {
        v.system_mut(&SystemId::new(nova.as_str())).units.push(unit("carrier", &a));
    }
    out.push(("muaat", v));

    let mut v = gs.clone();
    v.players[0].faction = FactionId::new("saar");
    leader(&mut v, 0, "saarcommander", LeaderStatus::Unlocked);
    if let Some(pl) = planet_of(&v, &first) {
        v.system_mut(&first).planet_units.entry(pl).or_default().push(unit("spacedock", &a));
    }
    if let Some(other) = hot.get(1) {
        v.system_mut(other).units.push(unit("destroyer", &b));
        if let Some(pl) = planet_of(&v, other) {
            v.system_mut(other).planet_units.entry(pl).or_default().push(unit("spacedock", &a));
        }
    }
    out.push(("saar", v));

    let mut v = gs.clone();
    v.players[0].faction = FactionId::new("nomad");
    v.players[0].technologies.insert(TechnologyId::new("tcs"));
    v.faction_marks.insert(format!("nomad:tcs:negotiating:{a}:{b}"), "true".to_owned());
    v.system_mut(&first).units.extend([unit("nomad_mech", &a), unit("destroyer", &b), unit("destroyer", &b)]);
    v.system_mut(&first).units.last_mut().map(|u| u.sustained_damage = true);
    let note = crate::promissory::note_id("cavalry", "nomad");
    v.promissory_notes.insert(note.clone(), b.clone());
    v.promissory_faceup.insert(note);
    v.faction_marks.insert(format!("nomad:cavalry:{b}"), format!("{first}|destroyer"));
    leader(&mut v, 0, "nomadagentthundarian", LeaderStatus::Exhausted);
    leader(&mut v, 1, "yssarilagent", LeaderStatus::Readied);
    out.push(("nomad", v));

    let mut v = gs.clone();
    v.players[0].faction = FactionId::new("obsidian");
    crate::factions::firmament::place_card(&mut v, &a, "enervate", &b);
    v.faction_marks.insert("firmament:enervate".to_owned(), a.to_string());
    let note = crate::promissory::note_id("malevolency", "obsidian");
    v.promissory_notes.insert(note, b.clone());
    out.push(("obsidian", v));

    let mut v = gs.clone();
    v.players[0].faction = FactionId::new("arborec");
    out.push(("arborec", v));

    let mut v = gs.clone();
    v.players[0].faction = FactionId::new("empyrean");
    out.push(("empyrean", v));
    out
}

const ALL_CARDS: [Option<ti4_content::StrategyCardKind>; 3] =
    [None, Some(ti4_content::StrategyCardKind::Leadership), Some(ti4_content::StrategyCardKind::Politics)];

/// Compare every ported hook on one position: the string module hooks next to the typed functions.
fn compare_state(label: &str, gs: &GameState, _galaxy: &Galaxy, sources: SourceSet, out: &mut CeCheck) {
    out.positions += 1;
    let Some(pos) = Pos::of(gs, sources) else {
        out.unmapped += 1;
        return;
    };
    let pos = &pos;
    let content = ContentStore::embedded();
    let ids: Vec<PlayerId> = gs.players.iter().map(|p| p.id.clone()).collect();
    let systems: Vec<SystemId> = gs.board.keys().cloned().collect();
    let bases = ["infantry", "fighter", "mech", "pds", "spacedock", "carrier"];
    let (muaat, saar, arborec, nomad, obsidian) =
        (&module("muaat").hooks, &module("saar").hooks, &module("arborec").hooks, &module("nomad").hooks, &module("obsidian").hooks);

    for p in &ids {
        out.eq(label, &format!("saar nomadic {p}"), &(saar.strategy.scores_without_home.expect("slot"))(gs, p), &saar_nomadic_at(pos, p));
        for sys in &systems {
            out.eq(
                label,
                &format!("muaat extra_production {p} {sys}"),
                &(muaat.economy.extra_production.expect("slot"))(gs, content, sources, p, sys),
                &muaat_extra_production_at(pos, p, sys),
            );
            for base in bases {
                let string = (saar.economy.production_destinations.expect("slot"))(gs, content, sources, p, sys, base);
                let sorted = |mut l: Vec<(SystemId, Option<PlanetId>)>| {
                    l.sort();
                    l.dedup();
                    l
                };
                out.eq(label, &format!("saar commander_docks {p} {sys} {base}"), &sorted(string), &saar_commander_docks_at(pos, p, sys, base).map(sorted));
            }
            for kind in ["nomad_mech", "destroyer", "cruiser", "carrier", "nomad_flagship"] {
                for context in ["space", "ground", "space_cannon"] {
                    let unit = crate::factions::CombatUnit { player: p, system: Some(sys), planet: None, unit_type: kind, context };
                    out.eq(
                        label,
                        &format!("nomad grants_sustain {p} {sys} {kind} {context}"),
                        &(nomad.combat.grants_sustain.expect("slot"))(gs, content, sources, &unit),
                        &nomad_grants_sustain_at(pos, &unit),
                    );
                }
            }
            let string = (nomad.combat.borrowed_stats.expect("slot"))(gs, content, sources, p, sys);
            let facts = string.map(|b| (b.unit.type_id.to_string(), b.hits_on, b.barrage));
            out.eq(label, &format!("nomad borrowed_stats {p} {sys}"), &facts, &nomad_borrowed_stats_at(pos, p, sys));
        }
        for unit_base in bases {
            for producer in bases {
                out.eq(
                    label,
                    &format!("arborec cannot_produce {p} {unit_base} {producer}"),
                    &(arborec.economy.cannot_produce.expect("slot"))(gs, content, p, unit_base, producer),
                    &arborec_cannot_produce_at(pos, p, unit_base, producer),
                );
            }
        }
        for q in &ids {
            out.eq(
                label,
                &format!("nomad transaction_reach {p} {q}"),
                &(nomad.cards.transaction_reach.expect("slot"))(gs, content, p, q),
                &nomad_transaction_reach_at(pos, p, q),
            );
            for card in ALL_CARDS {
                let string = (obsidian.strategy.secondary_waivers.expect("slot"))(gs, content, p, q, card);
                out.eq(label, &format!("obsidian secondary_waivers {p} {q} {card:?}"), &string, &obsidian_secondary_waivers_at(pos, p, q, card));
            }
        }
        for card in ALL_CARDS {
            out.eq(
                label,
                &format!("obsidian substitutes_primary {p} {card:?}"),
                &(obsidian.substitutes_primary.expect("slot"))(gs, content, p, card),
                &obsidian_substitutes_primary_at(pos, p, card),
            );
        }
    }
    out.eq(label, "nomad watches_agent_exhaustion", &crate::factions::nomad::watches_agent_exhaustion(gs), &nomad_watches_agent_exhaustion(gs));
    out.eq(label, "nomad watches_roll_step", &crate::factions::nomad_agents::watches_roll_step(gs), &nomad_watches_roll_step(gs));
    out.eq(label, "obsidian listens_for_tactical_end", &crate::factions::obsidian::listens_for_tactical_end(gs), &obsidian_listens_for_tactical_end(gs));
    // The order audit over the same position.
    audit(gs, sources);
    // Tier O: the Empyrean deals its notes to each seat.
    for p in &ids {
        let mut string = gs.clone();
        crate::factions::empyrean::deal_notes(&mut string, content, p);
        let mut dual = gs.clone();
        dual_empyrean_deal_notes(&mut dual, sources, p, |s| crate::factions::empyrean::deal_notes_text_only(s, content, p));
        out.eq(label, &format!("empyrean deal_notes {p}"), &empyrean_notes(&string), &Some(empyrean_notes(&dual)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scenarios_load_and_every_ported_hook_agrees_with_its_string_function() {
        let sources = ti4_model::content_types::POK;
        let mut gs = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan"), ("c", "yin")], sources);
        gs.promissory_notes.retain(|note, _| !note.ends_with(":generic"));
        gs.promissory_faceup.retain(|note| !note.ends_with(":generic"));
        let galaxy = Galaxy::build(ContentStore::embedded(), &["19", "117"], ti4_model::content_types::DEFAULT, 1).unwrap();
        let mut out = CeCheck::default();
        compare("fixture", &gs, &galaxy, sources, &mut out);
        assert_eq!(out.mismatches, Vec::<String>::new());
        assert_eq!(out.unmapped, 0, "every scenario loads");
        assert!(out.compared > 1000, "{} compared", out.compared);
        assert_eq!(out.positions, 7);
        // The scenarios are not vacuous: the Saar's commander opens a dock, the Cavalry lends the flagship's value.
        let (_, saar) = variants(&gs).into_iter().find(|(n, _)| *n == "saar").unwrap();
        let docks: usize = saar
            .board
            .keys()
            .map(|sys| saar_commander_docks(&saar, sources, &saar.players[0].id, sys, "infantry").unwrap().len())
            .sum();
        assert!(docks > 0, "the Saar scenario has a dock to place at");
        let (_, nomad) = variants(&gs).into_iter().find(|(n, _)| *n == "nomad").unwrap();
        let lent = nomad.board.keys().filter_map(|sys| nomad_borrowed_stats(&nomad, sources, &nomad.players[1].id, sys).unwrap()).count();
        assert_eq!(lent, 1, "the Cavalry lends in the system it was played in");
        let (_, obsidian) = variants(&gs).into_iter().find(|(n, _)| *n == "obsidian").unwrap();
        let waivers = obsidian_secondary_waivers(&obsidian, &obsidian.players[0].id, &obsidian.players[1].id, None).unwrap();
        assert_eq!(waivers.len(), 1);
    }

    #[test]
    fn the_ids_the_ports_name_are_the_variants_they_expect() {
        assert_eq!(<Unit as IdStr>::from_id("nomad_mech"), Some(Unit::QuantumManipulator));
        assert_eq!(<Unit as IdStr>::from_id("destroyer"), Some(Unit::DestroyerI));
    }
}
