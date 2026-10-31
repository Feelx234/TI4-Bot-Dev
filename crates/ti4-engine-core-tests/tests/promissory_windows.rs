//! D4A4: `rules::promissory_windows` against the printed cards: `PlaceUnits` (Military Support, the ???_ERROR_ERROR_??? PDS), Stall Tactics,
//! Mageon Implants and the ???_ERROR_ERROR_??? draw.
//!
//! Printed text: Military Support (Sol): "At the start of your turn, remove 1 token from the Sol player's strategy pool and return it to their
//! reinforcements. Then, place 2 infantry from your reinforcements on any planet you control. Return this card to the Sol player."
//! Stall Tactics: "ACTION: Discard 1 action card." (Yssaril mech DEPLOY: "After you use your STALL TACTICS faction ability, you may place this
//! unit on a planet you control.") Mageon Implants: "ACTION: Exhaust this card to look at another player's action cards. Choose 1 of those
//! cards and add it to your hand." ???_ERROR_ERROR_???: "ACTION: Exhaust this card and discard 1 action card to draw 1 action card"; "ACTION:
//! Exhaust this card to place 1 PDS on a planet you control."

use ti4_engine_core::choice::cards::{Ask as CAsk, Opt as CO, Reason};
use ti4_engine_core::choice::promissory::{Ask as PAsk, Opt as POpt};
use ti4_engine_core::containers::Deck;
use ti4_engine_core::generated::{ActionCard, Planet, PromissoryNote, Technology as T};
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::rules::promissory_windows::{C4rDraw, MageonImplants, PlaceUnits, StallTactics, Then};
use ti4_engine_core::state::{Code, NoteKey};
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{plain_systems, system_of};

fn table_with_groups() -> AbilityTable {
    let mut t = AbilityTable::default();
    ti4_engine_core::driver::children::register(&mut t);
    t.facts.card_groups = (0..ActionCard::COUNT).map(|i| u8::try_from(i).unwrap()).collect();
    t
}

fn planets(n: usize) -> Vec<Planet> {
    plain_systems().into_iter().map(|s| s.data().planets[0]).take(n).collect()
}

fn map_of(ps: &[Planet]) -> Vec<System> {
    ps.iter().map(|p| system_of(*p)).collect()
}

fn count(s: &CoreState, p: Planet, seat: u8, unit: Unit) -> usize {
    s.board().planet(p).units.as_slice().iter().filter(|u| u.owner() == Seat(seat) && u.kind() == unit).count()
}

// ---- PlaceUnits -------------------------------------------------------------------------------

fn place(unit: Unit, count: u8, within: Option<System>, optional: bool, then: Then) -> PlaceUnits {
    PlaceUnits::new(Seat(0), Some(unit), count, within, optional, ALL_SOURCES, Code(5), then)
}

#[test]
fn units_go_to_the_one_planet_chosen_among_those_the_placer_controls_and_a_may_can_be_declined() {
    covers!("rules::promissory_windows::PlaceUnits");
    let ps = planets(3);
    let sc = || Scenario::new(&[Faction::TheArborec, Faction::TheFederationOfSol]).map_from(&map_of(&ps)).control(ps[0], 0).control(ps[1], 0).control(ps[2], 1);
    let mut rig = Rig::new(sc(), place(Unit::InfantryI, 2, None, false, Then::Nothing));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Promissory(PAsk::PlaceUnits { unit: Unit::InfantryI, fit: 2 })));
    assert_eq!(rig.opts().len(), 2, "only the planets seat 0 controls, no decline when it is not a may");
    rig.assert_options_sound();
    rig.expect_illegal(Pick::Opt(Opt::Promissory(POpt::Spot(system_of(ps[2]), ps[2]))));
    rig.expect_illegal(Pick::Opt(Opt::Decline));
    rig.pick_opt(Opt::Promissory(POpt::Spot(system_of(ps[1]), ps[1])));
    assert!(rig.is_done());
    assert_eq!((count(&rig.state, ps[0], 0, Unit::InfantryI), count(&rig.state, ps[1], 0, Unit::InfantryI)), (0, 2), "all the units go to the planet chosen");
    assert_eq!(rig.driver.window.placed, 2);
    smoke("promissory_windows/place", || Rig::new(sc(), place(Unit::InfantryI, 2, None, false, Then::Nothing)));
    let mut may = Rig::new(sc(), place(Unit::InfantryI, 2, None, true, Then::Nothing));
    may.run();
    assert_eq!(may.opts().len(), 3);
    may.decline();
    assert!(may.is_done() && may.driver.window.placed == 0);
    assert_eq!(delta(may.start_state(), &may.state), "");
}

