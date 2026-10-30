//! D4-2b: `rules::explore_windows` (the choice of a deck, Distant Suns, the Ion Storm, a reward, paying with a mech or an infantry, the Dominus
//! Orb and the Crown of Emphidia).

use ti4_engine_core::choice::explore::{Opt as EOpt, PayReward, Step};
use ti4_engine_core::choice::relic::Opt as ROpt;
use ti4_engine_core::containers::Deck;
use ti4_engine_core::generated::{Explore, ExploreType, Planet, Relic, Wormhole};
use ti4_engine_core::rules::explore_windows::*;
use ti4_engine_core::state::PlanetFlags;
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{first_planet, plain_systems};

fn table() -> Scenario {
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec, Faction::TheYinBrotherhood]).map_ring(7)
}

fn ex(o: EOpt) -> Opt {
    Opt::Explore(o)
}

// ---- ChooseDeck -------------------------------------------------------------------------------

#[test]
fn a_planet_with_several_traits_asks_for_the_deck_and_one_with_a_single_trait_does_not() {
    covers!("rules::explore_windows::ChooseDeck");
    let mut rig = Rig::new(table(), ChooseDeck::new(Seat(0), &[ExploreType::Cultural, ExploreType::Hazardous]));
    rig.run();
    assert_eq!(rig.opts(), vec![ex(EOpt::Deck(ExploreType::Cultural)), ex(EOpt::Deck(ExploreType::Hazardous))]);
    rig.assert_options_sound();
    rig.expect_illegal(Pick::Opt(ex(EOpt::Deck(ExploreType::Industrial))));
    rig.pick(1);
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.chosen, Some(ExploreType::Hazardous));
    assert_eq!(delta(rig.start_state(), &rig.state), "", "choosing a deck changes no state");

    let mut one = Rig::new(table(), ChooseDeck::new(Seat(0), &[ExploreType::Industrial]));
    one.run();
    assert!(one.is_done());
    assert_eq!(one.driver.window.chosen, Some(ExploreType::Industrial), "a single trait is taken without a question");
    let mut none = Rig::new(table(), ChooseDeck::new(Seat(0), &[]));
    none.run();
    assert!(none.is_done() && none.driver.window.chosen.is_none());
    smoke("explore_windows/choose_deck", || Rig::new(table(), ChooseDeck::new(Seat(0), &[ExploreType::Cultural, ExploreType::Hazardous, ExploreType::Industrial])));
}

// ---- DistantSuns ------------------------------------------------------------------------------

fn deck_state(cards: &[Explore]) -> CoreState {
    let mut s = table().state();
    s.decks_mut().explore[ExploreType::Cultural as usize] = Deck::new(cards.to_vec());
    s.decks_mut().explore_present |= 1 << ExploreType::Cultural as u8;
    s
}

#[test]
fn distant_suns_draws_a_second_card_and_lets_the_explorer_keep_either() {
    covers!("rules::explore_windows::DistantSuns");
    let (a, b) = (Explore::ALL[0], Explore::ALL[1]);
    let w = || DistantSuns::new(Seat(0), ExploreType::Cultural, a);
    let mut rig = Rig::from_parts(deck_state(&[b, Explore::ALL[2]]), ti4_engine_core_tests::kernel_all(), w());
    rig.run();
    assert_eq!(rig.opts(), vec![ex(EOpt::Card(a)), ex(EOpt::Card(b))]);
    assert_eq!(rig.state.decks().explore[ExploreType::Cultural as usize].len(), 1, "the second card was drawn");
    rig.expect_illegal(Pick::Opt(ex(EOpt::Card(Explore::ALL[2]))));
    rig.pick(1);
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.chosen, b);
    let mut empty = Rig::from_parts(deck_state(&[]), ti4_engine_core_tests::kernel_all(), w());
    empty.run();
    assert!(empty.is_done(), "an empty deck: the first card stands");
    assert_eq!(empty.driver.window.chosen, a);
    smoke("explore_windows/distant_suns", || Rig::from_parts(deck_state(&[b, Explore::ALL[2]]), ti4_engine_core_tests::kernel_all(), w()));
}

// ---- IonStorm ---------------------------------------------------------------------------------

#[test]
fn the_ion_storm_token_lands_on_the_system_with_the_side_chosen() {
    covers!("rules::explore_windows::IonStorm");
    let s = plain_systems()[2];
    let mut rig = Rig::new(table(), IonStorm::new(Seat(0), s));
    rig.run();
    assert_eq!(rig.opts(), vec![ex(EOpt::Face(Wormhole::Alpha)), ex(EOpt::Face(Wormhole::Beta))]);
    rig.expect_illegal(Pick::Opt(ex(EOpt::Face(Wormhole::Gamma))));
    rig.pick_opt(ex(EOpt::Face(Wormhole::Beta)));
    assert!(rig.is_done());
    assert_eq!(rig.state.meta().ion_storm, Some((s, Wormhole::Beta)));
    assert_eq!(rig.driver.window.face, Some(Wormhole::Beta));
    ti4_engine_core_tests::assert_snapshot("explore_windows/ion_storm", &rig.snapshot());
}

