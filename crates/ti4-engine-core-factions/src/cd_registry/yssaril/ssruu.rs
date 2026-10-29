//! yssaril: Ssruu's copies of other seats' agents that act in the combat and production windows (package B12;
//! docs/ENGINE_REACTIONS.md 10): "Ssruu: you may exhaust this card to use the ability of another player's agent as if you
//! were that player". Each copy is a pair rule: registered for `(borrower, lender)`, owned by the borrower, and
//! `AbilityDef::pair_source` is the lender.
//!
//! * Brother Milor (Yin), the ship and the ground branch: the same placement as the agent's, with Ssruu exhausted.
//! * Evelyn De Louis (Sol) and Viscount Unlenn (Letnev): the same choice and mark as the agents' (the mark names the borrower and
//!   the lender), with Ssruu exhausted; the question says "Ssruu copying".
//! * Trilossa Aun Mirik (Argent): the same grant of destinations, with Ssruu exhausted.
//!
//! A copy is eligible while the borrower's Ssruu is readied and the lender holds the agent, readied or exhausted
//! (`hooks_cards::borrowable_agents`). The string registers them from the static roster: a Yin/Argent lender that holds the
//! agent card and a borrower that holds Ssruu; every Sol/Letnev lender and every Yssaril borrower.

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::{Faction, Leader, UnitBaseType};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, Applies, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::state::{CoreState, EventKind, LeaderStatus};

use crate::cd_registry::yin::timing as yin;

/// Ssruu copying Brother Milor, ships (`leader:{lender faction}:{lender}:yssarilagent:yinagent:SHIP_DESTROYED:after`).
pub const YIN_SHIPS: Slot = Slot(14001);
/// ... and ground forces (`...:yinagent:GROUND_FORCE_DESTROYED:after`).
pub const YIN_GROUND: Slot = Slot(14002);
/// Ssruu copying Evelyn De Louis (`leader:ssruu-copy:{borrower}:solagent:{lender}:GROUND_COMBAT_ROUND_STARTED:after`).
pub const SOL: Slot = Slot(14003);
/// Ssruu copying Viscount Unlenn (`leader:ssruu-copy:{borrower}:letnevagent:{lender}:COMBAT_ROUND_STARTED:after`).
pub const LETNEV: Slot = Slot(14004);
/// Ssruu copying Trilossa Aun Mirik (`leader:{lender faction}:{lender}:yssarilagent:argentagent:GROUND_FORCES_BEING_PRODUCED:when`).
pub const ARGENT: Slot = Slot(14005);

fn holds_card(s: &CoreState, who: Seat, card: Leader) -> bool {
    s.players().by_seat(who).is_some_and(|p| p.leaders.status.contains_key(&card))
}

/// `hooks_cards::borrowable_agents` for one pair: the borrower's Ssruu is readied and the lender holds `agent`, readied or exhausted.
pub(crate) fn borrowable(s: &CoreState, borrower: Seat, lender: Option<Seat>, agent: Leader) -> bool {
    let Some(lender) = lender else { return false };
    let status = |who: Seat, card: Leader| s.players().by_seat(who).and_then(|p| p.leaders.status.get(&card).copied());
    lender != borrower && status(borrower, Leader::Ssruu) == Some(LeaderStatus::Readied) && matches!(status(lender, agent), Some(LeaderStatus::Readied | LeaderStatus::Exhausted))
}

fn yin_lends(s: &CoreState, borrower: Seat, lender: Seat) -> bool {
    holds_card(s, lender, Leader::BrotherMilor) && holds_card(s, borrower, Leader::Ssruu)
}

fn argent_lends(s: &CoreState, borrower: Seat, lender: Seat) -> bool {
    holds_card(s, lender, Leader::TrilossaAunMirik) && holds_card(s, borrower, Leader::Ssruu)
}

fn yssaril_borrows(s: &CoreState, borrower: Seat, _lender: Seat) -> bool {
    s.players().by_seat(borrower).is_some_and(|p| p.faction == Faction::TheYssarilTribes)
}

fn milor_ship_when(c: &CondCx<'_>) -> bool {
    borrowable(c.state, c.def.owner, c.def.pair_source(), Leader::BrotherMilor) && yin::ship_ok(c, Leader::Ssruu)
}

fn milor_ground_when(c: &CondCx<'_>) -> bool {
    borrowable(c.state, c.def.owner, c.def.pair_source(), Leader::BrotherMilor) && yin::ground_ok(c, Leader::Ssruu)
}

fn milor_copy(eff: &mut Eff<'_, '_>, base: UnitBaseType, on_planet: bool) -> Result<(), TimingError> {
    let (me, lender) = (eff.owner(), eff.def.pair_source());
    if !borrowable(eff.state(), me, lender, Leader::BrotherMilor) {
        return Ok(());
    }
    yin::place_by(eff, base, on_planet, Leader::Ssruu)
}

