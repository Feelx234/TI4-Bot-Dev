//! D4A4: the free technology windows of `rules::technology_windows` against the printed cards: Psychoarchaeology, Production Biomes, Vortex,
//! Wormhole Generator, the end of a turn (Predictive Intelligence, Bio-Stims), Sarween Tools / AI Development Algorithm, Transit Diodes.

use ti4_engine_core::choice::technology::{Ask as TAsk, Opt as TOpt};
use ti4_engine_core::generated::{Planet, Technology as T, Wormhole};
use ti4_engine_core::rules::technology_windows::{ProductionBiomes, ProductionUsed, Psychoarchaeology, TransitDiodes, TurnEnd, TurnStart, Vortex, WormholeGenerator};
use ti4_engine_core::state::Code;
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{plain_systems, system_of};

fn three() -> Scenario {
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec, Faction::TheYinBrotherhood])
}

fn t(o: TOpt) -> Opt {
    Opt::Technology(o)
}

fn tech_state(sc: Scenario, exhausted: &[T]) -> CoreState {
    let mut s = sc.state();
    for x in exhausted {
        s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.exhausted.insert(*x);
    }
    s
}

fn is_exhausted(s: &CoreState, tech: T) -> bool {
    s.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(tech)
}

fn goods(s: &CoreState, seat: u8) -> i16 {
    s.players().by_seat(Seat(seat)).unwrap().econ.trade_goods
}

/// Planets of plain systems with a technology specialty, and without.
fn specialty_planets(n: usize) -> Vec<Planet> {
    let mut out: Vec<Planet> = Vec::new();
    for s in plain_systems() {
        for p in s.data().planets {
            if !p.data().specialties.is_empty() && !out.iter().any(|q| system_of(*q) == s) {
                out.push(*p);
            }
        }
    }
    out.truncate(n);
    assert_eq!(out.len(), n, "enough specialty planets on plain tiles");
    out
}

fn plain_planet() -> Planet {
    plain_systems().into_iter().flat_map(|s| s.data().planets.iter().copied().collect::<Vec<_>>()).find(|p| p.data().specialties.is_empty()).expect("a planet without specialty")
}

fn map_of(ps: &[Planet]) -> Vec<System> {
    let mut v: Vec<System> = ps.iter().map(|p| system_of(*p)).collect();
    v.sort();
    v.dedup();
    v
}

const SRC: Code = Code(41);

// ---- Psychoarchaeology ------------------------------------------------------------------------

#[test]
fn psychoarchaeology_exhausts_a_ready_specialty_planet_for_one_trade_good_and_asks_again_while_one_is_left() {
    covers!("rules::technology_windows::Psychoarchaeology");
    let sp = specialty_planets(2);
    let sc = || three().map_from(&map_of(&sp)).tech(0, T::Psychoarchaeology).control(sp[0], 0).control(sp[1], 0).goods(0, 2);
    let mut rig = Rig::new(sc(), Psychoarchaeology::new(Seat(0), SRC));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Technology(TAsk::Psychoarchaeology)));
    assert_eq!(rig.opts().len(), 3, "two specialty planets and a decline");
    rig.assert_options_sound();
    rig.pick_where(|o| matches!(o, Opt::Technology(TOpt::Planet(p, _)) if *p == sp[0]));
    assert_eq!(goods(&rig.state, 0), 3, "1 trade good");
    assert!(ti4_engine_core::query::board::exhausted(&rig.state, sp[0]));
    assert_eq!(rig.opts().len(), 2, "the exhausted planet is no longer offered; the other still is");
    rig.expect_illegal(Pick::Opt(t(TOpt::Planet(sp[0], Default::default()))));
    rig.pick(0);
    assert!(rig.is_done(), "no ready specialty planet left: it ends by itself");
    assert_eq!(goods(&rig.state, 0), 4);
    smoke("technology_free/psycho", || Rig::new(sc(), Psychoarchaeology::new(Seat(0), SRC)));
}

