//! Lane G: the formerly silent abilities of the string engine as rules of the core table (`ti4_engine_core_factions::silent`): Gift of Prescience, the
//! units that wait on a card, Scavenge, Mitosis and Pillage. The lockstep against the string is in `ti4-engine` (`silent_live_tests`); these tests pin the rule.

use ti4_engine_core::choice::reaction::{Ask as RAsk, Opt as ROpt};
use ti4_engine_core::choice::silent::{Ask as GAsk, Landing, Opt as GOpt};
use ti4_engine_core::generated::{PromissoryNote, Technology, Unit};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::query::board;
use ti4_engine_core::state::{EventKind, Key, NoteKey, Payload, Val};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::windows::EmitEvent;

fn table() -> AbilityTable {
    let mut t = AbilityTable::default();
    ti4_engine_core_factions::silent::register_before_movement(&mut t);
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

fn player_event(kind: EventKind, seat: u8) -> ti4_engine_core::state::EventRec {
    fx::event(kind, Payload::new().with(Key::Player, Val::Seat(Seat(seat))))
}

/// A plain system with at least two planets, and its planets.
fn two_planet_system() -> (System, Vec<Planet>) {
    let system = ti4_engine_core_tests::scenario::plain_systems().into_iter().find(|s| s.data().planets.len() >= 2).expect("a plain system with two planets");
    (system, system.data().planets.to_vec())
}

const GIFT: NoteKey = NoteKey { note: PromissoryNote::GiftOfPrescience, faction: Faction::TheNaaluCollective };

fn gift_edit(holder: u8) -> impl FnOnce(&mut CoreState) {
    move |s| {
        s.cards_mut().promissory.edit().insert(GIFT, Seat(holder));
    }
}

// -- Gift of Prescience -----------------------------------------------------------------------------------------------

#[test]
fn the_holder_of_gift_of_prescience_may_play_it_at_the_end_of_the_strategy_phase() {
    covers!("silent::gift::GIFT");
    let mut r = rig(Scenario::new(&[Faction::TheNaaluCollective, Faction::TheEmiratesOfHacan]), gift_edit(1), fx::event(EventKind::StrategyPhaseEnded, Payload::new()));
    assert_eq!(r.asked().actor, Seat(1), "{}", r.transcript());
    assert!(matches!(r.asked().ask, Ask::Reaction(RAsk::Window { .. })));
    let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the note");
    r.pick(use_it);
    assert!(r.is_done());
    assert_eq!(r.state.strategy().initiative_overrides.get(&Seat(1)), Some(&0), "the holder is first");
    assert!(r.state.cards().promissory_faceup.contains_key(&GIFT), "the note lies faceup");
    assert_eq!(r.state.marks().tail.get(&MarkKey::NaaluGiftRound()), Some(&MarkVal::Atom(Atom::Num(1))), "Telepathic is off this round");
}

#[test]
fn declining_the_gift_changes_nothing_and_the_note_needs_a_holder_who_is_not_the_naalu_and_a_round_it_was_not_played() {
    covers!("silent::gift::GIFT");
    let mut declined = rig(Scenario::new(&[Faction::TheNaaluCollective, Faction::TheEmiratesOfHacan]), gift_edit(1), fx::event(EventKind::StrategyPhaseEnded, Payload::new()));
    declined.decline();
    assert!(declined.is_done() && declined.state.strategy().initiative_overrides.is_empty() && !declined.state.cards().promissory_faceup.contains_key(&GIFT));
    // The Naalu keeps its own note: nothing is offered.
    let own = rig(Scenario::new(&[Faction::TheNaaluCollective, Faction::TheEmiratesOfHacan]), |_| {}, fx::event(EventKind::StrategyPhaseEnded, Payload::new()));
    assert!(own.is_done(), "{}", own.transcript());
    // Played this round already: nothing is offered.
    let again = rig(
        Scenario::new(&[Faction::TheNaaluCollective, Faction::TheEmiratesOfHacan]),
        |s| {
            gift_edit(1)(s);
            s.marks_mut().tail.edit().insert(MarkKey::NaaluGiftRound(), MarkVal::Atom(Atom::Num(1)));
        },
        fx::event(EventKind::StrategyPhaseEnded, Payload::new()),
    );
    assert!(again.is_done(), "{}", again.transcript());
}

#[test]
fn the_gift_goes_home_at_the_end_of_the_status_phase() {
    covers!("silent::gift::GIFT_RETURN");
    let played = |s: &mut CoreState| {
        gift_edit(1)(s);
        s.cards_mut().promissory_faceup.edit().insert(GIFT, ());
        s.marks_mut().tail.edit().insert(MarkKey::NaaluGiftRound(), MarkVal::Atom(Atom::Num(1)));
    };
    let r = rig(Scenario::new(&[Faction::TheNaaluCollective, Faction::TheEmiratesOfHacan]), played, fx::event(EventKind::StatusPhaseEnded, Payload::new()));
    assert!(r.is_done(), "mandatory: {}", r.transcript());
    assert_eq!(r.state.cards().promissory.get(&GIFT), Some(&Seat(0)), "back with the Naalu");
    assert!(!r.state.cards().promissory_faceup.contains_key(&GIFT));
    assert!(r.state.marks().tail.get(&MarkKey::NaaluGiftRound()).is_none(), "Telepathic is on again");
    // Nothing was played: the note stays where it is.
    let unplayed = rig(Scenario::new(&[Faction::TheNaaluCollective, Faction::TheEmiratesOfHacan]), gift_edit(1), fx::event(EventKind::StatusPhaseEnded, Payload::new()));
    assert_eq!(unplayed.state.cards().promissory.get(&GIFT), Some(&Seat(1)));
}

// -- the units that wait on a card ----------------------------------------------------------------------------------------

fn homed(seat: u8, system: System) -> impl FnOnce(&mut CoreState) {
    move |s| {
        s.players_mut().by_seat_mut(Seat(seat)).unwrap().home_system = Some(system);
    }
}

fn on_card(s: &CoreState, key: MarkKey) -> u32 {
    match s.marks().tail.get(&key) {
        Some(MarkVal::Atom(Atom::Num(n))) => *n,
        _ => 0,
    }
}

fn units_on(s: &CoreState, planet: Planet, seat: u8) -> usize {
    board::planet_units(s, planet).iter().filter(|p| p.owner() == Seat(seat)).count()
}

#[test]
fn infantry_ii_units_on_the_card_land_on_the_first_planet_of_the_home_system_at_the_start_of_the_turn() {
    covers!("silent::returns::INF2_RETURN");
    let (system, planets) = two_planet_system();
    let scenario = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol]).tech(0, Technology::InfantryII).control(planets[0], 0).control(planets[1], 0);
    let edit = |s: &mut CoreState| {
        homed(0, system)(s);
        ti4_engine_core::rules::reactions::infantry_ii::set_on_card(s, Seat(0), 2);
    };
    let r = rig(scenario, edit, player_event(EventKind::TurnBegan, 0));
    assert!(r.is_done(), "no question: {}", r.transcript());
    assert_eq!(units_on(&r.state, planets[0], 0), 2, "both on the first planet");
    assert_eq!(units_on(&r.state, planets[1], 0), 0);
    assert_eq!(ti4_engine_core::rules::reactions::infantry_ii::units_on_card(&r.state, Seat(0)), 0);
    // Another seat's turn: nothing.
    let other = rig(Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol]).tech(0, Technology::InfantryII).control(planets[0], 0), edit, player_event(EventKind::TurnBegan, 1));
    assert_eq!(ti4_engine_core::rules::reactions::infantry_ii::units_on_card(&other.state, Seat(0)), 2);
    // The card is gone: what was on it goes with it.
    let gone = rig(Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol]).control(planets[0], 0), edit, player_event(EventKind::TurnBegan, 0));
    assert_eq!(ti4_engine_core::rules::reactions::infantry_ii::units_on_card(&gone.state, Seat(0)), 0);
    assert_eq!(units_on(&gone.state, planets[0], 0), 0);
}

