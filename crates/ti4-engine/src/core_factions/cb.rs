//! Package CB (economy line): the typed side of the hook duals. Each function answers one string faction
//! function of keleres, argent, winnu, deepwrought or cabal with the same function of
//! `ti4_engine_core_factions::cb_economy` over a typed view of the position, in the string function's own types.
//! `None` means "the typed core cannot express this position" (an unknown id, more than 16 owners): the caller keeps the
//! string answer.

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

/// The slices the CB hooks read.
const READS: SliceSet = SliceSet::of(&[
    Slice::Meta,
    Slice::Seats,
    Slice::Players,
    Slice::Board,
    Slice::Agenda,
    Slice::Strategy,
    Slice::Cards,
    Slice::Trade,
    Slice::Marks,
    Slice::Transient,
]);

/// One position as the CB hooks read it.
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

    fn player(&self, seat: Seat) -> Option<PlayerId> {
        self.cx.owners.id(seat).map(PlayerId::new)
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

// -- argent ---------------------------------------------------------------------------------------------

use ti4_engine_core_factions::cb_economy::{argent, cabal, deepwrought, keleres, winnu};

pub fn argent_vote_bonus(gs: &GameState, who: &PlayerId) -> Option<i64> {
    with(gs, full(), |p| Some(argent::query::vote_bonus(p.s(), p.seat(who)?)))
}

pub fn argent_votes_first(gs: &GameState, who: &PlayerId) -> Option<bool> {
    with(gs, full(), |p| Some(argent::query::votes_first(p.s(), p.seat(who)?)))
}

pub fn argent_space_cannon_barred(gs: &GameState, sources: SourceSet, shooter: &PlayerId, target: &PlayerId, sys: &SystemId) -> Option<bool> {
    with(gs, sources, |p| Some(argent::query::space_cannon_barred(p.s(), p.k, p.seat(shooter)?, p.seat(target)?, system(sys)?)))
}

pub fn argent_extra_production_planet(gs: &GameState, sources: SourceSet, who: &PlayerId, sys: &SystemId, pl: &PlanetId) -> Option<i64> {
    with(gs, sources, |p| Some(argent::query::extra_production_planet(p.s(), p.k, p.seat(who)?, system(sys)?, planet(pl)?)))
}

pub fn argent_production_destinations(
    gs: &GameState,
    sources: SourceSet,
    who: &PlayerId,
    sys: &SystemId,
    base: &str,
) -> Option<Vec<(SystemId, Option<PlanetId>)>> {
    with(gs, sources, |p| {
        let found = argent::query::agent_production_destinations(p.s(), p.k, p.seat(who)?, system(sys)?, base_type(base)?);
        Some(found.as_slice().iter().map(|(s, pl)| (SystemId::new(s.as_str()), pl.map(|pl| PlanetId::new(pl.as_str())))).collect())
    })
}

// -- winnu ----------------------------------------------------------------------------------------------

pub fn winnu_custodians_free(gs: &GameState, who: &PlayerId) -> Option<bool> {
    with(gs, full(), |p| Some(winnu::query::custodians_free(p.s(), p.k, p.seat(who)?)))
}

pub fn winnu_agent_ready(gs: &GameState, who: &PlayerId) -> Option<bool> {
    with(gs, full(), |p| Some(winnu::query::agent_ready(p.s(), p.seat(who)?)))
}

pub fn winnu_planet_spend_value(
    gs: &GameState,
    who: &PlayerId,
    pl: &PlanetId,
    kind: crate::production::Spend,
    value: i64,
) -> Option<i64> {
    let kind = match kind {
        crate::production::Spend::Resources => ti4_engine_core::hooks::Spend::Resources,
        crate::production::Spend::Influence => ti4_engine_core::hooks::Spend::Influence,
    };
    with(gs, full(), |p| Some(winnu::query::planet_spend_value(p.s(), p.k, p.seat(who)?, planet(pl)?, kind, value)))
}

/// `unit_type` is a unit id (`winnu_flagship`); `ground` is the string's `context` ("space" or "ground").
pub fn winnu_unit_dice(
    gs: &GameState,
    sources: SourceSet,
    who: &PlayerId,
    sys: Option<&SystemId>,
    pl: Option<&PlanetId>,
    unit_type: &str,
    ground: bool,
    dice: i64,
) -> Option<i64> {
    with(gs, sources, |p| {
        let unit = <Unit as IdStr>::from_id(unit_type)?;
        let site = ti4_engine_core::hooks::CombatUnit {
            owner: p.seat(who)?,
            system: match sys { Some(x) => Some(system(x)?), None => None },
            planet: match pl { Some(x) => Some(planet(x)?), None => None },
            unit,
            ctx: if ground { ti4_engine_core::hooks::CombatCtx::Ground } else { ti4_engine_core::hooks::CombatCtx::Space },
        };
        Some(winnu::query::unit_dice(p.s(), p.k, site, dice))
    })
}

/// The ids of the secondary waivers (`acq`) the typed core offers.
pub fn winnu_secondary_waivers(gs: &GameState, follower: &PlayerId, primary: &PlayerId) -> Option<Vec<crate::factions::hooks_strategy::SecondaryWaiver>> {
    with(gs, full(), |p| {
        let found = winnu::query::secondary_waivers(p.s(), p.k, p.seat(follower)?, p.seat(primary)?, None);
        Some(
            found
                .as_slice()
                .iter()
                .map(|w| {
                    debug_assert_eq!(w.code, winnu::query::ACQUIESCENCE);
                    crate::factions::hooks_strategy::SecondaryWaiver {
                        id: "acq".to_owned(),
                        label: "resolve the secondary using Acquiescence".to_owned(),
                    }
                })
                .collect(),
        )
    })
}

// -- keleres --------------------------------------------------------------------------------------------

pub fn keleres_extra_production_planet(gs: &GameState, sources: SourceSet, who: &PlayerId, sys: &SystemId, pl: &PlanetId) -> Option<i64> {
    with(gs, sources, |p| Some(keleres::query::extra_production_planet(p.s(), p.k, p.seat(who)?, system(sys)?, planet(pl)?)))
}

pub fn keleres_spendable_commodities(gs: &GameState, who: &PlayerId) -> Option<i64> {
    with(gs, full(), |p| Some(keleres::query::spendable_commodities(p.s(), p.seat(who)?)))
}

pub fn keleres_agent_could_grant(gs: &GameState, payer: &PlayerId) -> Option<bool> {
    with(gs, full(), |p| Some(keleres::query::agent_could_grant(p.s(), p.seat(payer)?)))
}

/// `with_agent_granted(..).is_some()`, and the commodities the probe state would let `payer` spend.
pub fn keleres_with_agent_granted(gs: &GameState, payer: &PlayerId) -> Option<Option<i64>> {
    with(gs, full(), |p| {
        let payer = p.seat(payer)?;
        let granted = keleres::query::with_agent_granted(p.s(), ti4_engine_core::hooks::direct::AgentGrant { payer });
        Some(granted.map(|s| keleres::query::spendable_commodities(&s, payer)))
    })
}

pub fn keleres_asn_ready(gs: &GameState, who: &PlayerId) -> Option<bool> {
    with(gs, full(), |p| Some(keleres::query::asn_ready(p.s(), p.seat(who)?)))
}

pub fn keleres_custodian_cannon(gs: &GameState, pl: &PlanetId, owner: &PlayerId) -> Option<Option<(PlayerId, u32, usize)>> {
    with(gs, full(), |p| {
        let found = keleres::query::custodian_cannon(p.s(), planet(pl)?, p.seat(owner)?);
        found.map(|(seat, hits_on, dice)| Some((p.player(seat)?, hits_on, usize::from(dice)))).map_or(Some(None), |x| x.map(Some))
    })
}

// -- deepwrought ----------------------------------------------------------------------------------------

fn planets_text(list: &[Planet]) -> Vec<PlanetId> {
    list.iter().map(|pl| PlanetId::new(pl.as_str())).collect()
}

pub fn deepwrought_oceans(gs: &GameState, who: &PlayerId) -> Option<Vec<PlanetId>> {
    with(gs, full(), |p| Some(planets_text(deepwrought::query::oceans(p.s(), p.seat(who)?).as_slice())))
}

pub fn deepwrought_spendable_oceans(gs: &GameState, who: &PlayerId) -> Option<Vec<PlanetId>> {
    with(gs, full(), |p| Some(planets_text(deepwrought::query::spendable_oceans(p.s(), p.seat(who)?).as_slice())))
}

pub fn deepwrought_research_team_open(gs: &GameState, invader: &PlayerId, sys: &SystemId, pl: &PlanetId) -> Option<bool> {
    with(gs, full(), |p| Some(deepwrought::query::research_team_open(p.s(), p.k, p.seat(invader)?, system(sys)?, planet(pl)?)))
}

pub fn deepwrought_research_team_defender_open(gs: &GameState, defender: &PlayerId, invader: &PlayerId, sys: &SystemId, pl: &PlanetId) -> Option<bool> {
    with(gs, full(), |p| {
        Some(deepwrought::query::research_team_defender_open(p.s(), p.k, p.seat(defender)?, p.seat(invader)?, system(sys)?, planet(pl)?))
    })
}

pub fn deepwrought_waived_prerequisites(gs: &GameState, who: &PlayerId) -> Option<usize> {
    with(gs, full(), |p| {
        Some(deepwrought::query::waived_prerequisites(p.s(), p.k, p.seat(who)?, ti4_engine_core::generated::Technology::ALL[0]))
    })
}

// -- cabal ----------------------------------------------------------------------------------------------

pub fn cabal_amalgamation_offer(gs: &GameState, sources: SourceSet, who: &PlayerId, sys: &SystemId, unit_id: &str) -> Option<bool> {
    with(gs, sources, |p| {
        let unit = <Unit as IdStr>::from_id(unit_id)?;
        Some(cabal::query::amalgamation_offer(p.s(), p.k, p.seat(who)?, system(sys)?, unit))
    })
}

// -- the order audit ------------------------------------------------------------------------------------

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

/// The order audit of the commutative slots CB filled (`order::<slot>` in the report): the answer of each slot over every seat
/// and every site of the position, with the providers registered in reverse and in two shuffles. Sampled: call it from a
/// hot wrapper and it audits one position in [`AUDIT_EVERY`]. Only runs in `Dual`.
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
    let planets: Vec<Planet> = s
        .board()
        .planets()
        .filter(|(_, c)| c.control.is_some() || !c.units.is_empty())
        .map(|(p, _)| p)
        .collect();
    let systems: Vec<System> = s.board().systems().filter(|(_, c)| !c.units.is_empty()).map(|(sys, _)| sys).collect();
    let home = |p: Planet| ti4_engine_core::query::board::home_system(s, p).unwrap_or_else(|| ti4_engine_core::ids::system_at(0));

    audit_hook_order(HookGroup::Economy, "extra_production_planet", table, |t| {
        let k = kernel(t);
        seats.iter().flat_map(|w| planets.iter().map(move |p| (*w, *p))).map(|(w, p)| t.economy.extra_production_planet(s, &k, w, home(p), p)).collect::<Vec<_>>()
    });
    audit_hook_order(HookGroup::Economy, "production_destinations", table, |t| {
        let k = kernel(t);
        let mut out = Vec::new();
        for w in &seats {
            for sys in &systems {
                for base in [UnitBaseType::Infantry, UnitBaseType::Mech, UnitBaseType::Pds] {
                    out.push(t.economy.production_destinations(s, &k, *w, *sys, base).as_slice().to_vec());
                }
            }
        }
        out
    });
    audit_hook_order(HookGroup::Economy, "production_unit_exchange", table, |t| {
        let k = kernel(t);
        let sys = systems.first().copied().unwrap_or_else(|| ti4_engine_core::ids::system_at(0));
        seats
            .iter()
            .flat_map(|w| [Unit::CruiserI, Unit::CarrierI, Unit::InfantryI, Unit::DreadnoughtII].into_iter().map(move |u| (*w, u)))
            .map(|(w, u)| t.economy.production_unit_exchange(s, &k, w, sys, u))
            .collect::<Vec<_>>()
    });
    audit_hook_order(HookGroup::Combat, "space_cannon_barred", table, |t| {
        let k = kernel(t);
        let mut out = Vec::new();
        for a in &seats {
            for b in &seats {
                for sys in &systems {
                    out.push(t.combat.space_cannon_barred(s, &k, *a, *b, *sys));
                }
            }
        }
        out
    });
    audit_hook_order(HookGroup::Ground, "custodians_free", table, |t| {
        let k = kernel(t);
        seats.iter().map(|w| t.ground.custodians_free(s, &k, *w)).collect::<Vec<_>>()
    });
    audit_hook_order(HookGroup::Cards, "votes_first", table, |t| seats.iter().map(|w| t.cards.votes_first(s, *w)).collect::<Vec<_>>());
    audit_hook_order(HookGroup::General, "vote_bonus", table, |t| seats.iter().map(|w| t.general.vote_bonus(s, *w)).collect::<Vec<_>>());
    audit_hook_order(HookGroup::General, "waived_prerequisites", table, |t| {
        let k = kernel(t);
        seats.iter().map(|w| t.general.waived_prerequisites(s, &k, *w, ti4_engine_core::generated::Technology::ALL[0])).collect::<Vec<_>>()
    });
}

