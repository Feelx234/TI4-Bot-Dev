//! D4A4: windows that had no rule test: Fabrication / Black Market Forgery (`rules::relic_windows`), the Titan relic's structure
//! (`TitanBuild`), `PayStep`, Self-Assembly Routines (factions crate), `ControlGained`, the Imperial secondary, Courageous to the End's
//! `Destroy` and the hits the modules produce at the start of a space combat (`ProduceHits`).

use ti4_engine_core::choice::combat::{Ask as CAsk, Opt as COpt};
use ti4_engine_core::choice::relic::{Ask as RAsk, Opt as ROpt};
use ti4_engine_core::choice::strategy::{Opt as SOpt, Pool};
use ti4_engine_core::choice::technology::{Ask as TAsk, Opt as TOpt};
use ti4_engine_core::containers::Deck;
use ti4_engine_core::generated::{ActionCard, ExploreType, Planet, PromissoryNote, Relic, SecretObjective, Source, Technology as T};
use ti4_engine_core::kernel::children::{HitTexts, StructureFacts};
use ti4_engine_core::kernel::hooks::CombatMoment;
use ti4_engine_core::query::economy::Spend;
use ti4_engine_core::rules::combat::effect_hits::Destroy;
use ti4_engine_core::rules::combat::produce::{Facts as ProduceFacts, ProduceHits};
use ti4_engine_core::rules::invasion::control::{ControlGained, Facts as ControlFacts};
use ti4_engine_core::rules::production::pay::{PayStep, Stage as PayStage};
use ti4_engine_core::rules::production::strategy_cards::TitanBuild;
use ti4_engine_core::rules::relic_windows::{FabMode, Fabrication};
use ti4_engine_core::rules::strategy_primaries::ImperialSecondary;
use ti4_engine_core::state::{Code, NoteKey};
use ti4_engine_core_factions::self_assembly::SelfAssembly;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::rig::Policy;
use ti4_engine_core_tests::scenario::{plain_systems, system_of};

fn two(a: Faction, b: Faction) -> Scenario {
    Scenario::new(&[a, b])
}

fn rich_planet() -> Planet {
    plain_systems().into_iter().flat_map(|s| s.data().planets.iter().copied().collect::<Vec<_>>()).find(|p| p.data().resources >= 3).expect("a 3-resource planet")
}

fn count(s: &CoreState, p: Planet, seat: u8, unit: Unit) -> usize {
    s.board().planet(p).units.as_slice().iter().filter(|u| u.owner() == Seat(seat) && u.kind() == unit).count()
}

// ---- Fabrication ------------------------------------------------------------------------------

fn naaz() -> Scenario {
    two(Faction::TheNaazRokhaAlliance, Faction::TheArborec)
}

fn with_fragments(sc: Scenario, frags: &[(ExploreType, i16)]) -> CoreState {
    let mut s = sc.state();
    s.decks_mut().relic.set(vec![Relic::ALL[3], Relic::ALL[4]]);
    for (k, n) in frags {
        s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.fragments.set(*k, *n);
    }
    s
}

fn frag(s: &CoreState, kind: ExploreType) -> i16 {
    s.players().by_seat(Seat(0)).unwrap().cards.fragments.get(kind).unwrap_or(0)
}

#[test]
fn fabrication_purges_two_fragments_of_one_type_for_the_top_relic_asking_the_type_when_several_qualify() {
    covers!("rules::relic_windows::Fabrication");
    let st = || with_fragments(naaz(), &[(ExploreType::Cultural, 2), (ExploreType::Hazardous, 3), (ExploreType::Industrial, 1)]);
    let mut rig = Rig::from_parts(st(), kernel_all(), Fabrication::new(Seat(0), FabMode::Relic { bmf: false }, Code(9)));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Relic(RAsk::FragmentType { count: 2 })));
    assert_eq!(rig.opts(), vec![Opt::Relic(ROpt::Fragment(ExploreType::Cultural)), Opt::Relic(ROpt::Fragment(ExploreType::Hazardous))], "only types with two fragments");
    rig.assert_options_sound();
    rig.pick_opt(Opt::Relic(ROpt::Fragment(ExploreType::Hazardous)));
    assert!(rig.is_done() && rig.driver.window.done);
    assert_eq!(frag(&rig.state, ExploreType::Hazardous), 1, "purge 2");
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().cards.relics.as_slice(), &[Relic::ALL[3]], "the top relic");
    smoke("windows_misc/fabrication_relic", || Rig::from_parts(st(), kernel_all(), Fabrication::new(Seat(0), FabMode::Relic { bmf: false }, Code(9))));
}

