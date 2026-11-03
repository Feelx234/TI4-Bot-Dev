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
    // T'ro's mark of the activation is an ability of the activation window (`activation::register` registers it in the string's module order).
    ti4_engine_core_factions::silent::g3::tro_marks::register_activation(&mut t);
    // (Every card is a name of its own: the printed-name groups a draw's discard question lists.)
    t.facts.card_groups = (0..ti4_engine_core::generated::ActionCard::COUNT).map(|i| u8::try_from(i).unwrap_or(u8::MAX)).collect();
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

mod tro_activation_mark {
    use super::*;
    use ti4_engine_core::choice::reaction::Relation;
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
    use ti4_engine_core::state::LeaderStatus;

    fn activated(seat: u8, system: System) -> ti4_engine_core::state::EventRec {
        fx::event(EventKind::SystemActivated, Payload::new().with(Key::Player, Val::Seat(Seat(seat))).with(Key::System, Val::System(system)))
    }

    fn remembered(r: &Rig<EmitEvent>, seat: u8) -> Option<MarkVal> {
        r.state.marks().tail.get(&MarkKey::SardakkAgentTactical(Seat(seat))).cloned()
    }

    fn with_agent(status: LeaderStatus) -> impl FnOnce(&mut CoreState) {
        move |s| {
            s.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::Tro, status);
        }
    }

    /// The string registers the mark as an ability of the Sardakk seat (`leader:*:sardakkagent_mark:SYSTEM_ACTIVATED:after`): it runs on whoever's activation,
    /// while the card is held readied or exhausted (an exhausted T'ro may still be copied), and stays unasked when it is the owner's only ability of the window.
    #[test]
    fn the_activation_of_any_player_is_remembered_as_a_registered_ability_of_the_agents_owner() {
        covers!("silent::g3::tro_marks::MARK");
        let (system, _) = two_planet_system();
        let scenario = || Scenario::new(&[Faction::SardakkNorr, Faction::TheEmiratesOfHacan]);
        for owner_active in [0u8, 1] {
            let r = rig(scenario(), with_agent(LeaderStatus::Readied), activated(owner_active, system));
            assert!(r.is_done(), "{owner_active}: the mark alone is not asked: {}", r.transcript());
            assert_eq!(remembered(&r, owner_active), Some(MarkVal::Atom(Atom::System(system))), "the system the activating player activated");
        }
        let spent = rig(scenario(), with_agent(LeaderStatus::Exhausted), activated(1, system));
        assert!(remembered(&spent, 1).is_some(), "an exhausted agent is still tracked");
        let locked = rig(scenario(), with_agent(LeaderStatus::Locked), activated(1, system));
        assert!(remembered(&locked, 1).is_none(), "a locked or absent agent leaves no mark");
    }

    /// The string offers a window's abilities of one owner as ONE choice: with another ability of the owner eligible, the mark is an option (the string asks
    /// `[the other, the mark, decline]`) and a pass over the window skips it. A mark that is only a hook of the core's activation chain was not offered.
    #[test]
    fn with_another_ability_in_the_window_the_mark_is_one_of_the_offers_and_a_decline_skips_it() {
        covers!("silent::g3::tro_marks::MARK");
        use ti4_engine_core::choice::reaction::AbilityKind;
        use ti4_engine_core::kernel::ability::{no_effect, Slot, Spec};
        let (system, _) = two_planet_system();
        let mut table = table();
        // An optional ability of every seat that is eligible at every activation: a stand-in for a reaction card in the owner's hand.
        table.add(Spec::new(Slot(60_000), AbilityKind::Other, EventKind::SystemActivated, Relation::After, no_effect).optional());
        let start = |table: &ti4_engine_core::kernel::ability::AbilityTable| {
            let (mut state, kernel) = Scenario::new(&[Faction::SardakkNorr, Faction::TheEmiratesOfHacan]).build();
            with_agent(LeaderStatus::Readied)(&mut state);
            let mut r = Rig::from_parts(state, kernel, EmitEvent::new(activated(1, system))).with_abilities(table.clone());
            r.run();
            // The window asks the seats one after the other; the question of the Sardakk seat is the one with the mark among its offers.
            while !r.is_done() && r.asked().actor != Seat(0) {
                r.decline();
            }
            r
        };
        let mut declined = start(&table);
        assert_eq!(declined.opts().len(), 3, "the stand-in, the mark and decline: {}", declined.transcript());
        declined.decline();
        while !declined.is_done() {
            declined.decline();
        }
        assert_eq!(remembered(&declined, 1), None, "the owner passed over the window: the mark is skipped, as in the string");
        let mut played = start(&table);
        played.pick(0);
        while !played.is_done() {
            played.decline();
        }
        assert_eq!(remembered(&played, 1), Some(MarkVal::Atom(Atom::System(system))), "after the stand-in resolved, the mark is the owner's only ability left: it runs unasked");
    }
}

// -- The Alastor ---------------------------------------------------------------------------------------------------------------

mod alastor {
    use super::*;
    use ti4_engine_core::choice::reaction::{Ask as RAsk, Opt as ROpt};
    use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt};
    use ti4_engine_core::choice::silent3::{Ask as TAsk, Opt as TOpt};
    use ti4_engine_core::keys::{Atom, DecimalTextOrd, MarkKey, MarkVal};

    fn started(system: System) -> ti4_engine_core::state::EventRec {
        fx::event(EventKind::SpaceCombatStarted, Payload::new().with(Key::System, Val::System(system)).with(Key::Attacker, Val::Seat(Seat(0))).with(Key::Defender, Val::Seat(Seat(1))))
    }

    fn scenario(system: System, planets: &[Planet]) -> Scenario {
        Scenario::new(&[Faction::TheNekroVirus, Faction::TheEmiratesOfHacan])
            .unit(system, None, Unit::TheAlastor, 0, 1)
            .unit(system, Some(planets[0]), Unit::InfantryI, 0, 2)
            .unit(system, Some(planets[0]), Unit::Mordred, 0, 1)
            .unit(system, Some(planets[1]), Unit::InfantryI, 0, 1)
    }

    fn marks(r: &Rig<EmitEvent>, system: System, planet: Planet, unit: Unit) -> Option<MarkVal> {
        r.state.marks().tail.get(&MarkKey::NekroAlastor(DecimalTextOrd(r.state.meta().activation_seq), system, Seat(0), planet, unit)).cloned()
    }

    #[test]
    fn the_owner_says_per_planet_and_unit_how_many_ground_forces_fight_as_ships() {
        covers!("silent::g3::alastor::ALASTOR");
        let (system, planets) = two_planet_system();
        let mut r = rig(scenario(system, &planets), |_| {}, started(system));
        // One question per planet and unit (planet order, then unit order), each with "none".
        let mut asked = Vec::new();
        while !r.is_done() {
            let q = r.asked().ask;
            asked.push(q);
            match q {
                Ask::Reaction(RAsk::G(GAsk::T(TAsk::Alastor { planet, unit }))) if planet == planets[0] && unit == Unit::InfantryI => {
                    assert_eq!(r.opts(), vec![Opt::Reaction(ROpt::G(GOpt::T(TOpt::Count(1)))), Opt::Reaction(ROpt::G(GOpt::T(TOpt::Count(2)))), Opt::Decline]);
                    r.pick(1);
                }
                Ask::Reaction(RAsk::G(GAsk::T(TAsk::Alastor { planet, unit }))) if unit == Unit::Mordred => {
                    assert_eq!(planet, planets[0]);
                    r.decline();
                }
                _ => {
                    r.pick(0);
                }
            }
        }
        assert_eq!(asked.len(), 3, "infantry and Mordred on the first planet, infantry on the second: {}", r.transcript());
        assert_eq!(marks(&r, system, planets[0], Unit::InfantryI), Some(MarkVal::Atom(Atom::Num(2))));
        assert_eq!(marks(&r, system, planets[0], Unit::Mordred), None, "declined");
        assert_eq!(marks(&r, system, planets[1], Unit::InfantryI), Some(MarkVal::Atom(Atom::Num(1))), "the lone infantry: the first option");
    }

    #[test]
    fn it_needs_the_flagship_in_space_a_side_of_the_combat_and_a_ground_force_on_a_planet() {
        covers!("silent::g3::alastor::ALASTOR");
        let (system, planets) = two_planet_system();
        let no_flagship = Scenario::new(&[Faction::TheNekroVirus, Faction::TheEmiratesOfHacan]).unit(system, Some(planets[0]), Unit::InfantryI, 0, 2);
        assert!(rig(no_flagship, |_| {}, started(system)).is_done());
        let no_ground = Scenario::new(&[Faction::TheNekroVirus, Faction::TheEmiratesOfHacan]).unit(system, None, Unit::TheAlastor, 0, 1);
        assert!(rig(no_ground, |_| {}, started(system)).is_done());
        // Neither side of the combat: the flagship's owner watches.
        let bystander = fx::event(EventKind::SpaceCombatStarted, Payload::new().with(Key::System, Val::System(system)).with(Key::Attacker, Val::Seat(Seat(1))).with(Key::Defender, Val::Seat(Seat(2))));
        let three = Scenario::new(&[Faction::TheNekroVirus, Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol]).unit(system, None, Unit::TheAlastor, 0, 1).unit(system, Some(planets[0]), Unit::InfantryI, 0, 2);
        assert!(rig(three, |_| {}, bystander).is_done());
    }
}