#[test]
fn psychoarchaeology_may_be_declined_and_asks_nothing_without_the_card_or_a_ready_specialty() {
    covers!("rules::technology_windows::Psychoarchaeology");
    let sp = specialty_planets(1);
    let owned = || three().map_from(&map_of(&sp)).tech(0, T::Psychoarchaeology).control(sp[0], 0);
    let mut dec = Rig::new(owned(), Psychoarchaeology::new(Seat(0), SRC));
    dec.run().decline();
    assert!(dec.is_done());
    assert_eq!(goods(&dec.state, 0), goods(dec.start_state(), 0));
    let mut nocard = Rig::new(three().map_from(&map_of(&sp)).control(sp[0], 0), Psychoarchaeology::new(Seat(0), SRC));
    nocard.run();
    assert!(nocard.is_done(), "without the technology there is nothing to ask");
    let mut tired = Rig::new(owned().exhaust(sp[0]), Psychoarchaeology::new(Seat(0), SRC));
    tired.run();
    assert!(tired.is_done(), "an exhausted specialty planet cannot be exhausted again");
    let plain = plain_planet();
    let mut none = Rig::new(three().map_from(&map_of(&[plain])).tech(0, T::Psychoarchaeology).control(plain, 0), Psychoarchaeology::new(Seat(0), SRC));
    none.run();
    assert!(none.is_done(), "a planet without a technology specialty does not count");
    let mut foreign = Rig::new(three().map_from(&map_of(&sp)).tech(0, T::Psychoarchaeology).control(sp[0], 1), Psychoarchaeology::new(Seat(0), SRC));
    foreign.run();
    assert!(foreign.is_done(), "only planets the owner controls");
}

// ---- Production Biomes ------------------------------------------------------------------------

#[test]
fn production_biomes_pays_a_strategy_token_for_four_goods_and_gives_the_chosen_player_two() {
    covers!("rules::technology_windows::ProductionBiomes");
    let sc = || three().tech(0, T::ProductionBiomes).tokens(0, 3, 3, 2).goods(0, 1).goods(2, 5);
    let mut rig = Rig::new(sc(), ProductionBiomes::new(Seat(0), SRC));
    rig.run();
    assert_eq!(rig.opts(), vec![t(TOpt::Player(Seat(1))), t(TOpt::Player(Seat(2)))], "every other player, never the owner");
    rig.assert_options_sound();
    rig.expect_illegal(Pick::Opt(t(TOpt::Player(Seat(0)))));
    rig.pick(1);
    assert!(rig.is_done() && rig.driver.window.done);
    assert_eq!(goods(&rig.state, 0), 5, "+4 trade goods");
    assert_eq!(goods(&rig.state, 2), 7, "+2 for the chosen player");
    assert_eq!(goods(&rig.state, 1), goods(rig.start_state(), 1), "the third player gets nothing");
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.strategic_tokens, 1, "1 token from the strategy pool");
    assert!(is_exhausted(&rig.state, T::ProductionBiomes), "ACTION: exhaust this card");
    smoke("technology_free/biomes", || Rig::new(sc(), ProductionBiomes::new(Seat(0), SRC)));
}

#[test]
fn production_biomes_without_a_strategy_token_or_without_another_player_does_not_resolve() {
    covers!("rules::technology_windows::ProductionBiomes");
    let mut broke = Rig::new(three().tech(0, T::ProductionBiomes).tokens(0, 3, 3, 0), ProductionBiomes::new(Seat(0), SRC));
    broke.run();
    assert!(broke.is_done() && !broke.driver.window.done, "no token in the strategy pool: the action does not resolve");
    assert!(!is_exhausted(&broke.state, T::ProductionBiomes));
    let mut alone = Rig::new(Scenario::new(&[Faction::TheEmbersOfMuaat]).tech(0, T::ProductionBiomes).tokens(0, 3, 3, 2), ProductionBiomes::new(Seat(0), SRC));
    alone.run();
    assert!(alone.is_done() && !alone.driver.window.done, "no other player to choose");
}

// ---- Vortex -----------------------------------------------------------------------------------

