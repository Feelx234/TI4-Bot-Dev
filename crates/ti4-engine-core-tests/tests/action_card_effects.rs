//! D4 lane A2: the action cards against their printed text. One rich position (four seats, a board with every kind of unit, planets of every
//! trait, hands, decks); each test plays one card and checks what the card text says changed, not just that the contract held.

use ti4_engine_core::containers::Deck;
use ti4_engine_core::generated::{ActionCard, Agenda, Explore, ExploreType, Planet, PublicObjective, Relic, SecretObjective, Unit};
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::rules::action_cards::effects::Play;
use ti4_engine_core::rules::action_cards::{Env, Facts, Placed};
use ti4_engine_core::state::{Code, Outcome};
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn four() -> Scenario {
    Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol, Faction::TheWinnu]).map_ring(7)
}

fn linked() -> AbilityTable {
    let mut t = AbilityTable::default();
    ti4_engine_core::driver::children::register(&mut t);
    t
}

fn env() -> Env {
    Env {
        mahact_held: 0,
        scope: ALL_SOURCES,
        blanked: false,
        action_card: Code(0),
        supply_base: Code(0),
        note_order: |a, b| a.cmp(&b),
        infantry: Placed { unit: Some(Unit::InfantryI), on_card: 0 },
        cruiser: Placed { unit: Some(Unit::CruiserI), on_card: 0 },
        fighter: Placed { unit: Some(Unit::FighterI), on_card: 0 },
        facts: Facts { neutral: Some(Seat(1)), action_draw_plain: true, rules_version: 35, ..Facts::default() },
    }
}

// ---- every ported card, one position ----------------------------------------------------------

fn rich_state() -> CoreState {
    let s = plain_systems();
    let p = |i: usize| s[i].data().planets[0];
    let mut sc = four()
        .active(Some(1))
        .active_system(s[1])
        .tokens(0, 3, 3, 2)
        .tokens(1, 3, 2, 1)
        .tokens(2, 2, 2, 1)
        .tokens(3, 2, 2, 1)
        .goods(0, 4)
        .goods(1, 3)
        .commodities(1, 2)
        .command_token(s[2], 0)
        .command_token(s[3], 1)
        .unit(s[1], None, Unit::CruiserI, 0, 2)
        .unit(s[1], None, Unit::DreadnoughtI, 0, 1)
        .unit(s[1], None, Unit::FighterI, 0, 2)
        .unit(s[1], None, Unit::CruiserI, 1, 2)
        .unit(s[1], None, Unit::DestroyerI, 1, 1)
        .unit(s[2], None, Unit::DestroyerI, 0, 1)
        .unit(s[2], None, Unit::CarrierI, 1, 1)
        .unit(s[3], None, Unit::CarrierI, 0, 1)
        .unit(s[3], None, Unit::CruiserI, 2, 1)
        .control(p(4), 0)
        .unit(s[4], Some(p(4)), Unit::InfantryI, 0, 3)
        .control(p(5), 0)
        .unit(s[5], Some(p(5)), Unit::SpaceDockI, 0, 1)
        .control(p(6), 1)
        .unit(s[6], Some(p(6)), Unit::InfantryI, 1, 2)
        .unit(s[6], Some(p(6)), Unit::PDSI, 1, 1)
        .control(p(2), 2)
        .unit(s[2], Some(p(2)), Unit::InfantryI, 2, 2)
        .hand(1, &[ActionCard::ALL[10], ActionCard::ALL[11], ActionCard::ALL[12]])
        .hand(2, &[ActionCard::ALL[20], ActionCard::ALL[21]])
        .hand(3, &[ActionCard::ALL[30]]);
    sc = sc.control(p(1), 1);
    let mut st = sc.state();
    let d = st.decks_mut();
    d.action = Deck::new(ActionCard::ALL[40..60].to_vec());
    d.agenda = Deck::new(Agenda::ALL[..8].to_vec());
    d.secret = Deck::new(SecretObjective::ALL[..8].to_vec());
    d.relic = Deck::new(vec![Relic::ALL[0], Relic::ALL[1], Relic::ALL[2]]);
    d.objective = Deck::new(PublicObjective::ALL[..6].to_vec());
    for (i, t) in [ExploreType::Cultural, ExploreType::Frontier, ExploreType::Hazardous, ExploreType::Industrial].into_iter().enumerate() {
        let cards: Vec<Explore> = Explore::ALL.into_iter().filter(|e| e.data().deck == t).take(6).collect();
        d.explore[t as usize] = Deck::new(cards);
        d.explore_present |= 1 << i as u8;
    }
    st.cards_mut().discarded_action.edit().extend(ActionCard::ALL[60..64].iter().copied());
    st.agenda_mut().laws.insert(Agenda::FleetRegulations, Outcome::Empty);
    st.objectives_mut().revealed.push(ti4_engine_core::state::ObjectiveRef::Public(PublicObjective::ALL[6]));
    let _ = Planet::COUNT;
    st
}