// -- Exotrireme II ---------------------------------------------------------------------------------------------------------------

mod exo2 {
    use super::*;
    use ti4_engine_core::choice::reaction::{Ask as RAsk, Opt as ROpt};
    use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt};
    use ti4_engine_core::choice::silent3::{Ask as TAsk, Opt as TOpt};
    use ti4_engine_core::generated::Technology;

    fn round_ended(system: System) -> ti4_engine_core::state::EventRec {
        fx::event(EventKind::SpaceCombatRoundEnded, Payload::new().with(Key::System, Val::System(system)).with(Key::Round, Val::Int(1)).with(Key::Attacker, Val::Seat(Seat(0))).with(Key::Defender, Val::Seat(Seat(1))))
    }

    fn scenario(system: System) -> Scenario {
        Scenario::new(&[Faction::SardakkNorr, Faction::TheEmiratesOfHacan])
            .tech(0, Technology::ExotriremeII)
            .unit(system, None, Unit::ExotriremeII, 0, 1)
            .unit(system, None, Unit::CruiserI, 0, 1)
            .unit(system, None, Unit::CruiserI, 1, 2)
            .unit(system, None, Unit::DreadnoughtI, 1, 1)
    }

    fn ships_left(r: &Rig<EmitEvent>, system: System, seat: u8) -> usize {
        board::space_units(&r.state, system).iter().filter(|p| p.owner() == Seat(seat)).count()
    }

    #[test]
    fn it_destroys_itself_and_up_to_two_chosen_ships_staging_their_destruction() {
        covers!("silent::g3::exo2::EXO2");
        let (system, _) = two_planet_system();
        let mut r = rig(scenario(system), |_| {}, round_ended(system));
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the ability");
        r.pick(use_it);
        // The only Exotrireme II needs no question; then up to two ships: the distinct ones of the list in order (the Exotrireme II itself is not among them).
        assert_eq!(r.asked().ask, Ask::Reaction(RAsk::G(GAsk::T(TAsk::ExoVictim { round: 0 }))), "{}", r.transcript());
        assert_eq!(
            r.opts().len(),
            3,
            "my cruiser, their cruiser (two of them are one choice), their dreadnought: {}",
            r.transcript()
        );
        assert!(matches!(r.opts()[0], Opt::Reaction(ROpt::G(GOpt::T(TOpt::Ship { index: 0, owner: Seat(0), unit: Unit::CruiserI, victim: true, .. })))));
        r.pick(1);
        assert_eq!(r.asked().ask, Ask::Reaction(RAsk::G(GAsk::T(TAsk::ExoVictim { round: 1 }))));
        assert_eq!(r.opts().last(), Some(&Opt::Decline), "the second ship may be declined");
        r.pick(1);
        assert!(r.is_done(), "{}", r.transcript());
        assert_eq!(ships_left(&r, system, 0), 1, "the Exotrireme II is gone, my cruiser stays");
        assert_eq!(ships_left(&r, system, 1), 1, "two of their three ships are gone");
        assert_eq!(r.state.transient().combat.pending_destructions.len(), 3, "its own destruction and the two it chose are staged");
    }

    #[test]
    fn declining_the_second_ship_destroys_one_and_the_ability_needs_the_technology_a_side_and_another_ship() {
        covers!("silent::g3::exo2::EXO2");
        let (system, _) = two_planet_system();
        let mut r = rig(scenario(system), |_| {}, round_ended(system));
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the ability");
        r.pick(use_it);
        r.pick(2);
        r.decline();
        assert!(r.is_done());
        assert_eq!(r.state.transient().combat.pending_destructions.len(), 2);
        // Declined at the window: nothing happens.
        let mut declined = rig(scenario(system), |_| {}, round_ended(system));
        declined.decline();
        assert!(declined.is_done() && declined.state.transient().combat.pending_destructions.is_empty());
        // No technology: not a side's ability. (The unit alone does nothing.)
        let plain = Scenario::new(&[Faction::SardakkNorr, Faction::TheEmiratesOfHacan]).unit(system, None, Unit::ExotriremeII, 0, 1).unit(system, None, Unit::CruiserI, 1, 2);
        assert!(rig(plain, |_| {}, round_ended(system)).is_done());
        // Alone in the system: no other ship to destroy.
        let alone = Scenario::new(&[Faction::SardakkNorr, Faction::TheEmiratesOfHacan]).tech(0, Technology::ExotriremeII).unit(system, None, Unit::ExotriremeII, 0, 1);
        assert!(rig(alone, |_| {}, round_ended(system)).is_done());
        // The owner watches a combat of two others.
        let watched = fx::event(EventKind::SpaceCombatRoundEnded, Payload::new().with(Key::System, Val::System(system)).with(Key::Round, Val::Int(1)).with(Key::Attacker, Val::Seat(Seat(1))).with(Key::Defender, Val::Seat(Seat(2))));
        let three = Scenario::new(&[Faction::SardakkNorr, Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol]).tech(0, Technology::ExotriremeII).unit(system, None, Unit::ExotriremeII, 0, 1).unit(system, None, Unit::CruiserI, 1, 2);
        assert!(rig(three, |_| {}, watched).is_done());
    }

    #[test]
    fn a_damaged_and_an_undamaged_exotrireme_are_asked_which_one_goes() {
        covers!("silent::g3::exo2::EXO2");
        let (system, _) = two_planet_system();
        let scenario = scenario(system).unit(system, None, Unit::ExotriremeII, 0, 1);
        let damage = |s: &mut CoreState| {
            let units = &mut s.board_mut().sys_mut(system).units;
            let at = units.as_slice().iter().position(|p| p.kind() == Unit::ExotriremeII).unwrap();
            let damaged = units.as_slice()[at].with_sustained(true);
            units.remove(at);
            units.push(damaged);
        };
        let mut r = rig(scenario, damage, round_ended(system));
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the ability");
        r.pick(use_it);
        assert_eq!(r.asked().ask, Ask::Reaction(RAsk::G(GAsk::T(TAsk::ExoSelf))), "{}", r.transcript());
        assert_eq!(r.opts().len(), 2, "one damaged, one not");
    }
}

// -- Z'eu ---------------------------------------------------------------------------------------------------------------------

mod zeu {
    use super::*;
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::state::LeaderStatus;

    fn placed(system: System, seat: u8) -> ti4_engine_core::state::EventRec {
        fx::event(EventKind::CommandTokenPlaced, Payload::new().with(Key::Player, Val::Seat(Seat(seat))).with(Key::System, Val::System(system)))
    }