// -- performing hooks (state-changing, no ask) -----------------------------------------------------------

/// The captured models of `captor` as text, for comparing the state a performing hook leaves.
fn captured_text(gs: &GameState, captor: &PlayerId) -> Vec<(PlayerId, String)> {
    crate::supply::captured_by(gs, captor).iter().map(|(owner, unit)| (owner.clone(), unit.to_string())).collect()
}

/// `cabal::amalgamation_perform` in `Dual`: `string` runs on the real state; the typed port runs on the typed view taken
/// before it. Both answer the result and the captor's captured models afterwards. Without a typed view the string answer stands.
pub fn dual_amalgamation_perform(
    gs: &mut GameState,
    sources: SourceSet,
    captor: &PlayerId,
    sys: &SystemId,
    unit_id: &str,
    string: impl FnOnce(&mut GameState) -> bool,
) -> bool {
    use ti4_engine_core::kernel::{Cx, FirstOption};
    let before = Pos::of(gs, sources);
    let typed_ids = (|| Some((system(sys)?, <Unit as IdStr>::from_id(unit_id)?)))();
    let (Some(before), Some((sys_ix, unit))) = (before, typed_ids) else { return string(gs) };
    let Some(seat) = before.seat(captor) else { return string(gs) };
    let mut result = false;
    let out = crate::dual::run_dual_hook(
        crate::dual::HookGroup::Economy,
        "cabal::amalgamation_perform",
        || {
            result = string(gs);
            (result, captured_text(gs, captor))
        },
        || {
            let mut core = before.s().clone();
            let mut ask = FirstOption;
            let done = cabal::offer::amalgamation_perform(&mut Cx::new(&mut core, before.k, &mut ask), seat, sys_ix, unit);
            let held = core.players().by_seat(seat).map(|p| p.misc.captured_units.as_slice().to_vec()).unwrap_or_default();
            (done, held.iter().filter_map(|(o, u)| Some((before.player(*o)?, u.as_str().to_owned()))).collect())
        },
    );
    out.0
}