fn play_rig(card: ActionCard) -> Option<Rig<Play>> {
    let st = rich_state();
    let play = Play::of(&st, card, Seat(0), env())?;
    Some(Rig::from_parts(st, ti4_engine_core_tests::kernel_all(), play).with_table(linked()).seeded(card.index() as u64 + 1))
}



fn solo(card: ActionCard, st: CoreState, seed: u64) -> Option<Rig<Play>> {
    let play = Play::of(&st, card, Seat(0), env())?;
    Some(Rig::from_parts(st, ti4_engine_core_tests::kernel_all(), play).with_table(linked()).seeded(seed))
}

fn units_on(s: &CoreState, planet: Planet, seat: u8) -> Vec<Unit> {
    s.board().planet(planet).units.iter().filter(|p| p.owner() == Seat(seat)).map(|p| p.kind()).collect()
}

fn goods(s: &CoreState, seat: u8) -> i16 {
    s.players().by_seat(Seat(seat)).unwrap().econ.trade_goods
}

fn planet_of(i: usize) -> Planet {
    plain_systems()[i].data().planets[0]
}

fn sys_of(i: usize) -> System {
    plain_systems()[i]
}

fn count_in(s: &CoreState, system: System, seat: u8, kind: Unit) -> usize {
    s.board().sys(system).units.iter().filter(|p| p.owner() == Seat(seat) && p.kind() == kind).count()
}

#[test]
fn cripple_defenses_destroys_each_pds_on_the_chosen_planet_and_only_the_pds() {
    covers_card!(ActionCard::CrippleDefenses);
    let mut rig = play_rig(ActionCard::CrippleDefenses).expect("a play");
    rig.run();
    assert!(rig.is_done(), "one planet with a PDS: the choice is taken unasked");
    let p = planet_of(6);
    assert_eq!(units_on(&rig.state, p, 1), vec![Unit::InfantryI, Unit::InfantryI], "the PDS is gone, the infantry stay");
    // Two planets with a PDS: asked, and only the chosen one is cleared (several PDS on it all go).
    let sc = four().control(planet_of(3), 1).unit(sys_of(3), Some(planet_of(3)), Unit::PDSII, 1, 2).control(planet_of(5), 2).unit(sys_of(5), Some(planet_of(5)), Unit::PDSI, 2, 1);
    let mut two = solo(ActionCard::CrippleDefenses, sc.state(), 1).expect("a play");
    two.run();
    assert_eq!(two.opts().len(), 2);
    two.assert_options_sound();
    two.pick(0);
    assert!(two.is_done());
    let left = |p: Planet| two.state.board().planet(p).units.iter().filter(|u| matches!(u.kind(), Unit::PDSI | Unit::PDSII)).count();
    assert_eq!((left(planet_of(3)), left(planet_of(5))), (0, 1), "both PDS II of the first planet, none of the other");
}