fn vortex_scene() -> Scenario {
    let s = plain_systems();
    let dock_planet = s[0].data().planets[0];
    // The ring map puts the first tile in the middle and the next six around it: s[1] is adjacent to the centre.
    three().map_ring(7).control(dock_planet, 0).unit(s[0], Some(dock_planet), Unit::SpaceDockI, 0, 1).unit(s[1], None, Unit::CruiserI, 1, 1).unit(s[1], None, Unit::DestroyerI, 2, 1)
}

#[test]
fn vortex_captures_a_non_structure_unit_type_of_a_player_next_to_one_of_the_owners_space_docks() {
    covers!("rules::technology_windows::Vortex");
    let sc = || vortex_scene().tech(0, T::Vortex);
    let mut rig = Rig::new(sc(), Vortex::new(Seat(0), true, None, ALL_SOURCES));
    rig.run();
    assert_eq!(rig.opts(), vec![t(TOpt::Capture(Seat(1), Unit::CruiserI)), t(TOpt::Capture(Seat(2), Unit::DestroyerI))], "ordered by seat, one per unit type");
    rig.assert_options_sound();
    rig.expect_illegal(Pick::Opt(t(TOpt::Capture(Seat(1), Unit::DestroyerI))));
    rig.pick(1);
    assert!(rig.is_done() && rig.driver.window.done);
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().misc.captured_units.as_slice(), &[(Seat(2), Unit::DestroyerI)]);
    assert!(is_exhausted(&rig.state, T::Vortex));
    smoke("technology_free/vortex", || Rig::new(sc(), Vortex::new(Seat(0), true, None, ALL_SOURCES)));
}

#[test]
fn vortex_takes_a_single_candidate_unasked_ignores_structures_the_neutral_and_far_units_and_needs_a_map() {
    covers!("rules::technology_windows::Vortex");
    let s = plain_systems();
    let dock_planet = s[0].data().planets[0];
    let one = || three().map_ring(7).tech(0, T::Vortex).control(dock_planet, 0).unit(s[0], Some(dock_planet), Unit::SpaceDockI, 0, 1).unit(s[1], None, Unit::CruiserI, 1, 2);
    let mut rig = Rig::new(one(), Vortex::new(Seat(0), true, None, ALL_SOURCES));
    rig.run();
    assert!(rig.is_done() && rig.driver.window.done, "one candidate: taken without a question");
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().misc.captured_units.as_slice(), &[(Seat(1), Unit::CruiserI)]);
    let mut no_map = Rig::new(one(), Vortex::new(Seat(0), false, None, ALL_SOURCES));
    no_map.run();
    assert!(no_map.is_done() && !no_map.driver.window.done, "without a map nothing is found");
    let mut neutral = Rig::new(one(), Vortex::new(Seat(0), true, Some(Seat(1)), ALL_SOURCES));
    neutral.run();
    assert!(neutral.is_done() && !neutral.driver.window.done, "the neutral owner is not a player");
    let structure = three().map_ring(7).tech(0, T::Vortex).control(dock_planet, 0).unit(s[0], Some(dock_planet), Unit::SpaceDockI, 0, 1).unit(s[1], None, Unit::CruiserI, 1, 1);
    let far_planet = s[3].data().planets[0];
    let far = structure.clone().unit(s[3], Some(far_planet), Unit::SpaceDockI, 1, 1).control(far_planet, 1);
    let mut st = Rig::new(far, Vortex::new(Seat(0), true, Some(Seat(1)), ALL_SOURCES));
    st.run();
    assert!(st.is_done() && !st.driver.window.done, "the cruiser is the neutral's here; a structure or a unit not next to a dock is never taken");
    let none = three().map_ring(7).tech(0, T::Vortex).unit(s[1], None, Unit::CruiserI, 1, 1);
    let mut nodock = Rig::new(none, Vortex::new(Seat(0), true, None, ALL_SOURCES));
    nodock.run();
    assert!(nodock.is_done() && !nodock.driver.window.done, "no space dock of the owner: nothing is adjacent to one");
}

// ---- Wormhole Generator -----------------------------------------------------------------------

