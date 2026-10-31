//! Lane G3: the loud gap rows turned into rules of the core table (`ti4_engine_core_factions::silent::g3`), part 2. The lockstep against the string is in
//! `ti4-engine` (`invasion_live_g3_tests`, `silent3_live_tests`); these tests pin the rules.

use ti4_engine_core::generated::{Faction, Planet, System, Unit, UnitBaseType};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::query::board;
use ti4_engine_core::state::{EventKind, Key, Payload, Val};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::windows::EmitEvent;

fn table() -> AbilityTable {
    let mut t = AbilityTable::default();
    ti4_engine_core_factions::silent::register_before_movement(&mut t);
    ti4_engine_core_factions::silent::g3::register_after_ca_combat(&mut t);
    ti4_engine_core_factions::silent::register_abilities(&mut t);
    t
}

fn rig(scenario: Scenario, state_edit: impl FnOnce(&mut CoreState), event: ti4_engine_core::state::EventRec) -> Rig<EmitEvent> {
    let (mut state, kernel) = scenario.build();
    state_edit(&mut state);
    let mut r = Rig::from_parts(state, kernel, EmitEvent::new(event)).with_abilities(table());
    r.run();
    r
}

/// A plain system with at least two planets, and its planets.
fn two_planet_system() -> (System, Vec<Planet>) {
    let system = ti4_engine_core_tests::scenario::plain_systems().into_iter().find(|s| s.data().planets.len() >= 2).expect("a plain system with two planets");
    (system, system.data().planets.to_vec())
}

fn units_of(r: &Rig<EmitEvent>, planet: Planet, seat: u8, base: UnitBaseType) -> usize {
    board::planet_units(&r.state, planet).iter().filter(|p| p.owner() == Seat(seat) && p.kind().data().base_type == base).count()
}

fn goods(r: &Rig<EmitEvent>, seat: u8) -> i16 {
    r.state.players().by_seat(Seat(seat)).unwrap().econ.trade_goods
}

// -- Scavenger Zeta -----------------------------------------------------------------------------------------------------------

fn gained(seat: u8, system: System, planet: Planet) -> ti4_engine_core::state::EventRec {
    fx::event(EventKind::PlanetControlGained, Payload::new().with(Key::Player, Val::Seat(Seat(seat))).with(Key::Planet, Val::Planet(planet)).with(Key::System, Val::System(system)))
}

#[test]
fn scavenger_zeta_costs_a_trade_good_and_places_a_mech_on_the_planet_gained() {
    covers!("silent::g3::zeta::ZETA");
    let (system, planets) = two_planet_system();
    let scenario = || Scenario::new(&[Faction::TheClanOfSaar, Faction::TheEmiratesOfHacan]).control(planets[0], 0).goods(0, 2);
    let mut r = rig(scenario(), |_| {}, gained(0, system, planets[0]));
    // The window lists Scavenge (mandatory) and the deploy together: Scavenge first (it pays the good), then the optional deploy.
    r.pick(0);
    assert!(matches!(r.asked().ask, ti4_engine_core::choice::Ask::Reaction(_)), "{}", r.transcript());
    let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the deploy");
    r.pick(use_it);
    assert!(r.is_done(), "{}", r.transcript());
    assert_eq!(units_of(&r, planets[0], 0, UnitBaseType::Mech), 1);
    assert_eq!(goods(&r, 0), 2, "2 + 1 from Scavenge - 1 for the mech");
    // Declined: nothing is placed or paid.
    let mut declined = rig(scenario(), |_| {}, gained(0, system, planets[0]));
    declined.pick(0);
    declined.decline();
    assert!(declined.is_done());
    assert_eq!((units_of(&declined, planets[0], 0, UnitBaseType::Mech), goods(&declined, 0)), (0, 3));
}

#[test]
fn scavenger_zeta_needs_the_gainer_a_saar_seat_and_a_mech_in_the_box() {
    covers!("silent::g3::zeta::ZETA");
    let (system, planets) = two_planet_system();
    // Another seat gains the planet: the Saar is not asked.
    let other = rig(Scenario::new(&[Faction::TheClanOfSaar, Faction::TheEmiratesOfHacan]).control(planets[0], 1).goods(0, 2).goods(1, 2), |_| {}, gained(1, system, planets[0]));
    assert!(other.is_done(), "{}", other.transcript());
    assert_eq!(units_of(&other, planets[0], 1, UnitBaseType::Mech), 0);
    // A seat of another faction has no Scavenger Zeta.
    let plain = rig(Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheClanOfSaar]).control(planets[0], 0).goods(0, 2), |_| {}, gained(0, system, planets[0]));
    assert!(plain.is_done(), "{}", plain.transcript());
    // The four mechs stand elsewhere: the box is empty.
    let mut scenario = Scenario::new(&[Faction::TheClanOfSaar, Faction::TheEmiratesOfHacan]).control(planets[0], 0).goods(0, 2);
    scenario = scenario.unit(system, Some(planets[1]), Unit::ScavengerZeta, 0, 4);
    let spent = rig(scenario, |_| {}, gained(0, system, planets[0]));
    assert!(spent.is_done(), "no mech left to place: {}", spent.transcript());
    assert_eq!(units_of(&spent, planets[0], 0, UnitBaseType::Mech), 0);
}