#[test]
fn frontline_deployment_places_three_infantry_on_one_planet_we_control() {
    covers_card!(ActionCard::FrontlineDeployment);
    let mut rig = play_rig(ActionCard::FrontlineDeployment).expect("a play");
    rig.run();
    let ours = [planet_of(4), planet_of(5)];
    let offered: Vec<Planet> = rig.opts().iter().filter_map(|o| if let Opt::Cards(ti4_engine_core::choice::cards::Opt::Spot(_, p)) = o { Some(*p) } else { None }).collect();
    assert_eq!(offered, ours.to_vec(), "exactly the planets we control");
    rig.assert_options_sound();
    let before = rig.state.board().planet(planet_of(5)).units.iter().filter(|u| u.kind() == Unit::InfantryI).count();
    rig.pick(1);
    assert!(rig.is_done());
    let after = rig.state.board().planet(planet_of(5)).units.iter().filter(|u| u.kind() == Unit::InfantryI).count();
    assert_eq!(after, before + 3, "three infantry");
    assert_eq!(units_on(&rig.state, planet_of(4), 0).len(), 3, "the other planet is untouched");
}

#[test]
fn industrial_initiative_pays_one_trade_good_for_each_industrial_planet_we_control() {
    covers_card!(ActionCard::IndustrialInitiative);
    let st = rich_state();
    let industrial = ti4_engine_core::query::board::controlled_planets(&st, Seat(0)).as_slice().iter().filter(|(_, p)| p.data().traits.contains(&ti4_engine_core::generated::PlanetTrait::Industrial)).count();
    assert!(industrial > 0, "the fixture holds an industrial planet");
    let mut rig = play_rig(ActionCard::IndustrialInitiative).expect("a play");
    rig.run();
    assert!(rig.is_done());
    assert_eq!(goods(&rig.state, 0), goods(&st, 0) + industrial as i16);
    // Planets we do not control pay nothing: a seat with none gains nothing.
    let mut none = solo(ActionCard::IndustrialInitiative, four().state(), 1).expect("a play");
    none.run();
    assert_eq!(goods(&none.state, 0), 0);
}

#[test]
fn insubordination_takes_a_token_from_the_chosen_players_tactic_pool_only_and_not_from_players_without_one() {
    covers_card!(ActionCard::Insubordination);
    let mut rig = play_rig(ActionCard::Insubordination).expect("a play");
    rig.run();
    assert_eq!(rig.opts().len(), 3, "every other player with a token in the pool");
    rig.assert_options_sound();
    let before: Vec<i16> = (0..4).map(|i| rig.state.players().by_seat(Seat(i)).unwrap().econ.tactic_tokens).collect();
    rig.pick_opt(Opt::Cards(ti4_engine_core::choice::cards::Opt::Seat(Seat(2))));
    assert!(rig.is_done());
    let after: Vec<i16> = (0..4).map(|i| rig.state.players().by_seat(Seat(i)).unwrap().econ.tactic_tokens).collect();
    assert_eq!(after, vec![before[0], before[1], before[2] - 1, before[3]]);
    let empty = four().tokens(1, 0, 0, 0).tokens(2, 0, 0, 0).tokens(3, 0, 1, 0).state();
    let rig = solo(ActionCard::Insubordination, empty, 1);
    assert!(rig.is_none() || { let mut r = rig.unwrap(); r.run(); r.is_done() }, "nobody has a tactic token: nothing to choose");
}

#[test]
fn rise_of_a_messiah_places_one_infantry_on_each_planet_we_control() {
    covers_card!(ActionCard::RiseOfAMessiah);
    let st = rich_state();
    let ours: Vec<Planet> = ti4_engine_core::query::board::controlled_planets(&st, Seat(0)).as_slice().iter().map(|(_, p)| *p).collect();
    let mut rig = play_rig(ActionCard::RiseOfAMessiah).expect("a play");
    rig.run();
    assert!(rig.is_done());
    for p in &ours {
        let before = st.board().planet(*p).units.iter().filter(|u| u.owner() == Seat(0) && u.kind() == Unit::InfantryI).count();
        let after = rig.state.board().planet(*p).units.iter().filter(|u| u.owner() == Seat(0) && u.kind() == Unit::InfantryI).count();
        assert_eq!(after, before + 1, "{p:?}");
    }
    assert_eq!(units_on(&rig.state, planet_of(6), 0).len(), 0, "a planet of another player gets nothing");
}

