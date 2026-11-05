//! The effects of the reaction cards that read another rule area (docs/ENGINE_REACTIONS.md, section 9): the card plays
//! the reaction slot (`ti4_engine_core::rules::action_cards::reaction_slot`) announces at the close of a window and
//! cannot run itself, because the action-card area does not import the combat and invasion areas. They register in
//! the ability table as [`CardPlay`]s, one per group of copies.
//!
//! * Sabotage: the effect is empty (the cancel is the slot's, in the window of `ACTION_CARD_PLAYED`);
//! * Intercept: the opponents of the holder in the active system cannot retreat in this round;
//! * Parley: the units committed to the planet go back to the space area and the invader cannot land again (rules version 36).

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
    t.add_card_play(CardPlay { covers: |c| c == ActionCard::ScrambleFrequency, effect: scramble });
    register_after_play(t);
}

/// Install only the announcement of what a played card staged (`pending_destructions`): the table of the adapter's play window
/// carries no reaction rule, but a card that destroys ships itself (Courageous To The End) stages them for it.
pub fn register_after_play(t: &mut AbilityTable) {
    t.after_play = Some(announce_destructions);
}

/// Draw the dice at `picks` (`(entry, die)`) of the staged set of `roller` again from the dice stream, in order
/// (`combat::apply_reroll_dice`): the face is replaced, the die is marked rerolled and the adjustment it carried dies with it.
/// `record`: each draw is also kept in the ledger as the string's dice history keeps it (the faces of the whole group after the draw,
/// the die that was drawn again marked), for the host to put into its own record of the roll (Scramble Frequency; the ground roll
/// of Fire Team is not kept).
fn redraw(host: &mut RulesHost<'_>, roller: Seat, picks: &[(usize, usize)], record: bool) {
    use ti4_engine_core::state::RngDomain;
    for (e, d) in picks {
        let face = host.state.rng_mut().die(RngDomain::Dice, host.seeds, 10);
        let staging = host.state.transient_mut().combat.reroll_staging.edit();
        let Some((_, set)) = staging.iter_mut().find(|(s, _)| *s == roller) else { continue };
        let system = set.system;
        let Some(entry) = set.rolls.get_mut(*e) else { continue };
        if *d >= entry.faces.len() {
            continue;
        }
        entry.faces[*d] = u8::try_from(face).unwrap_or(u8::MAX);
        if *d < 64 {
            entry.rerolled |= 1 << *d;
        }
        entry.deltas.remove(&u8::try_from(*d).unwrap_or(u8::MAX));
        let group = combat::FleetRoll { hits_on: entry.hits_on.map_or(0, u32::from), faces: entry.faces.iter().map(|f| u32::from(*f)).collect(), rerolled: vec![u32::try_from(*d).unwrap_or(u32::MAX)] };
        if record && host.state.loaded.has(ti4_engine_core::state::Slice::Ledger) {
            combat::record_rolls(host.state.ledger_mut(), roller, Some(system), &[group], ti4_engine_core::state::RollPurpose::Reroll);
        }
    }
}

/// Scramble Frequency: "after another player makes a BOMBARDMENT, SPACE CANNON, or ANTI-FIGHTER BARRAGE roll: that player rerolls all of
/// their dice." Forced, no question: every die of the set the roller staged is drawn again, entry by entry, die by die
/// (`action_cards::scramble`). The roller is the seat that staged the roll (`last_reroll_player`); the kind check of the string is
/// redundant here, because the only events this reaction answers (`UNIT_ABILITY_ROLLED`) are emitted by the windows of those three rolls.
/// The host reads the faces back after the event and takes the hits from them.
#[allow(clippy::unnecessary_wraps)]
fn scramble(host: &mut RulesHost<'_>, _: &mut Resolver, _: Seat, _: ActionCard) -> Result<(), TimingError> {
    let Some(roller) = host.state.transient().combat.last_reroll_player else { return Ok(()) };
    let Some(set) = host.state.transient().combat.reroll_staging.iter().find(|(s, _)| *s == roller).map(|(_, set)| set.clone()) else { return Ok(()) };
    let picks: Vec<(usize, usize)> = set.rolls.iter().enumerate().flat_map(|(e, entry)| (0..entry.faces.len()).map(move |d| (e, d))).collect();
    redraw(host, roller, &picks, true);
    Ok(())
}