#[test]
fn units_with_no_planet_to_land_on_stay_on_the_card() {
    covers!("silent::returns::INF2_RETURN");
    let (system, planets) = two_planet_system();
    let scenario = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol]).tech(0, Technology::InfantryII);
    let r = rig(scenario, |s| { homed(0, system)(s); ti4_engine_core::rules::reactions::infantry_ii::set_on_card(s, Seat(0), 3); }, player_event(EventKind::TurnBegan, 0));
    assert_eq!(ti4_engine_core::rules::reactions::infantry_ii::units_on_card(&r.state, Seat(0)), 3);
    assert_eq!(units_on(&r.state, planets[0], 0), 0);
}

#[test]
fn letani_warriors_choose_a_planet_per_unit_and_a_lone_planet_is_not_asked() {
    covers!("silent::returns::WARRIOR_RETURN");
    let (system, planets) = two_planet_system();
    let scenario = || Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan]).control(planets[0], 0).control(planets[1], 0);
    let edit = |s: &mut CoreState| {
        homed(0, system)(s);
        s.marks_mut().tail.edit().insert(MarkKey::ArborecLw2(Seat(0)), MarkVal::Atom(Atom::Num(2)));
    };
    let mut r = rig(scenario(), edit, player_event(EventKind::TurnBegan, 0));
    assert!(matches!(r.asked().ask, Ask::Reaction(RAsk::G(GAsk::Landing(Landing::Warrior)))), "{}", r.transcript());
    assert_eq!(r.opts(), vec![Opt::Reaction(ROpt::G(GOpt::Planet(planets[0]))), Opt::Reaction(ROpt::G(GOpt::Planet(planets[1])))]);
    r.pick(1);
    assert!(matches!(r.asked().ask, Ask::Reaction(RAsk::G(GAsk::Landing(_)))), "one question per unit: {}", r.transcript());
    r.pick(1);
    assert!(r.is_done());
    assert_eq!((units_on(&r.state, planets[0], 0), units_on(&r.state, planets[1], 0)), (0, 2));
    assert_eq!(on_card(&r.state, MarkKey::ArborecLw2(Seat(0))), 0);
    // One controlled planet: no question.
    let lone = rig(Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan]).control(planets[0], 0), edit, player_event(EventKind::TurnBegan, 0));
    assert!(lone.is_done(), "{}", lone.transcript());
    assert_eq!(units_on(&lone.state, planets[0], 0), 2);
}