    fn naalu(system: System, token: Option<u8>, agent: LeaderStatus) -> (Scenario, impl FnOnce(&mut CoreState)) {
        let mut scenario = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheNaaluCollective]);
        if let Some(seat) = token {
            scenario = scenario.command_token(system, seat);
        }
        (scenario, move |s: &mut CoreState| {
            s.players_mut().by_seat_mut(Seat(1)).unwrap().leaders.status.insert(Leader::ZeuTe, agent);
        })
    }

    fn tokens(r: &Rig<EmitEvent>, system: System) -> Vec<Seat> {
        (0..2).map(Seat).filter(|s| r.state.board().sys(system).cmd.contains(*s)).collect()
    }

    #[test]
    fn the_naalu_exhausts_the_agent_to_remove_the_token_that_was_placed() {
        covers!("silent::g3::zeu::ZEU");
        let (system, _) = two_planet_system();
        let (scenario, edit) = naalu(system, Some(0), LeaderStatus::Readied);
        let mut r = rig(scenario, edit, placed(system, 0));
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the agent");
        r.pick(use_it);
        assert!(r.is_done(), "{}", r.transcript());
        assert!(tokens(&r, system).is_empty(), "the token is gone");
        assert_eq!(r.state.players().by_seat(Seat(1)).unwrap().leaders.status.get(&Leader::ZeuTe), Some(&LeaderStatus::Exhausted));
        // Declined: the token stays and the card stays ready.
        let (scenario, edit) = naalu(system, Some(0), LeaderStatus::Readied);
        let mut declined = rig(scenario, edit, placed(system, 0));
        declined.decline();
        assert_eq!(tokens(&declined, system), vec![Seat(0)]);
        assert_eq!(declined.state.players().by_seat(Seat(1)).unwrap().leaders.status.get(&Leader::ZeuTe), Some(&LeaderStatus::Readied));
    }

    #[test]
    fn it_needs_a_readied_agent_and_the_token_on_the_board() {
        covers!("silent::g3::zeu::ZEU");
        let (system, _) = two_planet_system();
        let (scenario, edit) = naalu(system, Some(0), LeaderStatus::Exhausted);
        assert!(rig(scenario, edit, placed(system, 0)).is_done(), "an exhausted agent");
        let (scenario, edit) = naalu(system, None, LeaderStatus::Readied);
        assert!(rig(scenario, edit, placed(system, 0)).is_done(), "no token in the system");
    }
}

// -- Acidos ---------------------------------------------------------------------------------------------------------------------

mod acidos {
    use super::*;
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::state::LeaderStatus;

    fn gained(seat: u8) -> ti4_engine_core::state::EventRec {
        fx::event(EventKind::TechnologyGained, Payload::new().with(Key::Player, Val::Seat(Seat(seat))))
    }

    fn unlocked(s: &mut CoreState) {
        s.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::NekroAcidos, LeaderStatus::Unlocked);
    }

    fn hand(r: &Rig<EmitEvent>, seat: u8) -> usize {
        r.state.players().by_seat(Seat(seat)).unwrap().cards.hand.len()
    }

    fn deck(s: &mut CoreState) {
        // A deck of known cards.
        s.decks_mut().action.set(vec![ti4_engine_core::generated::ActionCard::ALL[0], ti4_engine_core::generated::ActionCard::ALL[1]]);
    }

    #[test]
    fn the_commander_may_draw_a_card_after_a_technology_is_gained() {
        covers!("silent::g3::nekro_commander::ACIDOS");
        let scenario = || Scenario::new(&[Faction::TheNekroVirus, Faction::TheEmiratesOfHacan]);
        let edit = |s: &mut CoreState| {
            unlocked(s);
            deck(s);
        };
        let mut r = rig(scenario(), edit, gained(0));
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the commander");
        r.pick(use_it);
        assert!(r.is_done());
        assert_eq!(hand(&r, 0), 1);
        let mut declined = rig(scenario(), edit, gained(0));
        declined.decline();
        assert_eq!(hand(&declined, 0), 0);
    }

    #[test]
    fn it_needs_the_unlocked_commander_the_gainer_and_a_deck() {
        covers!("silent::g3::nekro_commander::ACIDOS");
        let scenario = || Scenario::new(&[Faction::TheNekroVirus, Faction::TheEmiratesOfHacan]);
        assert!(rig(scenario(), deck, gained(0)).is_done(), "locked");
        assert!(rig(scenario(), |s| { unlocked(s); deck(s); }, gained(1)).is_done(), "another seat gained it");
        assert!(rig(scenario(), |s| { unlocked(s); s.decks_mut().action.set(vec![]); }, gained(0)).is_done(), "no deck");
    }
}

// -- Suffi An --------------------------------------------------------------------------------------------------------------------

mod suffi_an {
    use super::*;
    use ti4_engine_core::choice::reaction::{Ask as RAsk, Opt as ROpt};
    use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt};
    use ti4_engine_core::choice::silent3::{Ask as TAsk, Opt as TOpt};
    use ti4_engine_core::generated::{ActionCard, Leader};
    use ti4_engine_core::state::LeaderStatus;

    fn gained(seat: u8) -> ti4_engine_core::state::EventRec {
        fx::event(EventKind::TradeGoodsGained, Payload::new().with(Key::Player, Val::Seat(Seat(seat))))
    }

    fn scenario(system: System, planets: &[Planet], factions: &[Faction]) -> Scenario {
        Scenario::new(factions).map_from(&[system]).unit(system, Some(planets[0]), Unit::InfantryI, 0, 1).unit(system, Some(planets[1]), Unit::InfantryI, 1, 1).goods(1, 3)
    }

    fn edit(copy: bool) -> impl FnOnce(&mut CoreState) {
        move |s| {
            s.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::SuffiAnMentakagent, LeaderStatus::Readied);
            if copy {
                s.players_mut().by_seat_mut(Seat(2)).unwrap().leaders.status.insert(Leader::Ssruu, LeaderStatus::Readied);
            }
            s.decks_mut().action.set(vec![ActionCard::ALL[0], ActionCard::ALL[1], ActionCard::ALL[2]]);
        }
    }

    fn hand(r: &Rig<EmitEvent>, seat: u8) -> usize {
        r.state.players().by_seat(Seat(seat)).unwrap().cards.hand.len()
    }

    #[test]
    fn the_agent_is_offered_right_after_the_take_and_both_draw_a_card() {
        covers!("silent::g3::suffi_an::SUFFI_AN");
        let (system, planets) = two_planet_system();
        let mut r = rig(scenario(system, &planets, &[Faction::TheMentakCoalition, Faction::TheEmiratesOfHacan]), edit(false), gained(1));
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("pillage");
        r.pick(use_it);
        assert_eq!(r.asked().ask, Ask::Reaction(RAsk::G(GAsk::T(TAsk::SuffiAn { target: Seat(1) }))), "{}", r.transcript());
        assert_eq!(r.opts(), vec![Opt::Reaction(ROpt::G(GOpt::T(TOpt::Exhaust))), Opt::Decline]);
        r.pick(0);
        assert!(r.is_done(), "{}", r.transcript());
        assert_eq!((hand(&r, 0), hand(&r, 1)), (1, 1), "one card each");
        assert_eq!(r.state.players().by_seat(Seat(0)).unwrap().leaders.status.get(&Leader::SuffiAnMentakagent), Some(&LeaderStatus::Exhausted));
        // Declined: nothing is drawn and the agent stays ready.
        let mut declined = rig(scenario(system, &planets, &[Faction::TheMentakCoalition, Faction::TheEmiratesOfHacan]), edit(false), gained(1));
        declined.pick(use_it);
        declined.decline();
        assert!(declined.is_done());
        assert_eq!((hand(&declined, 0), hand(&declined, 1)), (0, 0));
        // An exhausted agent or an empty deck: Pillage alone.
        let mut exhausted = rig(scenario(system, &planets, &[Faction::TheMentakCoalition, Faction::TheEmiratesOfHacan]), |s| { edit(false)(s); s.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::SuffiAnMentakagent, LeaderStatus::Exhausted); }, gained(1));
        exhausted.pick(use_it);
        assert!(exhausted.is_done(), "{}", exhausted.transcript());
    }

    #[test]
    fn with_a_ssruu_copy_at_the_table_the_use_is_announced_and_both_answer() {
        covers!("silent::g3::suffi_an::SUFFI_AN");
        covers!("silent::g3::suffi_an::SUFFI_AN_COPY");
        let (system, planets) = two_planet_system();
        let factions = [Faction::TheMentakCoalition, Faction::TheEmiratesOfHacan, Faction::TheYssarilTribes];
        let mut r = rig(scenario(system, &planets, &factions), edit(true), gained(1));
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("pillage");
        r.pick(use_it);
        // The owner's window runs the question; the copy of the Yssaril comes after (its own window, optional).
        assert_eq!(r.asked().ask, Ask::Reaction(RAsk::G(GAsk::T(TAsk::SuffiAn { target: Seat(1) }))), "{}", r.transcript());
        r.decline();
        assert_eq!(r.asked().actor, Seat(2), "the Yssaril is asked whether to use Ssruu's copy: {}", r.transcript());
        let copy = r.opts().iter().position(|o| *o != Opt::Decline).expect("the copy");
        r.pick(copy);
        // The Yssaril draws with Scheming (a bonus card, then a discard it is asked for): 2 - 1 cards, and the pillaged player 1.
        assert_eq!(r.asked().actor, Seat(2), "{}", r.transcript());
        r.pick(0);
        assert!(r.is_done(), "{}", r.transcript());
        assert_eq!((hand(&r, 0), hand(&r, 1), hand(&r, 2)), (0, 1, 1), "the Yssaril and the pillaged player draw");
        assert_eq!(r.state.players().by_seat(Seat(2)).unwrap().leaders.status.get(&Leader::Ssruu), Some(&LeaderStatus::Exhausted));
        assert_eq!(r.state.players().by_seat(Seat(0)).unwrap().leaders.status.get(&Leader::SuffiAnMentakagent), Some(&LeaderStatus::Readied), "the lender's card is not exhausted");
    }
}