/// Fire Team: "after your ground forces make combat rolls during a round of ground combat: reroll any number of your dice."
/// One optional question per staged die, in the order of the staged entries; the chosen dice are drawn again.
#[allow(clippy::unnecessary_wraps)]
fn fire_team(host: &mut RulesHost<'_>, _: &mut Resolver, who: Seat, card: ActionCard) -> Result<(), TimingError> {
    use ti4_engine_core::choice::combat::{Ask as CAsk, Opt as COpt, RerollSource, Subtype as CSub};
    use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype, Target};
    // The texts of the ground dice are the invasion's own (`codes.invasion`: the kind `ground` the roll stages under and the group names it
    // calls its dice by); an adapter that lends other ones (`facts.reroll`) wins. No adapter ever did, so a Fire Team played in a ground round
    // stopped the step with `MissingFact` (policy `scored_games_stay...`, seed 7850 rotation 1).
    let facts = host.table.facts.reroll.clone().unwrap_or_else(|| {
        let inv = &host.table.codes.invasion;
        ti4_engine_core::kernel::ability::RerollFacts { ground: inv.ground, shown: inv.names.iter().map(|n| (*n, *n)).collect() }
    });
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
    redraw(host, who, &picks, false);
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

/// Where Skilled Retreat may send the fleet of `who` from `system` when the position holds the map (`core_combat::open_retreat_destinations`):
/// none under the Mahact hero's bar, else the adjacent systems of the plain map the movement rules let the fleet enter and that hold no other
/// player's ship.
fn open_destinations(host: &RulesHost<'_>, who: Seat, system: System) -> Vec<System> {
    let (core, k) = (&*host.state, host.kernel);
    if k.hooks.direct.mahact_units_ship_movement_barred(core) {
        return Vec::new();
    }
    // (The string's reach is built without the movement effects of the cards in play, `effects: false`; no position the test can build holds an effect that changes the neighbours.)
    let (Some(reach), Some(base)) = (reach::SeatReach::new(core, k, who, false), core.map().adjacency()) else { return Vec::new() }; // mutants:skip: `effects: false` is the string's, see above
    let rd = Rd::new(core, k).with_blanked(reach::blanked(core));
    combat::open_retreat_destinations(rd, who, system, base, &|dest| reach.rules_for(core, k, dest), false)
}