#[test]
fn fabrication_counts_frontier_fragments_as_any_type_and_needs_two_fragments_and_a_relic_in_the_deck() {
    covers!("rules::relic_windows::Fabrication");
    let mut frontier = Rig::from_parts(with_fragments(naaz(), &[(ExploreType::Frontier, 2)]), kernel_all(), Fabrication::new(Seat(0), FabMode::Relic { bmf: false }, Code(9)));
    frontier.run();
    assert!(frontier.is_done() && frontier.driver.window.done, "only frontier fragments: one choice, taken unasked");
    assert_eq!(frag(&frontier.state, ExploreType::Frontier), 0);
    let mut mixed = Rig::from_parts(with_fragments(naaz(), &[(ExploreType::Cultural, 1), (ExploreType::Frontier, 1)]), kernel_all(), Fabrication::new(Seat(0), FabMode::Relic { bmf: false }, Code(9)));
    mixed.run();
    assert!(mixed.is_done() && mixed.driver.window.done);
    assert_eq!((frag(&mixed.state, ExploreType::Cultural), frag(&mixed.state, ExploreType::Frontier)), (0, 0), "the matching one first, the shortfall from the frontier");
    let mut short = Rig::from_parts(with_fragments(naaz(), &[(ExploreType::Cultural, 1)]), kernel_all(), Fabrication::new(Seat(0), FabMode::Relic { bmf: false }, Code(9)));
    short.run();
    assert!(short.is_done() && !short.driver.window.done, "one fragment is not enough");
    let mut empty_deck = with_fragments(naaz(), &[(ExploreType::Cultural, 2)]);
    empty_deck.decks_mut().relic.set(vec![]);
    let mut nodeck = Rig::from_parts(empty_deck, kernel_all(), Fabrication::new(Seat(0), FabMode::Relic { bmf: false }, Code(9)));
    nodeck.run();
    assert!(nodeck.is_done() && !nodeck.driver.window.done);
    assert_eq!(frag(&nodeck.state, ExploreType::Cultural), 2, "nothing is purged when no relic can be gained");
}

#[test]
fn fabrication_may_purge_one_fragment_for_a_command_token_of_a_chosen_pool() {
    covers!("rules::relic_windows::Fabrication");
    let st = || with_fragments(naaz().tokens(0, 2, 2, 2), &[(ExploreType::Cultural, 1), (ExploreType::Industrial, 1)]);
    let mut rig = Rig::from_parts(st(), kernel_all(), Fabrication::new(Seat(0), FabMode::Token, Code(9)));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Relic(RAsk::FragmentType { count: 1 })));
    assert_eq!(rig.opts().len(), 2);
    rig.pick_opt(Opt::Relic(ROpt::Fragment(ExploreType::Industrial)));
    assert!(matches!(rig.asked().ask, Ask::Relic(RAsk::TokenPool)));
    assert_eq!(rig.opts().len(), 3, "the three pools");
    rig.assert_options_sound();
    rig.pick_opt(Opt::Relic(ROpt::Pool(Pool::Fleet)));
    assert!(rig.is_done() && rig.driver.window.done);
    assert_eq!(frag(&rig.state, ExploreType::Industrial), 0, "purge 1 fragment");
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.fleet_tokens, 3, "gain 1 command token");
    smoke("windows_misc/fabrication_token", || Rig::from_parts(st(), kernel_all(), Fabrication::new(Seat(0), FabMode::Token, Code(9))));
}