// -- S'ula Mentarion ------------------------------------------------------------------------------------------------------------

mod sula {
    use super::*;
    use ti4_engine_core::choice::reaction::{Ask as RAsk, Opt as ROpt};
    use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt};
    use ti4_engine_core::choice::silent3::{Ask as TAsk, Opt as TOpt};
    use ti4_engine_core::generated::{Leader, PromissoryNote};
    use ti4_engine_core::state::{Code, LeaderStatus, List, NoteKey};

    const FACTIONS: [Faction; 3] = [Faction::TheMentakCoalition, Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol];

    fn won(system: System, opponents: &[u8]) -> ti4_engine_core::state::EventRec {
        let seats: ti4_engine_core::containers::SmallList<Seat, 4> = opponents.iter().map(|s| Seat(*s)).collect();
        fx::event(EventKind::SpaceCombatWon, Payload::new().with(Key::Player, Val::Seat(Seat(0))).with(Key::System, Val::System(system)).with_list(Key::Other(Code(0)), List::Seats(seats)))
    }

    fn note(note: PromissoryNote, faction: Faction) -> NoteKey {
        NoteKey { note, faction }
    }

    fn edit(holds: &'static [(u8, PromissoryNote, Faction)]) -> impl FnOnce(&mut CoreState) {
        move |s| {
            s.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::SulaMentarion, LeaderStatus::Unlocked);
            for (holder, n, faction) in holds {
                s.cards_mut().promissory.edit().insert(note(*n, *faction), Seat(*holder));
            }
        }
    }

    #[test]
    fn the_opponent_gives_a_note_of_its_hand_and_the_commander_is_asked_which_opponent_only_with_several() {
        covers!("silent::g3::mentak_commander::SULA");
        let (system, _) = two_planet_system();
        // Two opponents with a note each: the commander picks one, the opponent gives.
        let holds: &'static [(u8, PromissoryNote, Faction)] = &[(1, PromissoryNote::Ceasefire, Faction::TheFederationOfSol), (2, PromissoryNote::Ceasefire, Faction::TheEmiratesOfHacan)];
        let mut r = rig(Scenario::new(&FACTIONS), edit(holds), won(system, &[1, 2]));
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the commander");
        r.pick(use_it);
        assert_eq!(r.asked().ask, Ask::Reaction(RAsk::G(GAsk::T(TAsk::SulaOpponent))), "{}", r.transcript());
        assert_eq!(r.opts(), vec![Opt::Reaction(ROpt::G(GOpt::T(TOpt::Seat(Seat(1))))), Opt::Reaction(ROpt::G(GOpt::T(TOpt::Seat(Seat(2)))))]);
        r.pick(0);
        assert_eq!((r.asked().actor, r.asked().ask), (Seat(1), Ask::Reaction(RAsk::G(GAsk::T(TAsk::SulaGive { to: Seat(0) })))), "{}", r.transcript());
        // The hand: the notes held and not faceup, then the opponent's own Support for the Throne.
        assert!(r.opts().contains(&Opt::Reaction(ROpt::G(GOpt::T(TOpt::Note(note(PromissoryNote::Ceasefire, Faction::TheFederationOfSol)))))));
        assert_eq!(r.opts().last(), Some(&Opt::Reaction(ROpt::G(GOpt::T(TOpt::Support(Faction::TheEmiratesOfHacan))))));
        let given = r.opts().iter().position(|o| *o == Opt::Reaction(ROpt::G(GOpt::T(TOpt::Note(note(PromissoryNote::Ceasefire, Faction::TheFederationOfSol)))))).unwrap();
        r.pick(given);
        assert!(r.is_done(), "{}", r.transcript());
        assert_eq!(r.state.cards().promissory.get(&note(PromissoryNote::Ceasefire, Faction::TheFederationOfSol)), Some(&Seat(0)), "taken");
    }

    #[test]
    fn a_support_for_the_throne_scores_a_point_on_arrival_and_one_opponent_is_not_asked_which() {
        covers!("silent::g3::mentak_commander::SULA");
        let (system, _) = two_planet_system();
        let mut r = rig(Scenario::new(&FACTIONS), edit(&[]), won(system, &[1]));
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the commander");
        r.pick(use_it);
        // One opponent: straight to the note (the opponent's own Support for the Throne is in its hand).
        assert_eq!(r.asked().actor, Seat(1), "{}", r.transcript());
        let support = r.opts().iter().position(|o| matches!(o, Opt::Reaction(ROpt::G(GOpt::T(TOpt::Support(_)))))).expect("its Support for the Throne");
        let before = r.state.players().by_seat(Seat(0)).unwrap().econ.victory_points;
        r.pick(support);
        assert!(r.is_done());
        assert_eq!(r.state.players().by_seat(Seat(0)).unwrap().econ.victory_points, before + 1);
        assert_eq!(r.state.trade().support_holders.get(&Seat(1)), Some(&Seat(0)));
    }

    #[test]
    fn it_needs_the_unlocked_commander_the_winner_and_an_opponent_with_a_note() {
        covers!("silent::g3::mentak_commander::SULA");
        let (system, _) = two_planet_system();
        // Locked: nobody is asked.
        assert!(rig(Scenario::new(&FACTIONS), |_| {}, won(system, &[1])).is_done());
        // The Support is lent already and no note is held: nothing to give.
        let nothing = |s: &mut CoreState| {
            edit(&[])(s);
            s.trade_mut().support_holders.insert(Seat(1), Seat(2));
        };
        assert!(rig(Scenario::new(&FACTIONS), nothing, won(system, &[1])).is_done());
        // Another seat won.
        let other = fx::event(EventKind::SpaceCombatWon, Payload::new().with(Key::Player, Val::Seat(Seat(1))).with(Key::System, Val::System(system)));
        assert!(rig(Scenario::new(&FACTIONS), edit(&[]), other).is_done());
    }
}

// -- the edges the first mutation run found ---------------------------------------------------------------------------------------

mod edges {
    use super::*;
    use ti4_engine_core::generated::{ActionCard, Leader, Technology};
    use ti4_engine_core::kernel::ability::Unknown;
    use ti4_engine_core::state::{LeaderStatus, PlanetFlags};

    fn rig_without_groups(scenario: Scenario, state_edit: impl FnOnce(&mut CoreState), event: ti4_engine_core::state::EventRec) -> Rig<EmitEvent> {
        let (mut state, kernel) = scenario.build();
        state_edit(&mut state);
        let mut t = AbilityTable::default();
        ti4_engine_core_factions::silent::register_before_movement(&mut t);
        ti4_engine_core_factions::silent::g3::register_after_ca_combat(&mut t);
        ti4_engine_core_factions::silent::register_abilities(&mut t);
        let mut r = Rig::from_parts(state, kernel, EmitEvent::new(event)).with_abilities(t);
        r.run();
        r
    }

    #[test]
    fn scavenger_zeta_is_possible_with_exactly_the_one_good_scavenge_paid() {
        covers!("silent::g3::zeta::ZETA");
        let (system, planets) = two_planet_system();
        let mut r = rig(Scenario::new(&[Faction::TheClanOfSaar, Faction::TheEmiratesOfHacan]).control(planets[0], 0), |_| {}, gained(0, system, planets[0]));
        // (No good in hand: Scavenge pays one first, so the deploy is possible with exactly one and leaves none.)
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the deploy");
        r.pick(use_it);
        assert!(r.is_done(), "{}", r.transcript());
        assert_eq!((units_of(&r, planets[0], 0, UnitBaseType::Mech), goods(&r, 0)), (1, 0));
    }