#[test]
fn a_lone_planet_takes_the_units_unasked_and_nothing_is_placed_without_a_form_a_unit_in_the_box_or_a_planet() {
    covers!("rules::promissory_windows::PlaceUnits");
    let ps = planets(2);
    let sc = || Scenario::new(&[Faction::TheArborec, Faction::TheFederationOfSol]).map_from(&map_of(&ps)).control(ps[0], 0).control(ps[1], 0);
    let mut lone = Rig::new(sc(), place(Unit::InfantryI, 2, Some(system_of(ps[1])), false, Then::Nothing));
    lone.run();
    assert!(lone.is_done(), "a system limit leaves one planet: no decision");
    assert_eq!((count(&lone.state, ps[0], 0, Unit::InfantryI), count(&lone.state, ps[1], 0, Unit::InfantryI)), (0, 2));
    let mut form = Rig::new(sc(), PlaceUnits::new(Seat(0), None, 2, None, false, ALL_SOURCES, Code(5), Then::Nothing));
    form.run();
    assert!(form.is_done() && form.driver.window.placed == 0, "no form to place: nothing happens");
    let mut nowhere = Rig::new(Scenario::new(&[Faction::TheArborec, Faction::TheFederationOfSol]).map_from(&map_of(&ps)), place(Unit::InfantryI, 2, None, false, Then::Nothing));
    nowhere.run();
    assert!(nowhere.is_done() && nowhere.driver.window.placed == 0, "no planet controlled");
}

#[test]
fn military_support_spends_a_strategy_token_of_the_sol_player_and_returns_the_note_to_them() {
    covers!("rules::promissory_windows::PlaceUnits");
    let ps = planets(2);
    let key = NoteKey { note: PromissoryNote::MilitarySupport, faction: Faction::TheFederationOfSol };
    let state = || {
        let mut s = Scenario::new(&[Faction::TheArborec, Faction::TheFederationOfSol]).map_from(&map_of(&ps)).control(ps[0], 0).tokens(1, 3, 3, 2).state();
        s.cards_mut().promissory.edit().insert(key, Seat(0));
        s
    };
    let then = Then::MilitarySupport { owner: Seat(1), reason: Code(6) };
    let mut rig = Rig::from_parts(state(), kernel_all(), place(Unit::InfantryI, 2, None, false, then));
    rig.run();
    assert!(rig.is_done());
    assert_eq!(count(&rig.state, ps[0], 0, Unit::InfantryI), 2, "place 2 infantry");
    assert_eq!(rig.state.players().by_seat(Seat(1)).unwrap().econ.strategic_tokens, 1, "1 token from the Sol player's strategy pool");
    assert_eq!(rig.state.cards().promissory.get(&key), Some(&Seat(1)), "return this card to the Sol player");
    // With an empty pool the token is skipped ("if able") and the card still goes home.
    let mut broke = state();
    broke.players_mut().by_seat_mut(Seat(1)).unwrap().econ.strategic_tokens = 0;
    let mut r2 = Rig::from_parts(broke, kernel_all(), place(Unit::InfantryI, 2, None, false, then));
    r2.run();
    assert_eq!(r2.state.players().by_seat(Seat(1)).unwrap().econ.strategic_tokens, 0);
    assert_eq!(r2.state.cards().promissory.get(&key), Some(&Seat(1)));
    // The note goes home even when nothing could be placed.
    let mut nothing = Rig::from_parts(state(), kernel_all(), PlaceUnits::new(Seat(0), None, 2, None, false, ALL_SOURCES, Code(5), then));
    nothing.run();
    assert_eq!(nothing.state.cards().promissory.get(&key), Some(&Seat(1)));
}