#[test]
fn wormhole_generator_places_or_moves_a_creuss_token_and_exhausts_the_card() {
    covers!("rules::technology_windows::WormholeGenerator");
    let s = plain_systems();
    let dests = || vec![(Wormhole::Alpha, s[1]), (Wormhole::Beta, s[1]), (Wormhole::Alpha, s[2])];
    let sc = || three().map_ring(7).tech(0, T::WormholeGenerator);
    let mut rig = Rig::new(sc(), WormholeGenerator::new(Seat(0), dests()));
    rig.run();
    assert_eq!(rig.opts().len(), 4, "three destinations and a decline");
    rig.assert_options_sound();
    rig.expect_illegal(Pick::Opt(t(TOpt::Token(Wormhole::Gamma, s[1]))));
    rig.pick_opt(t(TOpt::Token(Wormhole::Beta, s[1])));
    assert!(rig.is_done() && rig.driver.window.done);
    assert_eq!(rig.state.explore().wormhole_tokens.get(&Wormhole::Beta), Some(&s[1]));
    assert!(is_exhausted(&rig.state, T::WormholeGenerator));
    smoke("technology_free/wormhole", || Rig::new(sc(), WormholeGenerator::new(Seat(0), dests())));
}

#[test]
fn wormhole_generator_may_be_declined_and_needs_a_ready_card_and_a_destination() {
    covers!("rules::technology_windows::WormholeGenerator");
    let s = plain_systems();
    let mut dec = Rig::new(three().tech(0, T::WormholeGenerator), WormholeGenerator::new(Seat(0), vec![(Wormhole::Alpha, s[1])]));
    dec.run().decline();
    assert!(dec.is_done() && !dec.driver.window.done);
    assert!(!is_exhausted(&dec.state, T::WormholeGenerator));
    assert_eq!(delta(dec.start_state(), &dec.state), "");
    let mut tired = Rig::from_parts(tech_state(three().tech(0, T::WormholeGenerator), &[T::WormholeGenerator]), kernel_all(), WormholeGenerator::new(Seat(0), vec![(Wormhole::Alpha, s[1])]));
    tired.run();
    assert!(tired.is_done(), "an exhausted card cannot be used");
    let mut nowhere = Rig::new(three().tech(0, T::WormholeGenerator), WormholeGenerator::new(Seat(0), vec![]));
    nowhere.run();
    assert!(nowhere.is_done());
    let mut nocard = Rig::new(three(), WormholeGenerator::new(Seat(0), vec![(Wormhole::Alpha, s[1])]));
    nocard.run();
    assert!(nocard.is_done(), "the card is not owned");
}

// ---- end of turn: Predictive Intelligence, Bio-Stims ------------------------------------------

#[test]
fn predictive_intelligence_asks_one_split_of_every_token_and_exhausts_the_card_only_when_something_moved() {
    covers!("rules::technology_windows::TurnEnd");
    let sc = || three().tech(0, T::PredictiveIntelligence).tokens(0, 2, 1, 0);
    let mut rig = Rig::new(sc(), TurnEnd::new(Seat(0)));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Technology(TAsk::Restack)));
    assert_eq!(rig.opts().len(), 10, "every split of the three tokens held, no fleet floor, no Done: the current split keeps them");
    assert!(!rig.opts().contains(&Opt::Done));
    rig.assert_options_sound();
    rig.expect_illegal(Pick::Opt(t(TOpt::Split { tactic: 1, fleet: 1, strategic: 0 })));
    rig.pick_opt(t(TOpt::Split { tactic: 0, fleet: 1, strategic: 2 }));
    assert!(rig.is_done());
    let p = rig.state.players().by_seat(Seat(0)).unwrap();
    assert_eq!((p.econ.tactic_tokens, p.econ.fleet_tokens, p.econ.strategic_tokens), (0, 1, 2), "two moves in one answer");
    assert!(is_exhausted(&rig.state, T::PredictiveIntelligence));
    let mut none = Rig::new(sc(), TurnEnd::new(Seat(0)));
    none.run().pick_opt(t(TOpt::Split { tactic: 2, fleet: 1, strategic: 0 }));
    assert!(none.is_done() && !is_exhausted(&none.state, T::PredictiveIntelligence), "the split as it stands: nothing moved, the card stays ready");
    smoke("technology_free/predictive", || Rig::new(sc(), TurnEnd::new(Seat(0))));
}