#[test]
fn black_market_forgery_needs_the_note_and_returns_it_to_the_naaz_player_after_the_relic() {
    covers!("rules::relic_windows::Fabrication");
    let key = NoteKey { note: PromissoryNote::BlackMarketForgery, faction: Faction::TheNaazRokhaAlliance };
    let sc = || two(Faction::TheArborec, Faction::TheNaazRokhaAlliance);
    let st = |held: bool| {
        let mut s = with_fragments(sc(), &[(ExploreType::Cultural, 2)]);
        if held {
            s.cards_mut().promissory.edit().insert(key, Seat(0));
        }
        s
    };
    let mut rig = Rig::from_parts(st(true), kernel_all(), Fabrication::new(Seat(0), FabMode::Relic { bmf: true }, Code(10)));
    rig.run();
    assert!(rig.is_done() && rig.driver.window.done);
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().cards.relics.len(), 1);
    assert_eq!(rig.state.cards().promissory.get(&key), Some(&Seat(1)), "return this card to the Naaz player");
    let mut without = Rig::from_parts(st(false), kernel_all(), Fabrication::new(Seat(0), FabMode::Relic { bmf: true }, Code(10)));
    without.run();
    assert!(without.is_done() && !without.driver.window.done, "a seat that holds no note cannot play it");
    let mut no_room = Rig::from_parts(with_fragments(naaz().tokens(0, 8, 8, 8), &[(ExploreType::Cultural, 1)]), kernel_all(), Fabrication::new(Seat(0), FabMode::Token, Code(9)));
    no_room.run();
    assert!(no_room.is_done() && !no_room.driver.window.done, "no command token left in the reinforcements to gain");
    assert_eq!(frag(&no_room.state, ExploreType::Cultural), 1);
}

// ---- TitanBuild and PayStep -------------------------------------------------------------------

fn structure_facts() -> StructureFacts {
    StructureFacts { pds: Some((Unit::PDSI, false)), dock: Some((Unit::SpaceDockI, false)), source: Code(3), alt: None }
}

#[test]
fn the_titan_prototype_pays_three_resources_and_places_a_structure_otherwise_the_consolation_is_a_trade_good() {
    covers!("rules::production::strategy_cards::TitanBuild");
    let rich = rich_planet();
    let sc = || two(Faction::TheArborec, Faction::TheEmbersOfMuaat).map_from(&[system_of(rich)]).control(rich, 0);
    let mut rig = Rig::new(sc(), TitanBuild::new(Seat(0), structure_facts(), Code(4)));
    rig.run();
    assert!(rig.stages.iter().any(|s| s == "Paying" || s == "Placing"), "{:?}", rig.stages);
    rig.auto(Policy::First);
    assert!(rig.is_done());
    if rig.driver.window.built {
        assert!(count(&rig.state, rich, 0, Unit::PDSI) + count(&rig.state, rich, 0, Unit::SpaceDockI) >= 1, "the structure stands");
        assert!(ti4_engine_core::query::board::exhausted(&rig.state, rich), "3 resources were spent");
    }
    assert!(rig.driver.window.built, "{}", rig.transcript());
    smoke("windows_misc/titan_build", || Rig::new(sc(), TitanBuild::new(Seat(0), structure_facts(), Code(4))));
    // No planet to build on: the consolation prize.
    let mut none = Rig::new(two(Faction::TheArborec, Faction::TheEmbersOfMuaat).goods(0, 1), TitanBuild::new(Seat(0), structure_facts(), Code(4)));
    none.run();
    assert!(none.is_done() && !none.driver.window.built);
    assert_eq!(none.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 2, "1 trade good instead");
    // A planet but not 3 resources to spend: the same.
    let poor_planet = plain_systems().into_iter().flat_map(|s| s.data().planets.iter().copied().collect::<Vec<_>>()).find(|p| p.data().resources == 0 && p.data().source == Source::Base).expect("a 0-resource planet");
    let mut poor = Rig::new(two(Faction::TheArborec, Faction::TheEmbersOfMuaat).map_from(&[system_of(poor_planet)]).control(poor_planet, 0).goods(0, 0), TitanBuild::new(Seat(0), structure_facts(), Code(4)));
    poor.run();
    assert!(poor.is_done() && !poor.driver.window.built);
    assert_eq!(poor.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 1);
}