fn round_when(c: &CondCx<'_>, ground: bool) -> bool {
    let agent = if ground { Leader::EvelynDeLouis } else { Leader::ViscountUnlenn };
    borrowable(c.state, c.def.owner, c.def.pair_source(), agent)
        && crate::cc_ground::borrowed::timing::plan_of(c.state, c.kernel, c.event, ground).is_some()
        && crate::cc_ground::borrowed::timing::nomad_quiet(c)
}

pub fn register_abilities(t: &mut AbilityTable) {
    use EventKind as E;
    t.add(Spec::new(YIN_SHIPS, AbilityKind::Leader, E::ShipDestroyed, Relation::After, |e| milor_copy(e, UnitBaseType::Fighter, false)).applies(Applies::Faction(Faction::TheYinBrotherhood)).pair(yin_lends).optional().when(milor_ship_when));
    t.add(Spec::new(YIN_GROUND, AbilityKind::Leader, E::GroundForceDestroyed, Relation::After, |e| milor_copy(e, UnitBaseType::Infantry, true)).applies(Applies::Faction(Faction::TheYinBrotherhood)).pair(yin_lends).optional().when(milor_ground_when));
    t.add(Spec::new(SOL, AbilityKind::Leader, E::GroundCombatRoundStarted, Relation::After, |e| crate::cc_ground::borrowed::timing::copy_effect(e, true)).applies(Applies::Faction(Faction::TheFederationOfSol)).pair(yssaril_borrows).optional().when(|c| round_when(c, true)));
    t.add(Spec::new(LETNEV, AbilityKind::Leader, E::CombatRoundStarted, Relation::After, |e| crate::cc_ground::borrowed::timing::copy_effect(e, false)).applies(Applies::Faction(Faction::TheBaronyOfLetnev)).pair(yssaril_borrows).optional().when(|c| round_when(c, false)));
    t.add(Spec::new(ARGENT, AbilityKind::Leader, E::GroundForcesBeingProduced, Relation::When, crate::cb_economy::argent::timing::copy_agent).applies(Applies::Faction(Faction::TheArgentFlight)).pair(argent_lends).optional().when(crate::cb_economy::argent::timing::copy_when));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::testkit::table;

    fn set(s: &mut CoreState, who: u8, card: Leader, status: LeaderStatus) {
        s.players_mut().by_seat_mut(Seat(who)).unwrap().leaders.status.insert(card, status);
    }

    /// The conditions of a copy are pure: a window without nested activity arms no table, so no lockstep test sees a copy
    /// that is wrongly offered.
    #[test]
    fn a_copy_needs_a_readied_ssruu_and_a_lender_that_holds_the_agent() {
        let (mut s, _) = table(&[Faction::TheYssarilTribes, Faction::TheYinBrotherhood, Faction::TheEmiratesOfHacan]);
        set(&mut s, 0, Leader::Ssruu, LeaderStatus::Readied);
        set(&mut s, 1, Leader::BrotherMilor, LeaderStatus::Readied);
        let lends = |s: &CoreState, lender: Option<Seat>| borrowable(s, Seat(0), lender, Leader::BrotherMilor);
        assert!(lends(&s, Some(Seat(1))));
        set(&mut s, 1, Leader::BrotherMilor, LeaderStatus::Exhausted);
        assert!(lends(&s, Some(Seat(1))), "an exhausted agent is lent too");
        for gone in [LeaderStatus::Locked, LeaderStatus::Purged] {
            set(&mut s, 1, Leader::BrotherMilor, gone);
            assert!(!lends(&s, Some(Seat(1))), "{gone:?} is not an agent in play");
        }
        set(&mut s, 1, Leader::BrotherMilor, LeaderStatus::Readied);
        assert!(!lends(&s, None), "an ability that names no lender");
        assert!(!lends(&s, Some(Seat(2))), "a lender that does not hold the agent");
        assert!(!borrowable(&s, Seat(1), Some(Seat(1)), Leader::BrotherMilor), "no one borrows from itself");
        set(&mut s, 0, Leader::Ssruu, LeaderStatus::Exhausted);
        assert!(!lends(&s, Some(Seat(1))), "Ssruu itself must be readied");
    }

    #[test]
    fn the_registered_roster_is_the_strings_static_one() {
        let (mut s, _) = table(&[Faction::TheYssarilTribes, Faction::TheYinBrotherhood, Faction::TheEmiratesOfHacan]);
        assert!(!yin_lends(&s, Seat(0), Seat(1)), "no cards in the maps");
        set(&mut s, 0, Leader::Ssruu, LeaderStatus::Exhausted);
        set(&mut s, 1, Leader::BrotherMilor, LeaderStatus::Locked);
        assert!(yin_lends(&s, Seat(0), Seat(1)), "registered whatever the status: the live check is the condition's");
        assert!(!yin_lends(&s, Seat(2), Seat(1)), "a borrower without Ssruu");
        assert!(!argent_lends(&s, Seat(0), Seat(1)), "a lender without the Argent agent");
        assert!(yssaril_borrows(&s, Seat(0), Seat(1)) && !yssaril_borrows(&s, Seat(2), Seat(1)));
    }
}