// -- Ragh's Call --------------------------------------------------------------------------------------------------------------

mod ragh {
    use super::*;
    use ti4_engine_core::choice::reaction::{Ask as RAsk, Opt as ROpt};
    use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt};
    use ti4_engine_core::choice::silent3::{Ask as TAsk, Opt as TOpt};
    use ti4_engine_core::generated::PromissoryNote;
    use ti4_engine_core::state::NoteKey;

    const NOTE: NoteKey = NoteKey { note: PromissoryNote::RaghsCall, faction: Faction::TheClanOfSaar };

    fn committed(system: System, planet: Planet) -> ti4_engine_core::state::EventRec {
        fx::event(EventKind::UnitsCommitted, Payload::new().with(Key::Player, Val::Seat(Seat(0))).with(Key::Planet, Val::Planet(planet)).with(Key::System, Val::System(system)))
    }

    fn holds(seat: u8) -> impl FnOnce(&mut CoreState) {
        move |s| {
            s.cards_mut().promissory.edit().insert(NOTE, Seat(seat));
        }
    }

    /// Seat 0 (Hacan) holds the Saar's note; the Saar (seat 1) has two ground forces on `planets[0]` and controls `planets[1]`.
    fn scenario(system: System, planets: &[Planet]) -> Scenario {
        Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheClanOfSaar])
            .control(planets[0], 1)
            .control(planets[1], 1)
            .unit(system, Some(planets[0]), Unit::InfantryI, 1, 2)
            .unit(system, Some(planets[0]), Unit::ScavengerZeta, 1, 1)
            .unit(system, Some(planets[0]), Unit::PDSI, 1, 1)
    }

    #[test]
    fn the_saar_picks_where_its_ground_forces_go_and_the_note_goes_home() {
        covers!("silent::g3::ragh::RAGH");
        let (system, planets) = two_planet_system();
        let mut r = rig(scenario(system, &planets), holds(0), committed(system, planets[0]));
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the note");
        r.pick(use_it);
        assert_eq!(r.asked().actor, Seat(1), "the Saar chooses: {}", r.transcript());
        assert_eq!(r.asked().ask, Ask::Reaction(RAsk::G(GAsk::T(TAsk::Ragh { planet: planets[0] }))));
        assert_eq!(r.opts(), vec![Opt::Reaction(ROpt::G(GOpt::T(TOpt::Site { system, planet: planets[1] })))]);
        r.pick(0);
        assert!(r.is_done(), "{}", r.transcript());
        let on = |r: &Rig<EmitEvent>, planet| board::planet_units(&r.state, planet).iter().map(|p| p.kind()).collect::<Vec<_>>();
        assert_eq!(on(&r, planets[0]), vec![Unit::PDSI], "the structure stays");
        assert_eq!(on(&r, planets[1]), vec![Unit::InfantryI, Unit::InfantryI, Unit::ScavengerZeta]);
        assert_eq!(r.state.cards().promissory.get(&NOTE), Some(&Seat(1)), "back with the Saar");
    }

    #[test]
    fn declining_keeps_the_note_and_the_forces() {
        covers!("silent::g3::ragh::RAGH");
        let (system, planets) = two_planet_system();
        let mut r = rig(scenario(system, &planets), holds(0), committed(system, planets[0]));
        r.decline();
        assert!(r.is_done());
        assert_eq!(r.state.cards().promissory.get(&NOTE), Some(&Seat(0)));
        assert_eq!(units_of(&r, planets[0], 1, UnitBaseType::Infantry), 2);
    }

    #[test]
    fn the_note_needs_a_holder_who_committed_saar_ground_forces_there_and_another_planet_to_go_to() {
        covers!("silent::g3::ragh::RAGH");
        let (system, planets) = two_planet_system();
        // Another seat's commitment: the holder is not asked.
        let other = rig(scenario(system, &planets), holds(0), fx::event(EventKind::UnitsCommitted, Payload::new().with(Key::Player, Val::Seat(Seat(1))).with(Key::Planet, Val::Planet(planets[0])).with(Key::System, Val::System(system))));
        assert!(other.is_done(), "{}", other.transcript());
        // The note is at home: nobody holds it.
        let home = rig(scenario(system, &planets), |_| {}, committed(system, planets[0]));
        assert!(home.is_done(), "{}", home.transcript());
        // No Saar ground force on the planet committed to.
        let empty = rig(scenario(system, &planets), holds(0), committed(system, planets[1]));
        assert!(empty.is_done(), "{}", empty.transcript());
        // The Saar controls no other planet.
        let lone = rig(Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheClanOfSaar]).control(planets[0], 1).unit(system, Some(planets[0]), Unit::InfantryI, 1, 2), holds(0), committed(system, planets[0]));
        assert!(lone.is_done(), "{}", lone.transcript());
        // The holder is the Saar itself: it never plays its own note.
        let own = rig(scenario(system, &planets), holds(1), fx::event(EventKind::UnitsCommitted, Payload::new().with(Key::Player, Val::Seat(Seat(1))).with(Key::Planet, Val::Planet(planets[0])).with(Key::System, Val::System(system))));
        assert!(own.is_done(), "{}", own.transcript());
    }
}