#[test]
fn one_pay_step_takes_the_option_chosen_and_a_lone_option_without_a_question() {
    covers!("rules::production::pay::PayStep");
    let a = plain_systems()[1].data().planets[0];
    let b = plain_systems()[2].data().planets[0];
    let step = || PayStep { who: Seat(0), kind: Spend::Resources, cost: 2, used_credit: 0, owed: 2, paid: 0, took: None, stage: PayStage::Paying };
    let sc = || two(Faction::TheArborec, Faction::TheEmbersOfMuaat).map_from(&[system_of(a), system_of(b)]).control(a, 0).control(b, 0).goods(0, 1);
    let mut rig = Rig::new(sc(), step());
    rig.run();
    assert!(rig.opts().len() >= 2, "two planets and a trade good to pay with");
    rig.assert_options_sound();
    rig.pick(0);
    assert!(rig.is_done());
    assert!(rig.driver.window.took.is_some_and(|w| w >= 1), "the worth taken is recorded");
    let lone = two(Faction::TheArborec, Faction::TheEmbersOfMuaat).map_from(&[system_of(a)]).control(a, 0);
    let mut one = Rig::new(lone, step());
    one.run();
    assert!(one.is_done(), "a single way to pay is taken unasked");
    assert!(one.driver.window.took.is_some());
    let mut none = Rig::new(two(Faction::TheArborec, Faction::TheEmbersOfMuaat), step());
    none.run();
    assert!(none.is_done() && none.driver.window.took.is_none(), "nothing to pay with");
}

// ---- Self-Assembly Routines -------------------------------------------------------------------

#[test]
fn self_assembly_routines_may_exhaust_to_place_a_mech_on_a_planet_in_the_system_that_produced() {
    covers!("factions::self_assembly::SelfAssembly");
    let ps = plain_systems();
    let (p1, p2) = (ps[1].data().planets[0], ps[1].data().planets.get(1).copied());
    let planets = [Some(p1), p2];
    let sc = || {
        let mut s = two(Faction::TheArborec, Faction::TheEmbersOfMuaat).map_from(&[ps[1]]).tech(0, T::SelfAssemblyRoutines).control(p1, 0);
        if let Some(p2) = p2 {
            s = s.control(p2, 0);
        }
        s
    };
    let mut rig = Rig::new(sc(), SelfAssembly::new(Seat(0), ps[1]));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Technology(TAsk::SelfAssembly)));
    assert_eq!(rig.opts().len(), planets.iter().flatten().count() + 1, "each planet of the system the seat controls, and a decline: a \"may\"");
    rig.assert_options_sound();
    rig.pick_where(|o| matches!(o, Opt::Technology(TOpt::Planet(p, _)) if *p == p1));
    assert!(rig.is_done());
    let mechs = rig.state.board().planet(p1).units.as_slice().iter().filter(|u| u.owner() == Seat(0) && u.kind().data().base_type == ti4_engine_core::generated::UnitBaseType::Mech).count();
    assert_eq!(mechs, 1, "1 mech from the reinforcements");
    assert!(rig.state.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(T::SelfAssemblyRoutines), "exhaust this card");
    smoke("windows_misc/self_assembly", || Rig::new(sc(), SelfAssembly::new(Seat(0), ps[1])));
    let mut dec = Rig::new(sc(), SelfAssembly::new(Seat(0), ps[1]));
    dec.run().decline();
    assert!(dec.is_done());
    assert_eq!(delta(dec.start_state(), &dec.state), "");
    let mut nocard = Rig::new(two(Faction::TheArborec, Faction::TheEmbersOfMuaat).map_from(&[ps[1]]).control(p1, 0), SelfAssembly::new(Seat(0), ps[1]));
    nocard.run();
    assert!(nocard.is_done(), "without the card there is no question");
    let mut elsewhere = Rig::new(sc(), SelfAssembly::new(Seat(0), ps[2]));
    elsewhere.run();
    assert!(elsewhere.is_done(), "no planet controlled in that system");
}

