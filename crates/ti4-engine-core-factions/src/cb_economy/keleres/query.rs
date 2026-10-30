//! keleres: tier Q: pure hooks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CB). Owned by CB only.
//! Only `MODULE_M` carries the hooks (see `mod.rs`).
//!
//! | string slot | string function | tier | target |
//! |---|---|---|---|
//! | `economy.extra_production_planet` | `extra_production_planet` | Q | `economy.extra_production_planet` (Custodia Vigilia) |
//! | `direct.keleres_spendable_commodities` | `spendable_commodities` | Q | `direct` (a `call`) |
//! | `direct.keleres_agent_could_grant` | `agent_could_grant` | Q | `direct` |
//! | `direct.keleres_with_agent_granted` | `with_agent_granted` | Q | `direct`: a clone of the state with the probe mark |
//! | `direct.keleres_asn_ready` | `asn_ready` | Q | `direct` |
//! | `direct.keleres_custodian_cannon` | `custodian_cannon` | Q | `direct` |
//! | `direct.keleres_units_commit_blocked` | `commit_blocked` (keleres_units) | Q, BLOCKED | needs `payment::affordable` (B3's plan search) |
//! | `direct.keleres_asn_mark`, `keleres_close_agent_window`, `keleres_reconcile` | | O | `observe.rs` |
//! | `direct.keleres_offer_agent`, `keleres_units_pay_commit_toll`, `keleres_take_executive_order` | | W | `offer.rs` |
//! | `timing_abilities`, `component_actions`, `perform_component`, leaders | | W | A5 / B7 |

use ti4_engine_core::generated::{Faction, Leader, Planet, Technology};
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::hooks::direct::AgentGrant;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal, Word};
use ti4_engine_core::state::{CoreState, LeaderStatus};

use crate::common::{has_tech_text, mark, player};

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.economy.extra_production_planet.push(extra_production_planet);
    t.direct.keleres_spendable_commodities.push(spendable_commodities);
    t.direct.keleres_agent_could_grant.push(agent_could_grant);
    t.direct.keleres_with_agent_granted.push(with_agent_granted);
    t.direct.keleres_asn_ready.push(asn_ready);
    t.direct.keleres_custodian_cannon.push(custodian_cannon);
}

/// Custodian's Favour: Mecatol Rex "gains ... PRODUCTION 3" for its controller while they hold Custodia Vigilia.
const CUSTODIAN_PRODUCTION: i64 = 3;

fn is_mecatol_planet(planet: Planet) -> bool {
    matches!(planet, Planet::MecatolRex | Planet::MecatolRexTe)
}

fn holds_custodia(s: &CoreState, who: Seat) -> bool {
    ti4_engine_core::query::board::controls(s, who, Planet::CustodiaVigilia)
}

pub fn extra_production_planet(s: &CoreState, _: &Kernel, who: Seat, _: ti4_engine_core::generated::System, planet: Planet) -> i64 {
    if ti4_engine_core::query::board::controls(s, who, planet) && is_mecatol_planet(planet) && holds_custodia(s, who) {
        CUSTODIAN_PRODUCTION
    } else {
        0
    }
}

/// Commodities `who` may spend as if they were trade goods right now: all of them inside a payment window the
/// agent was used for, else none.
pub fn spendable_commodities(s: &CoreState, who: Seat) -> i64 {
    match mark(s, MarkKey::KeleresAgentGoods(who)) {
        Some(MarkVal::Atom(Atom::Word(Word::Declined))) | None => 0,
        Some(_) => player(s, who).map_or(0, |p| i64::from(p.econ.commodities.max(0))),
    }
}

fn is_keleres(s: &CoreState, who: Seat) -> bool {
    let keleres = player(s, who).is_some_and(|p| {
        matches!(p.faction, Faction::TheCouncilKeleresArgent | Faction::TheCouncilKeleresMentak | Faction::TheCouncilKeleresXxcha)
    });
    if keleres {
        ti4_engine_core::unsupported::hit(ti4_engine_core::unsupported::Disabled::Keleres);
    }
    keleres
}

/// Whether a readied Keleres agent could be offered to `payer` now: no window open for them yet, they hold
/// commodities, and some Keleres seat has the agent readied.
pub fn agent_could_grant(s: &CoreState, payer: Seat) -> bool {
    if mark(s, MarkKey::KeleresAgentGoods(payer)).is_some() || player(s, payer).is_none_or(|p| p.econ.commodities <= 0) {
        return false;
    }
    s.players().iter().any(|p| {
        is_keleres(s, p.seat) && p.leaders.status.get(&Leader::XanderAlexinVictoriIII) == Some(&LeaderStatus::Readied)
    })
}

/// `with_agent_granted`: the position in which `payer` has the agent's permission, or `None`.
pub fn with_agent_granted(s: &CoreState, grant: AgentGrant) -> Option<CoreState> {
    if !agent_could_grant(s, grant.payer) {
        return None;
    }
    let mut granted = s.clone();
    granted.marks_mut().tail.edit().insert(MarkKey::KeleresAgentGoods(grant.payer), MarkVal::Atom(Atom::Word(Word::Probe)));
    Some(granted)
}