#[test]
fn reactor_meltdown_destroys_a_space_dock_outside_home_systems_only() {
    covers_card!(ActionCard::ReactorMeltdown);
    let home = System::ALL.iter().copied().find(|s| s.data().home_system && !s.data().planets.is_empty() && !s.data().hyperlane).expect("a home system tile");
    let hp = home.data().planets[0];
    let sc = four().map_from(&[sys_of(0), sys_of(1), home]).control(planet_of(1), 1).unit(sys_of(1), Some(planet_of(1)), Unit::SpaceDockI, 1, 1).control(hp, 2).unit(home, Some(hp), Unit::SpaceDockI, 2, 1);
    let mut rig = solo(ActionCard::ReactorMeltdown, sc.state(), 1).expect("a play");
    rig.run();
    assert!(rig.is_done(), "only the dock outside a home system is a candidate: taken unasked");
    assert!(units_on(&rig.state, planet_of(1), 1).is_empty(), "that dock is destroyed");
    assert_eq!(units_on(&rig.state, hp, 2), vec![Unit::SpaceDockI], "the home system's dock stays");
}

#[test]
fn scuttle_returns_one_or_two_of_our_non_fighter_ships_and_pays_their_combined_cost() {
    covers_card!(ActionCard::Scuttle);
    let mut rig = play_rig(ActionCard::Scuttle).expect("a play");
    rig.run();
    assert_eq!(rig.opts().len(), 5, "every non-fighter ship of ours, never the fighters, never another player's");
    rig.assert_options_sound();
    let before = (goods(&rig.state, 0), count_in(&rig.state, sys_of(1), 0, Unit::DreadnoughtI), count_in(&rig.state, sys_of(1), 0, Unit::CruiserI));
    rig.pick(2);
    assert!(!rig.is_done(), "up to two ships: a second question");
    rig.assert_options_sound();
    rig.pick_opt(Opt::Cards(ti4_engine_core::choice::cards::Opt::Stop));
    assert!(rig.is_done());
    assert_eq!(count_in(&rig.state, sys_of(1), 0, Unit::DreadnoughtI), before.1 - 1, "the dreadnought is returned");
    assert_eq!(goods(&rig.state, 0), before.0 + 4, "a dreadnought costs 4");
    assert_eq!(count_in(&rig.state, sys_of(1), 0, Unit::CruiserI), before.2);
}

#[test]
fn unexpected_action_returns_one_of_our_command_tokens_from_the_board() {
    covers_card!(ActionCard::UnexpectedAction);
    let st = rich_state();
    let on_board = |s: &CoreState| plain_systems().iter().filter(|sy| s.board().sys(**sy).cmd.contains(Seat(0))).count();
    let before = on_board(&st);
    let mut rig = play_rig(ActionCard::UnexpectedAction).expect("a play");
    rig.run();
    assert!(rig.is_done(), "one token on the board: taken unasked");
    assert_eq!(on_board(&rig.state), before - 1);
    assert!(!rig.state.board().sys(sys_of(2)).cmd.contains(Seat(0)));
    assert!(rig.state.board().sys(sys_of(3)).cmd.contains(Seat(1)), "another player's token stays");
}

#[test]
fn war_effort_places_a_cruiser_in_a_system_with_our_ships() {
    covers_card!(ActionCard::WarEffort);
    let mut rig = play_rig(ActionCard::WarEffort).expect("a play");
    rig.run();
    let systems: Vec<System> = rig.opts().iter().filter_map(|o| if let Opt::Cards(ti4_engine_core::choice::cards::Opt::System(s)) = o { Some(*s) } else { None }).collect();
    assert_eq!(systems, vec![sys_of(1), sys_of(2), sys_of(3)], "exactly the systems that hold ships of ours");
    rig.assert_options_sound();
    let before = count_in(&rig.state, sys_of(3), 0, Unit::CruiserI);
    rig.pick(2);
    assert!(rig.is_done());
    assert_eq!(count_in(&rig.state, sys_of(3), 0, Unit::CruiserI), before + 1);
}