    #[test]
    fn the_alastor_counts_only_the_owners_ground_forces_and_only_its_owners_flagship() {
        covers!("silent::g3::alastor::ALASTOR");
        use ti4_engine_core::choice::reaction::Ask as RAsk;
        use ti4_engine_core::choice::silent::Ask as GAsk;
        use ti4_engine_core::choice::silent3::Ask as TAsk;
        let (system, planets) = two_planet_system();
        let started = fx::event(EventKind::SpaceCombatStarted, Payload::new().with(Key::System, Val::System(system)).with(Key::Attacker, Val::Seat(Seat(0))).with(Key::Defender, Val::Seat(Seat(1))));
        // A structure of the owner and a ground force of the other player are no choice.
        let scenario = Scenario::new(&[Faction::TheNekroVirus, Faction::TheEmiratesOfHacan])
            .unit(system, None, Unit::TheAlastor, 0, 1)
            .unit(system, Some(planets[0]), Unit::InfantryI, 0, 1)
            .unit(system, Some(planets[0]), Unit::PDSI, 0, 1)
            .unit(system, Some(planets[0]), Unit::InfantryI, 1, 3)
            .unit(system, Some(planets[1]), Unit::InfantryI, 1, 1);
        let mut r = rig(scenario, |_| {}, started.clone());
        let mut asked = 0;
        while !r.is_done() {
            assert!(matches!(r.asked().ask, Ask::Reaction(RAsk::G(GAsk::T(TAsk::Alastor { planet, unit: Unit::InfantryI }))) if planet == planets[0]), "{}", r.transcript());
            assert_eq!(r.opts().len(), 2, "one infantry: 'one' or none");
            asked += 1;
            r.pick(0);
        }
        assert_eq!(asked, 1);
        // Another player's Alastor in the system is no flagship of the owner's.
        let theirs = Scenario::new(&[Faction::TheNekroVirus, Faction::TheEmiratesOfHacan]).unit(system, None, Unit::TheAlastor, 1, 1).unit(system, None, Unit::CruiserI, 0, 1).unit(system, Some(planets[0]), Unit::InfantryI, 0, 1);
        assert!(rig(theirs, |_| {}, started.clone()).is_done());
        // No system in the event.
        let nowhere = fx::event(EventKind::SpaceCombatStarted, Payload::new().with(Key::Attacker, Val::Seat(Seat(0))).with(Key::Defender, Val::Seat(Seat(1))));
        let own = Scenario::new(&[Faction::TheNekroVirus, Faction::TheEmiratesOfHacan]).unit(system, None, Unit::TheAlastor, 0, 1).unit(system, Some(planets[0]), Unit::InfantryI, 0, 1);
        assert!(rig(own, |_| {}, nowhere).is_done());
    }

    #[test]
    fn exotrireme_ii_needs_the_unit_itself_exactly_one_other_ship_is_enough_and_the_destruction_is_a_combat_one() {
        covers!("silent::g3::exo2::EXO2");
        let (system, _) = two_planet_system();
        let ended = fx::event(EventKind::SpaceCombatRoundEnded, Payload::new().with(Key::System, Val::System(system)).with(Key::Round, Val::Int(1)).with(Key::Attacker, Val::Seat(Seat(0))).with(Key::Defender, Val::Seat(Seat(1))));
        // The technology and a cruiser of the owner, but no Exotrireme II: nothing.
        let no_unit = Scenario::new(&[Faction::SardakkNorr, Faction::TheEmiratesOfHacan]).tech(0, Technology::ExotriremeII).unit(system, None, Unit::CruiserI, 0, 2).unit(system, None, Unit::CruiserI, 1, 1);
        assert!(rig(no_unit, |_| {}, ended.clone()).is_done());
        // The unit and exactly one other ship (an enemy's): the ability is there.
        let two = Scenario::new(&[Faction::SardakkNorr, Faction::TheEmiratesOfHacan]).tech(0, Technology::ExotriremeII).unit(system, None, Unit::ExotriremeII, 0, 1).unit(system, None, Unit::CruiserI, 1, 1);
        let mut r = rig(two, |_| {}, ended);
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("exactly two ships are enough");
        r.pick(use_it);
        r.pick(0);
        assert!(r.is_done(), "{}", r.transcript());
        let staged = &r.state.transient().combat.pending_destructions;
        assert_eq!(staged.len(), 2);
        assert!(staged.iter().all(|d| d.during_space_combat && d.system == system), "destroyed during the space combat");
    }

    #[test]
    fn ragh_s_call_leaves_the_other_players_ground_forces_and_marks_the_destination_entered() {
        covers!("silent::g3::ragh::RAGH");
        let (system, planets) = two_planet_system();
        let committed = fx::event(EventKind::UnitsCommitted, Payload::new().with(Key::Player, Val::Seat(Seat(0))).with(Key::Planet, Val::Planet(planets[0])).with(Key::System, Val::System(system)));
        let scenario = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheClanOfSaar, Faction::TheFederationOfSol])
            .control(planets[0], 1)
            .control(planets[1], 1)
            .unit(system, Some(planets[0]), Unit::InfantryI, 1, 2)
            .unit(system, Some(planets[0]), Unit::InfantryI, 2, 1);
        let mut r = rig(scenario, |s| { s.cards_mut().promissory.edit().insert(ti4_engine_core::state::NoteKey { note: ti4_engine_core::generated::PromissoryNote::RaghsCall, faction: Faction::TheClanOfSaar }, Seat(0)); }, committed);
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the note");
        r.pick(use_it);
        r.pick(0);
        assert!(r.is_done(), "{}", r.transcript());
        assert_eq!((units_of(&r, planets[0], 1, UnitBaseType::Infantry), units_of(&r, planets[0], 2, UnitBaseType::Infantry)), (0, 1), "only the Saar's forces left");
        assert_eq!(units_of(&r, planets[1], 1, UnitBaseType::Infantry), 2);
        assert!(r.state.board().planet(planets[1]).flags.has(PlanetFlags::UNITS_ENTRY), "the destination has a unit entry now");
    }

    #[test]
    fn ragh_s_call_needs_saar_ground_forces_on_the_planet_and_touches_the_system_it_goes_to() {
        covers!("silent::g3::ragh::RAGH");
        let (system, planets) = two_planet_system();
        let other = ti4_engine_core_tests::scenario::plain_systems().into_iter().find(|s| *s != system).expect("another plain system");
        let far = other.data().planets[0];
        let committed = fx::event(EventKind::UnitsCommitted, Payload::new().with(Key::Player, Val::Seat(Seat(0))).with(Key::Planet, Val::Planet(planets[0])).with(Key::System, Val::System(system)));
        let hold = |s: &mut CoreState| {
            s.cards_mut().promissory.edit().insert(ti4_engine_core::state::NoteKey { note: ti4_engine_core::generated::PromissoryNote::RaghsCall, faction: Faction::TheClanOfSaar }, Seat(0));
        };
        // Only the Sol has a ground force there (the Saar only a structure): nothing to move.
        let nobody = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheClanOfSaar, Faction::TheFederationOfSol])
            .control(planets[0], 1)
            .control(far, 1)
            .unit(system, Some(planets[0]), Unit::PDSI, 1, 1)
            .unit(system, Some(planets[0]), Unit::InfantryI, 2, 2);
        assert!(rig(nobody, hold, committed.clone()).is_done());
        // The destination is in another system: that system now has an entry on the board.
        let scenario = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheClanOfSaar]).control(planets[0], 1).control(far, 1).unit(system, Some(planets[0]), Unit::InfantryI, 1, 1);
        let mut r = rig(scenario, hold, committed);
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the note");
        r.pick(use_it);
        r.pick(0);
        assert_eq!(units_of(&r, far, 1, UnitBaseType::Infantry), 1);
        assert!(r.state.board().sys(other).flags.has(ti4_engine_core::state::SysFlags::TOUCHED));
    }

    fn suffi_scenario(system: System, planets: &[Planet]) -> Scenario {
        Scenario::new(&[Faction::TheMentakCoalition, Faction::TheEmiratesOfHacan, Faction::TheYssarilTribes]).map_from(&[system]).unit(system, Some(planets[0]), Unit::InfantryI, 0, 1).unit(system, Some(planets[1]), Unit::InfantryI, 1, 1).goods(1, 3)
    }

    fn set_up(agent: Option<LeaderStatus>, ssruu: Option<LeaderStatus>, deck: bool) -> impl FnOnce(&mut CoreState) {
        move |s| {
            if let Some(status) = agent {
                s.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::SuffiAnMentakagent, status);
            }
            if let Some(status) = ssruu {
                s.players_mut().by_seat_mut(Seat(2)).unwrap().leaders.status.insert(Leader::Ssruu, status);
            }
            s.decks_mut().action.set(if deck { vec![ActionCard::ALL[0], ActionCard::ALL[1], ActionCard::ALL[2]] } else { vec![] });
        }
    }

    fn pillaged(r: &mut Rig<EmitEvent>) {
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("pillage");
        r.pick(use_it);
    }

    #[test]
    fn suffi_an_is_announced_only_where_a_copy_could_answer_and_the_copy_needs_a_lender_a_borrower_and_a_deck() {
        covers!("silent::g3::suffi_an::SUFFI_AN");
        covers!("silent::g3::suffi_an::SUFFI_AN_COPY");
        let (system, planets) = two_planet_system();
        let gained = fx::event(EventKind::TradeGoodsGained, Payload::new().with(Key::Player, Val::Seat(Seat(1))));
        // Both a ready agent and a ready Ssruu: the use is announced, the owner is asked first.
        let mut r = rig(suffi_scenario(system, &planets), set_up(Some(LeaderStatus::Readied), Some(LeaderStatus::Readied), true), gained.clone());
        pillaged(&mut r);
        assert_eq!(r.asked().actor, Seat(0), "{}", r.transcript());
        r.decline();
        assert_eq!(r.asked().actor, Seat(2), "the copy answers the announced use: {}", r.transcript());
        // No Ssruu in hand: the owner is asked and that is all.
        let mut alone = rig(suffi_scenario(system, &planets), set_up(Some(LeaderStatus::Readied), None, true), gained.clone());
        pillaged(&mut alone);
        assert_eq!(alone.asked().actor, Seat(0), "{}", alone.transcript());
        alone.decline();
        assert!(alone.is_done(), "{}", alone.transcript());
        // An exhausted agent is lent all the same: the copy is asked, the owner is not.
        let mut lent = rig(suffi_scenario(system, &planets), set_up(Some(LeaderStatus::Exhausted), Some(LeaderStatus::Readied), true), gained.clone());
        pillaged(&mut lent);
        assert_eq!(lent.asked().actor, Seat(2), "{}", lent.transcript());
        // A locked agent, an exhausted agent beside an exhausted Ssruu, an empty deck: nothing to ask.
        for (agent, ssruu, deck) in [(LeaderStatus::Locked, LeaderStatus::Readied, true), (LeaderStatus::Exhausted, LeaderStatus::Exhausted, true), (LeaderStatus::Readied, LeaderStatus::Readied, false)] {
            let mut none = rig(suffi_scenario(system, &planets), set_up(Some(agent), Some(ssruu), deck), gained.clone());
            pillaged(&mut none);
            assert!(none.is_done(), "{agent:?} {ssruu:?} {deck}: {}", none.transcript());
        }
    }

    #[test]
    fn a_draw_leaves_the_unofficial_cards_in_the_deck_and_a_table_without_groups_is_a_loud_gap() {
        covers!("silent::g3::nekro_commander::ACIDOS");
        covers!("silent::g3::draw::draw");
        let gained = fx::event(EventKind::TechnologyGained, Payload::new().with(Key::Player, Val::Seat(Seat(0))));
        let edit = |s: &mut CoreState| {
            s.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::NekroAcidos, LeaderStatus::Unlocked);
            s.decks_mut().action.set(vec![ActionCard::Veto3, ActionCard::ALL[0]]);
        };
        let mut r = rig(Scenario::new(&[Faction::TheNekroVirus, Faction::TheEmiratesOfHacan]), edit, gained.clone());
        r.pick(0);
        assert_eq!(r.state.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice(), &[ActionCard::ALL[0]], "the unofficial card did not come");
        // A Yssaril copy draws with Scheming: its discard needs the printed-name groups; without them the draw is a gap (and nothing is drawn).
        let (system, planets) = two_planet_system();
        let pillage = fx::event(EventKind::TradeGoodsGained, Payload::new().with(Key::Player, Val::Seat(Seat(1))));
        let mut r = rig_without_groups(suffi_scenario(system, &planets), set_up(Some(LeaderStatus::Exhausted), Some(LeaderStatus::Readied), true), pillage);
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("pillage");
        r.pick(use_it);
        let copy = r.opts().iter().position(|o| *o != Opt::Decline).expect("the copy");
        r.pick(copy);
        assert_eq!(r.driver.unknown(), Some(Unknown::AbilityGap(ti4_engine_core_factions::silent::g3::suffi_an::SUFFI_AN_COPY)));
        // The fine path: with the groups the same draw is asked, not a gap.
        let mut ok = rig(suffi_scenario(system, &planets), set_up(Some(LeaderStatus::Exhausted), Some(LeaderStatus::Readied), true), fx::event(EventKind::TradeGoodsGained, Payload::new().with(Key::Player, Val::Seat(Seat(1)))));
        let use_it = ok.opts().iter().position(|o| *o != Opt::Decline).expect("pillage");
        ok.pick(use_it);
        let copy = ok.opts().iter().position(|o| *o != Opt::Decline).expect("the copy");
        ok.pick(copy);
        assert_eq!(ok.driver.unknown(), None);
    }
}