#[test]
fn crimson_legionnaires_ask_like_the_warriors() {
    covers!("silent::returns::LEGIONNAIRE_RETURN");
    let (system, planets) = two_planet_system();
    let scenario = Scenario::new(&[Faction::TheMahactGeneSorcerers, Faction::TheEmiratesOfHacan]).control(planets[0], 0).control(planets[1], 0);
    let mut r = rig(
        scenario,
        |s| {
            homed(0, system)(s);
            s.marks_mut().tail.edit().insert(MarkKey::MahactCl2(Seat(0)), MarkVal::Atom(Atom::Num(1)));
        },
        player_event(EventKind::TurnBegan, 0),
    );
    assert!(matches!(r.asked().ask, Ask::Reaction(RAsk::G(GAsk::Landing(Landing::Legionnaire)))), "{}", r.transcript());
    r.pick(0);
    assert!(r.is_done());
    assert_eq!(units_on(&r.state, planets[0], 0), 1);
    assert_eq!(on_card(&r.state, MarkKey::MahactCl2(Seat(0))), 0);
    assert_eq!(Unit::CrimsonLegionnaireII.data().ground_force, true);
}

// -- Scavenge ------------------------------------------------------------------------------------------------------------------

#[test]
fn scavenge_pays_a_trade_good_to_the_saar_who_gained_a_planet_and_only_to_them() {
    covers!("silent::scavenge::SCAVENGE");
    let (_, planets) = two_planet_system();
    let gained = |seat: u8| fx::event(EventKind::PlanetControlGained, Payload::new().with(Key::Player, Val::Seat(Seat(seat))).with(Key::Planet, Val::Planet(planets[0])).with(Key::System, Val::System(planets[0].data().system.unwrap())));
    let goods = |r: &Rig<EmitEvent>, seat: u8| r.state.players().by_seat(Seat(seat)).unwrap().econ.trade_goods;
    let mut r = rig(Scenario::new(&[Faction::TheClanOfSaar, Faction::TheEmiratesOfHacan]), |_| {}, gained(0));
    // (Scavenge pays the good that makes Scavenger Zeta's deploy possible: lane G3's optional rule, declined here.)
    r.decline();
    assert!(r.is_done());
    assert_eq!((goods(&r, 0), goods(&r, 1)), (1, 0));
    // Another seat gains the planet: the Saar earns nothing.
    let other = rig(Scenario::new(&[Faction::TheClanOfSaar, Faction::TheEmiratesOfHacan]), |_| {}, gained(1));
    assert_eq!((goods(&other, 0), goods(&other, 1)), (0, 0));
    // A seat without the ability earns nothing from its own gain.
    let plain = rig(Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheClanOfSaar]), |_| {}, gained(0));
    assert_eq!(goods(&plain, 0), 0);
}

// -- Mitosis -------------------------------------------------------------------------------------------------------------------