/// The multi-ask conversion: the one split question reaches exactly the final positions the string's move-per-question loop reaches
/// (each move takes a token of a pool that holds one to another pool; at most as many moves as tokens; `done` at any time).
#[test]
fn the_split_question_offers_exactly_what_the_move_loop_could_reach() {
    use std::collections::BTreeSet;
    covers!("rules::technology_windows::TurnEnd");
    for (t0, f0, s0) in [(2, 1, 0), (0, 0, 1), (3, 0, 2), (1, 1, 1), (0, 4, 0)] {
        let total = t0 + f0 + s0;
        // Breadth-first over the move loop.
        let mut reach: BTreeSet<[i16; 3]> = BTreeSet::from([[t0, f0, s0]]);
        let mut front = reach.clone();
        for _ in 0..total {
            let mut next = BTreeSet::new();
            for at in &front {
                for from in 0..3 {
                    for to in 0..3 {
                        if from != to && at[from] > 0 {
                            let mut b = *at;
                            b[from] -= 1;
                            b[to] += 1;
                            next.insert(b);
                        }
                    }
                }
            }
            reach.extend(next.iter().copied());
            front = next;
        }
        let mut rig = Rig::new(three().tech(0, T::PredictiveIntelligence).tokens(0, t0, f0, s0), TurnEnd::new(Seat(0)));
        rig.run();
        let offered: BTreeSet<[i16; 3]> = rig
            .opts()
            .iter()
            .filter_map(|o| if let Opt::Technology(TOpt::Split { tactic, fleet, strategic }) = o { Some([*tactic, *fleet, *strategic].map(i16::from)) } else { None })
            .collect();
        assert_eq!(offered, reach, "{t0}/{f0}/{s0}");
    }
}

#[test]
fn predictive_intelligence_asks_nothing_without_a_token() {
    covers!("rules::technology_windows::TurnEnd");
    let mut empty = Rig::new(three().tech(0, T::PredictiveIntelligence).tokens(0, 0, 0, 0), TurnEnd::new(Seat(0)));
    empty.run();
    assert!(empty.is_done(), "no token to move");
    assert!(TurnEnd::applies(empty.start_state(), Seat(0)) && !TurnEnd::applies(empty.start_state(), Seat(1)));
}

#[test]
fn bio_stims_readies_a_specialty_planet_or_another_technology_and_exhausts_itself() {
    covers!("rules::technology_windows::TurnEnd");
    let sp = specialty_planets(1);
    let sc = || three().map_from(&map_of(&sp)).tech(0, T::BioStims).tech(0, T::SarweenTools).control(sp[0], 0).exhaust(sp[0]);
    let st = || tech_state(sc(), &[T::SarweenTools]);
    let mut rig = Rig::from_parts(st(), kernel_all(), TurnEnd::new(Seat(0)));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Technology(TAsk::BioStims)));
    assert_eq!(rig.opts().len(), 3, "the exhausted specialty planet, the exhausted technology, a decline: Bio-Stims itself is not offered");
    rig.assert_options_sound();
    rig.pick_opt(t(TOpt::ReadyTechnology(T::SarweenTools)));
    assert!(rig.is_done());
    assert!(!is_exhausted(&rig.state, T::SarweenTools));
    assert!(is_exhausted(&rig.state, T::BioStims), "exhaust this card");
    assert!(ti4_engine_core::query::board::exhausted(&rig.state, sp[0]), "the planet stays exhausted: one choice only");
    let mut planet = Rig::from_parts(st(), kernel_all(), TurnEnd::new(Seat(0)));
    planet.run().pick_where(|o| matches!(o, Opt::Technology(TOpt::Planet(p, _)) if *p == sp[0]));
    assert!(!ti4_engine_core::query::board::exhausted(&planet.state, sp[0]));
    let mut dec = Rig::from_parts(st(), kernel_all(), TurnEnd::new(Seat(0)));
    dec.run().decline();
    assert!(dec.is_done() && !is_exhausted(&dec.state, T::BioStims));
    let mut nothing = Rig::new(three().tech(0, T::BioStims), TurnEnd::new(Seat(0)));
    nothing.run();
    assert!(nothing.is_done(), "nothing exhausted to ready");
    smoke("technology_free/biostims", || Rig::from_parts(st(), kernel_all(), TurnEnd::new(Seat(0))));
}

