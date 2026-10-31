//! winnu: Reclamation (and the marks it reads), Reclaimer, Hegemonic Trade Policy and Berekar Berekon with Ssruu's copy of it
//! (`factions::winnu::{gained_mark, gained_clear, reclamation, reclaimer, htp, agent, borrowed_agent}`). The string registers all of them for
//! every seat (the conditions limit them to the Winnu seat), so the rules here are registered for every seat as well.
//!
//! * `reclamation_mark` (PLANET_CONTROL_GAINED): a planet the Winnu gained during its own tactical action is noted (`private:{seat}:winnu:gained`).
//! * `reclamation_clear` (TURN_BEGAN): the noted planets and the "a tactical action began" mark go.
//! * Reclamation (ACTION_COMPLETED, optional): a PDS and a space dock from the reinforcements on Mecatol Rex, both or none.
//! * Reclaimer (ACTION_COMPLETED, optional, the mech): one PDS or space dock on a planet the action took and the mech stands on.
//! * Hegemonic Trade Policy (PRODUCTION_USED, optional): exhaust, and swap the resource and influence values of one planet for this use.
//! * Berekar Berekon (PRODUCTION_USED, optional): from the rules version of the production dialog it is for another player's use only.

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt, Subtype as GSub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Faction, Leader, Planet, System, Technology, Unit, UnitBaseType};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, Applies, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::kernel::{fx, fx_strategy, Kernel};
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal, Word};
use ti4_engine_core::query::board;
use ti4_engine_core::query::economy::{value_now, Spend};
use ti4_engine_core::state::{CoreState, EventKind, Key, LeaderStatus};

use crate::cd_registry::yin::timing::placed_form_overridden;
use crate::common::{is_faction, mark, rec, scope};

/// `ability:*:reclamation_mark:PLANET_CONTROL_GAINED:after`.
pub const RECLAMATION_MARK: Slot = Slot(42_001);
/// `ability:*:reclamation_clear:TURN_BEGAN:after`.
pub const RECLAMATION_CLEAR: Slot = Slot(42_002);
/// `ability:*:reclamation:ACTION_COMPLETED:after`.
pub const RECLAMATION: Slot = Slot(42_003);
/// `unit:*:winnu_mech:ACTION_COMPLETED:after`.
pub const RECLAIMER: Slot = Slot(42_004);
/// `technology:*:htp:PRODUCTION_USED:after`.
pub const HTP: Slot = Slot(42_005);
/// `leader:*:winnuagent:PRODUCTION_USED:after`.
pub const AGENT: Slot = Slot(42_006);
/// Ssruu copying Berekar Berekon (`leader:{lender faction}:{lender}:yssarilagent:winnuagent:PRODUCTION_USED:after`).
pub const AGENT_COPY: Slot = Slot(42_007);

fn is_winnu(s: &CoreState, who: Seat) -> bool {
    is_faction(s, who, Faction::TheWinnu)
}

// -- the gained-control marks ----------------------------------------------------------------------------------------------------

fn planet_atom(v: &MarkVal) -> Option<Planet> {
    match v {
        MarkVal::Atom(Atom::Planet(p)) => Some(*p),
        _ => None,
    }
}

/// `gained`: the planets (and their systems) the seat gained control of during the tactical action of this turn, in the order they were gained.
fn gained(s: &CoreState, who: Seat) -> Vec<(System, Planet)> {
    let Some(MarkVal::List(items)) = mark(s, MarkKey::PrivateSeat(who, Word::WinnuGained)) else { return Vec::new() };
    items
        .iter()
        .filter_map(|item| match rec(item)? {
            [system, planet] => Some((crate::common::system(system)?, planet_atom(planet)?)),
            _ => None,
        })
        .collect()
}

fn tactical_marked(s: &CoreState, who: Seat) -> bool {
    mark(s, MarkKey::PrivateSeat(who, Word::WinnuTactical)).is_some()
}

// mutants:skip (read by `clear_when` only)
fn gained_marked(s: &CoreState, who: Seat) -> bool {
    mark(s, MarkKey::PrivateSeat(who, Word::WinnuGained)).is_some()
}

fn mark_when(c: &CondCx<'_>) -> bool {
    let me = c.def.owner;
    is_winnu(c.state, me) && c.event.payload.seat(Key::Player) == Some(me) && c.state.meta().active == Some(me) && tactical_marked(c.state, me)
}

