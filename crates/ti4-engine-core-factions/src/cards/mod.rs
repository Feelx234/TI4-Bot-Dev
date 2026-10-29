//! The effects of the reaction cards that read another rule area (docs/ENGINE_REACTIONS.md, section 9): the card plays
//! the reaction slot (`ti4_engine_core::rules::action_cards::reaction_slot`) announces at the close of a window and
//! cannot run itself, because the action-card area does not import the combat and invasion areas. They register in
//! the ability table as [`CardPlay`]s, one per group of copies.
//!
//! * Sabotage: the effect is empty (the cancel is the slot's, in the window of `ACTION_CARD_PLAYED`);
//! * Intercept: the opponents of the holder in the active system cannot retreat in this round;
//! * Parley: the unit just committed goes back to the space area.

use ti4_engine_core::generated::{ActionCard, Faction, System, UnitBaseType};
use ti4_engine_core::kernel::stage::Sx;
use ti4_engine_core::kernel::ability::Unknown;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CardPlay, RulesHost};
use ti4_engine_core::kernel::read::Rd;
use ti4_engine_core::kernel::resolver::{Resolver, TimingError};
use ti4_engine_core::query::reach;
use ti4_engine_core::rules::combat;

/// Register every card effect of this module.
pub fn register(t: &mut AbilityTable) {
    t.add_card_play(CardPlay { covers: sabotage_covers, effect: sabotage });
    t.add_card_play(CardPlay { covers: |c| c == ActionCard::Intercept, effect: intercept });
    t.add_card_play(CardPlay { covers: |c| c == ActionCard::Parley, effect: parley });
    t.add_card_play(CardPlay { covers: direct_hit_covers, effect: direct_hit });
    t.after_play = Some(announce_destructions);
}

const fn direct_hit_covers(card: ActionCard) -> bool {
    matches!(card, ActionCard::DirectHit1 | ActionCard::DirectHit2 | ActionCard::DirectHit3 | ActionCard::DirectHit4)
}

/// A window's services over the host of a card play (the services of a stage, for the helpers of the combat area).
fn with_sx<R>(host: &mut RulesHost<'_>, resolver: &mut Resolver, f: impl FnOnce(&mut Sx<'_>) -> R) -> R {
    let mut emitted = Vec::new();
    let mut sx = Sx {
        state: &mut *host.state,
        kernel: host.kernel,
        resolver,
        table: host.table,
        tape: &mut *host.tape,
        next_id: &mut *host.next_id,
        emitted: &mut emitted,
        seeds: host.seeds,
    };
    f(&mut sx)
}

/// `reactions::announce`'s tail: every destruction an effect staged is announced as a `SHIP_DESTROYED`, in order, so the
/// event's own windows open around it (the ship is off the board already; `last_ship_destroyed` is the handoff).
fn announce_destructions(host: &mut RulesHost<'_>, resolver: &mut Resolver) -> Result<(), TimingError> {
    let staged = std::mem::take(&mut host.state.transient_mut().combat.pending_destructions);
    for d in staged.as_slice() {
        with_sx(host, resolver, |sx| combat::assign::announce_loss(sx, d.system, d.owner, d.unit, d.reason, d.during_space_combat, true));
    }
    Ok(())
}