// ---- Sarween Tools / AI Development Algorithm -------------------------------------------------

fn used_state(techs: &[T]) -> CoreState {
    let mut sc = three();
    for x in techs {
        sc = sc.tech(0, *x);
    }
    let mut s = sc.state();
    s.transient_mut().production.discount_remaining = 99;
    s
}

#[test]
fn sarween_tools_takes_one_off_the_cost_and_the_value_is_written_not_added() {
    covers!("rules::technology_windows::ProductionUsed");
    let mut rig = Rig::from_parts(used_state(&[T::SarweenTools]), kernel_all(), ProductionUsed::new(Seat(0)));
    rig.run();
    assert!(rig.is_done());
    assert_eq!(rig.state.transient().production.discount_remaining, 1, "reduce the combined cost by 1; a leftover value is overwritten");
    let mut bare = Rig::from_parts(used_state(&[]), kernel_all(), ProductionUsed::new(Seat(0)));
    bare.run();
    assert_eq!(bare.state.transient().production.discount_remaining, 0);
}

#[test]
fn ai_development_algorithm_may_exhaust_for_one_off_per_unit_upgrade_owned() {
    covers!("rules::technology_windows::ProductionUsed");
    let techs = [T::SarweenTools, T::AIDevelopmentAlgorithm, T::CruiserII, T::DestroyerII];
    let mut rig = Rig::from_parts(used_state(&techs), kernel_all(), ProductionUsed::new(Seat(0)));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Technology(TAsk::Aida { upgrades: 2 })), "{:?}", rig.asked().ask);
    assert_eq!(rig.opts(), vec![t(TOpt::Exhaust), Opt::Decline]);
    rig.assert_options_sound();
    rig.pick(0);
    assert!(rig.is_done());
    assert_eq!(rig.state.transient().production.discount_remaining, 1 + 2, "Sarween's 1 plus one per unit upgrade");
    assert!(is_exhausted(&rig.state, T::AIDevelopmentAlgorithm));
    let mut dec = Rig::from_parts(used_state(&techs), kernel_all(), ProductionUsed::new(Seat(0)));
    dec.run().decline();
    assert_eq!(dec.state.transient().production.discount_remaining, 1);
    assert!(!is_exhausted(&dec.state, T::AIDevelopmentAlgorithm));
    let mut noupg = Rig::from_parts(used_state(&[T::AIDevelopmentAlgorithm]), kernel_all(), ProductionUsed::new(Seat(0)));
    noupg.run();
    assert!(noupg.is_done(), "no unit upgrade owned: nothing to reduce, nothing asked");
    smoke("technology_free/aida", || Rig::from_parts(used_state(&techs), kernel_all(), ProductionUsed::new(Seat(0))));
}

// ---- Transit Diodes ---------------------------------------------------------------------------

fn diodes_scene() -> (Scenario, Vec<Planet>) {
    let ps: Vec<Planet> = specialty_planets(3);
    let sc = three().map_from(&map_of(&ps)).tech(0, T::TransitDiodes).control(ps[0], 0).control(ps[1], 0).unit(system_of(ps[0]), Some(ps[0]), Unit::InfantryI, 0, 2);
    (sc, ps)
}