#[test]
fn plague_destroys_one_infantry_for_each_die_that_shows_six_or_more() {
    covers_card!(ActionCard::Plague);
    let (mut some, mut none) = (false, false);
    for seed in 0..30u64 {
        let mut rig = play_rig(ActionCard::Plague).expect("a play").seeded(seed);
        rig.run();
        rig.assert_options_sound();
        let target = planet_of(2);
        let idx = rig.opts().iter().position(|o| matches!(o, Opt::Cards(ti4_engine_core::choice::cards::Opt::Spot(_, p)) if *p == target)).expect("the planet with two infantry of Sol");
        rig.pick(idx);
        assert!(rig.is_done());
        let faces = rig.driver.window.faces().to_vec();
        assert_eq!(faces.len(), 2, "one die for each infantry on the planet");
        assert!(faces.iter().all(|f| (1..=10).contains(f)));
        let killed = faces.iter().filter(|f| **f >= 6).count();
        assert_eq!(units_on(&rig.state, target, 2).len(), 2 - killed, "seed {seed}");
        some |= killed > 0;
        none |= killed == 0;
        assert_eq!(units_on(&rig.state, planet_of(6), 1).len(), 3, "other planets are untouched");
    }
    assert!(some && none, "the seeds cover both outcomes");
}

#[test]
fn unstable_planet_exhausts_a_hazardous_planet_and_destroys_up_to_three_infantry_on_it() {
    covers_card!(ActionCard::UnstablePlanet);
    let mut rig = play_rig(ActionCard::UnstablePlanet).expect("a play");
    rig.run();
    let spots: Vec<Planet> = rig.opts().iter().filter_map(|o| if let Opt::Cards(ti4_engine_core::choice::cards::Opt::Spot(_, p)) = o { Some(*p) } else { None }).collect();
    assert!(spots.iter().all(|p| p.data().traits.contains(&ti4_engine_core::generated::PlanetTrait::Hazardous)), "only hazardous planets: {spots:?}");
    assert!(!spots.is_empty());
    rig.assert_options_sound();
    // The planet with 3 of our infantry: all three go, and it is exhausted.
    let idx = spots.iter().position(|p| *p == planet_of(4)).expect("Lesab is hazardous in the fixture");
    rig.pick(idx);
    assert!(rig.is_done());
    assert!(units_on(&rig.state, planet_of(4), 0).is_empty());
    assert!(ti4_engine_core::query::board::exhausted(&rig.state, planet_of(4)));
    // More than three infantry: exactly three go.
    let many = four().control(planet_of(4), 1).unit(sys_of(4), Some(planet_of(4)), Unit::InfantryI, 1, 5);
    let mut five = solo(ActionCard::UnstablePlanet, many.state(), 1).expect("a play");
    five.run();
    assert!(five.is_done(), "one hazardous planet: taken unasked");
    assert_eq!(units_on(&five.state, planet_of(4), 1).len(), 2, "up to three");
}

#[test]
fn ancient_burial_sites_exhausts_each_cultural_planet_of_the_chosen_player_and_only_those() {
    covers_card!(ActionCard::AncientBurialSites);
    let cultural: Vec<Planet> = plain_systems().iter().flat_map(|s| s.data().planets.iter().copied()).filter(|p| p.data().traits.contains(&ti4_engine_core::generated::PlanetTrait::Cultural)).take(2).collect();
    let other = plain_systems().iter().flat_map(|s| s.data().planets.iter().copied()).find(|p| !p.data().traits.contains(&ti4_engine_core::generated::PlanetTrait::Cultural) && !cultural.contains(p)).unwrap();
    let mut sc = four().map_ring(12);
    for p in &cultural {
        sc = sc.control(*p, 1);
    }
    sc = sc.control(other, 1);
    let mut rig = solo(ActionCard::AncientBurialSites, sc.state(), 1).expect("a play");
    rig.run();
    rig.assert_options_sound();
    rig.pick_opt(Opt::Cards(ti4_engine_core::choice::cards::Opt::Seat(Seat(1))));
    assert!(rig.is_done());
    for p in &cultural {
        assert!(ti4_engine_core::query::board::exhausted(&rig.state, *p), "{p:?}");
    }
    assert!(!ti4_engine_core::query::board::exhausted(&rig.state, other), "a planet of another trait stays ready");
}

