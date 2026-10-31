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



/// The environment with the plain units the string side would hand to the cards that place them.
fn env_units() -> Env {
    let mut e = env();
    let placed = |u: Unit| Placed { unit: Some(u), on_card: 0 };
    e.facts.destroyer = placed(Unit::DestroyerI);
    e.facts.dreadnought = placed(Unit::DreadnoughtI);
    e.facts.mech = placed(Unit::FactionlessMech);
    e.facts.pds = placed(Unit::PDSI);
    e.facts.spacedock = placed(Unit::SpaceDockI);
    e
}

fn solo(card: ActionCard, st: CoreState, seed: u64) -> Option<Rig<Play>> {
    let play = Play::of(&st, card, Seat(0), env_units())?;
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
        assert_eq!(rig.driver.window.ability_roll(), None, "W15: only Courageous To The End's dice are noted as the roll of an ability");
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

#[test]
fn courageous_to_the_end_rolls_two_dice_and_costs_the_opponent_a_ship_for_each_die_at_the_lost_ships_combat_value() {
    covers_card!(ActionCard::CourageousToTheEnd);
    let (mut some, mut none) = (false, false);
    for seed in 0..40u64 {
        let mut st = four().map_ring(4).unit(sys_of(1), None, Unit::CruiserI, 1, 3).state();
        st.transient_mut().combat.last_ship_destroyed = Some((sys_of(1), Seat(0), Unit::DreadnoughtI));
        let mut rig = solo(ActionCard::CourageousToTheEnd, st, seed).expect("a play");
        rig.run();
        rig.auto(ti4_engine_core_tests::rig::Policy::First);
        assert!(rig.is_done());
        let faces = rig.driver.window.faces().to_vec();
        assert_eq!(faces.len(), 2, "two dice");
        let value = 5; // the dreadnought's combat value
        let hits = faces.iter().filter(|f| **f >= value).count();
        assert_eq!(count_in(&rig.state, sys_of(1), 1, Unit::CruiserI), 3 - hits, "seed {seed}: the opponent loses one ship for each die that reached {value}");
        assert_eq!(rig.state.transient().combat.pending_destructions.len(), hits, "the losses wait for the announcement of the play");
        some |= hits > 0;
        none |= hits == 0;
    }
    assert!(some && none);
    // The ship must be the holder's own and there must be an opponent with a ship there: otherwise no dice are rolled.
    let mut other = four().map_ring(4).unit(sys_of(1), None, Unit::CruiserI, 1, 2).state();
    other.transient_mut().combat.last_ship_destroyed = Some((sys_of(1), Seat(2), Unit::DreadnoughtI));
    let mut rig = solo(ActionCard::CourageousToTheEnd, other, 1).expect("a play");
    rig.run();
    assert!(rig.is_done() && rig.driver.window.faces().is_empty(), "someone else's ship was destroyed");
    let mut alone = four().map_ring(4).state();
    alone.transient_mut().combat.last_ship_destroyed = Some((sys_of(1), Seat(0), Unit::DreadnoughtI));
    let mut rig = solo(ActionCard::CourageousToTheEnd, alone, 1).expect("a play");
    rig.run();
    assert!(rig.is_done() && rig.driver.window.faces().is_empty(), "no opponent ship left");
}

/// W15: the two dice of Courageous To The End are the roll of an ability (the host keeps them in the ledger as the string notes them,
/// the holder's, no hit number); a play that rolled nothing (someone else's ship, no opponent left) has no roll to keep, and neither has
/// a card that rolls no ability dice.
#[test]
fn courageous_to_the_end_hands_its_two_dice_to_the_host_as_the_roll_of_an_ability() {
    covers_card!(ActionCard::CourageousToTheEnd);
    for seed in 0..6u64 {
        let mut st = four().map_ring(4).unit(sys_of(1), None, Unit::CruiserI, 1, 3).state();
        st.transient_mut().combat.last_ship_destroyed = Some((sys_of(1), Seat(0), Unit::DreadnoughtI));
        let mut rig = solo(ActionCard::CourageousToTheEnd, st, seed).expect("a play");
        rig.run();
        rig.auto(ti4_engine_core_tests::rig::Policy::First);
        let faces: Vec<u8> = rig.driver.window.faces().iter().map(|f| u8::try_from(*f).unwrap()).collect();
        assert_eq!(rig.driver.window.ability_roll(), Some((Seat(0), faces)), "seed {seed}");
    }
    let mut other = four().map_ring(4).unit(sys_of(1), None, Unit::CruiserI, 1, 2).state();
    other.transient_mut().combat.last_ship_destroyed = Some((sys_of(1), Seat(2), Unit::DreadnoughtI));
    let mut rig = solo(ActionCard::CourageousToTheEnd, other, 1).expect("a play");
    rig.run();
    assert_eq!(rig.driver.window.ability_roll(), None, "nothing was rolled");
}

#[test]
fn experimental_battlestation_fires_three_dice_at_five_for_each_hit_the_active_player_loses_a_ship() {
    covers_card!(ActionCard::ExperimentalBattlestation);
    let mut seen = [false; 4];
    for seed in 0..40u64 {
        let planet = planet_of(1);
        let st = four().map_ring(4).active(Some(1)).active_system(sys_of(1)).control(planet, 0).unit(sys_of(1), Some(planet), Unit::SpaceDockI, 0, 1).unit(sys_of(1), None, Unit::CruiserI, 1, 4).state();
        let mut rig = solo(ActionCard::ExperimentalBattlestation, st, seed).expect("a play");
        rig.run();
        rig.auto(ti4_engine_core_tests::rig::Policy::First);
        assert!(rig.is_done());
        let faces = rig.driver.window.faces().to_vec();
        assert_eq!(faces.len(), 3, "SPACE CANNON 5 (x3)");
        let hits = faces.iter().filter(|f| **f >= 5).count();
        seen[hits] = true;
        assert_eq!(count_in(&rig.state, sys_of(1), 1, Unit::CruiserI), 4 - hits, "seed {seed}");
        assert_eq!(count_in(&rig.state, sys_of(1), 0, Unit::CruiserI), 0);
    }
    assert!(seen.iter().all(|s| *s), "0..3 hits all occur: {seen:?}");
}

fn tokens_of(s: &CoreState, seat: u8) -> (i16, i16, i16) {
    let e = s.players().by_seat(Seat(seat)).unwrap().econ;
    (e.tactic_tokens, e.fleet_tokens, e.strategic_tokens)
}

fn token_on(s: &CoreState, system: System, seat: u8) -> bool {
    s.board().sys(system).cmd.contains(Seat(seat))
}

#[test]
fn forward_supply_base_pays_three_goods_and_one_to_a_chosen_other_player() {
    covers_card!(ActionCard::ForwardSupplyBase);
    // Seat 1 activated a system that holds seat 0's ships.
    let st = four().map_ring(4).active(Some(1)).active_system(sys_of(1)).unit(sys_of(1), None, Unit::CruiserI, 0, 1).goods(0, 2).goods(2, 5).state();
    let mut rig = solo(ActionCard::ForwardSupplyBase, st, 1).expect("a play");
    rig.run();
    rig.assert_options_sound();
    assert!(!rig.opts().contains(&Opt::Cards(ti4_engine_core::choice::cards::Opt::Seat(Seat(0)))), "another player gets the one trade good");
    rig.pick_opt(Opt::Cards(ti4_engine_core::choice::cards::Opt::Seat(Seat(2))));
    assert!(rig.is_done());
    assert_eq!(goods(&rig.state, 0), 2 + 3);
    assert_eq!(goods(&rig.state, 2), 5 + 1);
    assert_eq!(goods(&rig.state, 1), 0);
}

#[test]
fn counterstroke_returns_our_command_token_in_the_activated_system_to_our_tactic_pool() {
    covers_card!(ActionCard::Counterstroke);
    let st = four().map_ring(4).active(Some(1)).active_system(sys_of(2)).command_token(sys_of(2), 0).command_token(sys_of(3), 0).tokens(0, 1, 3, 2).state();
    let mut rig = solo(ActionCard::Counterstroke, st, 1).expect("a play");
    rig.run();
    assert!(rig.is_done());
    assert!(!token_on(&rig.state, sys_of(2), 0), "the token of the activated system is gone");
    assert!(token_on(&rig.state, sys_of(3), 0), "the token elsewhere stays");
    assert_eq!(tokens_of(&rig.state, 0), (2, 3, 2), "it went to the tactic pool");
}

#[test]
fn upgrade_replaces_one_cruiser_in_the_activated_system_with_a_dreadnought() {
    covers_card!(ActionCard::Upgrade);
    let st = four().map_ring(4).active(Some(0)).active_system(sys_of(1)).unit(sys_of(1), None, Unit::CruiserI, 0, 2).unit(sys_of(1), None, Unit::DestroyerI, 0, 1).state();
    let mut rig = solo(ActionCard::Upgrade, st, 1).expect("a play");
    rig.run();
    if !rig.is_done() {
        rig.pick(0);
    }
    assert!(rig.is_done());
    assert_eq!(count_in(&rig.state, sys_of(1), 0, Unit::CruiserI), 1, "one cruiser is replaced");
    assert_eq!(count_in(&rig.state, sys_of(1), 0, Unit::DreadnoughtI), 1, "by a dreadnought");
    assert_eq!(count_in(&rig.state, sys_of(1), 0, Unit::DestroyerI), 1);
    // No cruiser there: nothing happens.
    let none = four().map_ring(4).active(Some(0)).active_system(sys_of(1)).unit(sys_of(1), None, Unit::DestroyerI, 0, 1).state();
    if let Some(mut rig) = solo(ActionCard::Upgrade, none, 1) {
        rig.run();
        assert!(rig.is_done() && count_in(&rig.state, sys_of(1), 0, Unit::DreadnoughtI) == 0);
    }
}

#[test]
fn harness_energy_replenishes_our_commodities_to_the_faction_value() {
    covers_card!(ActionCard::HarnessEnergy);
    let st = four().map_ring(4).active(Some(0)).active_system(sys_of(1)).commodities(0, 0).state();
    let want = st.players().by_seat(Seat(0)).unwrap().faction.data().commodities;
    let mut rig = solo(ActionCard::HarnessEnergy, st, 1).expect("a play");
    rig.run();
    assert!(rig.is_done());
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.commodities, i16::from(want), "back to the printed value");
}