/// `action_cards::direct_hit`: "after another player's ship uses SUSTAIN DAMAGE to cancel a hit produced by your units
/// or abilities: destroy that ship." The sustained ship leaves the board and its destruction is staged for the
/// announcement that follows the discard.
///
/// Bails (`Unknown`, the string answers): the exact ship is the one the sustain handoff names; the core derives it as the
/// first sustained ship of the type, which is the same ship unless a planet-standing combatant of the victim could be it or
/// the sustained ships of the type differ in their galvanize token.
#[allow(clippy::unnecessary_wraps)]
fn direct_hit(host: &mut RulesHost<'_>, _: &mut Resolver, who: Seat, _: ActionCard) -> Result<(), TimingError> {
    let Some((system, victim, unit, producer, during_space_combat)) = host.state.transient().combat.last_sustain else { return Ok(()) };
    if producer != who {
        return Ok(());
    }
    let scope = Rd::new(&*host.state, host.kernel).scope();
    if !combat::direct_hittable(scope, unit) {
        return Ok(());
    }
    if host.table.facts.staging {
        let rd = Rd::new(&*host.state, host.kernel).with_blanked(reach::blanked(&*host.state));
        let ships = combat::ships_of(rd, victim, system);
        let sustained: Vec<_> = ships.iter().filter(|p| p.kind() == unit && p.sustained()).copied().collect();
        let Some(&first) = sustained.first() else { return Ok(()) };
        let in_space = host.state.board().sys(system).units.iter().filter(|p| **p == first).count();
        if ships.len() != host.state.board().sys(system).units.iter().filter(|p| p.owner() == victim && rd.is_ship(p.kind())).count()
            || sustained.iter().any(|p| *p != first)
            || in_space == 0
        {
            host.tape.unknown.set(Some(Unknown::MissingFact));
            return Ok(());
        }
        let units = &mut host.state.board_mut().sys_mut(system).units;
        if let Some(at) = units.iter().position(|p| *p == first) {
            units.remove(at);
        }
        // The handoff of the sustained ship is spent with it.
        host.state.marks_mut().tail.edit().remove(&ti4_engine_core::keys::MarkKey::CombatSustainTarget());
    } else {
        let units = &mut host.state.board_mut().sys_mut(system).units;
        let Some(at) = units.iter().position(|p| p.owner() == victim && p.kind() == unit) else { return Ok(()) };
        units.remove(at);
    }
    let reason = host.table.codes.direct_hit;
    host.state.transient_mut().combat.pending_destructions.push(ti4_engine_core::state::PendingDestruction { system, owner: victim, unit, reason, during_space_combat });
    Ok(())
}

const fn sabotage_covers(card: ActionCard) -> bool {
    matches!(card, ActionCard::Sabotage1 | ActionCard::Sabotage2 | ActionCard::Sabotage3 | ActionCard::Sabotage4)
}

/// `action_cards::sabotage`: nothing; the slot cancels the event that opened its window.
#[allow(clippy::unnecessary_wraps)]
fn sabotage(_: &mut RulesHost<'_>, _: &mut Resolver, _: Seat, _: ActionCard) -> Result<(), TimingError> {
    Ok(())
}

/// `action_cards::intercept`: every other combatant of the active system cannot retreat in this round of combat.
#[allow(clippy::unnecessary_wraps)]
fn intercept(host: &mut RulesHost<'_>, _: &mut Resolver, who: Seat, _: ActionCard) -> Result<(), TimingError> {
    let Some(system) = host.state.meta().active_system else { return Ok(()) };
    let combatants = combatants(host, system);
    if !combatants.contains(&who) {
        return Ok(());
    }
    let round = host.state.meta().combat_round_seq;
    for opponent in combatants.into_iter().filter(|c| *c != who) {
        if let Some(p) = host.state.players_mut().by_seat_mut(opponent) {
            p.card_timers.retreat_barred_round = ti4_engine_core::containers::Opt32::some(round);
        }
    }
    Ok(())
}

fn combatants(host: &RulesHost<'_>, system: System) -> Vec<Seat> {
    let rd = Rd::new(&*host.state, host.kernel).with_blanked(reach::blanked(&*host.state));
    combat::combatants(rd, system).as_slice().to_vec()
}