// ---- ControlGained ----------------------------------------------------------------------------

#[test]
fn a_planet_changing_hands_records_the_capture_and_destroys_the_structures_of_the_other_players_left_on_it() {
    covers!("rules::invasion::control::ControlGained");
    let planet = plain_systems()[1].data().planets[0];
    let system = system_of(planet);
    let sc = || {
        two(Faction::TheArborec, Faction::TheEmbersOfMuaat)
            .map_from(&[system])
            .control(planet, 0)
            .unit(system, Some(planet), Unit::InfantryI, 0, 2)
            .unit(system, Some(planet), Unit::PDSI, 1, 1)
            .unit(system, Some(planet), Unit::SpaceDockI, 1, 1)
    };
    let facts = |announce: bool| ControlFacts { invader: Seat(0), system, planet, previous: Some(Seat(1)), infiltrate_fix: true, announce };
    let mut rig = Rig::new(sc(), ControlGained::new(facts(false), None));
    rig.run();
    assert!(rig.is_done() && !rig.driver.window.unsupported, "a window that never asks");
    assert_eq!(rig.state.transient().combat.last_control_gained, Some((system, planet, Seat(0), Some(Seat(1)))), "the capture is recorded for the windows that follow");
    assert_eq!(count(&rig.state, planet, 1, Unit::PDSI) + count(&rig.state, planet, 1, Unit::SpaceDockI), 0, "the rival structures are destroyed");
    assert_eq!(count(&rig.state, planet, 0, Unit::InfantryI), 2, "the invader's units stay");
    let mut ann = Rig::new(sc(), ControlGained::new(facts(true), None));
    ann.run();
    assert!(ann.is_done());
    let mut fresh = Rig::new(sc(), ControlGained::new(ControlFacts { previous: None, infiltrate_fix: false, ..facts(false) }, None));
    fresh.run();
    assert_eq!(fresh.state.transient().combat.last_control_gained, Some((system, planet, Seat(0), None)));
    assert_eq!(count(&fresh.state, planet, 1, Unit::PDSI), 0);
    smoke_quiet(|| Rig::new(sc(), ControlGained::new(facts(false), None)));
}

#[test]
fn assimilate_replaces_the_pds_and_space_docks_of_the_other_players_with_the_l1z1x_own() {
    covers!("rules::invasion::control::ControlGained");
    let planet = plain_systems()[1].data().planets[0];
    let system = system_of(planet);
    let roster: &'static ti4_engine_core::query::tech::UnitRoster = Box::leak(Box::new(Default::default()));
    let sc = || two(Faction::TheL1Z1XMindnet, Faction::TheArborec).map_from(&[system]).control(planet, 0).unit(system, Some(planet), Unit::PDSI, 1, 1).unit(system, Some(planet), Unit::SpaceDockI, 1, 1).unit(system, Some(planet), Unit::InfantryI, 1, 1);
    let facts = ControlFacts { invader: Seat(0), system, planet, previous: Some(Seat(1)), infiltrate_fix: true, announce: false };
    let mut rig = Rig::new(sc(), ControlGained::new(facts, Some(roster)));
    rig.run();
    assert!(rig.is_done() && !rig.driver.window.unsupported);
    assert_eq!((count(&rig.state, planet, 0, Unit::PDSI), count(&rig.state, planet, 0, Unit::SpaceDockI)), (1, 1), "replaced with the L1Z1X structures");
    assert_eq!((count(&rig.state, planet, 1, Unit::PDSI), count(&rig.state, planet, 1, Unit::SpaceDockI)), (0, 0));
    let mut missing = Rig::new(sc(), ControlGained::new(facts, None));
    missing.run();
    assert!(missing.driver.window.unsupported, "no roster: Assimilate cannot be followed, the caller declines");
}