fn mark_gained(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let (Some(system), Some(planet)) = (eff.event.payload.system(Key::System), eff.event.payload.planet(Key::Planet)) else { return Ok(()) };
    let entry = MarkVal::Rec(vec![MarkVal::Atom(Atom::System(system)), MarkVal::Atom(Atom::Planet(planet))]);
    let mut list = match mark(eff.read().0, MarkKey::PrivateSeat(me, Word::WinnuGained)) {
        Some(MarkVal::List(items)) => items.clone(),
        _ => Vec::new(),
    };
    list.push(entry);
    eff.state().marks_mut().tail.edit().insert(MarkKey::PrivateSeat(me, Word::WinnuGained), MarkVal::List(list));
    Ok(())
}

// mutants:skip (an eligible clear with nothing to clear changes nothing)
fn clear_when(c: &CondCx<'_>) -> bool {
    gained_marked(c.state, c.def.owner) || tactical_marked(c.state, c.def.owner)
}

fn clear(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let marks = eff.state().marks_mut().tail.edit();
    marks.remove(&MarkKey::PrivateSeat(me, Word::WinnuGained));
    marks.remove(&MarkKey::PrivateSeat(me, Word::WinnuTactical));
    Ok(())
}

// -- structures -------------------------------------------------------------------------------------------------------------------

/// `box_has`: the box holds a piece of the form the seat places for `base`, and no effect forbids placing it.
fn box_has(s: &CoreState, k: &Kernel, who: Seat, base: UnitBaseType) -> bool {
    placed_form_overridden(s, k, scope(k), who, base).is_some_and(|unit| fx::box_has(s, k, scope(k), who, unit))
}

/// `room_for`: a planet holds at most 1 space dock and 2 PDS of a seat.
fn room_for(s: &CoreState, who: Seat, planet: Planet, base: UnitBaseType) -> bool {
    let limit = if base == UnitBaseType::Pds { 2 } else { 1 };
    board::planet_units(s, planet).iter().filter(|p| p.owner() == who && p.kind().data().base_type == base).count() < limit
}

/// `reclamation_ready`: the planet of Mecatol Rex the seat gained during this tactical action and still controls, when both structures can be placed on it.
fn reclamation_ready(s: &CoreState, k: &Kernel, who: Seat) -> Option<Planet> {
    if !is_winnu(s, who) {
        return None;
    }
    let planet = gained(s, who).into_iter().find(|(system, planet)| ti4_engine_core::query::objectives::is_mecatol(*system) && board::controls(s, who, *planet))?.1;
    [UnitBaseType::Pds, UnitBaseType::Spacedock].iter().all(|base| box_has(s, k, who, *base) && room_for(s, who, planet, *base)).then_some(planet)
}

fn reclamation_when(c: &CondCx<'_>) -> bool {
    c.event.payload.seat(Key::Player) == Some(c.def.owner) && reclamation_ready(c.state, c.kernel, c.def.owner).is_some()
}

fn reclamation(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    if eff.event.payload.seat(Key::Player) != Some(me) {
        return Ok(());
    }
    let Some(planet) = ({
        let (s, k) = eff.read();
        reclamation_ready(s, k, me)
    }) else {
        return Ok(());
    };
    let (s, k) = eff.state_and_kernel();
    let system = ti4_engine_core::query::objectives::mecatol_on(s);
    for base in [UnitBaseType::Pds, UnitBaseType::Spacedock] {
        let form = placed_form_overridden(s, k, scope(k), me, base);
        let _ = fx::place_counted(s, k, scope(k), me, system, Some(planet), form, 1);
    }
    Ok(())
}