/// A bookkeeping observer in `Dual`: `string` runs on the real state, `core` on the typed view taken before it; both then
/// answer `facts` (the marks and flags the observer may change), compared by value.
fn dual_effect<F: PartialEq + std::fmt::Debug>(
    gs: &mut GameState,
    sources: SourceSet,
    slot: &'static str,
    string: impl FnOnce(&mut GameState),
    core: impl FnOnce(&mut ti4_engine_core::kernel::Cx<'_>, &Pos),
    facts_text: impl Fn(&GameState) -> F,
    facts_core: impl Fn(&CoreState, &Pos) -> F,
) {
    use ti4_engine_core::kernel::{Cx, FirstOption};
    let Some(before) = Pos::of(gs, sources) else { return string(gs) };
    crate::dual::run_dual_hook(
        crate::dual::HookGroup::Direct,
        slot,
        || {
            string(gs);
            facts_text(gs)
        },
        || {
            let mut state = before.s().clone();
            let mut ask = FirstOption;
            core(&mut Cx::new(&mut state, before.k, &mut ask), &before);
            facts_core(&state, &before)
        },
    );
}

/// Whether the marks text `key` is present.
fn has_mark(gs: &GameState, key: &str) -> bool {
    gs.faction_marks.contains_key(key)
}

pub fn dual_argent_clear(gs: &mut GameState, sources: SourceSet, who: &PlayerId, sys: &SystemId, string: impl FnOnce(&mut GameState)) {
    let Some(ix) = system(sys) else { return string(gs) };
    let key = format!("argent:argentagent:production:{who}:{sys}");
    dual_effect(
        gs,
        sources,
        "argent::clear_agent_production_destinations",
        string,
        |cx, p| {
            if let Some(seat) = p.seat(who) {
                argent::observe::clear_agent_production_destinations(cx, seat, ix);
            }
        },
        |gs| has_mark(gs, &key),
        |s, p| p.seat(who).is_some_and(|seat| s.marks().tail.contains_key(&ti4_engine_core::keys::MarkKey::ArgentProduction(seat, ix))),
    );
}

pub fn dual_keleres_close(gs: &mut GameState, sources: SourceSet, payer: &PlayerId, string: impl FnOnce(&mut GameState)) {
    let key = format!("keleres|agent_goods|{payer}");
    dual_effect(
        gs,
        sources,
        "keleres::close_agent_window",
        string,
        |cx, p| {
            if let Some(seat) = p.seat(payer) {
                keleres::observe::close_agent_window(cx, seat);
            }
        },
        |gs| has_mark(gs, &key),
        |s, p| p.seat(payer).is_some_and(|seat| s.marks().tail.contains_key(&ti4_engine_core::keys::MarkKey::KeleresAgentGoods(seat))),
    );
}

pub fn dual_keleres_asn_mark(gs: &mut GameState, sources: SourceSet, who: &PlayerId, string: impl FnOnce(&mut GameState)) {
    let key = format!("keleres|asn_used|{who}");
    dual_effect(
        gs,
        sources,
        "keleres::asn_mark",
        string,
        |cx, p| {
            if let Some(seat) = p.seat(who) {
                keleres::observe::asn_mark(cx, seat);
            }
        },
        |gs| gs.faction_marks.get(&key).cloned(),
        |s, p| {
            p.seat(who)
                .and_then(|seat| s.marks().tail.get(&ti4_engine_core::keys::MarkKey::KeleresAsnUsed(seat)))
                .map(|_| "1".to_owned())
        },
    );
}

/// The ocean cards as text: `(id, holder, exhausted)`.
pub fn dual_deepwrought_coexistence_began(gs: &mut GameState, sources: SourceSet, who: &PlayerId, string: impl FnOnce(&mut GameState)) {
    const OCEANS: [&str; 5] = ["ocean1", "ocean2", "ocean3", "ocean4", "ocean5"];
    dual_effect(
        gs,
        sources,
        "deepwrought::note_coexistence_began",
        string,
        |cx, p| {
            if let Some(seat) = p.seat(who) {
                deepwrought::observe::note_coexistence_began(cx, seat);
            }
        },
        |gs| {
            OCEANS
                .iter()
                .map(|id| (*id, gs.faction_marks.get(&format!("deepwrought:ocean:{id}")).cloned(), gs.exhausted_planets.contains(&PlanetId::new(*id))))
                .collect::<Vec<_>>()
        },
        |s, p| {
            deepwrought::observe::OCEANS
                .iter()
                .zip(OCEANS)
                .map(|(planet, id)| {
                    let holder = match s.marks().tail.get(&ti4_engine_core::keys::MarkKey::DeepwroughtOcean(*planet)) {
                        Some(ti4_engine_core::keys::MarkVal::Atom(ti4_engine_core::keys::Atom::Seat(h))) => p.player(*h).map(|h| h.to_string()),
                        _ => None,
                    };
                    (id, holder, ti4_engine_core::query::board::exhausted(s, *planet))
                })
                .collect::<Vec<_>>()
        },
    );
}
