//! The effects of the reaction cards that read another rule area (docs/ENGINE_REACTIONS.md, section 9): the card plays
//! the reaction slot (`ti4_engine_core::rules::action_cards::reaction_slot`) announces at the close of a window and
//! cannot run itself, because the action-card area does not import the combat and invasion areas. They register in
//! the ability table as [`CardPlay`]s, one per group of copies.
//!
//! * Sabotage: the effect is empty (the cancel is the slot's, in the window of `ACTION_CARD_PLAYED`);
//! * Intercept: the opponents of the holder in the active system cannot retreat in this round;
//! * Parley: the unit just committed goes back to the space area.

use ti4_engine_core::generated::{ActionCard, Faction, System};
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
    t.add_card_play(CardPlay { covers: skilled_retreat_covers, effect: skilled_retreat });
    t.add_card_play(CardPlay { covers: |c| c == ActionCard::FireTeam, effect: fire_team });
    register_after_play(t);
}

/// Install only the announcement of what a played card staged (`pending_destructions`): the table of the adapter's play window
/// carries no reaction rule, but a card that destroys ships itself (Courageous To The End) stages them for it.
pub fn register_after_play(t: &mut AbilityTable) {
    t.after_play = Some(announce_destructions);
}

/// Draw the dice at `picks` (`(entry, die)`) of the staged set of `roller` again from the dice stream, in order
/// (`combat::apply_reroll_dice`): the face is replaced, the die is marked rerolled and the adjustment it carried dies with it.
fn redraw(host: &mut RulesHost<'_>, roller: Seat, picks: &[(usize, usize)]) {
    use ti4_engine_core::state::RngDomain;
    for (e, d) in picks {
        let face = host.state.rng_mut().die(RngDomain::Dice, host.seeds, 10);
        let staging = host.state.transient_mut().combat.reroll_staging.edit();
        let Some((_, set)) = staging.iter_mut().find(|(s, _)| *s == roller) else { continue };
        let Some(entry) = set.rolls.get_mut(*e) else { continue };
        if *d >= entry.faces.len() {
            continue;
        }
        entry.faces[*d] = u8::try_from(face).unwrap_or(u8::MAX);
        if *d < 64 {
            entry.rerolled |= 1 << *d;
        }
        entry.deltas.remove(&u8::try_from(*d).unwrap_or(u8::MAX));
    }
}

/// Fire Team: "after your ground forces make combat rolls during a round of ground combat: reroll any number of your dice."
/// One optional question per staged die, in the order of the staged entries; the chosen dice are drawn again.
#[allow(clippy::unnecessary_wraps)]
fn fire_team(host: &mut RulesHost<'_>, _: &mut Resolver, who: Seat, card: ActionCard) -> Result<(), TimingError> {
    use ti4_engine_core::choice::combat::{Ask as CAsk, Opt as COpt, RerollSource, Subtype as CSub};
    use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype, Target};
    let Some(facts) = host.table.facts.reroll.clone() else {
        host.tape.unknown.set(Some(Unknown::MissingFact));
        return Ok(());
    };
    let Some(set) = host.state.transient().combat.reroll_staging.iter().find(|(s, _)| *s == who).map(|(_, set)| set.clone()) else { return Ok(()) };
    if set.kind != facts.ground {
        return Ok(());
    }
    let mut picks = Vec::new();
    for (e, entry) in set.rolls.iter().enumerate() {
        for (d, face) in entry.faces.iter().enumerate() {
            let Some(shown) = facts.shown_of(entry.unit) else {
                host.tape.unknown.set(Some(Unknown::MissingFact));
                return Ok(());
            };
            let (entry_no, die) = (u8::try_from(e).unwrap_or(u8::MAX), u8::try_from(d).unwrap_or(u8::MAX));
            let choice = Choice {
                actor: who,
                ask: Ask::Combat(CAsk::RerollDie { source: RerollSource::FireTeam, unit: shown, die }),
                ctx: Ctx {
                    origin: Origin::ActionCard(card),
                    subtype: Subtype::Combat(CSub::Reroll(RerollSource::FireTeam)),
                    optional: false,
                    target: Some(Target::System(set.system)),
                    outstanding: ti4_engine_core::containers::SmallList::new(),
                    space_battle: false,
                    invasion_seq: None,
                },
                opts: [Opt::Combat(COpt::RerollDie { unit: shown, entry: entry_no, die, face: *face, hits_on: entry.hits_on }), Opt::Decline].into_iter().collect::<Opts>(),
            };
            if matches!(host.ask(&choice), Pick::Opt(Opt::Combat(COpt::RerollDie { .. }))) {
                picks.push((e, d));
            }
        }
    }
    redraw(host, who, &picks);
    Ok(())
}