/// `reclaimer_options`: per planet the seat gained this action, still controls and holds a Reclaimer on, the structures that fit, PDS first.
fn reclaimer_options(s: &CoreState, k: &Kernel, who: Seat) -> Vec<(bool, System, Planet)> {
    if !is_winnu(s, who) {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (system, planet) in gained(s, who) {
        let has_mech = board::planet_units(s, planet).iter().any(|p| p.owner() == who && p.kind() == Unit::Reclaimer);
        if !has_mech || !board::controls(s, who, planet) {
            continue;
        }
        for (pds, base) in [(true, UnitBaseType::Pds), (false, UnitBaseType::Spacedock)] {
            if box_has(s, k, who, base) && room_for(s, who, planet, base) {
                out.push((pds, system, planet));
            }
        }
    }
    out
}

fn reclaimer_when(c: &CondCx<'_>) -> bool {
    c.event.payload.seat(Key::Player) == Some(c.def.owner) && !reclaimer_options(c.state, c.kernel, c.def.owner).is_empty()
}

fn reclaimer(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    if eff.event.payload.seat(Key::Player) != Some(me) {
        return Ok(());
    }
    let found = {
        let (s, k) = eff.read();
        reclaimer_options(s, k, me)
    };
    if found.is_empty() {
        return Ok(());
    }
    let choice = Choice {
        actor: me,
        ask: Ask::Reaction(RAsk::G(GAsk::Reclaimer)),
        ctx: Ctx {
            origin: Origin::FactionAbility(Content::Other(eff.codes().g2.winnu_mech)),
            subtype: Subtype::Reaction(RSub::G(GSub::Reclaimer)),
            optional: false,
            target: None,
            outstanding: SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        },
        opts: found.iter().map(|(pds, system, planet)| Opt::Reaction(ROpt::G(GOpt::Reclaimer { pds: *pds, system: *system, planet: *planet }))).collect::<Opts>(),
    };
    let Pick::Opt(Opt::Reaction(ROpt::G(GOpt::Reclaimer { pds, system, planet }))) = eff.ask(&choice) else { return Ok(()) };
    let (s, k) = eff.state_and_kernel();
    let base = if pds { UnitBaseType::Pds } else { UnitBaseType::Spacedock };
    let form = placed_form_overridden(s, k, scope(k), me, base);
    let _ = fx::place_counted(s, k, scope(k), me, system, Some(planet), form, 1);
    Ok(())
}

// -- Hegemonic Trade Policy --------------------------------------------------------------------------------------------------------

fn htp_ready(s: &CoreState, who: Seat) -> bool {
    ti4_engine_core::query::reach::technology_text(s, who, Technology::HegemonicTradePolicy)
        && !s.players().by_seat(who).is_some_and(|p| p.tech.exhausted.contains(Technology::HegemonicTradePolicy))
}

/// `htp_planets`: the controlled planets whose resource and influence values differ now.
fn htp_planets(s: &CoreState, k: &Kernel, who: Seat) -> Vec<(System, Planet)> {
    if !htp_ready(s, who) {
        return Vec::new();
    }
    let blanked = ti4_engine_core::query::reach::blanked(s);
    let value = |planet: Planet, spend: Spend| {
        let dock = if spend == Spend::Resources { k.hooks.direct.bastion_units_resource_bonus(s, planet) } else { 0 }; // mutants:skip (the dock is the dropped Last Bastion's)
        value_now(s, scope(k), blanked, planet, spend, dock)
    };
    board::controlled_planets(s, who).as_slice().iter().filter(|(_, planet)| value(*planet, Spend::Resources) != value(*planet, Spend::Influence)).copied().collect()
}

fn htp_when(c: &CondCx<'_>) -> bool {
    c.event.payload.seat(Key::Player) == Some(c.def.owner) && !htp_planets(c.state, c.kernel, c.def.owner).is_empty()
}

fn htp(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    if eff.event.payload.seat(Key::Player) != Some(me) {
        return Ok(());
    }
    let found = {
        let (s, k) = eff.read();
        htp_planets(s, k, me)
    };
    if found.is_empty() {
        return Ok(());
    }
    let choice = Choice {
        actor: me,
        ask: Ask::Reaction(RAsk::G(GAsk::TradePolicy)),
        ctx: Ctx {
            origin: Origin::FactionAbility(Content::Technology(Technology::HegemonicTradePolicy)),
            subtype: Subtype::Reaction(RSub::G(GSub::TradePolicy)),
            optional: false,
            target: None,
            outstanding: SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        },
        opts: found.iter().map(|(system, planet)| Opt::Reaction(ROpt::G(GOpt::Swap { system: *system, planet: *planet }))).collect::<Opts>(),
    };
    let Pick::Opt(Opt::Reaction(ROpt::G(GOpt::Swap { planet, .. }))) = eff.ask(&choice) else { return Ok(()) };
    if !found.iter().any(|(_, p)| *p == planet) {
        return Ok(());
    }
    let s = eff.state();
    fx_strategy::exhaust_technology(s, me, Technology::HegemonicTradePolicy);
    s.transient_mut().production.value_swapped_planet = Some(planet);
    let seq = s.meta().production_seq;
    s.marks_mut().tail.edit().insert(MarkKey::ProductionValueSwapSeq(), MarkVal::Atom(Atom::Num(seq)));
    Ok(())
}

// -- Berekar Berekon ---------------------------------------------------------------------------------------------------------------

fn status(s: &CoreState, who: Seat, card: Leader) -> Option<LeaderStatus> {
    s.players().by_seat(who).and_then(|p| p.leaders.status.get(&card).copied())
}

/// The rules version of the production dialog (the only one supported) offers the owner's own use the discount inside the dialog.
fn agent_when(c: &CondCx<'_>) -> bool {
    let me = c.def.owner;
    c.event.payload.seat(Key::Player) != Some(me) && status(c.state, me, Leader::BerekarBerekon) == Some(LeaderStatus::Readied)
}

fn agent(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    if status(eff.state(), me, Leader::BerekarBerekon) != Some(LeaderStatus::Readied) {
        return Ok(());
    }
    let (s, k) = eff.state_and_kernel();
    if fx_strategy::exhaust_agent(s, k, me, Leader::BerekarBerekon) {
        s.transient_mut().production.discount_remaining += 2;
    }
    Ok(())
}

fn holds_card(s: &CoreState, who: Seat, card: Leader) -> bool {
    s.players().by_seat(who).is_some_and(|p| p.leaders.status.contains_key(&card))
}

fn lends(s: &CoreState, borrower: Seat, lender: Seat) -> bool {
    holds_card(s, lender, Leader::BerekarBerekon) && holds_card(s, borrower, Leader::Ssruu)
}

fn copy_when(c: &CondCx<'_>) -> bool {
    crate::cd_registry::yssaril::ssruu::borrowable(c.state, c.def.owner, c.def.pair_source(), Leader::BerekarBerekon)
}

fn copy(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let (me, lender) = (eff.owner(), eff.def.pair_source());
    if !crate::cd_registry::yssaril::ssruu::borrowable(eff.state(), me, lender, Leader::BerekarBerekon) {
        return Ok(());
    }
    let (s, k) = eff.state_and_kernel();
    if status(s, me, Leader::Ssruu) == Some(LeaderStatus::Readied) && fx_strategy::exhaust_agent(s, k, me, Leader::Ssruu) {
        s.transient_mut().production.discount_remaining += 2;
    }
    Ok(())
}

/// Registered in the string order of the Winnu module: the marks, Reclamation, the Reclaimer, Hegemonic Trade Policy, the agent, then the copies.
pub fn register_abilities(t: &mut AbilityTable) {
    use EventKind as E;
    t.add(Spec::new(RECLAMATION_MARK, AbilityKind::Ability, E::PlanetControlGained, Relation::After, mark_gained).when(mark_when));
    t.add(Spec::new(RECLAMATION_CLEAR, AbilityKind::Ability, E::TurnBegan, Relation::After, clear).when(clear_when));
    t.add(Spec::new(RECLAMATION, AbilityKind::Ability, E::ActionCompleted, Relation::After, reclamation).optional().when(reclamation_when));
    t.add(Spec::new(RECLAIMER, AbilityKind::Unit, E::ActionCompleted, Relation::After, reclaimer).optional().when(reclaimer_when));
    t.add(Spec::new(HTP, AbilityKind::Technology, E::ProductionUsed, Relation::After, htp).optional().when(htp_when));
    t.add(Spec::new(AGENT, AbilityKind::Leader, E::ProductionUsed, Relation::After, agent).optional().when(agent_when));
    t.add(Spec::new(AGENT_COPY, AbilityKind::Leader, E::ProductionUsed, Relation::After, copy).applies(Applies::Faction(Faction::TheWinnu)).pair(lends).optional().when(copy_when));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_winnu_slots_are_distinct() {
        let mut t = AbilityTable::default();
        register_abilities(&mut t);
        assert_eq!(t.specs().len(), 7);
    }
}