#[test]
fn the_nekro_pds_goes_on_a_planet_under_the_structure_cap_and_exhausts_the_card_only_when_placed() {
    covers!("rules::promissory_windows::PlaceUnits");
    let ps = planets(3);
    let sc = || {
        Scenario::new(&[Faction::TheNekroVirus, Faction::TheArborec])
            .map_from(&map_of(&ps))
            .tech(0, T::ERRORERROR)
            .control(ps[0], 0)
            .control(ps[1], 0)
            .control(ps[2], 0)
            .unit(system_of(ps[0]), Some(ps[0]), Unit::PDSI, 0, 2)
    };
    let mut rig = Rig::new(sc(), place(Unit::PDSI, 1, None, false, Then::C4rPds));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Promissory(PAsk::C4rPds)));
    assert_eq!(rig.opts().len(), 2, "the planet with two PDS is at the cap of two: it is not offered; no decline");
    rig.assert_options_sound();
    rig.pick(0);
    assert!(rig.is_done());
    assert_eq!(count(&rig.state, ps[1], 0, Unit::PDSI), 1);
    assert!(rig.state.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(T::ERRORERROR), "exhaust this card");
    let full = Scenario::new(&[Faction::TheNekroVirus, Faction::TheArborec]).map_from(&map_of(&ps[..1])).tech(0, T::ERRORERROR).control(ps[0], 0).unit(system_of(ps[0]), Some(ps[0]), Unit::PDSI, 0, 2);
    let mut capped = Rig::new(full, place(Unit::PDSI, 1, None, false, Then::C4rPds));
    capped.run();
    assert!(capped.is_done());
    assert!(!capped.state.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(T::ERRORERROR), "nothing placed: the card stays ready");
}

// ---- Stall Tactics ----------------------------------------------------------------------------

fn yssaril(hand: &[ActionCard], planets: &[Planet]) -> Scenario {
    let mut sc = Scenario::new(&[Faction::TheYssarilTribes, Faction::TheArborec]).map_from(&map_of(planets)).hand(0, hand);
    for p in planets {
        sc = sc.control(*p, 0);
    }
    sc
}

fn stall(sc: Scenario) -> Rig<StallTactics> {
    Rig::new(sc, StallTactics::new(Seat(0), Some(Unit::FactionlessMech), ALL_SOURCES, Code(7), Code(8))).with_table(table_with_groups())
}

#[test]
fn stall_tactics_discards_the_card_chosen_and_then_offers_the_mech() {
    covers!("rules::promissory_windows::StallTactics");
    let ps = planets(2);
    let hand = [ActionCard::Rally, ActionCard::Spy, ActionCard::Rout];
    let mut rig = stall(yssaril(&hand, &ps));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Cards(CAsk::OwnHand { reason: Reason::StallTactics })));
    assert_eq!(rig.opts().len(), 3, "one option per distinct printed card");
    rig.assert_options_sound();
    rig.expect_illegal(Pick::Opt(Opt::Cards(CO::Card(ActionCard::Veto))));
    rig.pick_opt(Opt::Cards(CO::Card(ActionCard::Spy)));
    assert!(rig.driver.window.done);
    assert!(!rig.state.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice().contains(&ActionCard::Spy));
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().cards.hand.len(), 2);
    assert_eq!(rig.opts().len(), 3, "two planets and a decline: the mech is a may");
    rig.pick_where(|o| matches!(o, Opt::Promissory(POpt::Spot(_, p)) if *p == ps[1]));
    assert!(rig.is_done());
    assert_eq!(count(&rig.state, ps[1], 0, Unit::FactionlessMech), 1);
    smoke("promissory_windows/stall", || stall(yssaril(&hand, &ps)));
}

#[test]
fn stall_tactics_with_a_single_card_discards_it_unasked_and_with_an_empty_hand_does_not_resolve() {
    covers!("rules::promissory_windows::StallTactics");
    let ps = planets(2);
    let mut one = stall(yssaril(&[ActionCard::Rally], &ps));
    one.run();
    assert!(one.driver.window.done);
    assert_eq!(one.state.players().by_seat(Seat(0)).unwrap().cards.hand.len(), 0);
    assert!(matches!(one.asked().ask, Ask::Promissory(_)), "straight on to the mech");
    one.decline();
    assert!(one.is_done());
    assert_eq!(count(&one.state, ps[0], 0, Unit::FactionlessMech) + count(&one.state, ps[1], 0, Unit::FactionlessMech), 0);
    let mut none = stall(yssaril(&[], &ps));
    none.run();
    assert!(none.is_done() && !none.driver.window.done, "the hand holds no card: the action does not resolve");
}