/// Agency Supply Network is ready for `who`: owned (or carried) and not yet used since the last action ended.
pub fn asn_ready(s: &CoreState, who: Seat) -> bool {
    has_tech_text(s, who, Technology::AgencySupplyNetwork) && mark(s, MarkKey::KeleresAsnUsed(who)).is_none()
}

/// Custodian's Favour: Mecatol Rex gains SPACE CANNON 5 (one die) for the player who controls it with Custodia Vigilia.
pub fn custodian_cannon(s: &CoreState, planet: Planet, owner: Seat) -> Option<(Seat, u32, u8)> {
    (is_mecatol_planet(planet) && holds_custodia(s, owner)).then_some((owner, 5, 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cb_economy::testkit::table;

    const KM: Faction = Faction::TheCouncilKeleresMentak;
    const SOL: Faction = Faction::TheFederationOfSol;

    #[test]
    fn custodias_favour_needs_control_of_mecatol_and_the_card() {
        let (mut s, k) = table(&[KM, SOL]);
        let sys = ti4_engine_core::ids::system_at(18);
        assert_eq!(extra_production_planet(&s, &k, Seat(0), sys, Planet::MecatolRex), 0);
        s.board_mut().planet_mut(Planet::MecatolRex).control = Some(Seat(0));
        assert_eq!(extra_production_planet(&s, &k, Seat(0), sys, Planet::MecatolRex), 0, "no Custodia Vigilia");
        s.board_mut().planet_mut(Planet::CustodiaVigilia).control = Some(Seat(0));
        assert_eq!(extra_production_planet(&s, &k, Seat(0), sys, Planet::MecatolRex), 3);
        assert_eq!(extra_production_planet(&s, &k, Seat(0), sys, Planet::Abaddon), 0, "only Mecatol Rex");
        assert_eq!(custodian_cannon(&s, Planet::MecatolRex, Seat(0)), Some((Seat(0), 5, 1)));
        assert_eq!(custodian_cannon(&s, Planet::MecatolRex, Seat(1)), None);
    }

    #[test]
    fn commodities_are_trade_goods_only_inside_a_window_the_agent_opened() {
        let (mut s, _) = table(&[KM, SOL]);
        s.players_mut().edit(1).econ.commodities = 3;
        assert_eq!(spendable_commodities(&s, Seat(1)), 0);
        assert!(!agent_could_grant(&s, Seat(1)), "no readied agent");
        s.players_mut().edit(0).leaders.status.insert(Leader::XanderAlexinVictoriIII, LeaderStatus::Readied);
        assert!(agent_could_grant(&s, Seat(1)));
        let granted = with_agent_granted(&s, AgentGrant { payer: Seat(1) }).expect("a holder could grant");
        assert_eq!(spendable_commodities(&granted, Seat(1)), 3);
        assert_eq!(spendable_commodities(&s, Seat(1)), 0, "the probe is a copy");
        s.marks_mut().tail.edit().insert(MarkKey::KeleresAgentGoods(Seat(1)), MarkVal::Atom(Atom::Word(Word::Declined)));
        assert_eq!(spendable_commodities(&s, Seat(1)), 0, "a declined window opens nothing");
        assert!(!agent_could_grant(&s, Seat(1)), "a window is already open");
        assert!(with_agent_granted(&s, AgentGrant { payer: Seat(1) }).is_none());
    }

    #[test]
    fn agency_supply_network_is_once_per_action() {
        let (mut s, _) = table(&[KM, SOL]);
        assert!(!asn_ready(&s, Seat(0)));
        s.players_mut().edit(0).tech.owned.insert(Technology::AgencySupplyNetwork);
        assert!(asn_ready(&s, Seat(0)));
        s.marks_mut().tail.edit().insert(MarkKey::KeleresAsnUsed(Seat(0)), MarkVal::Atom(Atom::Num(1)));
        assert!(!asn_ready(&s, Seat(0)));
    }
}

#[cfg(test)]
mod unsupported_tests {
    use super::*;
    use crate::common::testkit::table;
    use ti4_engine_core::unsupported::{Policy, with_policy};

    fn panics_with(name: &str, f: impl FnOnce()) -> bool {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        std::panic::set_hook(prev);
        r.err().is_some_and(|e| {
            e.downcast_ref::<String>().is_some_and(|t| t.contains(name))
                || e.downcast_ref::<&str>().is_some_and(|t| t.contains(name))
        })
    }

    /// XF-3 family: the Council Keleres is switched off.
    #[test]
    fn the_keleres_predicate_panics_for_a_keleres_seat() {
        let (s, _) = table(&[Faction::TheArborec, Faction::TheCouncilKeleresMentak]);
        assert!(with_policy(Policy::Off, || is_keleres(&s, Seat(1))));
        assert!(panics_with("Keleres", || with_policy(Policy::Panic, || { is_keleres(&s, Seat(1)); })));
        assert!(!with_policy(Policy::Panic, || is_keleres(&s, Seat(0))));
    }
}