#[test]
fn infiltrate_swaps_the_pds_and_space_dock_of_a_planet_we_just_took_for_ours() {
    covers_card!(ActionCard::Infiltrate);
    let planet = planet_of(1);
    let mut st = four().map_ring(4).control(planet, 0).unit(sys_of(1), Some(planet), Unit::PDSI, 1, 1).unit(sys_of(1), Some(planet), Unit::SpaceDockI, 1, 1).unit(sys_of(1), Some(planet), Unit::InfantryI, 1, 1).state();
    st.transient_mut().combat.last_control_gained = Some((sys_of(1), planet, Seat(0), Some(Seat(1))));
    let play = Play::of(&st, ActionCard::Infiltrate, Seat(0), env_units());
    let play = play.expect("a play");
    let mut rig = Rig::from_parts(st, ti4_engine_core_tests::kernel_all(), play).with_table(linked()).seeded(1);
    rig.run();
    if !rig.is_done() {
        rig.pick(0);
    }
    rig.auto(ti4_engine_core_tests::rig::Policy::First);
    let pds_ours = units_on(&rig.state, planet, 0).iter().filter(|u| matches!(u, Unit::PDSI | Unit::PDSII)).count();
    let dock_ours = units_on(&rig.state, planet, 0).iter().filter(|u| matches!(u, Unit::SpaceDockI | Unit::SpaceDockII)).count();
    assert_eq!((pds_ours, dock_ours), (1, 1), "our own PDS and space dock replace theirs");
    assert!(units_on(&rig.state, planet, 1).iter().all(|u| *u == Unit::InfantryI), "their structures are gone, their infantry stays");
}