const fn direct_hit_covers(card: ActionCard) -> bool {
    matches!(card, ActionCard::DirectHit1 | ActionCard::DirectHit2 | ActionCard::DirectHit3 | ActionCard::DirectHit4)
}

const fn skilled_retreat_covers(card: ActionCard) -> bool {
    matches!(card, ActionCard::SkilledRetreat1 | ActionCard::SkilledRetreat2 | ActionCard::SkilledRetreat3 | ActionCard::SkilledRetreat4)
}

/// `dual_window::retreat_unported`: what the retreat window of the core does not carry (Bastion's retreat note; the dual-form
/// units, a Voltron on a planet and a structure that moves like a ship are carried since package W3).
fn retreat_unported(host: &RulesHost<'_>, _who: Seat, _system: System, _destination: System) -> bool {
    if host.state.players().iter().any(|p| p.faction == Faction::LastBastion) {
        ti4_engine_core::unsupported::hit(ti4_engine_core::unsupported::Disabled::LastBastion);
        return true;
    }
    false
}

/// `action_cards::skilled_retreat`: "move all of your ships from the active system into an adjacent system that does not
/// contain another player's ships. The space combat ends in a draw. Then place a command token in that system." The
/// destinations are the map's (the adapter noted them: the core is lent no galaxy); the move is the retreat window's.
fn skilled_retreat(host: &mut RulesHost<'_>, resolver: &mut Resolver, who: Seat, card: ActionCard) -> Result<(), TimingError> {
    let Some(system) = host.state.meta().active_system else { return Ok(()) };
    let Some(listed) = host.table.facts.withdraw(who, system, host.state.meta().combat_round_seq).map(<[System]>::to_vec) else { return Ok(()) };
    let Some(&first) = listed.first() else { return Ok(()) };
    let rd = Rd::new(&*host.state, host.kernel).with_blanked(reach::blanked(&*host.state));
    let there = |d: System| u16::try_from(combat::ships_of(rd, who, d).len()).unwrap_or(u16::MAX);
    let fleet = u16::try_from(combat::ships_of(rd, who, system).len()).unwrap_or(u16::MAX);
    let chosen = if listed.len() == 1 {
        first
    } else {
        let opts: ti4_engine_core::choice::Opts = listed
            .iter()
            .map(|d| ti4_engine_core::choice::Opt::Combat(ti4_engine_core::choice::combat::Opt::Withdraw { system: *d, there: there(*d) }))
            .collect();
        let choice = ti4_engine_core::choice::Choice {
            actor: who,
            ask: ti4_engine_core::choice::Ask::Combat(ti4_engine_core::choice::combat::Ask::Withdraw { system, fleet }),
            ctx: ti4_engine_core::choice::Ctx {
                origin: ti4_engine_core::choice::Origin::ActionCard(card),
                subtype: ti4_engine_core::choice::Subtype::Combat(ti4_engine_core::choice::combat::Subtype::Withdraw),
                optional: false,
                target: Some(ti4_engine_core::choice::Target::System(system)),
                outstanding: ti4_engine_core::containers::SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts,
        };
        match host.ask(&choice) {
            ti4_engine_core::choice::Pick::Opt(ti4_engine_core::choice::Opt::Combat(ti4_engine_core::choice::combat::Opt::Withdraw { system, .. })) if listed.contains(&system) => system,
            _ => return Ok(()),
        }
    };
    if retreat_unported(host, who, system, chosen) {
        host.tape.unknown.set(Some(Unknown::UnportedCard));
        return Ok(());
    }
    let facts = combat::retreat::Facts {
        player: who,
        system,
        destination: chosen,
        staging: host.table.facts.staging,
        cause: host.table.codes.retreat_capacity,
        pool: host.table.codes.reinforcements,
    };
    let mut window = combat::retreat::Retreat::new(facts);
    with_sx(host, resolver, |sx| {
        let _ = ti4_engine_core::kernel::stage::Window::step(&mut window, sx);
    });
    let round = host.state.meta().combat_round_seq;
    host.state.meta_mut().combat_draw_round = ti4_engine_core::containers::Opt32::some(round);
    Ok(())
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
    let done = f(&mut sx);
    // What the helpers emitted at the top level is the events of the window that called the hook (`Tape::emitted`).
    host.tape.emitted.append(&mut emitted);
    done
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
    let standing = host.state.board().planet(planet).units.contains(&unit);
    if !standing {
        // An earlier window in the same emission took the unit from the planet.
        return Ok(());
    }
    let units = &mut host.state.board_mut().planet_mut(planet).units;
    if let Some(at) = units.iter().position(|u| *u == unit) {
        units.remove(at);
    }
    host.state.board_mut().sys_mut(system).units.push(unit);
    // A dual-form unit that landed as its ground form goes back to space in its ship form (the string's `flip_form`
    // on the first unit of that owner and kind in the space area): the ground form is never in a space area.
    let scope = ti4_engine_core::tables::Scope(u32::from(host.kernel.sources.0));
    let ship = |u: ti4_engine_core::generated::Unit| scope.unit(u).is_some_and(|d| d.ship);
    if !ship(unit.kind()) && ti4_engine_core::query::tech::other_form(unit.kind()).is_some_and(ship) {
        ti4_engine_core::kernel::fx_strategy::flip_form(host.state, scope, owner, system, None, unit.kind());
    }
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
        for card in [ActionCard::FireTeam, ActionCard::Sabotage1, ActionCard::Sabotage4, ActionCard::Intercept, ActionCard::Parley, ActionCard::DirectHit1, ActionCard::DirectHit4, ActionCard::SkilledRetreat1, ActionCard::SkilledRetreat4] {
            assert!(t.card_play(card).is_some(), "{card:?}");
        }
        for card in [ActionCard::ScrambleFrequency, ActionCard::MoraleBoost1] {
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
        assert!(!s.board().sys(system).units.contains(&hurt), "the sustained ship is gone");
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
        assert!(s.board().sys(system).units.contains(&hurt), "the first ship of the type was removed, damaged or not");
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

    /// The destructions a card staged are announced by the hook after its discard, and the events the hook emits are the events of
    /// the window that plays the card (found by the lockstep test of Courageous To The End: the component action's window dropped
    /// them, so the string's `SHIP_DESTROYED` had no counterpart).
    #[test]
    fn the_events_of_the_after_play_hook_are_the_events_of_the_window_that_plays_the_card() {
        use ti4_engine_core::driver::{Driver, Status};
        use ti4_engine_core::kernel::ability::CardPlay;
        use ti4_engine_core::rules::action_cards::announce::Announce;
        use ti4_engine_core::rules::action_cards::{Env, Facts, Placed};
        use ti4_engine_core::state::{Code, EventKind};
        fn stage_one(host: &mut RulesHost<'_>, _: &mut Resolver, _: Seat, _: ActionCard) -> Result<(), TimingError> {
            host.state.transient_mut().combat.pending_destructions.push(ti4_engine_core::state::PendingDestruction {
                system: System::ALL[0],
                owner: Seat(1),
                unit: Unit::CruiserI,
                reason: Code(0),
                during_space_combat: true,
            });
            Ok(())
        }
        let env = Env {
            mahact_held: 0,
            scope: ti4_engine_core::tables::Scope(u32::MAX),
            blanked: false,
            action_card: Code(0),
            supply_base: Code(0),
            note_order: |a, b| a.cmp(&b),
            infantry: Placed::NONE,
            cruiser: Placed::NONE,
            fighter: Placed::NONE,
            facts: Facts::default(),
        };
        let (mut s, _) = table(&[Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan]);
        let mut t = AbilityTable::default();
        register_after_play(&mut t);
        t.add_card_play(CardPlay { covers: |c| c == ActionCard::Parley, effect: stage_one });
        let announce = Announce::of(&s, ActionCard::Parley, Seat(0), env, true, true).expect("a registered play");
        let k = ti4_engine_core::kernel::Kernel::plain();
        let mut d = Driver::new(announce, Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), t, 1);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        let kinds: Vec<EventKind> = d.emitted.iter().map(|e| e.kind).collect();
        assert_eq!(kinds, [EventKind::ActionCardPlayed, EventKind::ActionCardDiscarded, EventKind::ShipDestroyed], "the loss is announced after the discard, by the window");
        assert!(s.transient().combat.pending_destructions.is_empty());
    }

    /// Integration of B6d and B12: ONE owner per window for each of the three cards that both lines touched
    /// (`docs/INTEGRATION5.md`). Inside another window (the reactions table, where the facts of the play window are not lent)
    /// the registered play is Fire Team's; Scramble Frequency and Courageous To The End are not played there (the census names
    /// them, B6d owns their play window) and a table that registers Fire Team must not make them registered plays.
    #[test]
    fn one_owner_per_card_fire_team_is_registered_and_scramble_and_courageous_are_not() {
        use ti4_engine_core::rules::action_cards::announce::{Announce, plays_in};
        use ti4_engine_core::rules::action_cards::{Env, Facts, Placed};
        use ti4_engine_core::state::Code;
        let t = registered();
        assert!(plays_in(&t, ActionCard::FireTeam), "Fire Team is played inside the windows by the registration");
        assert!(!plays_in(&AbilityTable::default(), ActionCard::FireTeam), "without the registration it is the play window's alone");
        for card in [ActionCard::ScrambleFrequency, ActionCard::CourageousToTheEnd, ActionCard::ExperimentalBattlestation] {
            assert!(t.card_play(card).is_none(), "{card:?} is not registered");
            assert!(!plays_in(&t, card), "{card:?} stays out of the windows (its play window is the action-card area's)");
        }
        let env = Env {
            mahact_held: 0,
            scope: ti4_engine_core::tables::Scope(u32::MAX),
            blanked: false,
            action_card: Code(0),
            supply_base: Code(0),
            note_order: |a, b| a.cmp(&b),
            infantry: Placed::NONE,
            cruiser: Placed::NONE,
            fighter: Placed::NONE,
            facts: Facts::default(),
        };
        let (s, _) = table(&[Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan]);
        // The announcement of the window: the registration for Fire Team (no facts of the play window), nothing for the others.
        // (`registered` is what the inline runner passes: `rt.has_registered_play(card)`.)
        let registered_in = |card: ActionCard| t.card_play(card).is_some();
        assert!(Announce::of(&s, ActionCard::FireTeam, Seat(0), env, true, registered_in(ActionCard::FireTeam)).is_some());
        assert!(Announce::of(&s, ActionCard::FireTeam, Seat(0), env, true, false).is_none(), "nothing registered, no facts of the play window");
        // The others never get this far inside a window: `plays_in` (above) is the gate of the slot, and the census names them.
    }

    fn retreat_fight(destinations: &[System]) -> (CoreState, AbilityTable, System) {
        let (mut s, _) = table(&[Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan]);
        let system = System::ALL[0];
        s.meta_mut().active_system = Some(system);
        s.meta_mut().combat_round_seq = 3;
        for kind in [Unit::CruiserI, Unit::CruiserI, Unit::CarrierI] {
            s.board_mut().sys_mut(system).units.push(Piece::new(kind, Seat(0)));
        }
        s.board_mut().sys_mut(system).units.push(Piece::new(Unit::CruiserI, Seat(1)));
        let mut t = registered();
        t.facts.set_withdraw(Seat(0), system, 3, destinations.to_vec());
        (s, t, system)
    }

    fn play(s: &mut CoreState, t: &AbilityTable, card: ActionCard) -> (Option<Unknown>, Option<ti4_engine_core::choice::Choice>) {
        let kernel = ti4_engine_core::kernel::Kernel::plain();
        let mut tape = Tape::default();
        let mut next_id = 1_u64;
        let mut resolver = Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0)));
        let mut host = RulesHost { state: s, kernel: &kernel, table: t, tape: &mut tape, next_id: &mut next_id, seeds: &NoSeeds };
        let effect = t.card_play(card).copied().expect("registered").effect;
        effect(&mut host, &mut resolver, Seat(0), card).unwrap();
        (tape.take_unknown(), tape.needed().cloned())
    }

    #[test]
    fn skilled_retreat_moves_the_fleet_to_the_only_destination_and_draws_the_combat() {
        let to = System::ALL[1];
        let (mut s, t, system) = retreat_fight(&[to]);
        let (unknown, asked) = play(&mut s, &t, ActionCard::SkilledRetreat2);
        assert_eq!((unknown, asked.is_none()), (None, true), "one destination is not a question");
        let mine = |s: &CoreState, sys: System| s.board().sys(sys).units.iter().filter(|p| p.owner() == Seat(0)).count();
        assert_eq!((mine(&s, system), mine(&s, to)), (0, 3), "all three ships left");
        assert_eq!(s.board().sys(system).units.iter().filter(|p| p.owner() == Seat(1)).count(), 1, "the opponent stays");
        assert!(s.meta().combat_draw_round.is(3), "the combat is declared a draw");
        assert!(ti4_engine_core::query::board::systems_with_token(&s, Seat(0)).contains(to as usize), "a command token goes there");
    }

    #[test]
    fn skilled_retreat_asks_when_there_are_several_destinations() {
        let (a, b) = (System::ALL[1], System::ALL[2]);
        let (mut s, t, _) = retreat_fight(&[a, b]);
        s.board_mut().sys_mut(b).units.push(Piece::new(Unit::CruiserI, Seat(0)));
        let (unknown, asked) = play(&mut s, &t, ActionCard::SkilledRetreat1);
        assert_eq!(unknown, None);
        let q = asked.expect("a question was put");
        assert_eq!(q.actor, Seat(0));
        assert!(matches!(q.ask, ti4_engine_core::choice::Ask::Combat(ti4_engine_core::choice::combat::Ask::Withdraw { fleet: 3, .. })));
        let there: Vec<u16> = q
            .opts
            .as_slice()
            .iter()
            .filter_map(|o| match o {
                ti4_engine_core::choice::Opt::Combat(ti4_engine_core::choice::combat::Opt::Withdraw { there, .. }) => Some(*there),
                _ => None,
            })
            .collect();
        assert_eq!(there, vec![0, 1], "each option shows the ships already there");
    }

    /// A window can hold several plays of the card (a round of each seat, the map changes between them): the play of round 3 reads the
    /// destinations the string found in round 3, not the ones of an earlier or a later round (found by the golden-lent census, where the
    /// last note of the seat won).
    #[test]
    fn skilled_retreat_reads_the_destinations_of_its_own_round() {
        let (a, b) = (System::ALL[1], System::ALL[2]);
        let (mut s, mut t, system) = retreat_fight(&[a]);
        t.facts = Default::default();
        t.facts.set_withdraw(Seat(0), system, 2, vec![b]);
        t.facts.set_withdraw(Seat(0), system, 3, vec![a]);
        t.facts.set_withdraw(Seat(0), system, 4, vec![b]);
        let (unknown, asked) = play(&mut s, &t, ActionCard::SkilledRetreat1);
        assert_eq!((unknown, asked.is_none()), (None, true), "one destination is not a question");
        let mine = |s: &CoreState, sys: System| s.board().sys(sys).units.iter().filter(|p| p.owner() == Seat(0)).count();
        assert_eq!((mine(&s, a), mine(&s, b)), (3, 0), "the fleet went to the destination of round 3");
    }

    #[test]
    fn skilled_retreat_without_a_map_does_nothing() {
        let (mut s, t, system) = retreat_fight(&[]);
        let mut t = t;
        t.facts = Default::default();
        let (unknown, asked) = play(&mut s, &t, ActionCard::SkilledRetreat3);
        assert_eq!((unknown, asked.is_none()), (None, true));
        assert_eq!(s.board().sys(system).units.iter().filter(|p| p.owner() == Seat(0)).count(), 3);
        assert!(s.meta().combat_draw_round.is(3) == false);
    }

    #[test]
    fn bail_skilled_retreat_with_the_last_bastion() {
        let to = System::ALL[1];
        let (mut s, t, system) = retreat_fight(&[to]);
        s.players_mut().by_seat_mut(Seat(1)).unwrap().faction = Faction::LastBastion;
        let (unknown, _) = play(&mut s, &t, ActionCard::SkilledRetreat1);
        assert_eq!(unknown, Some(Unknown::UnportedCard));
        assert_eq!(s.board().sys(system).units.iter().filter(|p| p.owner() == Seat(0)).count(), 3);
    }

    /// Package W3: the retreat window carries the dual-form units. A Z-Grav Eidolon retreats as part of the fleet and takes its
    /// ground form where it lands (with every other ship-form Eidolon of the owner already there); an Eidolon Maximum on a planet
    /// of the system leaves its planet with the fleet; a Floating Factory (a structure that moves like a ship) moves on its own.
    #[test]
    fn skilled_retreat_carries_the_dual_form_units_the_voltron_and_the_floating_factory() {
        let to = System::ALL[1];
        let (mut s, t, system) = retreat_fight(&[to]);
        let planet = ti4_engine_core::generated::Planet::ALL[0];
        // the system's planet: the Maximum stands on it
        s.board_mut().planet_mut(planet).placed_in = Some(system);
        s.board_mut().planet_mut(planet).units.push(Piece::new(Unit::EidolonMaximum, Seat(0)));
        s.board_mut().sys_mut(system).units.push(Piece::new(Unit::ZGravEidolonNaazMechSpace, Seat(0)));
        s.board_mut().sys_mut(to).units.push(Piece::new(Unit::ZGravEidolonNaazMechSpace, Seat(0)));
        s.board_mut().sys_mut(to).units.push(Piece::new(Unit::ZGravEidolonNaazMechSpace, Seat(1)));
        let (unknown, _) = play(&mut s, &t, ActionCard::SkilledRetreat1);
        assert_eq!(unknown, None);
        let kinds = |s: &CoreState, sys: System, who: Seat| s.board().sys(sys).units.iter().filter(|p| p.owner() == who).map(|p| p.kind()).collect::<Vec<_>>();
        assert!(s.board().planet(planet).units.is_empty(), "the Maximum left its planet");
        let landed = kinds(&s, to, Seat(0));
        assert!(landed.contains(&Unit::EidolonMaximum), "{landed:?}");
        assert!(landed.contains(&Unit::Eidolon) && !landed.contains(&Unit::ZGravEidolonNaazMechSpace), "the Z-Grav forms took their ground form where they landed: {landed:?}");
        assert_eq!(kinds(&s, to, Seat(1)), [Unit::ZGravEidolonNaazMechSpace], "another player's units are not flipped");
        assert_eq!(kinds(&s, system, Seat(0)), [], "the fleet left");
    }

    /// A Floating Factory (a structure that moves like a ship) leaves under its own power, and an Eidolon Maximum on a planet joins the
    /// retreat only while its owner has a ship in the space area (`planetary_maximum_participates`).
    #[test]
    fn a_floating_factory_moves_and_a_maximum_follows_only_a_fleet_with_a_ship() {
        let to = System::ALL[1];
        let planet = ti4_engine_core::generated::Planet::ALL[0];
        let (mut s, t, system) = retreat_fight(&[to]);
        s.board_mut().sys_mut(system).units.push(Piece::new(Unit::FloatingFactoryI, Seat(0)));
        let (unknown, _) = play(&mut s, &t, ActionCard::SkilledRetreat1);
        assert_eq!(unknown, None);
        assert!(s.board().sys(to).units.iter().any(|p| p.owner() == Seat(0) && p.kind() == Unit::FloatingFactoryI), "the factory left");
        assert!(!s.board().sys(system).units.iter().any(|p| p.owner() == Seat(0) && p.kind() == Unit::FloatingFactoryI), "and is no longer there");
        // No ship in the space area: the Maximum stays on its planet.
        let (mut s, t, system) = retreat_fight(&[to]);
        s.board_mut().sys_mut(system).units.retain(|p| p.owner() != Seat(0));
        s.board_mut().sys_mut(system).units.push(Piece::new(Unit::InfantryI, Seat(0)));
        s.board_mut().planet_mut(planet).placed_in = Some(system);
        s.board_mut().planet_mut(planet).units.push(Piece::new(Unit::EidolonMaximum, Seat(0)));
        let (unknown, _) = play(&mut s, &t, ActionCard::SkilledRetreat1);
        assert_eq!(unknown, None);
        assert!(s.board().planet(planet).units.iter().any(|p| p.kind() == Unit::EidolonMaximum), "the Maximum stayed: its owner had no ship in space");
        assert!(!s.board().sys(to).units.iter().any(|p| p.kind() == Unit::EidolonMaximum));
    }

    #[test]
    fn parley_returns_a_landed_naaz_mech_to_space_in_its_ship_form() {
        let (mut s, _) = table(&[Faction::TheNaazRokhaAlliance, Faction::TheEmiratesOfHacan]);
        let system = System::ALL[0];
        let planet = ti4_engine_core::generated::Planet::ALL[0];
        let piece = Piece::new(Unit::Eidolon, Seat(0));
        s.board_mut().planet_mut(planet).units.push(piece);
        // an earlier Eidolon of the same owner already stands in space: the string flips the FIRST one of that kind in the list
        s.board_mut().sys_mut(system).units.push(Piece::new(Unit::Eidolon, Seat(0)).with_sustained(true));
        s.transient_mut().combat.last_committed_unit = Some((Seat(0), system, planet, piece));
        assert_eq!(run(&mut s, &registered(), ActionCard::Parley, Seat(1), false), Ok(None));
        assert_eq!(s.board().planet(planet).units.len(), 0, "left the planet");
        let space: Vec<_> = s.board().sys(system).units.iter().map(|p| (p.kind(), p.sustained())).collect();
        assert_eq!(space, [(Unit::ZGravEidolonNaazMechSpace, true), (Unit::Eidolon, false)]);
        // an ordinary unit is not flipped
        let (mut s, _) = table(&[Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan]);
        let piece = Piece::new(Unit::InfantryI, Seat(0));
        s.board_mut().planet_mut(planet).units.push(piece);
        s.transient_mut().combat.last_committed_unit = Some((Seat(0), system, planet, piece));
        assert_eq!(run(&mut s, &registered(), ActionCard::Parley, Seat(1), false), Ok(None));
        assert_eq!(s.board().sys(system).units.iter().map(|p| p.kind()).collect::<Vec<_>>(), [Unit::InfantryI]);
    }

    #[test]
    fn skilled_retreat_strands_what_does_not_fit_and_stages_the_loss_only_with_staging() {
        for staging in [false, true] {
            let to = System::ALL[1];
            let (mut s, mut t, system) = retreat_fight(&[to]);
            for _ in 0..6 {
                s.board_mut().sys_mut(system).units.push(Piece::new(Unit::FighterI, Seat(0)));
            }
            t.facts.staging = staging;
            let (unknown, _) = play(&mut s, &t, ActionCard::SkilledRetreat4);
            assert_eq!(unknown, None);
            let staged = s.transient().combat.pending_destructions.len();
            assert_eq!(staged, if staging { 2 } else { 0 }, "staging {staging}: six fighters, room for four");
            assert_eq!(s.board().sys(to).units.iter().filter(|p| p.kind() == Unit::FighterI).count(), 4);
        }
    }
}