// -- Tekklar Legion -----------------------------------------------------------------------------------------------------------

mod tekklar {
    use super::*;
    use ti4_engine_core::generated::PromissoryNote;
    use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
    use ti4_engine_core::state::NoteKey;

    const NOTE: NoteKey = NoteKey { note: PromissoryNote::TekklarLegion, faction: Faction::SardakkNorr };

    fn combat(kind: EventKind, system: System, planet: Planet, attacker: u8, defender: u8) -> ti4_engine_core::state::EventRec {
        fx::event(kind, Payload::new().with(Key::System, Val::System(system)).with(Key::Planet, Val::Planet(planet)).with(Key::Attacker, Val::Seat(Seat(attacker))).with(Key::Defender, Val::Seat(Seat(defender))))
    }

    fn holds(seat: u8) -> impl FnOnce(&mut CoreState) {
        move |s| {
            s.cards_mut().promissory.edit().insert(NOTE, Seat(seat));
        }
    }

    fn mark(r: &Rig<EmitEvent>) -> Option<MarkVal> {
        r.state.marks().tail.get(&MarkKey::SardakkTekklarCombat()).cloned()
    }

    fn record(system: System, planet: Planet, holder: u8, norr: Option<u8>) -> MarkVal {
        MarkVal::Rec(vec![MarkVal::Atom(Atom::System(system)), MarkVal::Atom(Atom::Planet(planet)), MarkVal::Atom(Atom::Seat(Seat(holder))), MarkVal::Atom(norr.map_or(Atom::Absent, |n| Atom::Seat(Seat(n))))])
    }