// ---- Mageon Implants --------------------------------------------------------------------------

/// The holder of the card is a Yssaril seat when the hand limit is lifted (Crafty is the kernel's hook since lane 14: the draw window the
/// card ends with follows it from the faction), and a seat without any hook (a Nekro seat that copied the card) when the limit applies.
fn mageon(hand0: &[ActionCard], hand1: &[ActionCard], limited: bool) -> Rig<MageonImplants> {
    let holder = if limited { Faction::TheNekroVirus } else { Faction::TheYssarilTribes };
    let sc = Scenario::new(&[holder, Faction::TheArborec]).tech(0, T::MageonImplants).hand(0, hand0).hand(1, hand1);
    Rig::new(sc, MageonImplants::new(Seat(0), Seat(1), limited)).with_table(table_with_groups())
}

#[test]
fn mageon_implants_exhausts_the_card_and_adds_the_chosen_card_of_the_other_hand_to_ours() {
    covers!("rules::promissory_windows::MageonImplants");
    let theirs = [ActionCard::Rally, ActionCard::Spy, ActionCard::Rout];
    let mut rig = mageon(&[ActionCard::Veto], &theirs, true);
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Cards(CAsk::TakeRevealed { owner: Seat(1) })));
    assert!(rig.state.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(T::MageonImplants), "exhaust this card");
    assert_eq!(rig.opts().len(), 3, "no decline: the card is already spent");
    rig.assert_options_sound();
    rig.pick_opt(Opt::Cards(CO::Card(ActionCard::Spy)));
    assert!(rig.is_done() && rig.driver.window.done && !rig.driver.window.unsupported);
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice(), &[ActionCard::Veto, ActionCard::Spy], "added to the end of the taker's hand");
    assert_eq!(rig.state.players().by_seat(Seat(1)).unwrap().cards.hand.len(), 2);
    smoke("promissory_windows/mageon", || mageon(&[ActionCard::Veto], &theirs, true));
}

#[test]
fn mageon_implants_takes_a_single_card_unasked_and_a_hand_over_the_limit_asks_which_card_goes() {
    covers!("rules::promissory_windows::MageonImplants");
    let seven = [ActionCard::Rally, ActionCard::NavSuite, ActionCard::CoupDEtat, ActionCard::Spy, ActionCard::Sabotage1, ActionCard::Veto, ActionCard::Uprising];
    let mut over = mageon(&seven, &[ActionCard::Plague], true);
    over.run();
    assert!(matches!(over.asked().ask, Ask::Cards(CAsk::HandLimit { held: 8 })), "seven cards and the one taken: {:?}", over.asked().ask);
    over.pick(0);
    assert!(over.is_done());
    assert_eq!(over.state.players().by_seat(Seat(0)).unwrap().cards.hand.len(), 7);
    let mut free = mageon(&seven, &[ActionCard::Plague], false);
    free.run();
    assert!(free.is_done(), "a seat that lifts the limit (Crafty) is not asked");
    assert_eq!(free.state.players().by_seat(Seat(0)).unwrap().cards.hand.len(), 8);
    // Since lane 14 the draw window the card ends with follows the faction hook itself: a Yssaril holder has no limit whatever flag the
    // caller hands in (the fixture of this test once seated Yssaril for the "limited" case and the question vanished).
    let sc = Scenario::new(&[Faction::TheYssarilTribes, Faction::TheArborec]).tech(0, T::MageonImplants).hand(0, &seven).hand(1, &[ActionCard::Plague]);
    let mut crafty = Rig::new(sc, MageonImplants::new(Seat(0), Seat(1), true)).with_table(table_with_groups());
    crafty.run();
    assert!(crafty.is_done(), "Crafty: no hand limit question for a Yssaril holder");
    assert_eq!(crafty.state.players().by_seat(Seat(0)).unwrap().cards.hand.len(), 8);
}