#[test]
fn mitosis_asks_where_the_infantry_goes_or_which_infantry_becomes_a_mech() {
    covers!("silent::mitosis::MITOSIS");
    let (system, planets) = two_planet_system();
    let scenario = || Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan]).control(planets[0], 0).control(planets[1], 0).unit(system, Some(planets[0]), Unit::InfantryI, 0, 1);
    let mut r = rig(scenario(), |_| {}, fx::event(EventKind::StatusPhaseBegan, Payload::new()));
    assert!(matches!(r.asked().ask, Ask::Reaction(RAsk::G(GAsk::Mitosis))), "{}", r.transcript());
    // Two placements, then the replacement of the infantry on the first planet; a placement is available, so no decline.
    assert_eq!(
        r.opts(),
        vec![
            Opt::Reaction(ROpt::G(GOpt::Mitosis { replace: false, system, planet: Some(planets[0]) })),
            Opt::Reaction(ROpt::G(GOpt::Mitosis { replace: false, system, planet: Some(planets[1]) })),
            Opt::Reaction(ROpt::G(GOpt::Mitosis { replace: true, system, planet: Some(planets[0]) })),
        ]
    );
    r.pick(1);
    assert!(r.is_done());
    assert_eq!(units_on(&r.state, planets[1], 0), 1, "the infantry stands where it was placed");
    // The replacement swaps the infantry for a mech.
    let mut swap = rig(scenario(), |_| {}, fx::event(EventKind::StatusPhaseBegan, Payload::new()));
    swap.pick(2);
    let kinds: Vec<Unit> = board::planet_units(&swap.state, planets[0]).iter().map(|p| p.kind()).collect();
    assert_eq!(kinds.len(), 1);
    assert_eq!(kinds[0].data().base_type, ti4_engine_core::generated::UnitBaseType::Mech);
    // A lone planet and no infantry on the board to replace: placed without a question.
    let lone = rig(Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan]).control(planets[0], 0), |_| {}, fx::event(EventKind::StatusPhaseBegan, Payload::new()));
    assert!(lone.is_done(), "{}", lone.transcript());
    assert_eq!(units_on(&lone.state, planets[0], 0), 1);
    // Another faction has no Mitosis.
    let other = rig(Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheArborec]).control(planets[0], 0), |_| {}, fx::event(EventKind::StatusPhaseBegan, Payload::new()));
    assert!(other.is_done());
    assert_eq!(units_on(&other.state, planets[0], 0), 0);
}

// -- Pillage -------------------------------------------------------------------------------------------------------------------

#[test]
fn pillage_takes_a_trade_good_or_a_commodity_from_a_neighbour_with_three_goods() {
    covers!("silent::pillage::PILLAGE_GAINED");
    let (system, planets) = two_planet_system();
    let scenario = |goods: i16| {
        Scenario::new(&[Faction::TheMentakCoalition, Faction::TheEmiratesOfHacan])
            .map_from(&[system])
            .unit(system, Some(planets[0]), Unit::InfantryI, 0, 1)
            .unit(system, Some(planets[1]), Unit::InfantryI, 1, 1)
            .goods(1, goods)
            .commodities(1, 2)
    };
    let gained = || fx::event(EventKind::TradeGoodsGained, Payload::new().with(Key::Player, Val::Seat(Seat(1))));
    let mut r = rig(scenario(3), |_| {}, gained());
    assert!(matches!(r.asked().ask, Ask::Reaction(RAsk::Window { .. })), "optional: {}", r.transcript());
    let use_it = r.opts().iter().position(|o| *o != Opt::Decline).unwrap();
    r.pick(use_it);
    assert!(matches!(r.asked().ask, Ask::Reaction(RAsk::G(GAsk::Pillage))), "{}", r.transcript());
    assert_eq!(r.opts(), vec![Opt::Reaction(ROpt::G(GOpt::Pillage { commodity: false, target: Seat(1) })), Opt::Reaction(ROpt::G(GOpt::Pillage { commodity: true, target: Seat(1) }))]);
    r.pick(1);
    let econ = |r: &Rig<EmitEvent>, seat: u8| {
        let e = &r.state.players().by_seat(Seat(seat)).unwrap().econ;
        (e.trade_goods, e.commodities)
    };
    assert_eq!((econ(&r, 0), econ(&r, 1)), ((1, 0), (3, 1)), "a commodity becomes the Mentak's trade good");
    // Fewer than three goods: nothing is offered.
    let poor = rig(scenario(2), |_| {}, gained());
    assert!(poor.is_done(), "{}", poor.transcript());
    // The Mentak's own gain is not pillaged.
    let own = rig(scenario(3), |_| {}, fx::event(EventKind::TradeGoodsGained, Payload::new().with(Key::Player, Val::Seat(Seat(0)))));
    assert!(own.is_done());
}