// ---- Imperial secondary -----------------------------------------------------------------------

#[test]
fn the_imperial_secondary_draws_a_secret_and_over_the_hand_limit_asks_which_unscored_secret_goes_back() {
    covers!("rules::strategy_primaries::ImperialSecondary");
    let deck = || Deck::new(vec![SecretObjective::ALL[7], SecretObjective::ALL[8]]);
    let st = |held: usize| {
        let mut s = two(Faction::TheArborec, Faction::TheEmbersOfMuaat).state();
        s.decks_mut().secret = deck();
        for i in 0..held {
            s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.secrets.push(SecretObjective::ALL[i]);
        }
        s
    };
    let mut rig = Rig::from_parts(st(1), kernel_all(), ImperialSecondary::new(Seat(0)));
    rig.run();
    assert!(rig.is_done() && !rig.driver.window.unsupported);
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().cards.secrets.as_slice(), &[SecretObjective::ALL[0], SecretObjective::ALL[7]], "draw 1 secret objective");
    let mut over = Rig::from_parts(st(3), kernel_all(), ImperialSecondary::new(Seat(0)));
    over.run();
    assert!(!over.is_done(), "four secrets: over the hand limit of three");
    over.assert_options_sound();
    over.pick(0);
    assert!(over.is_done());
    assert_eq!(over.state.players().by_seat(Seat(0)).unwrap().cards.secrets.len(), 3);
    let mut empty = two(Faction::TheArborec, Faction::TheEmbersOfMuaat).state();
    empty.decks_mut().secret = Deck::new(vec![]);
    let mut none = Rig::from_parts(empty, kernel_all(), ImperialSecondary::new(Seat(0)));
    none.run();
    assert!(none.is_done(), "an empty deck draws nothing");
    smoke("windows_misc/imperial_secondary", || Rig::from_parts(st(3), kernel_all(), ImperialSecondary::new(Seat(0))));
}

// ---- Destroy (Courageous to the End) ----------------------------------------------------------

#[test]
fn destroy_asks_the_owner_which_ship_goes_unless_there_is_only_one_or_none() {
    covers!("rules::combat::effect_hits::Destroy");
    let system = plain_systems()[1];
    let sc = |ships: &[(Unit, usize)]| {
        let mut s = two(Faction::TheArborec, Faction::TheEmbersOfMuaat).map_from(&[system]);
        for (u, n) in ships {
            s = s.unit(system, None, *u, 1, *n);
        }
        s.unit(system, None, Unit::CruiserI, 0, 1)
    };
    let mk = || Destroy::new(Seat(1), system, ActionCard::CourageousToTheEnd, Code(21), Code(22));
    let mut rig = Rig::new(sc(&[(Unit::CruiserI, 1), (Unit::DestroyerI, 1)]), mk());
    rig.run();
    assert_eq!(rig.asked().actor, Seat(1), "the owner of the ships chooses");
    assert!(matches!(rig.asked().ask, Ask::Combat(CAsk::AssignHit)));
    assert_eq!(rig.opts().len(), 2);
    rig.assert_options_sound();
    rig.pick_where(|o| matches!(o, Opt::Combat(COpt::Destroy { .. })));
    assert!(rig.is_done() && rig.driver.window.destroyed);
    let left: usize = rig.state.board().sys(system).units.as_slice().iter().filter(|u| u.owner() == Seat(1)).count();
    assert_eq!(left, 1);
    let pending = &rig.state.transient().combat.pending_destructions;
    assert_eq!(pending.len(), 1, "the destruction is staged for the announcement");
    assert_eq!(pending[0].owner, Seat(1));
    assert_eq!(pending[0].reason, Code(22));
    smoke("windows_misc/destroy", || Rig::new(sc(&[(Unit::CruiserI, 1), (Unit::DestroyerI, 1)]), mk()));
    let mut lone = Rig::new(sc(&[(Unit::CruiserI, 1)]), mk());
    lone.run();
    assert!(lone.is_done() && lone.driver.window.destroyed, "a single ship goes without a question");
    let mut none = Rig::new(sc(&[]), mk());
    none.run();
    assert!(none.is_done() && !none.driver.window.destroyed, "no ship of the owner: the card does nothing");
    let mut s = sc(&[(Unit::CruiserI, 1), (Unit::DestroyerI, 1)]).state();
    s.board_mut().sys_mut(system).units.clear();
    let _ = s;
}