// -- Bioplasmosis, the Oracle and Sh'val's skip --------------------------------------------------------------------------------------

mod status {
    use super::*;
    use ti4_engine_core::choice::reaction::{Ask as RAsk, Opt as ROpt};
    use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt};
    use ti4_engine_core::choice::silent3::{Ask as TAsk, Opt as TOpt};
    use ti4_engine_core::generated::{Leader, PromissoryNote, Technology};
    use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
    use ti4_engine_core::state::{LeaderStatus, NoteKey};

    fn status_phase_ended() -> ti4_engine_core::state::EventRec {
        fx::event(EventKind::StatusPhaseEnded, Payload::new())
    }

    #[test]
    fn bioplasmosis_moves_one_infantry_per_question_until_the_owner_stops() {
        covers!("silent::g3::bio::BIO");
        let (system, planets) = two_planet_system();
        let scenario = || Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan]).tech(0, Technology::Bioplasmosis).control(planets[0], 0).control(planets[1], 0).unit(system, Some(planets[0]), Unit::InfantryI, 0, 3);
        let mut r = rig(scenario(), |_| {}, status_phase_ended());
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the technology");
        r.pick(use_it);
        // Only the planet with infantry can give: one move to the other planet, or stop.
        assert_eq!(r.asked().ask, Ask::Reaction(RAsk::G(GAsk::T(TAsk::Bio { so_far: 0 }))), "{}", r.transcript());
        assert_eq!(r.opts(), vec![Opt::Reaction(ROpt::G(GOpt::T(TOpt::Shift { index: 0, from: (system, planets[0]), to: (system, planets[1]) }))), Opt::Decline]);
        r.pick(0);
        assert_eq!(r.asked().ask, Ask::Reaction(RAsk::G(GAsk::T(TAsk::Bio { so_far: 1 }))));
        // Nothing has moved yet.
        assert_eq!(units_of(&r, planets[0], 0, UnitBaseType::Infantry), 3);
        r.pick(0);
        r.decline();
        assert!(r.is_done(), "{}", r.transcript());
        assert_eq!((units_of(&r, planets[0], 0, UnitBaseType::Infantry), units_of(&r, planets[1], 0, UnitBaseType::Infantry)), (1, 2));
        // Declined at the window: nothing moves.
        let mut declined = rig(scenario(), |_| {}, status_phase_ended());
        declined.decline();
        assert_eq!(units_of(&declined, planets[0], 0, UnitBaseType::Infantry), 3);
    }

    #[test]
    fn bioplasmosis_needs_the_technology_infantry_and_another_planet_in_reach() {
        covers!("silent::g3::bio::BIO");
        let (system, planets) = two_planet_system();
        let base = || Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan]).control(planets[0], 0).control(planets[1], 0).unit(system, Some(planets[0]), Unit::InfantryI, 0, 2);
        assert!(rig(base(), |_| {}, status_phase_ended()).is_done(), "no technology");
        // Only one controlled planet: nowhere to go.
        let lone = Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan]).tech(0, Technology::Bioplasmosis).control(planets[0], 0).unit(system, Some(planets[0]), Unit::InfantryI, 0, 2);
        assert!(rig(lone, |_| {}, status_phase_ended()).is_done(), "one planet");
        // A planet in a system out of reach (no map, a different system): not a destination.
        let other = ti4_engine_core_tests::scenario::plain_systems().into_iter().find(|s| *s != system).expect("another plain system");
        let far = Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan]).tech(0, Technology::Bioplasmosis).control(planets[0], 0).control(other.data().planets[0], 0).unit(system, Some(planets[0]), Unit::InfantryI, 0, 2);
        assert!(rig(far, |_| {}, status_phase_ended()).is_done(), "not the same or an adjacent system");
        // The infantry that is not the owner's does not count.
        let theirs = Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan]).tech(0, Technology::Bioplasmosis).control(planets[0], 0).control(planets[1], 0).unit(system, Some(planets[0]), Unit::InfantryI, 1, 2);
        assert!(rig(theirs, |_| {}, status_phase_ended()).is_done(), "only the owner's infantry");
    }

    fn note(n: PromissoryNote, faction: Faction) -> NoteKey {
        NoteKey { note: n, faction }
    }

    fn oracle_edit(s: &mut CoreState) {
        s.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::TheOracle, LeaderStatus::Unlocked);
        s.cards_mut().promissory.edit().insert(note(PromissoryNote::Ceasefire, Faction::TheFederationOfSol), Seat(1));
        s.cards_mut().promissory.edit().insert(note(PromissoryNote::Ceasefire, Faction::TheArborec), Seat(1));
        s.cards_mut().promissory.edit().insert(note(PromissoryNote::Ceasefire, Faction::TheEmiratesOfHacan), Seat(2));
    }

    #[test]
    fn the_oracle_takes_a_note_from_every_other_player_and_is_purged() {
        covers!("silent::g3::oracle::ORACLE");
        let factions = [Faction::TheNaaluCollective, Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol];
        // Seats 1 and 2 hold notes (own notes are held at the start of a game; the scenario deals none).
        let mut r = rig(Scenario::new(&factions), oracle_edit, status_phase_ended());
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the hero");
        r.pick(use_it);
        // Seat 1 holds two notes: it chooses. Seat 2 holds one: it gives it without a question.
        assert_eq!((r.asked().actor, r.asked().ask), (Seat(1), Ask::Reaction(RAsk::G(GAsk::T(TAsk::OracleGive)))), "{}", r.transcript());
        assert_eq!(r.opts().len(), 2);
        r.pick(1);
        assert!(r.is_done(), "{}", r.transcript());
        let given = |r: &Rig<EmitEvent>, key: NoteKey| r.state.cards().promissory.get(&key).copied();
        assert_eq!(given(&r, note(PromissoryNote::Ceasefire, Faction::TheEmiratesOfHacan)), Some(Seat(0)), "the lone note of seat 2");
        let first = r.state.cards().promissory.iter().filter(|(k, h)| **h == Seat(0) && k.faction != Faction::TheEmiratesOfHacan).count();
        assert_eq!(first, 1, "one of the two of seat 1");
        assert_eq!(r.state.players().by_seat(Seat(0)).unwrap().leaders.status.get(&Leader::TheOracle), Some(&LeaderStatus::Purged));
        // Locked, or nobody has a note: not offered.
        assert!(rig(Scenario::new(&factions), |s| { oracle_edit(s); s.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::TheOracle, LeaderStatus::Locked); }, status_phase_ended()).is_done());
        assert!(rig(Scenario::new(&factions), |s| { s.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::TheOracle, LeaderStatus::Unlocked); }, status_phase_ended()).is_done());
    }

    #[test]
    fn a_skip_that_never_reached_its_commitment_is_forgotten_when_a_turn_begins() {
        covers!("silent::g3::tro_marks::HERO_TURN");
        let (system, _) = two_planet_system();
        let skip = |s: &mut CoreState| {
            s.marks_mut().tail.edit().insert(MarkKey::SardakkHeroSkip(), MarkVal::Rec(vec![MarkVal::Atom(Atom::Seat(Seat(0))), MarkVal::Atom(Atom::System(system))]));
        };
        let turn = || fx::event(EventKind::TurnBegan, Payload::new().with(Key::Player, Val::Seat(Seat(1))));
        let r = rig(Scenario::new(&[Faction::SardakkNorr, Faction::TheEmiratesOfHacan]), skip, turn());
        assert!(r.is_done() && r.state.marks().tail.get(&MarkKey::SardakkHeroSkip()).is_none());
        // Whatever seats are at the table: the mark goes (the string registers it for every seat).
        let r = rig(Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol]), skip, turn());
        assert!(r.state.marks().tail.get(&MarkKey::SardakkHeroSkip()).is_none());
    }
}