#[test]
fn mageon_implants_does_nothing_when_the_card_is_spent_the_target_is_empty_or_it_is_our_own_hand() {
    covers!("rules::promissory_windows::MageonImplants");
    let mut empty = mageon(&[], &[], true);
    empty.run();
    assert!(empty.is_done() && !empty.driver.window.done, "nothing to look at: the action does not resolve");
    assert!(!empty.state.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(T::MageonImplants));
    let sc = Scenario::new(&[Faction::TheYssarilTribes, Faction::TheArborec]).tech(0, T::MageonImplants).hand(0, &[ActionCard::Rally]);
    let mut own = Rig::new(sc.clone(), MageonImplants::new(Seat(0), Seat(0), true)).with_table(table_with_groups());
    own.run();
    assert!(own.is_done() && !own.driver.window.done, "another player's cards only");
    let mut s = sc.hand(1, &[ActionCard::Plague]).state();
    s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.exhausted.insert(T::MageonImplants);
    let mut tired = Rig::from_parts(s, kernel_all(), MageonImplants::new(Seat(0), Seat(1), true)).with_table(table_with_groups());
    tired.run();
    assert!(tired.is_done() && !tired.driver.window.done, "an exhausted card cannot be used");
    let mut bare = Rig::new(Scenario::new(&[Faction::TheYssarilTribes, Faction::TheArborec]).tech(0, T::MageonImplants).hand(1, &[ActionCard::Plague]), MageonImplants::new(Seat(0), Seat(1), true));
    bare.run();
    assert!(bare.is_done() && bare.driver.window.unsupported, "no printed-name groups: the caller declines the run");
}

// ---- ???_ERROR_ERROR_??? draw -------------------------------------------------------------------

fn c4r(hand: &[ActionCard], deck: &[ActionCard]) -> Rig<C4rDraw> {
    let mut s = Scenario::new(&[Faction::TheNekroVirus, Faction::TheArborec]).tech(0, T::ERRORERROR).hand(0, hand).state();
    s.decks_mut().action = Deck::new(deck.to_vec());
    Rig::from_parts(s, kernel_all(), C4rDraw::new(Seat(0), false)).with_table(table_with_groups())
}

#[test]
fn the_nekro_draw_discards_one_card_exhausts_the_card_and_draws_one() {
    covers!("rules::promissory_windows::C4rDraw");
    let mut rig = c4r(&[ActionCard::Rally, ActionCard::Spy], &[ActionCard::Plague, ActionCard::Rout]);
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Cards(CAsk::OwnHand { reason: Reason::C4rDiscard })));
    assert_eq!(rig.opts().len(), 2);
    rig.assert_options_sound();
    rig.pick_opt(Opt::Cards(CO::Card(ActionCard::Rally)));
    assert!(rig.is_done() && rig.driver.window.done);
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice(), &[ActionCard::Spy, ActionCard::Plague], "one discarded, one drawn from the top");
    assert!(rig.state.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(T::ERRORERROR));
    smoke("promissory_windows/c4r", || c4r(&[ActionCard::Rally, ActionCard::Spy], &[ActionCard::Plague, ActionCard::Rout]));
}

#[test]
fn the_nekro_draw_with_one_card_discards_it_unasked_and_with_none_does_not_resolve() {
    covers!("rules::promissory_windows::C4rDraw");
    let mut one = c4r(&[ActionCard::Rally], &[ActionCard::Plague]);
    one.run();
    assert!(one.is_done() && one.driver.window.done);
    assert_eq!(one.state.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice(), &[ActionCard::Plague]);
    let mut none = c4r(&[], &[ActionCard::Plague]);
    none.run();
    assert!(none.is_done() && !none.driver.window.done);
    assert!(!none.state.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(T::ERRORERROR), "nothing to discard: the card is not exhausted");
    let mut bare = Rig::new(Scenario::new(&[Faction::TheNekroVirus, Faction::TheArborec]).tech(0, T::ERRORERROR).hand(0, &[ActionCard::Rally]), C4rDraw::new(Seat(0), false)).with_table(table_with_groups());
    bare.run();
    assert!(bare.is_done() && bare.driver.window.done, "with an empty deck nothing is drawn but the discard stands");
}