// ---- Reward -----------------------------------------------------------------------------------

#[test]
fn a_reward_with_one_option_is_taken_unasked_only_when_the_caller_says_so() {
    covers!("rules::explore_windows::Reward");
    let opts = [EOpt::Gain, EOpt::Replenish];
    let mut rig = Rig::new(table(), Reward::new(Seat(0), Step::MerchantStation, &opts, false));
    rig.run();
    assert_eq!(rig.opts(), vec![ex(EOpt::Gain), ex(EOpt::Replenish)]);
    rig.expect_illegal(Pick::Opt(ex(EOpt::Convert)));
    rig.pick(1);
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.chosen, Some(EOpt::Replenish));
    let single = [EOpt::Gain];
    let mut asked = Rig::new(table(), Reward::new(Seat(0), Step::MerchantStation, &single, false));
    asked.run();
    assert!(!asked.is_done(), "one option is still a question");
    let mut skipped = Rig::new(table(), Reward::new(Seat(0), Step::MerchantStation, &single, true));
    skipped.run();
    assert!(skipped.is_done());
    assert_eq!(skipped.driver.window.chosen, Some(EOpt::Gain));
    smoke_quiet(|| Rig::new(table(), Reward::new(Seat(0), Step::LocalFabricators, &[EOpt::SpendTradeGood, EOpt::SpendCommodity], false)));
}

// ---- PayMechOrInfantry ------------------------------------------------------------------------

fn pay_table(units: &[(Unit, usize)], controlled: bool) -> (Scenario, Planet) {
    let (sys, planet) = first_planet();
    let mut sc = table();
    if controlled {
        sc = sc.control(planet, 0);
    }
    for (u, n) in units {
        sc = sc.unit(sys, Some(planet), *u, 0, *n);
    }
    (sc, planet)
}

#[test]
fn a_mech_on_the_planet_pays_without_a_question_an_infantry_asks_and_nothing_pays_nothing() {
    covers!("rules::explore_windows::PayMechOrInfantry");
    let card = Explore::ALL[0];
    let (sc, planet) = pay_table(&[(Unit::FactionlessMech, 1), (Unit::InfantryI, 1)], true);
    let mut rig = Rig::new(sc, PayMechOrInfantry::new(Seat(0), card, planet, PayReward::TradeGood));
    rig.run();
    assert!(rig.is_done() && rig.driver.window.paid, "a mech pays at no cost");
    assert_eq!(rig.state.board().planet(planet).units.len(), 2, "nothing is removed");

    let (sc, planet) = pay_table(&[(Unit::InfantryI, 2)], true);
    let w = || PayMechOrInfantry::new(Seat(0), card, planet, PayReward::ReadyPlanet);
    let mut rig = Rig::new(sc.clone(), w());
    rig.run();
    assert_eq!(rig.opts(), vec![ex(EOpt::RemoveInfantry), ex(EOpt::KeepInfantry)]);
    rig.pick_opt(ex(EOpt::RemoveInfantry));
    assert!(rig.is_done() && rig.driver.window.paid);
    assert_eq!(rig.state.board().planet(planet).units.len(), 1, "exactly one infantry went");
    let mut keep = Rig::new(sc, w());
    keep.run().pick_opt(ex(EOpt::KeepInfantry));
    assert!(keep.is_done() && !keep.driver.window.paid);
    assert_eq!(keep.state.board().planet(planet).units.len(), 2);

    let (sc, planet) = pay_table(&[], true);
    let mut none = Rig::new(sc, PayMechOrInfantry::new(Seat(0), card, planet, PayReward::CommandToken));
    none.run();
    assert!(none.is_done() && !none.driver.window.paid);
    let (sc, planet) = pay_table(&[(Unit::InfantryI, 1)], false);
    let mut unclaimed = Rig::new(sc, PayMechOrInfantry::new(Seat(0), card, planet, PayReward::CommandToken));
    unclaimed.run();
    assert!(unclaimed.is_done() && !unclaimed.driver.window.paid, "a planet nobody controls and without an entry: nothing to pay with");
    let (sc, planet) = pay_table(&[(Unit::InfantryI, 2)], true);
    smoke("explore_windows/pay_infantry", || Rig::new(sc.clone(), PayMechOrInfantry::new(Seat(0), card, planet, PayReward::TradeGood)));
    let _ = PlanetFlags::UNITS_ENTRY;
}

// ---- Dominus Orb ------------------------------------------------------------------------------