// -- Scepter of Dominion ---------------------------------------------------------------------------------------------------------

mod scepter {
    use super::*;
    use ti4_engine_core::choice::reaction::{Ask as RAsk, Opt as ROpt};
    use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt};
    use ti4_engine_core::choice::silent3::{Ask as TAsk, Opt as TOpt};
    use ti4_engine_core::generated::PromissoryNote;
    use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
    use ti4_engine_core::state::NoteKey;

    const NOTE: NoteKey = NoteKey { note: PromissoryNote::ScepterOfDominion, faction: Faction::TheMahactGeneSorcerers };
    const FACTIONS: [Faction; 3] = [Faction::TheMahactGeneSorcerers, Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol];

    fn begun() -> ti4_engine_core::state::EventRec {
        fx::event(EventKind::StrategyPhaseBegan, Payload::new())
    }

    /// Seat 1 holds the Scepter; the Mahact (seat 0) has seat 2's token on its sheet.
    fn edit(pool: &'static [u8]) -> impl FnOnce(&mut CoreState) {
        move |s| {
            s.cards_mut().promissory.edit().insert(NOTE, Seat(1));
            s.marks_mut().tail.edit().insert(MarkKey::MahactFleet(Seat(0)), MarkVal::List(pool.iter().map(|p| MarkVal::Atom(Atom::Seat(Seat(*p)))).collect()));
        }
    }

    fn two_systems() -> (System, System) {
        let mut plain = ti4_engine_core_tests::scenario::plain_systems().into_iter();
        (plain.next().unwrap(), plain.next().unwrap())
    }

    #[test]
    fn the_holder_chooses_a_system_with_its_units_and_the_pool_owners_place_a_token_there() {
        covers!("silent::g3::scepter::SCEPTER");
        let (a, b) = two_systems();
        let scenario = || Scenario::new(&FACTIONS).unit(a, None, Unit::CruiserI, 1, 1).unit(b, None, Unit::CruiserI, 1, 1);
        let mut r = rig(scenario(), edit(&[2]), begun());
        assert_eq!((r.asked().actor, r.asked().ask), (Seat(1), Ask::Reaction(RAsk::G(GAsk::T(TAsk::Scepter)))), "{}", r.transcript());
        assert_eq!(r.opts().len(), 2);
        assert!(matches!(r.opts()[0], Opt::Reaction(ROpt::G(GOpt::T(TOpt::Scepter { system, payers }))) if system == a.min(b) && payers[0] == Some(Faction::TheFederationOfSol)));
        let second = r.opts().iter().position(|o| matches!(o, Opt::Reaction(ROpt::G(GOpt::T(TOpt::Scepter { system, .. }))) if *system == a.max(b))).unwrap();
        r.pick(second);
        assert!(r.is_done(), "{}", r.transcript());
        let chosen = a.max(b);
        assert!(r.state.board().sys(chosen).cmd.contains(Seat(2)), "the pool's owner placed a token");
        assert!(!r.state.board().sys(a.min(b)).cmd.contains(Seat(2)));
        assert_eq!(r.state.cards().promissory.get(&NOTE), Some(&Seat(0)), "back with the Mahact");
    }

    #[test]
    fn one_system_is_not_asked_none_still_returns_the_card_and_a_placed_token_or_no_reserve_places_none() {
        covers!("silent::g3::scepter::SCEPTER");
        let (a, _) = two_systems();
        let one = rig(Scenario::new(&FACTIONS).unit(a, None, Unit::CruiserI, 1, 1), edit(&[2, 2]), begun());
        assert!(one.is_done(), "{}", one.transcript());
        assert!(one.state.board().sys(a).cmd.contains(Seat(2)));
        assert_eq!(one.state.cards().promissory.get(&NOTE), Some(&Seat(0)));
        // No unit outside a home system: the card goes home all the same.
        let none = rig(Scenario::new(&FACTIONS), edit(&[2]), begun());
        assert!(none.is_done());
        assert_eq!(none.state.cards().promissory.get(&NOTE), Some(&Seat(0)));
        // The holder's own token in the pool does not pay; a token already there does not pay twice.
        let own = rig(Scenario::new(&FACTIONS).unit(a, None, Unit::CruiserI, 1, 1).command_token(a, 2), edit(&[1, 2]), begun());
        assert!(!own.state.board().sys(a).cmd.contains(Seat(1)));
        assert!(own.state.board().sys(a).cmd.contains(Seat(2)));
        // Nobody holds the note: nothing is asked.
        assert!(rig(Scenario::new(&FACTIONS).unit(a, None, Unit::CruiserI, 1, 1), |_| {}, begun()).is_done());
    }
}

mod more_edges {
    use super::*;
    use ti4_engine_core::generated::{PromissoryNote, Technology};
    use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
    use ti4_engine_core::state::NoteKey;