/// `action_cards::skilled_retreat`: "move all of your ships from the active system into an adjacent system that does not
/// contain another player's ships. The space combat ends in a draw. Then place a command token in that system." The
/// destinations are the map's (the adapter noted them: the core is lent no galaxy); the move is the retreat window's.
fn skilled_retreat(host: &mut RulesHost<'_>, resolver: &mut Resolver, who: Seat, card: ActionCard) -> Result<(), TimingError> {
    let Some(system) = host.state.meta().active_system else { return Ok(()) };
    let round = host.state.meta().combat_round_seq;
    let listed = match host.table.facts.withdraw(who, system, round) {
        // The string's read of the map: the adjacent systems the movement rules let the fleet enter. Which of them are free of another player's ships is
        // the position's, and the position changes between two plays of the card in one round (the same seat, the same system, the same round).
        Some(noted) => {
            let rd = Rd::new(&*host.state, host.kernel).with_blanked(reach::blanked(&*host.state));
            noted.iter().copied().filter(|n| !combat::retreat_blocked_by_ships(rd, who, *n)).collect::<Vec<System>>()
        }
        // A live game (the table has a play provider) holds the galaxy itself: the destinations are the position's own map, as the string
        // finds them (`combat::skilled_retreat_destinations`). Without a note and without a live game there is no map to read.
        None if host.table.facts.plays_provider.is_some() => open_destinations(host, who, system),
        None => return Ok(()),
    };
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

/// The unit that just sustained, as `combat::remember_sustain_target` wrote it (`combat:sustain_target`): the planet it stands on (none: the space area) and
/// the unit with its damage and galvanize token. Only a record of this system, this owner and this type, damaged, names a target (the string's filter).
fn sustain_target(host: &RulesHost<'_>, system: System, victim: Seat, unit: ti4_engine_core::generated::Unit) -> Option<(Option<ti4_engine_core::generated::Planet>, ti4_engine_core::state::Piece)> {
    use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
    let Some(MarkVal::Rec(fields)) = host.state.marks().tail.get(&MarkKey::CombatSustainTarget()) else { return None };
    let [MarkVal::Atom(Atom::System(at)), MarkVal::Atom(planet), MarkVal::Atom(Atom::Seat(owner)), MarkVal::Atom(Atom::Unit(kind)), MarkVal::Atom(Atom::Num(damaged)), MarkVal::Atom(Atom::Num(galvanized))] = fields.as_slice() else { return None };
    if *at != system || *owner != victim || *kind != unit || *damaged == 0 {
        return None;
    }
    let planet = match planet {
        Atom::Planet(planet) => Some(*planet),
        _ => None,
    };
    Some((planet, ti4_engine_core::state::Piece::new(unit, victim).with_sustained(true).with_galvanized(*galvanized != 0)))
}

/// `action_cards::direct_hit`: "after another player's ship uses SUSTAIN DAMAGE to cancel a hit produced by your units
/// or abilities: destroy that ship." The sustained ship leaves the board and its destruction is staged for the
/// announcement that follows the discard.
///
/// The ship is the one the sustain handoff names (`combat:sustain_target`: the string's `exact`, which may stand on a planet: a Naaz Eidolon Maximum, an
/// Alastor ground force) and, when the handoff names none, the first sustained ship of the type among the victim's combatants (`ships_of`, planet-standing
/// ones included), removed from where it stands as `combat::remove_combat_ship` does.
#[allow(clippy::unnecessary_wraps)]
fn direct_hit(host: &mut RulesHost<'_>, resolver: &mut Resolver, who: Seat, _: ActionCard) -> Result<(), TimingError> {
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
        let Some(first) = combat::ships_of(rd, victim, system).into_iter().find(|p| p.kind() == unit && p.sustained()) else { return Ok(()) };
        if let Some((planet, piece)) = sustain_target(host, system, victim, unit) {
            let units = match planet {
                Some(planet) => &mut host.state.board_mut().planet_mut(planet).units,
                None => &mut host.state.board_mut().sys_mut(system).units,
            };
            let Some(at) = units.iter().position(|p| *p == piece) else { return Ok(()) };
            units.remove(at);
        } else {
            with_sx(host, resolver, |sx| combat::assign::remove_ship(sx, system, victim, first));
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

/// `action_cards::parley` (rules version 36): the committed units go back to the space area and the landing is cancelled. The card returns the
/// unit of the landing it answers; the invasion that drives the commit step returns the others it landed on that planet and ends the step
/// (`ti4_engine_core::rules::invasion::commit::parley`). The string card returned the last unit only, and the invader landed again (S7).
fn parley(host: &mut RulesHost<'_>, _: &mut Resolver, _: Seat, _: ActionCard) -> Result<(), TimingError> {
    let scope = ti4_engine_core::tables::Scope(u32::from(host.kernel.sources.0));
    ti4_engine_core::rules::invasion::commit::parley(host.state, scope);
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
        for card in [ActionCard::FireTeam, ActionCard::ScrambleFrequency, ActionCard::Sabotage1, ActionCard::Sabotage4, ActionCard::Intercept, ActionCard::Parley, ActionCard::DirectHit1, ActionCard::DirectHit4, ActionCard::SkilledRetreat1, ActionCard::SkilledRetreat4] {
            assert!(t.card_play(card).is_some(), "{card:?}");
        }
        for card in [ActionCard::MoraleBoost1, ActionCard::CourageousToTheEnd, ActionCard::ExperimentalBattlestation] {
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
        assert_eq!(s.transient().combat.parley_cancel, Some((Seat(0), system, planet)), "the landing is cancelled: the invasion ends the commit step");
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

    /// Ships of one type that differ by their galvanize token are no ambiguity: with no handoff the string removes the first sustained ship of the type
    /// among the combatants, and so does the core (it was a `MissingFact` bail).
    #[test]
    fn direct_hit_removes_the_first_sustained_ship_when_the_sustained_ships_differ_by_their_galvanize_token() {
        let (mut s, system, hurt) = sustained_dreadnought_fight();
        let galvanized = Piece::new(Unit::DreadnoughtI, Seat(1)).with_sustained(true).with_galvanized(true);
        s.board_mut().sys_mut(system).units.push(galvanized);
        assert_eq!(run(&mut s, &registered(), ActionCard::DirectHit1, Seat(0), true), Ok(None));
        assert!(!s.board().sys(system).units.contains(&hurt), "the first sustained ship of the type is gone");
        assert!(s.board().sys(system).units.contains(&galvanized), "the other stays");
        assert_eq!(s.transient().combat.pending_destructions.len(), 1);
    }

    /// Bench stops (`MissingFact` of a Direct Hit, 2 native games): the ship that sustained stands on a planet (a Naaz Eidolon Maximum fights the space combat from
    /// there). The sustain handoff names the exact unit and the planet it stands on, and that unit leaves its planet; the ships in space stay.
    #[test]
    fn direct_hit_removes_the_ship_named_by_the_handoff_from_the_planet_it_stands_on() {
        use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
        let (mut s, _) = table(&[Faction::TheFederationOfSol, Faction::TheNaazRokhaAlliance]);
        let system = System::ALL[0];
        let planet = *ti4_engine_core::query::board::planets_of(&s, system).collect::<Vec<_>>().first().unwrap_or(&ti4_engine_core::generated::Planet::ALL[0]);
        let maximum = Piece::new(Unit::EidolonMaximum, Seat(1)).with_sustained(true);
        let cruiser = Piece::new(Unit::CruiserI, Seat(1));
        s.board_mut().sys_mut(system).units.push(cruiser);
        s.board_mut().planet_mut(planet).units.push(maximum);
        s.transient_mut().combat.last_sustain = Some((system, Seat(1), Unit::EidolonMaximum, Seat(0), true));
        let num = |b: u32| MarkVal::Atom(Atom::Num(b));
        let target = MarkVal::Rec(vec![MarkVal::Atom(Atom::System(system)), MarkVal::Atom(Atom::Planet(planet)), MarkVal::Atom(Atom::Seat(Seat(1))), MarkVal::Atom(Atom::Unit(Unit::EidolonMaximum)), num(1), num(0)]);
        s.marks_mut().tail.edit().insert(MarkKey::CombatSustainTarget(), target);
        assert_eq!(run(&mut s, &registered(), ActionCard::DirectHit1, Seat(0), true), Ok(None));
        assert!(s.board().planet(planet).units.is_empty(), "the unit left its planet");
        assert_eq!(s.board().sys(system).units.as_slice(), &[cruiser], "the ships in space stay");
        let staged = s.transient().combat.pending_destructions.as_slice().to_vec();
        assert_eq!((staged.len(), staged[0].unit, staged[0].owner), (1, Unit::EidolonMaximum, Seat(1)));
        assert!(s.marks().tail.get(&MarkKey::CombatSustainTarget()).is_none(), "the handoff is spent");
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

    /// Bench stops (7 native games ended on `Unknown::PendingDestructions`): a Direct Hit that destroys the Yin flagship (Van Hauge) announces the removal
    /// of the card in one pass, as the string's `reactions::announce`; the `SHIP_DESTROYED` windows of that pass stage the destruction of every other ship in
    /// the system (the flagship takes them with it) and these wait for the next announcement of the enclosing window. The play is not unknown: a table with
    /// an announcement leaves nothing of the CARD's own unannounced.
    #[test]
    fn what_the_announcement_of_a_play_stages_in_turn_waits_for_the_next_announcement() {
        use ti4_engine_core::choice::reaction::{AbilityId, AbilityKind};
        use ti4_engine_core::driver::{Driver, Status};
        use ti4_engine_core::kernel::ability::CardPlay;
        use ti4_engine_core::rules::action_cards::announce::Announce;
        use ti4_engine_core::rules::action_cards::{Env, Facts, Placed};
        use ti4_engine_core::state::Code;
        fn destroy_the_flagship(host: &mut RulesHost<'_>, _: &mut Resolver, _: Seat, _: ActionCard) -> Result<(), TimingError> {
            host.state.transient_mut().combat.pending_destructions.push(ti4_engine_core::state::PendingDestruction {
                system: System::ALL[0],
                owner: Seat(0),
                unit: Unit::VanHauge,
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
        let (mut s, _) = table(&[Faction::TheYinBrotherhood, Faction::TheEmiratesOfHacan]);
        let system = System::ALL[0];
        s.board_mut().sys_mut(system).units.push(Piece::new(Unit::CruiserI, Seat(1)));
        s.board_mut().sys_mut(system).units.push(Piece::new(Unit::FighterI, Seat(1)));
        let mut t = AbilityTable::default();
        register_after_play(&mut t);
        crate::cd_registry::yin::timing::register_abilities(&mut t);
        t.add_card_play(CardPlay { covers: |c| c == ActionCard::Parley, effect: destroy_the_flagship });
        let announce = Announce::of(&s, ActionCard::Parley, Seat(1), env, true, true).expect("a registered play");
        let k = ti4_engine_core::kernel::Kernel::plain();
        let mut resolver = Resolver::new(&[Seat(0), Seat(1)], Some(Seat(1)));
        resolver.register(t.registry(&s, &[Seat(0), Seat(1)], &mut |spec, _, _| AbilityId { kind: AbilityKind::Other, key: Code(spec.slot.0) }));
        let mut d = Driver::new(announce, resolver, t, 1);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert_eq!(d.unknown(), None, "the card's own removal was announced; what that announcement staged is the next announcement's");
        assert_eq!(s.transient().combat.pending_destructions.len(), 2, "the cruiser and the fighter wait: {:?}", s.transient().combat.pending_destructions);
        assert!(s.board().sys(system).units.is_empty(), "the flagship's ability removed them");
    }

    /// Integration of B6d, B12 and W15: ONE owner per card (`docs/INTEGRATION5.md`). A card the action-card area plays nested
    /// (`Fx::nested`: its facts are handed in by the host, Courageous To The End) is never also a registered play; a card the area does
    /// not play from a window but that reads the dice a roll staged (Fire Team, Scramble Frequency) is the registered play of this
    /// module there, and its play window stays the area's for the component action; Experimental Battlestation is neither.
    #[test]
    fn one_owner_per_card_the_registered_plays_are_never_cards_the_area_plays_nested() {
        use ti4_engine_core::rules::action_cards::announce::{Announce, plays_in};
        use ti4_engine_core::rules::action_cards::effects::Fx;
        use ti4_engine_core::rules::action_cards::{Env, Facts, Placed};
        use ti4_engine_core::state::Code;
        let t = registered();
        // (A card both nested and registered would be played by the area's effect: `Announce::of` takes the registration only for a card
        // the area does not play nested. The cards of this test are the ones the two lines both touched.)
        for card in [ActionCard::FireTeam, ActionCard::ScrambleFrequency, ActionCard::CourageousToTheEnd, ActionCard::ExperimentalBattlestation, ActionCard::Upgrade, ActionCard::Infiltrate] {
            assert!(!(t.card_play(card).is_some() && Fx::of(card).is_some_and(Fx::nested)), "{card:?} has two owners inside a window");
        }
        for card in [ActionCard::FireTeam, ActionCard::ScrambleFrequency] {
            assert!(plays_in(&t, card), "{card:?} is played inside the windows by the registration");
            assert!(!plays_in(&AbilityTable::default(), card), "without the registration it is the play window's alone");
        }
        assert!(plays_in(&AbilityTable::default(), ActionCard::CourageousToTheEnd) && t.card_play(ActionCard::CourageousToTheEnd).is_none(), "the area plays Courageous To The End");
        assert!(t.card_play(ActionCard::ExperimentalBattlestation).is_none());
        assert!(plays_in(&t, ActionCard::ExperimentalBattlestation), "lane H: the area plays it nested, the hosts hand its facts");
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
        // The announcement of the window: the registration for Fire Team and Scramble Frequency (no facts of the play window).
        // (`registered` is what the inline runner passes: `rt.has_registered_play(card)`.)
        let registered_in = |card: ActionCard| t.card_play(card).is_some();
        assert!(Announce::of(&s, ActionCard::FireTeam, Seat(0), env, true, registered_in(ActionCard::FireTeam)).is_some());
        assert!(Announce::of(&s, ActionCard::FireTeam, Seat(0), env, true, false).is_none(), "nothing registered, no facts of the play window");
        assert!(Announce::of(&s, ActionCard::ScrambleFrequency, Seat(0), env, true, registered_in(ActionCard::ScrambleFrequency)).is_some());
        assert!(Announce::of(&s, ActionCard::ScrambleFrequency, Seat(0), env, true, false).is_none(), "nothing registered, no facts of the play window");
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

    /// The staged roll of seat 1 the event `UNIT_ABILITY_ROLLED` is about: two groups of dice (a destroyer's two and a relic's three).
    fn rolled(roller: Option<Seat>) -> CoreState {
        use ti4_engine_core::state::{Code, RerollEntry, RerollSet};
        let (mut s, _) = table(&[Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan]);
        let entry = |unit: u16, on: u8, faces: &[u8]| RerollEntry {
            unit: Code(unit),
            planet: None,
            hits_on: Some(on),
            faces: faces.iter().copied().collect(),
            rerolled: 0,
            deltas: ti4_engine_core::containers::SortedMap::default(),
            unit_types: ti4_engine_core::containers::SortedMap::default(),
            kind: None,
        };
        let mut first = entry(1, 9, &[3, 4]);
        first.deltas.insert(1, 1);
        let set = RerollSet { kind: Code(9), system: System::ALL[0], rolls: vec![first, entry(2, 8, &[1, 2, 3])] };
        s.transient_mut().combat.reroll_staging.edit().push((Seat(1), set));
        s.transient_mut().combat.last_reroll_player = roller;
        s
    }

    /// Scramble Frequency: every die of the staged set of the seat that rolled is drawn again, entry by entry, die by die; each draw is
    /// marked, replaces the face and is a ledger row of purpose `Reroll` holding the faces of the whole group after the draw.
    #[test]
    fn scramble_frequency_draws_every_die_of_the_roller_again_and_keeps_each_redraw_in_the_ledger() {
        use ti4_engine_core::state::{RngDomain, RollPurpose};
        let mut s = rolled(Some(Seat(1)));
        let mut stream = s.rng().clone();
        let faces: Vec<u8> = (0..5).map(|_| u8::try_from(stream.die(RngDomain::Dice, &NoSeeds, 10)).unwrap()).collect();
        assert_eq!(run(&mut s, &registered(), ActionCard::ScrambleFrequency, Seat(0), false), Ok(None));
        let set = s.transient().combat.reroll_staging[0].1.clone();
        assert_eq!(set.rolls[0].faces.to_vec(), faces[0..2].to_vec(), "the faces are the next five draws of the dice stream, in order");
        assert_eq!(set.rolls[1].faces.to_vec(), faces[2..5].to_vec());
        assert_eq!((set.rolls[0].rerolled, set.rolls[1].rerolled), (0b11, 0b111), "every die is marked as drawn again");
        assert!(set.rolls[0].deltas.is_empty(), "the adjustment a die carried dies with it");
        let rows: Vec<_> = s.ledger().dice.iter().cloned().collect();
        assert_eq!(rows.len(), 5, "one row per die");
        assert!(rows.iter().all(|r| r.purpose == RollPurpose::Reroll && r.owner == Some(Seat(1)) && r.scene == Some(System::ALL[0])), "the roller's rerolls in the scene of the roll");
        assert_eq!((rows[0].rerolled, rows[1].rerolled, rows[2].rerolled, rows[4].rerolled), (1, 2, 1, 4), "the bit of the die drawn again");
        assert_eq!((rows[0].hits_on, rows[2].hits_on), (Some(9), Some(8)));
        // The faces of the group after each draw: the second row of the first group shows both new faces, the first row only the first.
        assert_eq!(rows[0].faces.to_vec(), vec![faces[0], 4]);
        assert_eq!(rows[1].faces.to_vec(), faces[0..2].to_vec());
        assert_eq!(rows[2].faces.to_vec(), vec![faces[2], 2, 3]);
    }

    /// No roller named, or a roller with nothing staged: the card does nothing (the string `scramble` returns).
    #[test]
    fn scramble_frequency_without_a_roller_or_staged_dice_draws_nothing() {
        let mut nobody = rolled(None);
        let before = nobody.clone();
        assert_eq!(run(&mut nobody, &registered(), ActionCard::ScrambleFrequency, Seat(0), false), Ok(None));
        assert_eq!(nobody, before, "no roller: nothing changes");
        let mut other = rolled(Some(Seat(0)));
        let before = other.clone();
        assert_eq!(run(&mut other, &registered(), ActionCard::ScrambleFrequency, Seat(0), false), Ok(None));
        assert_eq!(other, before, "the roller staged nothing: nothing changes");
    }

    /// Fire Team played in a ground round with no adapter facts for the rerolls (`facts.reroll` is lent by nobody): the dice are asked about by the
    /// invasion's own texts, one optional question per die, and the step is not unknown (it stopped with `MissingFact`: policy seed 7850 rotation 1).
    #[test]
    fn fire_team_asks_about_the_dice_of_the_ground_round_with_the_invasions_own_texts() {
        let mut s = rolled(Some(Seat(1)));
        // `rolled` stages the roll of seat 1 under kind 9 with the groups 1 and 2; seat 1 plays the card.
        let mut t = registered();
        t.codes.invasion.ground = ti4_engine_core::state::Code(9);
        t.codes.invasion.names[0] = ti4_engine_core::state::Code(1);
        t.codes.invasion.names[1] = ti4_engine_core::state::Code(2);
        assert!(t.facts.reroll.is_none());
        let kernel = ti4_engine_core::kernel::Kernel::plain();
        let mut tape = Tape::default();
        let mut next_id = 1_u64;
        let mut resolver = Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0)));
        let mut host = RulesHost { state: &mut s, kernel: &kernel, table: &t, tape: &mut tape, next_id: &mut next_id, seeds: &NoSeeds };
        let effect = t.card_play(ActionCard::FireTeam).copied().expect("registered").effect;
        effect(&mut host, &mut resolver, Seat(1), ActionCard::FireTeam).unwrap();
        assert_eq!(tape.take_unknown(), None, "the texts are the invasion's");
        let q = tape.needed().cloned().expect("the first die is asked about");
        assert!(matches!(q.ask, ti4_engine_core::choice::Ask::Combat(ti4_engine_core::choice::combat::Ask::RerollDie { die: 0, .. })));
        assert_eq!(q.actor, Seat(1));
    }

    /// Fire Team keeps its redraw as registered (B12): the draws are made and marked but no ledger row is kept.
    #[test]
    fn the_redraw_of_fire_team_keeps_no_ledger_row() {
        let mut s = rolled(Some(Seat(1)));
        let kernel = ti4_engine_core::kernel::Kernel::plain();
        let t = registered();
        let mut tape = Tape::default();
        let mut next_id = 1_u64;
        let mut host = RulesHost { state: &mut s, kernel: &kernel, table: &t, tape: &mut tape, next_id: &mut next_id, seeds: &NoSeeds };
        redraw(&mut host, Seat(1), &[(0, 1), (1, 2)], false);
        assert!(s.ledger().dice.is_empty());
        let set = &s.transient().combat.reroll_staging[0].1;
        assert_eq!((set.rolls[0].rerolled, set.rolls[1].rerolled), (0b10, 0b100));
        assert_eq!(set.rolls[0].faces[0], 3, "the dice not named keep their faces");
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

    /// Found by the dual-lent run (golden-lent game 17): one seat played the card twice in one round of one system, and between the plays another seat's ships
    /// came to stand in a system the first play could enter. The note is the map's list, one per key, and the ships are read where the card is played; before, the
    /// note held the final list of the LAST play and the first play lost a destination.
    #[test]
    fn skilled_retreat_leaves_out_the_noted_systems_where_another_player_has_a_ship_now() {
        let (a, b) = (System::ALL[1], System::ALL[2]);
        let (mut s, t, _) = retreat_fight(&[a, b]);
        let (unknown, asked) = play(&mut s, &t, ActionCard::SkilledRetreat1);
        assert_eq!(unknown, None);
        let q = asked.expect("two free destinations: a question");
        assert_eq!(q.opts.as_slice().len(), 2);
        // A rival ship in b: one destination left, no question, the fleet goes to a.
        let (mut s, t, _) = retreat_fight(&[a, b]);
        s.board_mut().sys_mut(b).units.push(Piece::new(Unit::CruiserI, Seat(1)));
        let (unknown, asked) = play(&mut s, &t, ActionCard::SkilledRetreat1);
        assert_eq!((unknown, asked.is_none()), (None, true), "one destination is not a question");
        let mine = |s: &CoreState, sys: System| s.board().sys(sys).units.iter().filter(|p| p.owner() == Seat(0)).count();
        assert_eq!((mine(&s, a), mine(&s, b)), (3, 0));
        // Ground forces of the rival do not bar it (78.7c names ships).
        let (mut s, t, _) = retreat_fight(&[a, b]);
        s.board_mut().sys_mut(b).units.push(Piece::new(Unit::InfantryI, Seat(1)));
        let (_, asked) = play(&mut s, &t, ActionCard::SkilledRetreat1);
        assert!(asked.is_some(), "ground forces in the space area are no ships: both destinations stay");
    }

    /// A live game holds the galaxy: with no note of the adapter (the table has a play provider and the position a map) the card's destinations are the
    /// adjacent systems of the plain map the movement rules let the fleet enter and that hold none of another player's ships. Before, the card did
    /// nothing in a live game: the string withdrew the fleet.
    #[test]
    fn skilled_retreat_in_a_live_game_finds_its_destinations_on_the_position_map() {
        use ti4_engine_core::galaxy::CoreGalaxy;
        use ti4_engine_core::hex::Hex;
        static NO_LANES: [Option<&[(u8, u8)]>; System::COUNT] = [None; System::COUNT];
        let plain: Vec<System> = System::ALL.iter().copied().filter(|x| !x.data().planets.is_empty() && !x.data().hyperlane && !x.data().home_system).take(3).collect();
        let (to, blocked) = (plain[0], plain[1]);
        let build = |provider: bool| {
            let (mut s, mut t, system) = retreat_fight(&[]);
            t.facts = Default::default();
            if provider {
                t.facts.plays_provider = Some(ti4_engine_core::kernel::ability::PlaysProvider(std::sync::Arc::new(|_, _| None)));
            }
            let mut galaxy = CoreGalaxy::new(&NO_LANES);
            galaxy.place(system, Hex::new(0, 0)).unwrap();
            galaxy.place(to, Hex::new(1, 0)).unwrap();
            galaxy.place(blocked, Hex::new(-1, 0)).unwrap();
            galaxy.refresh_lane_links();
            s.map_mut().set_galaxy(galaxy);
            s.board_mut().sys_mut(blocked).units.push(Piece::new(Unit::CruiserI, Seat(1)));
            // Only SHIPS bar the card (Dark Energy Tap's variant would also refuse a destination with another player's ground force).
            s.board_mut().planet_mut(to.data().planets[0]).units.push(Piece::new(Unit::InfantryI, Seat(1)));
            (s, t, system)
        };
        let mine = |s: &CoreState, sys: System| s.board().sys(sys).units.iter().filter(|p| p.owner() == Seat(0)).count();
        let (mut s, t, system) = build(true);
        let (unknown, asked) = play(&mut s, &t, ActionCard::SkilledRetreat2);
        assert_eq!((unknown, asked.is_none()), (None, true), "one destination is not a question: the system with the other player's ship is none");
        assert_eq!((mine(&s, system), mine(&s, to), mine(&s, blocked)), (0, 3, 0), "the fleet left for the free neighbour");
        assert!(s.meta().combat_draw_round.is(3), "the combat is declared a draw");
        // The adapter of the string notes the destinations itself (or none, when the string found no map): a table without a live game's provider reads no map.
        let (mut s, t, system) = build(false);
        let (unknown, asked) = play(&mut s, &t, ActionCard::SkilledRetreat2);
        assert_eq!((unknown, asked.is_none(), mine(&s, system), mine(&s, to)), (None, true, 3, 0), "no note and no live game: the card does nothing");
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
        // (The rival's Z-Grav stands in the fight, not in the destination: a destination holds none of another player's ships, which the card reads from the position.)
        s.board_mut().sys_mut(system).units.push(Piece::new(Unit::ZGravEidolonNaazMechSpace, Seat(1)));
        let (unknown, _) = play(&mut s, &t, ActionCard::SkilledRetreat1);
        assert_eq!(unknown, None);
        let kinds = |s: &CoreState, sys: System, who: Seat| s.board().sys(sys).units.iter().filter(|p| p.owner() == who).map(|p| p.kind()).collect::<Vec<_>>();
        assert!(s.board().planet(planet).units.is_empty(), "the Maximum left its planet");
        let landed = kinds(&s, to, Seat(0));
        assert!(landed.contains(&Unit::EidolonMaximum), "{landed:?}");
        assert!(landed.contains(&Unit::Eidolon) && !landed.contains(&Unit::ZGravEidolonNaazMechSpace), "the Z-Grav forms took their ground form where they landed: {landed:?}");
        assert_eq!(kinds(&s, system, Seat(1)), [Unit::CruiserI, Unit::ZGravEidolonNaazMechSpace], "another player's units stay as they are");
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

    /// The commit step records the unit as it stood in space (its ship form) and lands it in its ground form: the card finds it by either form.
    #[test]
    fn parley_returns_a_naaz_mech_recorded_in_its_ship_form() {
        let (mut s, _) = table(&[Faction::TheNaazRokhaAlliance, Faction::TheEmiratesOfHacan]);
        let system = System::ALL[0];
        let planet = ti4_engine_core::generated::Planet::ALL[0];
        s.board_mut().planet_mut(planet).units.push(Piece::new(Unit::Eidolon, Seat(0)));
        s.transient_mut().combat.last_committed_unit = Some((Seat(0), system, planet, Piece::new(Unit::ZGravEidolonNaazMechSpace, Seat(0))));
        assert_eq!(run(&mut s, &registered(), ActionCard::Parley, Seat(1), false), Ok(None));
        assert_eq!(s.board().planet(planet).units.len(), 0, "left the planet");
        assert_eq!(s.board().sys(system).units.iter().map(|p| p.kind()).collect::<Vec<_>>(), [Unit::ZGravEidolonNaazMechSpace]);
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