#[test]
fn transit_diodes_redeploys_ground_forces_one_per_answer_and_exhausts_the_card_when_any_moved() {
    covers!("rules::technology_windows::TransitDiodes");
    let (sc, ps) = diodes_scene();
    let mut rig = Rig::new(sc.clone(), TransitDiodes::new(Seat(0)));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Technology(TAsk::TransitDiodes { left: 4 })));
    assert_eq!(rig.opts().len(), 2, "one infantry kind to the other planet, and a decline");
    rig.assert_options_sound();
    rig.pick(0);
    assert!(matches!(rig.asked().ask, Ask::Technology(TAsk::TransitDiodes { left: 3 })));
    let inf = |s: &CoreState, p: Planet| s.board().planet(p).units.as_slice().iter().filter(|u| u.owner() == Seat(0) && u.kind() == Unit::InfantryI).count();
    assert_eq!((inf(&rig.state, ps[0]), inf(&rig.state, ps[1])), (1, 1));
    assert!(!is_exhausted(&rig.state, T::TransitDiodes));
    assert!(rig.opts().iter().all(|o| !matches!(o, Opt::Technology(TOpt::Transit { from, .. }) if *from == Some(ps[1]))), "a planet that received one cannot send one out");
    rig.decline();
    assert!(rig.is_done());
    assert!(is_exhausted(&rig.state, T::TransitDiodes));
    smoke("technology_free/transit", || Rig::new(sc.clone(), TransitDiodes::new(Seat(0))));
}

#[test]
fn transit_diodes_stops_after_four_and_does_nothing_without_the_ready_card_or_a_destination() {
    covers!("rules::technology_windows::TransitDiodes");
    let (sc, ps) = diodes_scene();
    let many = sc.clone().unit(system_of(ps[0]), Some(ps[0]), Unit::InfantryI, 0, 4).unit(system_of(ps[2]), Some(ps[2]), Unit::InfantryI, 0, 1);
    let mut rig = Rig::new(many, TransitDiodes::new(Seat(0)));
    rig.run();
    let mut moves = 0;
    while !rig.is_done() {
        let i = rig.opts().iter().position(|o| matches!(o, Opt::Technology(TOpt::Transit { .. }))).expect("a move is offered");
        rig.pick(i);
        moves += 1;
        assert!(moves <= 4, "{}", rig.transcript());
    }
    assert_eq!(moves, 4, "up to four ground forces");
    assert!(is_exhausted(&rig.state, T::TransitDiodes));
    let mut dec = Rig::new(sc.clone(), TransitDiodes::new(Seat(0)));
    dec.run().decline();
    assert!(dec.is_done() && !is_exhausted(&dec.state, T::TransitDiodes), "nothing moved: the card stays ready");
    let mut tired = Rig::from_parts(tech_state(sc.clone(), &[T::TransitDiodes]), kernel_all(), TransitDiodes::new(Seat(0)));
    tired.run();
    assert!(tired.is_done());
    assert!(!TransitDiodes::applies(tired.start_state(), Seat(0)));
    let (alone, _) = diodes_scene();
    let mut lone = Rig::new(three().tech(0, T::TransitDiodes), TransitDiodes::new(Seat(0)));
    lone.run();
    assert!(lone.is_done(), "no ground force, no planet");
    let _ = alone;
}

#[test]
fn the_turn_start_runs_psychoarchaeology_then_transit_diodes() {
    covers!("rules::technology_windows::TurnStart");
    let (sc, ps) = diodes_scene();
    let sc = sc.tech(0, T::Psychoarchaeology);
    let mut rig = Rig::new(sc, TurnStart::new(Seat(0), SRC));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Technology(TAsk::Psychoarchaeology)));
    rig.pick_where(|o| matches!(o, Opt::Technology(TOpt::Planet(p, _)) if *p == ps[0]));
    assert!(matches!(rig.asked().ask, Ask::Technology(TAsk::Psychoarchaeology)), "asked again while a ready specialty is left");
    rig.decline();
    assert!(matches!(rig.asked().ask, Ask::Technology(TAsk::TransitDiodes { .. })), "then the diodes");
    rig.decline();
    assert!(rig.is_done());
    assert!(!rig.driver.window.unsupported());
}