#[test]
fn tactical_bombardment_exhausts_the_planets_of_others_in_a_system_where_we_have_bombardment() {
    covers_card!(ActionCard::TacticalBombardment);
    let mut rig = play_rig(ActionCard::TacticalBombardment).expect("a play");
    rig.run();
    let systems: Vec<System> = rig.opts().iter().filter_map(|o| if let Opt::Cards(ti4_engine_core::choice::cards::Opt::System(s)) = o { Some(*s) } else { None }).collect();
    assert_eq!(systems, vec![sys_of(1), sys_of(2)], "PINNED (D4A2-found-01): the card text says a system with a unit of ours that has BOMBARDMENT, but both engines offer every system with a ship of ours and a rival planet: the destroyer-only Andeara is listed");
    rig.assert_options_sound();
    let before_other = ti4_engine_core::query::board::exhausted(&rig.state, planet_of(2));
    rig.pick(0);
    assert!(rig.is_done());
    assert!(ti4_engine_core::query::board::exhausted(&rig.state, planet_of(1)), "the planet of another player in the chosen system is exhausted");
    assert_eq!(ti4_engine_core::query::board::exhausted(&rig.state, planet_of(2)), before_other, "the other system's planet is untouched");
}

#[test]
fn fighter_conscription_places_a_fighter_where_we_have_a_space_dock_unless_an_enemy_ship_is_there() {
    covers_card!(ActionCard::FighterConscription);
    let mut rig = play_rig(ActionCard::FighterConscription).expect("a play");
    rig.run();
    assert!(rig.is_done());
    assert_eq!(count_in(&rig.state, sys_of(5), 0, Unit::FighterI), 1, "the dock's system gets a fighter");
    let total: usize = plain_systems().iter().map(|s| count_in(&rig.state, *s, 0, Unit::FighterI)).sum();
    let before: usize = plain_systems().iter().map(|s| count_in(&rich_state(), *s, 0, Unit::FighterI)).sum();
    assert!(total >= before + 1);
    // An enemy ship in the dock's system forbids it.
    let blocked = four().control(planet_of(5), 0).unit(sys_of(5), Some(planet_of(5)), Unit::SpaceDockI, 0, 1).unit(sys_of(5), None, Unit::CruiserI, 1, 1);
    let mut rig = solo(ActionCard::FighterConscription, blocked.state(), 1).expect("a play");
    rig.run();
    assert_eq!(count_in(&rig.state, sys_of(5), 0, Unit::FighterI), 0);
}

#[test]
fn signal_jamming_puts_another_players_command_token_in_a_system_near_our_ships() {
    covers_card!(ActionCard::SignalJamming);
    let mut rig = play_rig(ActionCard::SignalJamming).expect("a play");
    rig.run();
    rig.assert_options_sound();
    let before = rig.state.clone();
    rig.pick(0);
    let first = rig.opts();
    if !rig.is_done() {
        assert!(!first.is_empty());
        rig.pick(0);
    }
    assert!(rig.is_done());
    let tokens_added: usize = plain_systems().iter().map(|s| rig.state.board().sys(*s).cmd.len() - before.board().sys(*s).cmd.len()).sum();
    assert_eq!(tokens_added, 1, "exactly one token is placed");
    assert!(!rig.state.board().sys(sys_of(0)).cmd.contains(Seat(0)), "never one of ours");
}