// ---- ProduceHits ------------------------------------------------------------------------------

fn produce_facts(moment: CombatMoment, wormhole: bool) -> ProduceFacts {
    ProduceFacts { attacker: Seat(0), defender: Seat(1), system: plain_systems()[1], moment, announce: false, wormhole, ambush: Code(41), cause_core: Code(42), cause_devotion: Code(43) }
}

fn arena(a: Faction, b: Faction) -> Scenario {
    two(a, b).map_from(&[plain_systems()[1]])
}

fn texts() -> HitTexts {
    HitTexts { staging: false, commander: Code(1), casualty: Code(2), effect_hit: Code(3) }
}

#[test]
fn ambush_rolls_up_to_two_cruisers_or_destroyers_against_their_combat_value_and_the_opponent_assigns_every_hit() {
    covers!("rules::combat::produce::ProduceHits");
    let system = plain_systems()[1];
    let sc = || arena(Faction::TheMentakCoalition, Faction::TheArborec).unit(system, None, Unit::CruiserI, 0, 2).unit(system, None, Unit::DestroyerI, 0, 1).unit(system, None, Unit::CruiserI, 1, 1);
    let mut seen = [false; 3];
    for seed in 0..40u64 {
        let mut rig = Rig::new(sc(), ProduceHits::new(produce_facts(CombatMoment::CombatStart, false))).seeded(seed);
        rig.run();
        assert!(matches!(rig.asked().ask, Ask::Combat(CAsk::Ambush)));
        assert_eq!(rig.asked().actor, Seat(0));
        assert!(rig.opts().len() >= 5, "single ships, pairs of the same kind and of two kinds, and a decline: {:?}", rig.opts());
        if seed == 0 {
            rig.assert_options_sound();
        }
        rig.pick_where(|o| matches!(o, Opt::Combat(COpt::Ambush { a: Unit::CruiserI, b: Some(Unit::DestroyerI) })));
        rig.auto(Policy::First);
        assert!(rig.is_done());
        let drawn = rig.driver.window.drawn.clone();
        assert_eq!(drawn.len(), 2, "one die per ship");
        let hits = drawn.iter().filter(|(on, faces)| faces[0] >= *on).count();
        assert_eq!(rig.driver.window.produced[0].any_ship, hits, "each result equal to or greater than the ship's combat value is a hit");
        assert_eq!(drawn[0].0, Unit::CruiserI.data().combat.unwrap().hits_on as u32);
        seen[hits.min(2)] = true;
    }
    assert!(seen[0] && seen[1], "both misses and hits occur: {seen:?}");
    let mut dec = Rig::new(sc(), ProduceHits::new(produce_facts(CombatMoment::CombatStart, false)));
    dec.run().decline();
    assert!(dec.is_done() && dec.driver.window.drawn.is_empty());
    smoke("windows_misc/ambush", || Rig::new(sc(), ProduceHits::new(produce_facts(CombatMoment::CombatStart, false))).seeded(3));
}