#[test]
fn the_dominus_orb_is_offered_when_a_ship_sits_under_a_token_outside_the_active_system() {
    covers!("rules::explore_windows::DominusOrb");
    let s = plain_systems();
    let base = || table().unit(s[1], None, Unit::CruiserI, 0, 1).command_token(s[1], 0).active_system(s[0]);
    let hold = |sc: Scenario| {
        let mut st = sc.state();
        st.players_mut().by_seat_mut(Seat(0)).unwrap().cards.relics.push(Relic::DominusOrb);
        st
    };
    let mut rig = Rig::from_parts(hold(base()), ti4_engine_core_tests::kernel_all(), DominusOrb::new(Seat(0)));
    rig.run();
    assert_eq!(rig.opts(), vec![Opt::Relic(ROpt::Purge), Opt::Decline]);
    rig.pick_opt(Opt::Relic(ROpt::Purge));
    assert!(rig.is_done() && rig.driver.window.used);
    assert!(!rig.state.players().by_seat(Seat(0)).unwrap().cards.relics.as_slice().contains(&Relic::DominusOrb), "purged");
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().round_marks.of(ti4_engine_core::state::RoundKind::DominusOrb).count(), 1, "the use is marked for the activation");
    let mut decl = Rig::from_parts(hold(base()), ti4_engine_core_tests::kernel_all(), DominusOrb::new(Seat(0)));
    decl.run().decline();
    assert!(decl.is_done() && !decl.driver.window.used);
    assert!(decl.state.players().by_seat(Seat(0)).unwrap().cards.relics.as_slice().contains(&Relic::DominusOrb));
    // Not offered: the pinned ship is in the active system, no ship, or no relic.
    let mut active = Rig::from_parts(hold(table().unit(s[1], None, Unit::CruiserI, 0, 1).command_token(s[1], 0).active_system(s[1])), ti4_engine_core_tests::kernel_all(), DominusOrb::new(Seat(0)));
    active.run();
    assert!(active.is_done(), "the active system is not pinned");
    let mut bare = Rig::new(base(), DominusOrb::new(Seat(0)));
    bare.run();
    assert!(bare.is_done(), "no relic");
    let mut noship = Rig::from_parts(hold(table().command_token(s[1], 0).active_system(s[0])), ti4_engine_core_tests::kernel_all(), DominusOrb::new(Seat(0)));
    noship.run();
    assert!(noship.is_done(), "no ship under the token");
    smoke_quiet(|| Rig::from_parts(hold(base()), ti4_engine_core_tests::kernel_all(), DominusOrb::new(Seat(0))));
}

// ---- Crown of Emphidia ------------------------------------------------------------------------

#[test]
fn the_crown_of_emphidia_exhausts_to_explore_one_of_our_planets_with_a_trait() {
    covers!("rules::explore_windows::CrownExplore");
    let s = plain_systems();
    let (p0, p1) = (s[0].data().planets[0], s[1].data().planets[0]);
    let state = || {
        let mut st = table().control(p0, 0).control(p1, 0).state();
        st.players_mut().by_seat_mut(Seat(0)).unwrap().cards.relics.push(Relic::TheCrownOfEmphidia);
        st
    };
    let traited: Vec<Planet> = [p0, p1].into_iter().filter(|p| !p.data().traits.is_empty()).collect();
    let mut rig = Rig::from_parts(state(), ti4_engine_core_tests::kernel_all(), CrownExplore::new(Seat(0), ALL_SOURCES));
    rig.run();
    let offered: Vec<Planet> = rig.opts().iter().filter_map(|o| if let Opt::Relic(ROpt::Planet(p, _)) = o { Some(*p) } else { None }).collect();
    assert_eq!(offered, traited, "controlled planets with a trait, in system order");
    assert_eq!(rig.opts().last(), Some(&Opt::Decline));
    rig.assert_options_sound();
    if let Some(&first) = traited.first() {
        rig.pick(0);
        assert!(rig.is_done());
        assert_eq!(rig.driver.window.planet, Some(first));
        assert!(rig.state.players().by_seat(Seat(0)).unwrap().cards.relics_exhausted.contains(Relic::TheCrownOfEmphidia), "the crown is exhausted");
    }
    let mut decl = Rig::from_parts(state(), ti4_engine_core_tests::kernel_all(), CrownExplore::new(Seat(0), ALL_SOURCES));
    decl.run().decline();
    assert!(decl.is_done() && decl.driver.window.planet.is_none());
    assert_eq!(delta(decl.start_state(), &decl.state), "");
    let mut spent = state();
    spent.players_mut().by_seat_mut(Seat(0)).unwrap().cards.relics_exhausted.insert(Relic::TheCrownOfEmphidia);
    let mut done = Rig::from_parts(spent, ti4_engine_core_tests::kernel_all(), CrownExplore::new(Seat(0), ALL_SOURCES));
    done.run();
    assert!(done.is_done(), "an exhausted crown asks nothing");
    smoke_quiet(|| Rig::from_parts(state(), ti4_engine_core_tests::kernel_all(), CrownExplore::new(Seat(0), ALL_SOURCES)));
}