    #[test]
    fn the_holder_plays_it_into_a_combat_it_fights_and_the_mark_names_the_norr_opponent() {
        covers!("silent::g3::tekklar::TEKKLAR");
        let (system, planets) = two_planet_system();
        // Seat 0 (Hacan) holds the note and attacks the N'orr (seat 1).
        let mut r = rig(Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::SardakkNorr]), holds(0), combat(EventKind::GroundCombatStarted, system, planets[0], 0, 1));
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the note");
        r.pick(use_it);
        assert!(r.is_done(), "{}", r.transcript());
        assert_eq!(mark(&r), Some(record(system, planets[0], 0, Some(1))));
        assert_eq!(r.state.cards().promissory.get(&NOTE), Some(&Seat(1)), "back with the N'orr");
        // Against a player who is not the N'orr there is no -1 side.
        let mut other = rig(Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol, Faction::SardakkNorr]), holds(0), combat(EventKind::GroundCombatStarted, system, planets[0], 1, 0));
        let use_it = other.opts().iter().position(|o| *o != Opt::Decline).expect("the note");
        other.pick(use_it);
        assert_eq!(mark(&other), Some(record(system, planets[0], 0, None)));
    }

    #[test]
    fn declining_or_not_fighting_or_holding_the_note_at_home_leaves_no_mark() {
        covers!("silent::g3::tekklar::TEKKLAR");
        let (system, planets) = two_planet_system();
        let seats = [Faction::TheEmiratesOfHacan, Faction::SardakkNorr, Faction::TheFederationOfSol];
        let mut declined = rig(Scenario::new(&seats), holds(0), combat(EventKind::GroundCombatStarted, system, planets[0], 0, 1));
        declined.decline();
        assert!(declined.is_done() && mark(&declined).is_none());
        assert_eq!(declined.state.cards().promissory.get(&NOTE), Some(&Seat(0)));
        // Seat 2 holds it but the combat is between seats 0 and 1.
        let bystander = rig(Scenario::new(&seats), holds(2), combat(EventKind::GroundCombatStarted, system, planets[0], 0, 1));
        assert!(bystander.is_done() && mark(&bystander).is_none(), "{}", bystander.transcript());
        // The N'orr never plays its own note.
        let norr = rig(Scenario::new(&seats), holds(1), combat(EventKind::GroundCombatStarted, system, planets[0], 0, 1));
        assert!(norr.is_done() && mark(&norr).is_none(), "{}", norr.transcript());
        // Nobody holds it (it is at home).
        let home = rig(Scenario::new(&seats), |_| {}, combat(EventKind::GroundCombatStarted, system, planets[0], 0, 1));
        assert!(home.is_done() && mark(&home).is_none());
    }

    #[test]
    fn the_bonus_ends_with_its_combat_and_at_the_latest_with_the_next_turn() {
        covers!("silent::g3::tekklar::TEKKLAR_END");
        covers!("silent::g3::tekklar::TEKKLAR_TURN");
        let (system, planets) = two_planet_system();
        let seats = || Scenario::new(&[Faction::SardakkNorr, Faction::TheEmiratesOfHacan]);
        let marked = |planet: Planet| move |s: &mut CoreState| {
            s.marks_mut().tail.edit().insert(MarkKey::SardakkTekklarCombat(), record(system, planet, 1, Some(0)));
        };
        // The combat it was played into ends: the mark goes.
        let ended = rig(seats(), marked(planets[0]), combat(EventKind::GroundCombatEnded, system, planets[0], 1, 0));
        assert!(ended.is_done() && mark(&ended).is_none());
        // Another planet's combat ends: the mark stays.
        let elsewhere = rig(seats(), marked(planets[0]), combat(EventKind::GroundCombatEnded, system, planets[1], 1, 0));
        assert_eq!(mark(&elsewhere), Some(record(system, planets[0], 1, Some(0))));
        // A turn begins: whatever is left goes.
        let turn = rig(seats(), marked(planets[0]), fx::event(EventKind::TurnBegan, Payload::new().with(Key::Player, Val::Seat(Seat(1)))));
        assert!(turn.is_done() && mark(&turn).is_none());
        // The clean-ups are the N'orr seat's: a table without one keeps the mark (there is none to keep in a game).
        let none = rig(Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol]), marked(planets[0]), fx::event(EventKind::TurnBegan, Payload::new().with(Key::Player, Val::Seat(Seat(1)))));
        assert!(mark(&none).is_some());
    }
}

// -- T'ro's activation mark ---------------------------------------------------------------------------------------------------

mod tro_marks {
    use super::*;
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
    use ti4_engine_core::state::LeaderStatus;

    fn marked(system: System, who: &'static [u8], agent: Option<LeaderStatus>) -> impl FnOnce(&mut CoreState) {
        move |s| {
            for seat in who {
                s.marks_mut().tail.edit().insert(MarkKey::SardakkAgentTactical(Seat(*seat)), MarkVal::Atom(Atom::System(system)));
            }
            if let Some(status) = agent {
                s.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::Tro, status);
            }
        }
    }

    fn has(r: &Rig<EmitEvent>, seat: u8) -> bool {
        r.state.marks().tail.get(&MarkKey::SardakkAgentTactical(Seat(seat))).is_some()
    }

    fn by(kind: EventKind, seat: u8) -> ti4_engine_core::state::EventRec {
        fx::event(kind, Payload::new().with(Key::Player, Val::Seat(Seat(seat))))
    }

    #[test]
    fn a_turn_or_a_strategic_action_forgets_the_activation_of_that_player_only() {
        covers!("silent::g3::tro_marks::TURN");
        covers!("silent::g3::tro_marks::STRATEGIC");
        let (system, _) = two_planet_system();
        let scenario = || Scenario::new(&[Faction::SardakkNorr, Faction::TheEmiratesOfHacan]);
        for kind in [EventKind::TurnBegan, EventKind::StrategicActionBegan] {
            let r = rig(scenario(), marked(system, &[0, 1], Some(LeaderStatus::Readied)), by(kind, 1));
            assert!(r.is_done() && has(&r, 0) && !has(&r, 1), "{kind:?}: {}", r.transcript());
            // An exhausted agent still cleans up (the card is held); a seat without the card does not (nothing registered a mark for it).
            let spent = rig(scenario(), marked(system, &[1], Some(LeaderStatus::Exhausted)), by(kind, 1));
            assert!(!has(&spent, 1), "{kind:?}");
            let without = rig(scenario(), marked(system, &[1], None), by(kind, 1));
            assert!(has(&without, 1), "{kind:?}: the Sardakk seat holds no agent card in this position");
        }
    }
}