    #[test]
    fn bioplasmosis_moves_the_owners_infantry_only_and_not_more_than_there_are() {
        covers!("silent::g3::bio::BIO");
        let (system, planets) = two_planet_system();
        // Another player's infantry stands first on the planet; the owner has exactly one.
        let scenario = Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan])
            .tech(0, Technology::Bioplasmosis)
            .control(planets[0], 0)
            .control(planets[1], 0)
            .unit(system, Some(planets[0]), Unit::InfantryI, 1, 1)
            .unit(system, Some(planets[0]), Unit::InfantryI, 0, 1);
        let mut r = rig(scenario, |_| {}, fx::event(EventKind::StatusPhaseEnded, Payload::new()));
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the technology");
        r.pick(use_it);
        r.pick(0);
        // The only infantry is moved: nothing more is asked.
        assert!(r.is_done(), "{}", r.transcript());
        assert_eq!((units_of(&r, planets[0], 0, UnitBaseType::Infantry), units_of(&r, planets[1], 0, UnitBaseType::Infantry)), (0, 1));
        assert_eq!(units_of(&r, planets[0], 1, UnitBaseType::Infantry), 1, "the other player's infantry stays");
        assert!(r.state.board().planet(planets[1]).flags.has(ti4_engine_core::state::PlanetFlags::UNITS_ENTRY));
    }

    const NOTE: NoteKey = NoteKey { note: PromissoryNote::ScepterOfDominion, faction: Faction::TheMahactGeneSorcerers };
    const FACTIONS: [Faction; 3] = [Faction::TheMahactGeneSorcerers, Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol];

    fn begun() -> ti4_engine_core::state::EventRec {
        fx::event(EventKind::StrategyPhaseBegan, Payload::new())
    }

    fn pool(holder: u8) -> impl FnOnce(&mut CoreState) {
        move |s| {
            s.cards_mut().promissory.edit().insert(NOTE, Seat(holder));
            s.marks_mut().tail.edit().insert(MarkKey::MahactFleet(Seat(0)), MarkVal::List(vec![MarkVal::Atom(Atom::Seat(Seat(2)))]));
        }
    }

    #[test]
    fn the_scepter_counts_units_on_planets_needs_the_holder_not_the_mahact_and_a_payer_with_a_token_to_place() {
        covers!("silent::g3::scepter::SCEPTER");
        let (system, planets) = two_planet_system();
        // Units on a planet make a system the holder's; another player's units do not.
        let on_planet = rig(Scenario::new(&FACTIONS).unit(system, Some(planets[0]), Unit::InfantryI, 1, 1), pool(1), begun());
        assert!(on_planet.state.board().sys(system).cmd.contains(Seat(2)), "a planet counts");
        let theirs = rig(Scenario::new(&FACTIONS).unit(system, Some(planets[0]), Unit::InfantryI, 2, 1), pool(1), begun());
        assert!(!theirs.state.board().sys(system).cmd.contains(Seat(2)), "another player's units do not");
        // The Mahact holding its own note plays nothing.
        let own = rig(Scenario::new(&FACTIONS).unit(system, Some(planets[0]), Unit::InfantryI, 0, 1), pool(0), begun());
        assert!(!own.state.board().sys(system).cmd.contains(Seat(2)));
        // A payer with no token in reinforcements places none (the whole box is on the sheet).
        let broke = rig(Scenario::new(&FACTIONS).unit(system, Some(planets[0]), Unit::InfantryI, 1, 1).tokens(2, 8, 4, 4), pool(1), begun());
        assert!(!broke.state.board().sys(system).cmd.contains(Seat(2)), "no token left to place");
        assert_eq!(broke.state.cards().promissory.get(&NOTE), Some(&Seat(0)), "the card goes home all the same");
    }
}

// -- Creuss IFF --------------------------------------------------------------------------------------------------------------------

mod iff {
    use super::*;
    use ti4_engine_core::choice::reaction::{Ask as RAsk, Opt as ROpt};
    use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt};
    use ti4_engine_core::choice::silent3::{Ask as TAsk, Opt as TOpt};
    use ti4_engine_core::generated::{PromissoryNote, Wormhole};
    use ti4_engine_core::state::{NoteKey, Phase};

    const NOTE: NoteKey = NoteKey { note: PromissoryNote::CreussIff, faction: Faction::TheGhostsOfCreuss };

    fn turn(seat: u8) -> ti4_engine_core::state::EventRec {
        fx::event(EventKind::TurnBegan, Payload::new().with(Key::Player, Val::Seat(Seat(seat))))
    }

    fn holds(seat: u8) -> impl FnOnce(&mut CoreState) {
        move |s| {
            s.cards_mut().promissory.edit().insert(NOTE, Seat(seat));
        }
    }

    /// Three plain systems on the map: `a` has a planet of seat 0, `b` holds seat 1's cruiser, `c` is empty.
    fn scenario() -> (Scenario, System, System, System) {
        let mut plain = ti4_engine_core_tests::scenario::plain_systems().into_iter();
        let (a, b, c) = (plain.next().unwrap(), plain.next().unwrap(), plain.next().unwrap());
        let s = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheGhostsOfCreuss]).phase(Phase::Action).map_from(&[a, b, c]).control(a.data().planets[0], 0).unit(b, None, Unit::CruiserI, 1, 1);
        (s, a, b, c)
    }

    #[test]
    fn the_holder_places_a_token_into_a_system_it_controls_or_an_empty_non_home_one_and_the_note_goes_home() {
        covers!("silent::g3::iff::IFF");
        let (scenario, a, b, c) = scenario();
        let mut r = rig(scenario, holds(0), turn(0));
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the note");
        r.pick(use_it);
        assert_eq!(r.asked().ask, Ask::Reaction(RAsk::G(GAsk::T(TAsk::Iff))), "{}", r.transcript());
        // Systems a (controlled) and c (empty, no home): both tokens each; b holds another player's ship.
        let offered: Vec<(Wormhole, System)> = r.opts().iter().filter_map(|o| if let Opt::Reaction(ROpt::G(GOpt::T(TOpt::Token { kind, system }))) = o { Some((*kind, *system)) } else { None }).collect();
        assert_eq!(offered.len(), 4, "{offered:?}");
        assert!(offered.iter().all(|(_, s)| *s == a || *s == c) && !offered.iter().any(|(_, s)| *s == b));
        assert!(offered.iter().take(2).all(|(k, _)| *k == Wormhole::Alpha) && offered.iter().skip(2).all(|(k, _)| *k == Wormhole::Beta), "alpha first, then beta");
        r.pick(1);
        assert!(r.is_done(), "{}", r.transcript());
        assert_eq!(r.state.explore().wormhole_tokens.get(&offered[1].0), Some(&offered[1].1));
        assert_eq!(r.state.cards().promissory.get(&NOTE), Some(&Seat(1)), "back with the Creuss");
    }

    #[test]
    fn a_token_already_in_a_system_is_not_offered_there_and_the_conditions_hold() {
        covers!("silent::g3::iff::IFF");
        let (scenario, a, _, c) = scenario();
        let mut r = rig(scenario, |s| { holds(0)(s); s.explore_mut().wormhole_tokens.insert(Wormhole::Alpha, a); }, turn(0));
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the note");
        r.pick(use_it);
        assert_eq!(r.opts().len(), 3, "alpha is in a already");
        assert!(!r.opts().contains(&Opt::Reaction(ROpt::G(GOpt::T(TOpt::Token { kind: Wormhole::Alpha, system: a })))));
        assert!(r.opts().contains(&Opt::Reaction(ROpt::G(GOpt::T(TOpt::Token { kind: Wormhole::Alpha, system: c })))));
        // Another seat's turn, the status phase, no holder, the Creuss itself: nothing.
        let (scenario, ..) = scenario_with(Phase::Action);
        assert!(rig(scenario, holds(0), turn(1)).is_done(), "another seat's turn");
        let (scenario, ..) = scenario_with(Phase::Status);
        assert!(rig(scenario, holds(0), turn(0)).is_done(), "the action phase only");
        let (scenario, ..) = scenario_with(Phase::Action);
        assert!(rig(scenario, |_| {}, turn(0)).is_done(), "not held");
        let (scenario, ..) = scenario_with(Phase::Action);
        assert!(rig(scenario, holds(1), turn(1)).is_done(), "the Creuss never plays its own note");
    }

    fn scenario_with(phase: Phase) -> (Scenario, (), (), ()) {
        let (s, ..) = scenario();
        (s.phase(phase), (), (), ())
    }
}