/// `action_cards::parley`: the unit the commit step just placed goes back to the space area of its system.
fn parley(host: &mut RulesHost<'_>, _: &mut Resolver, _: Seat, _: ActionCard) -> Result<(), TimingError> {
    let Some((owner, system, planet, unit)) = host.state.transient_mut().combat.last_committed_unit.take() else { return Ok(()) };
    if unit.owner() != owner {
        return Ok(());
    }
    let standing = host.state.board().planet(planet).units.iter().any(|u| *u == unit);
    if !standing {
        // An earlier window in the same emission took the unit from the planet.
        return Ok(());
    }
    // A dual-form unit goes back to space in its ship form: the Naaz mech's flip is the string's.
    let data = unit.kind().data();
    if data.base_type == UnitBaseType::Mech && data.faction.is_some() && host.state.players().by_seat(owner).is_some_and(|p| p.faction == Faction::TheNaazRokhaAlliance) {
        host.tape.unknown.set(Some(ti4_engine_core::kernel::ability::Unknown::UnportedCard));
        return Ok(());
    }
    let units = &mut host.state.board_mut().planet_mut(planet).units;
    if let Some(at) = units.iter().position(|u| *u == unit) {
        units.remove(at);
    }
    host.state.board_mut().sys_mut(system).units.push(unit);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::testkit::table;
    use ti4_engine_core::driver::NoSeeds;
    use ti4_engine_core::generated::Unit;
    use ti4_engine_core::kernel::ability::Tape;
    use ti4_engine_core::state::{CoreState, Piece};

    fn run(s: &mut CoreState, t: &AbilityTable, card: ActionCard, who: Seat, facts_staging: bool) -> Result<Option<Unknown>, TimingError> {
        let kernel = ti4_engine_core::kernel::Kernel::plain();
        let mut t = t.clone();
        t.facts.staging = facts_staging;
        let mut tape = Tape::default();
        let mut next_id = 1_u64;
        let mut resolver = Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0)));
        let mut host = RulesHost { state: s, kernel: &kernel, table: &t, tape: &mut tape, next_id: &mut next_id, seeds: &NoSeeds };
        let play = t.card_play(card).copied().expect("a registered card");
        (play.effect)(&mut host, &mut resolver, who, card)?;
        Ok(tape.take_unknown())
    }

    fn registered() -> AbilityTable {
        let mut t = AbilityTable::default();
        register(&mut t);
        t
    }

    #[test]
    fn the_registered_cards_are_the_ones_named() {
        let t = registered();
        for card in [ActionCard::Sabotage1, ActionCard::Sabotage4, ActionCard::Intercept, ActionCard::Parley, ActionCard::DirectHit1, ActionCard::DirectHit4] {
            assert!(t.card_play(card).is_some(), "{card:?}");
        }
        for card in [ActionCard::SkilledRetreat1, ActionCard::FireTeam, ActionCard::MoraleBoost1] {
            assert!(t.card_play(card).is_none(), "{card:?}");
        }
    }

    #[test]
    fn intercept_bars_the_retreat_of_the_other_combatants_for_this_round() {
        let (mut s, _) = table(&[Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan, Faction::TheBaronyOfLetnev]);
        let system = System::ALL[0];
        s.meta_mut().active_system = Some(system);
        s.meta_mut().combat_round_seq = 4;
        for seat in [Seat(0), Seat(1)] {
            s.board_mut().sys_mut(system).units.push(Piece::new(Unit::CruiserI, seat));
        }
        s.board_mut().sys_mut(system).units.push(Piece::new(Unit::CarrierI, Seat(2)));
        assert_eq!(run(&mut s, &registered(), ActionCard::Intercept, Seat(0), false), Ok(None));
        let barred = |s: &CoreState, who: u8| s.players().by_seat(Seat(who)).unwrap().card_timers.retreat_barred_round.is(4);
        assert!(barred(&s, 1) && barred(&s, 2), "both opponents are barred");
        assert!(!barred(&s, 0), "the holder is not");
    }

    #[test]
    fn intercept_fizzles_for_a_seat_that_is_not_in_the_fight() {
        let (mut s, _) = table(&[Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan]);
        let system = System::ALL[0];
        s.meta_mut().active_system = Some(system);
        s.board_mut().sys_mut(system).units.push(Piece::new(Unit::CruiserI, Seat(1)));
        assert_eq!(run(&mut s, &registered(), ActionCard::Intercept, Seat(0), false), Ok(None));
        assert!(!s.players().by_seat(Seat(1)).unwrap().card_timers.retreat_barred_round.is(0));
    }

    #[test]
    fn parley_returns_the_committed_unit_to_space() {
        let (mut s, _) = table(&[Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan]);
        let system = System::ALL[0];
        let planet = *ti4_engine_core::query::board::planets_of(&s, system).collect::<Vec<_>>().first().unwrap_or(&ti4_engine_core::generated::Planet::ALL[0]);
        let piece = Piece::new(Unit::InfantryI, Seat(0));
        s.board_mut().planet_mut(planet).units.push(piece);
        s.transient_mut().combat.last_committed_unit = Some((Seat(0), system, planet, piece));
        assert_eq!(run(&mut s, &registered(), ActionCard::Parley, Seat(1), false), Ok(None));
        assert!(s.board().planet(planet).units.is_empty(), "the unit left the planet");
        assert_eq!(s.board().sys(system).units.as_slice(), &[piece], "and stands in space");
        assert!(s.transient().combat.last_committed_unit.is_none(), "the handoff is spent");
        // Played again with nothing to return: nothing changes.
        assert_eq!(run(&mut s, &registered(), ActionCard::Parley, Seat(1), false), Ok(None));
        assert_eq!(s.board().sys(system).units.len(), 1);
    }

    fn sustained_dreadnought_fight() -> (CoreState, System, Piece) {
        let (mut s, _) = table(&[Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan]);
        let system = System::ALL[0];
        let hurt = Piece::new(Unit::DreadnoughtI, Seat(1)).with_sustained(true);
        s.board_mut().sys_mut(system).units.push(Piece::new(Unit::DreadnoughtI, Seat(1)));
        s.board_mut().sys_mut(system).units.push(hurt);
        s.transient_mut().combat.last_sustain = Some((system, Seat(1), Unit::DreadnoughtI, Seat(0), true));
        (s, system, hurt)
    }

    #[test]
    fn direct_hit_destroys_the_sustained_ship_and_stages_the_loss() {
        let (mut s, system, hurt) = sustained_dreadnought_fight();
        // The type is what the handoff names, the damaged piece is the one removed.
        let t = registered();
        let outcome = run(&mut s, &t, ActionCard::DirectHit1, Seat(0), true);
        assert_eq!(outcome, Ok(None));
        assert!(!s.board().sys(system).units.iter().any(|p| *p == hurt), "the sustained ship is gone");
        assert_eq!(s.board().sys(system).units.len(), 1, "the other stays");
        let staged = s.transient().combat.pending_destructions.as_slice().to_vec();
        assert_eq!(staged.len(), 1);
        assert_eq!((staged[0].system, staged[0].owner, staged[0].unit, staged[0].during_space_combat), (system, Seat(1), Unit::DreadnoughtI, true));
    }

    #[test]
    fn direct_hit_by_someone_else_than_the_producer_does_nothing() {
        let (mut s, system, _) = sustained_dreadnought_fight();
        assert_eq!(run(&mut s, &registered(), ActionCard::DirectHit2, Seat(1), true), Ok(None));
        assert_eq!(s.board().sys(system).units.len(), 2);
        assert!(s.transient().combat.pending_destructions.is_empty());
    }

    #[test]
    fn direct_hit_without_staging_removes_the_first_ship_of_the_type() {
        let (mut s, system, hurt) = sustained_dreadnought_fight();
        assert_eq!(run(&mut s, &registered(), ActionCard::DirectHit3, Seat(0), false), Ok(None));
        assert!(s.board().sys(system).units.iter().any(|p| *p == hurt), "the first ship of the type was removed, damaged or not");
        assert_eq!(s.board().sys(system).units.len(), 1);
        assert_eq!(s.transient().combat.pending_destructions.len(), 1);
    }

    #[test]
    fn bail_direct_hit_on_ships_that_differ_by_their_galvanize_token() {
        let (mut s, system, _) = sustained_dreadnought_fight();
        s.board_mut().sys_mut(system).units.push(Piece::new(Unit::DreadnoughtI, Seat(1)).with_sustained(true).with_galvanized(true));
        assert_eq!(run(&mut s, &registered(), ActionCard::DirectHit1, Seat(0), true), Ok(Some(Unknown::MissingFact)));
        assert_eq!(s.board().sys(system).units.len(), 3, "nothing was removed");
    }

    #[test]
    fn the_staged_destruction_is_announced_with_the_handoff_and_the_trace_id() {
        let (mut s, system, _) = sustained_dreadnought_fight();
        s.transient_mut().combat.pending_destructions.push(ti4_engine_core::state::PendingDestruction {
            system,
            owner: Seat(1),
            unit: Unit::DreadnoughtI,
            reason: ti4_engine_core::state::Code(0),
            during_space_combat: true,
        });
        let kernel = ti4_engine_core::kernel::Kernel::plain();
        let t = registered();
        let mut tape = Tape::default();
        let mut next_id = 7_u64;
        let mut resolver = Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0)));
        let mut host = RulesHost { state: &mut s, kernel: &kernel, table: &t, tape: &mut tape, next_id: &mut next_id, seeds: &NoSeeds };
        (t.after_play.expect("registered"))(&mut host, &mut resolver).unwrap();
        assert_eq!(next_id, 8, "one event was numbered");
        assert!(s.transient().combat.pending_destructions.is_empty(), "the staged loss is spent");
        assert_eq!(s.transient().combat.last_ship_destroyed, Some((system, Seat(1), Unit::DreadnoughtI)));
    }
}