#[test]
fn the_dimensional_splicer_produces_one_hit_in_a_wormhole_system_with_ships_on_both_sides() {
    covers!("rules::combat::produce::ProduceHits");
    let system = plain_systems()[1];
    let sc = || arena(Faction::TheGhostsOfCreuss, Faction::TheArborec).tech(0, T::DimensionalSplicer).unit(system, None, Unit::CruiserI, 0, 1).unit(system, None, Unit::CruiserI, 1, 1);
    let mut rig = Rig::new(sc(), ProduceHits::new(produce_facts(CombatMoment::CombatStart, true)));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Combat(CAsk::Splicer)));
    assert_eq!(rig.opts(), vec![Opt::Combat(COpt::ProduceHit), Opt::Decline]);
    rig.assert_options_sound();
    rig.pick(0);
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.produced[0].producer_assigned, 1, "the producer assigns this hit");
    let mut nowh = Rig::new(sc(), ProduceHits::new(produce_facts(CombatMoment::CombatStart, false)));
    nowh.run();
    assert!(nowh.is_done(), "no wormhole in the system");
    let mut late = Rig::new(sc(), ProduceHits::new(produce_facts(CombatMoment::RoundEnded, true)));
    late.run();
    assert!(late.is_done(), "only at the start of the combat");
    let mut alone = Rig::new(arena(Faction::TheGhostsOfCreuss, Faction::TheArborec).tech(0, T::DimensionalSplicer).unit(system, None, Unit::CruiserI, 0, 1), ProduceHits::new(produce_facts(CombatMoment::CombatStart, true)));
    alone.run();
    assert!(alone.is_done(), "ships on both sides are needed");
}

#[test]
fn impulse_core_and_devotion_destroy_a_cruiser_or_destroyer_for_one_hit() {
    covers!("rules::combat::produce::ProduceHits");
    let system = plain_systems()[1];
    let core = || arena(Faction::TheArborec, Faction::TheEmbersOfMuaat).tech(0, T::ImpulseCore).unit(system, None, Unit::CruiserI, 0, 1).unit(system, None, Unit::DestroyerI, 0, 1).unit(system, None, Unit::CruiserI, 1, 1);
    let mut rig = Rig::new(core(), ProduceHits::new(produce_facts(CombatMoment::CombatStart, false)));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Combat(CAsk::Sacrifice { devotion: false })));
    assert_eq!(rig.opts(), vec![Opt::Combat(COpt::Sacrifice { unit: Unit::CruiserI }), Opt::Combat(COpt::Sacrifice { unit: Unit::DestroyerI }), Opt::Decline]);
    rig.assert_options_sound();
    rig.pick_opt(Opt::Combat(COpt::Sacrifice { unit: Unit::DestroyerI }));
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.produced[0].non_fighter, 1, "Impulse Core: the opponent must assign it to a non-fighter if able");
    assert_eq!(rig.state.board().sys(system).units.as_slice().iter().filter(|u| u.owner() == Seat(0) && u.kind() == Unit::DestroyerI).count(), 0);
    let yin = || arena(Faction::TheYinBrotherhood, Faction::TheArborec).unit(system, None, Unit::CruiserI, 0, 1).unit(system, None, Unit::CruiserI, 1, 1);
    let mut dev = Rig::new(yin(), ProduceHits::new(produce_facts(CombatMoment::RoundEnded, false)));
    dev.run();
    assert!(matches!(dev.asked().ask, Ask::Combat(CAsk::Sacrifice { devotion: true })));
    dev.pick(0);
    assert!(dev.is_done());
    assert_eq!(dev.driver.window.produced[0].producer_assigned, 1, "Devotion: the Yin player assigns the hit");
    assert!(dev.state.marks().tail.iter().any(|(k, _)| matches!(k, ti4_engine_core::keys::MarkKey::YinAbilityUsed(_))), "a faction ability was used");
    let mut start = Rig::new(yin(), ProduceHits::new(produce_facts(CombatMoment::CombatStart, false)));
    start.run();
    assert!(start.is_done(), "Devotion is after a round, not at the start");
    let mut no_enemy = Rig::new(arena(Faction::TheYinBrotherhood, Faction::TheArborec).unit(system, None, Unit::CruiserI, 0, 1), ProduceHits::new(produce_facts(CombatMoment::RoundEnded, false)));
    no_enemy.run();
    assert!(no_enemy.is_done());
    let _ = texts();
}

#[allow(dead_code)]
fn unused(_: SOpt) {}
