//! W10 (lane 8): `rules::hero_windows`, the heroes composed from windows and kernel effects. UNIT.DSGN.FLAYESH (Nekro hero) here.

use ti4_engine_core::choice::leaders::{Ask as LAsk, Opt as LOpt};
use ti4_engine_core::generated::Planet;
use ti4_engine_core::keys::{DestroyReason, MarkKey};
use ti4_engine_core::state::SysFlags;
use ti4_engine_core::rules::hero_windows::{Devourable, NekroHero};
use ti4_engine_core::state::Code;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{plain_systems, system_of};

const SOURCE: Code = Code(7);

/// Two planets in two different systems, in system order.
fn two() -> [(System, Planet); 2] {
    let mut found: Vec<(System, Planet)> = Vec::new();
    for s in plain_systems() {
        if let Some(p) = s.data().planets.first()
            && s.data().planets.len() == 1
        {
            found.push((s, *p));
        }
        if found.len() == 2 {
            break;
        }
    }
    found.sort_unstable();
    [found[0], found[1]]
}

fn target(spot: (System, Planet), goods: i32, techs: &[Technology]) -> Devourable {
    Devourable { system: spot.0, planet: spot.1, goods, techs: techs.to_vec() }
}

fn scenario() -> (Scenario, [(System, Planet); 2]) {
    let [a, b] = two();
    let sc = Scenario::new(&[Faction::TheNekroVirus, Faction::TheArborec])
        .map_from(&[a.0, b.0])
        .unit(a.0, None, Unit::CruiserI, 0, 1)
        .unit(b.0, None, Unit::CruiserI, 0, 1)
        .unit(a.0, Some(a.1), Unit::InfantryI, 1, 2)
        .unit(a.0, Some(a.1), Unit::PDSI, 1, 1)
        .unit(a.0, Some(a.1), Unit::InfantryI, 0, 1)
        .unit(b.0, Some(b.1), Unit::InfantryI, 1, 1)
        .goods(0, 1);
    (sc, [a, b])
}

fn hero(sc: Scenario, planets: Vec<Devourable>) -> Rig<NekroHero> {
    Rig::new(sc, NekroHero::new(Seat(0), SOURCE, planets))
}

fn units_on(rig: &Rig<NekroHero>, planet: Planet, seat: u8) -> usize {
    rig.state.board().planet(planet).units.as_slice().iter().filter(|p| p.owner() == Seat(seat)).count()
}

fn staged_rows(rig: &Rig<NekroHero>) -> usize {
    rig.state.marks().tail.iter().filter(|(k, _)| matches!(k, MarkKey::PrivateStagedGround(_))).count()
}

#[test]
fn a_single_planet_is_devoured_unasked_when_one_technology_matches() {
    covers!("rules::hero_windows::NekroHero");
    let (sc, [a, b]) = scenario();
    let mut rig = hero(sc, vec![target(a, 3, &[Technology::GravityDrive])]);
    rig.run();
    assert!(rig.is_done(), "{}", rig.transcript());
    assert!(rig.driver.window.used);
    assert_eq!(units_on(&rig, a.1, 1), 0, "every other seat's unit on the planet is gone, the structure too");
    assert_eq!(units_on(&rig, a.1, 0), 1, "the seat's own unit stays");
    assert_eq!(units_on(&rig, b.1, 1), 1, "the other planet is untouched");
    assert_eq!(staged_rows(&rig), 2, "the two infantry are staged as destroyed, the structure is not");
    let row = MarkVal::Rec(vec![
        MarkVal::Atom(Atom::Word(Word::Destroyed)),
        MarkVal::Atom(Atom::System(a.0)),
        MarkVal::Atom(Atom::Planet(a.1)),
        MarkVal::Atom(Atom::Seat(Seat(1))),
        MarkVal::Atom(Atom::Unit(Unit::InfantryI)),
        MarkVal::Atom(Atom::Word(Word::False)),
        MarkVal::Atom(Atom::Destroy(DestroyReason::LeaderBare(Leader::UNITDSGNFLAYESH))),
    ]);
    assert!(rig.state.marks().tail.iter().any(|(k, v)| matches!(k, MarkKey::PrivateStagedGround(0)) && *v == row), "the cause is the hero: {:?}", rig.state.marks().tail.iter().collect::<Vec<_>>());
    let p = rig.state.players().by_seat(Seat(0)).unwrap();
    assert_eq!(p.econ.trade_goods, 1 + 3);
    assert!(p.tech.owned.contains(Technology::GravityDrive));
    assert_eq!(rig.state.pending().staged.len(), 1, "the goods are announced");
}

#[test]
fn several_planets_and_technologies_are_asked_and_nothing_changes_before_the_last_answer() {
    covers!("rules::hero_windows::NekroHero");
    let (sc, [a, b]) = scenario();
    let techs = [Technology::GravityDrive, Technology::SarweenTools, Technology::PlasmaScoring];
    let mut rig = hero(sc, vec![target(a, 3, &techs), target(b, 2, &techs[1..])]);
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Leaders(LAsk::FlayeshPlanet)));
    assert_eq!(rig.opts(), vec![Opt::Leaders(LOpt::Devour(a.0, a.1)), Opt::Leaders(LOpt::Devour(b.0, b.1))]);
    rig.assert_options_sound();
    assert_eq!(delta(rig.start_state(), &rig.state), "", "nothing changed yet");
    rig.pick_opt(Opt::Leaders(LOpt::Devour(b.0, b.1)));
    assert!(matches!(rig.asked().ask, Ask::Leaders(LAsk::FlayeshTech)));
    assert_eq!(rig.opts(), vec![Opt::Leaders(LOpt::Gain(Technology::SarweenTools)), Opt::Leaders(LOpt::Gain(Technology::PlasmaScoring))], "the technologies of the chosen planet");
    assert_eq!(delta(rig.start_state(), &rig.state), "", "still nothing");
    rig.pick_opt(Opt::Leaders(LOpt::Gain(Technology::PlasmaScoring)));
    assert!(rig.is_done());
    assert_eq!(units_on(&rig, b.1, 1), 0);
    assert_eq!(units_on(&rig, a.1, 1), 3, "the planet not chosen keeps its units");
    let p = rig.state.players().by_seat(Seat(0)).unwrap();
    assert!(p.tech.owned.contains(Technology::PlasmaScoring) && !p.tech.owned.contains(Technology::SarweenTools));
    assert_eq!(p.econ.trade_goods, 1 + 2, "the goods of the chosen planet");
}

#[test]
fn a_planet_nobody_else_holds_still_gives_the_goods_and_a_planet_without_a_technology_gives_none() {
    covers!("rules::hero_windows::NekroHero");
    let (sc, [a, _]) = scenario();
    let mut rig = hero(sc, vec![target(a, 4, &[])]);
    rig.state.board_mut().planet_mut(a.1).units.retain(|p| p.owner() == Seat(0));
    rig.state.board_mut().sys_mut(a.0).flags.set(SysFlags::TOUCHED, false);
    rig.run();
    assert!(!rig.state.board().sys(a.0).flags.has(SysFlags::TOUCHED), "nothing fell: the system entry is not created (`system_mut` is not called)");
    assert!(rig.is_done() && rig.driver.window.used);
    assert_eq!(staged_rows(&rig), 0);
    let p = rig.state.players().by_seat(Seat(0)).unwrap();
    assert_eq!(p.econ.trade_goods, 5);
    assert!(p.tech.owned.is_empty(), "no technology matched");
}

#[test]
fn no_planet_means_the_card_is_not_used() {
    covers!("rules::hero_windows::NekroHero");
    let (sc, _) = scenario();
    let mut rig = hero(sc, Vec::new());
    rig.run();
    assert!(rig.is_done() && !rig.driver.window.used);
    assert_eq!(delta(rig.start_state(), &rig.state), "");
}

#[test]
fn an_answer_outside_the_offer_is_refused() {
    covers!("rules::hero_windows::NekroHero");
    let (sc, [a, b]) = scenario();
    let techs = [Technology::GravityDrive, Technology::SarweenTools];
    let mut rig = hero(sc, vec![target(a, 3, &techs), target(b, 2, &techs)]);
    rig.run();
    let stranger = system_of(Planet::MecatolRex);
    rig.expect_illegal(Pick::Opt(Opt::Leaders(LOpt::Devour(stranger, Planet::MecatolRex))));
    rig.expect_illegal(Pick::Opt(Opt::Leaders(LOpt::Gain(Technology::GravityDrive))));
    rig.pick_opt(Opt::Leaders(LOpt::Devour(a.0, a.1)));
    rig.expect_illegal(Pick::Opt(Opt::Leaders(LOpt::Gain(Technology::PlasmaScoring))));
    rig.expect_illegal(Pick::Opt(Opt::Decline));
}

#[test]
fn a_galvanized_ground_force_that_falls_is_staged_with_the_galvanized_field() {
    covers!("rules::hero_windows::NekroHero");
    let (sc, [a, _]) = scenario();
    let mut rig = hero(sc, vec![target(a, 3, &[Technology::GravityDrive])]);
    for piece in rig.state.board_mut().planet_mut(a.1).units.as_mut_slice() {
        if piece.owner() == Seat(1) && piece.kind() == Unit::InfantryI {
            *piece = piece.with_galvanized(true);
        }
    }
    rig.run();
    let rows: Vec<&ti4_engine_core::keys::MarkVal> = rig
        .state
        .marks()
        .tail
        .iter()
        .filter(|(k, _)| matches!(k, ti4_engine_core::keys::MarkKey::PrivateStagedGround(_)))
        .map(|(_, v)| v)
        .collect();
    assert!(!rows.is_empty(), "the fallen ground forces are staged");
    for row in rows {
        let ti4_engine_core::keys::MarkVal::Rec(fields) = row else { panic!("a record: {row:?}") };
        assert_eq!(fields.len(), 8, "seven fields and the galvanized tail: {fields:?}");
        assert_eq!(fields[7], ti4_engine_core::keys::MarkVal::Atom(ti4_engine_core::keys::Atom::Word(ti4_engine_core::keys::Word::Galvanized)));
    }
}

// -- Kyver, Blade and Key ---------------------------------------------------------------------------

use ti4_engine_core::choice::cards::{Ask as CAsk, Opt as COpt, Reason};
use ti4_engine_core::choice::Origin;
use ti4_engine_core::generated::{ActionCard, Leader};
use ti4_engine_core::keys::{Atom, MarkVal, Word};
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::rules::hero_windows::KyverHero;

fn groups() -> AbilityTable {
    let mut t = AbilityTable::default();
    t.facts.card_groups = (0..ActionCard::COUNT).map(|i| u8::try_from(i).unwrap_or(u8::MAX)).collect();
    t
}

fn kyver(hands: [&[ActionCard]; 3], limited: bool) -> Rig<KyverHero> {
    // (The hand limit is the hook table's: a seat that is limited at this table is one whose faction hooks do not lift it, so the plain table.)
    let sc = Scenario::new(&[Faction::TheYssarilTribes, Faction::TheArborec, Faction::TheEmbersOfMuaat])
        .kernel(if limited { kernel_plain() } else { kernel_all() })
        .hand(0, hands[0])
        .hand(1, hands[1])
        .hand(2, hands[2]);
    Rig::new(sc, KyverHero::new(Seat(0), limited)).with_table(groups()).seeded(11)
}

fn hand(rig: &Rig<KyverHero>, seat: u8) -> Vec<ActionCard> {
    rig.state.players().by_seat(Seat(seat)).unwrap().cards.hand.as_slice().to_vec()
}

const C: [ActionCard; 8] = [ActionCard::ALL[3], ActionCard::ALL[7], ActionCard::ALL[9], ActionCard::ALL[11], ActionCard::ALL[13], ActionCard::ALL[15], ActionCard::ALL[17], ActionCard::ALL[19]];

#[test]
fn each_player_shows_a_card_in_seat_order_then_the_holder_decides_for_each_and_nothing_moves_before_the_last() {
    covers!("rules::hero_windows::KyverHero");
    let mut rig = kyver([&[], &[C[0], C[1]], &[C[2], C[3], C[4], C[5]]], false);
    rig.run();
    let q = rig.asked().clone();
    assert_eq!((q.actor, q.ask), (Seat(1), Ask::Cards(CAsk::OwnHand { reason: Reason::Show { viewer: Seat(0) } })));
    assert_eq!(q.ctx.origin, Origin::FactionAbility(ti4_engine_core::choice::generic::Content::Leader(Leader::KyverBladeAndKey)));
    rig.assert_options_sound();
    let mut by_name = [C[0], C[1]];
    by_name.sort_by_key(|card| card.name_rank());
    assert_eq!(rig.opts(), by_name.iter().map(|card| Opt::Cards(COpt::Card(*card))).collect::<Vec<_>>(), "one option per printed card, in the order of the names");
    rig.pick_opt(Opt::Cards(COpt::Card(C[1])));
    let q = rig.asked().clone();
    assert_eq!((q.actor, q.ask), (Seat(2), Ask::Cards(CAsk::OwnHand { reason: Reason::Show { viewer: Seat(0) } })));
    rig.pick_opt(Opt::Cards(COpt::Card(C[4])));
    let q = rig.asked().clone();
    assert_eq!(q.actor, Seat(0));
    assert_eq!(q.ask, Ask::Leaders(LAsk::KyverFate { target: Seat(1), card: C[1] }));
    assert_eq!(rig.opts(), vec![Opt::Leaders(LOpt::KyverTake), Opt::Leaders(LOpt::KyverDiscard), Opt::Decline]);
    rig.assert_options_sound();
    rig.pick_opt(Opt::Leaders(LOpt::KyverTake));
    assert_eq!(rig.asked().ask, Ask::Leaders(LAsk::KyverFate { target: Seat(2), card: C[4] }));
    assert_eq!(delta(rig.start_state(), &rig.state), "", "the decisions come before any change");
    rig.pick_opt(Opt::Leaders(LOpt::KyverDiscard));
    assert!(rig.is_done() && rig.driver.window.used && !rig.driver.window.unsupported, "{}", rig.transcript());
    assert_eq!(hand(&rig, 0), vec![C[1]], "the shown card was taken");
    assert_eq!(hand(&rig, 1), vec![C[0]]);
    assert_eq!(hand(&rig, 2).len(), 1, "three of four cards were discarded at random");
    let taken = MarkVal::Rec(vec![MarkVal::Atom(Atom::Word(Word::Taken)), MarkVal::Atom(Atom::Seat(Seat(0))), MarkVal::Atom(Atom::Seat(Seat(1))), MarkVal::Atom(Atom::Card(C[1]))]);
    assert!(rig.state.marks().tail.iter().any(|(_, v)| *v == taken), "the take is staged");
    let discards = rig.state.marks().tail.iter().filter(|(_, v)| matches!(v, MarkVal::Rec(r) if r.first() == Some(&MarkVal::Atom(Atom::Word(Word::Discarded))))).count();
    assert_eq!(discards, 3, "each random discard is staged");
    let drawn: Vec<ti4_engine_core::state::RngDomain> = rig.state.rng().positions().map(|(d, _)| d).collect();
    assert!(drawn.contains(&ti4_engine_core::state::RngDomain::YssarilHero), "the random discards draw from the hero's own stream: {drawn:?}");
    assert!(!drawn.contains(&ti4_engine_core::state::RngDomain::Dice), "and shift no other stream");
}

#[test]
fn a_single_printed_card_is_shown_unasked_a_decline_changes_nothing_and_a_hand_of_two_loses_both() {
    covers!("rules::hero_windows::KyverHero");
    let mut rig = kyver([&[], &[C[0], C[0]], &[C[2], C[3]]], false);
    rig.run();
    assert_eq!(rig.asked().actor, Seat(2), "seat 1 holds one printed card twice: shown without a question");
    rig.pick_opt(Opt::Cards(COpt::Card(C[2])));
    assert_eq!(rig.asked().ask, Ask::Leaders(LAsk::KyverFate { target: Seat(1), card: C[0] }));
    rig.pick_opt(Opt::Decline);
    rig.pick_opt(Opt::Leaders(LOpt::KyverDiscard));
    assert!(rig.is_done() && rig.driver.window.used);
    assert_eq!(hand(&rig, 1), vec![C[0], C[0]], "declined");
    assert!(hand(&rig, 2).is_empty(), "a hand of two loses both and the third draw finds none");
}

#[test]
fn nobody_holding_a_card_means_nothing_is_asked_and_the_card_is_not_used() {
    covers!("rules::hero_windows::KyverHero");
    let mut rig = kyver([&[C[0]], &[], &[]], false);
    rig.run();
    assert!(rig.is_done() && !rig.driver.window.used && !rig.driver.window.unsupported);
    assert_eq!(delta(rig.start_state(), &rig.state), "");
}

#[test]
fn a_table_without_printed_name_groups_makes_the_window_unsupported() {
    covers!("rules::hero_windows::KyverHero");
    let sc = Scenario::new(&[Faction::TheYssarilTribes, Faction::TheArborec]).hand(1, &[C[0]]);
    let mut rig = Rig::new(sc, KyverHero::new(Seat(0), false));
    rig.run();
    assert!(rig.is_done() && rig.driver.window.unsupported && !rig.driver.window.used);
}

#[test]
fn a_taker_over_the_hand_limit_discards_through_the_hand_limit_window_unless_the_faction_lifts_it() {
    covers!("rules::hero_windows::KyverHero");
    let full = [C[2], C[3], C[4], C[5], C[6], C[7], ActionCard::ALL[21]];
    let mut limited = kyver([&full, &[C[0]], &[]], true);
    limited.run();
    limited.pick_opt(Opt::Leaders(LOpt::KyverTake));
    assert!(matches!(limited.asked().ask, Ask::Cards(CAsk::HandLimit { held: 8 })), "{}", limited.transcript());
    assert_eq!(limited.asked().actor, Seat(0));
    let mut crafty = kyver([&full, &[C[0]], &[]], false);
    crafty.run();
    crafty.pick_opt(Opt::Leaders(LOpt::KyverTake));
    assert!(crafty.is_done());
    assert_eq!(hand(&crafty, 0).len(), 8, "no limit for a seat that lifts it");
}

#[test]
fn answers_outside_the_offer_are_refused() {
    covers!("rules::hero_windows::KyverHero");
    let mut rig = kyver([&[C[7]], &[C[0], C[1]], &[]], false);
    rig.run();
    rig.expect_illegal(Pick::Opt(Opt::Cards(COpt::Card(C[7]))));
    rig.expect_illegal(Pick::Opt(Opt::Decline));
    rig.pick_opt(Opt::Cards(COpt::Card(C[0])));
    rig.expect_illegal(Pick::Opt(Opt::Cards(COpt::Card(C[0]))));
    rig.expect_illegal(Pick::Opt(Opt::Leaders(LOpt::Gain(Technology::GravityDrive))));
}

// -- It Feeds on Carrion ------------------------------------------------------------------------------

use ti4_engine_core::kernel::children::SeeingLimits;
use ti4_engine_core::rules::hero_windows::{CabalHero, Stranded};
use ti4_engine_core::state::Piece;

fn limits() -> SeeingLimits {
    SeeingLimits { enforce: 8, shown: 8, halves: [None, None] }
}

fn carrion(sc: Scenario, playable: bool, targets: Vec<(System, Piece)>, stranded: Vec<Stranded>) -> Rig<CabalHero> {
    Rig::new(sc, CabalHero::new(Seat(0), playable, targets, stranded))
}

fn captured(rig: &Rig<CabalHero>) -> Vec<(Seat, Unit)> {
    rig.state.players().by_seat(Seat(0)).unwrap().misc.captured_units.as_slice().to_vec()
}

fn at(rig: &Rig<CabalHero>, system: System, seat: u8, unit: Unit) -> usize {
    rig.state.board().sys(system).units.as_slice().iter().filter(|p| p.owner() == Seat(seat) && p.kind() == unit).count()
}

/// Run the hero on `seed` and answer every enforcement question with its first option.
fn play(sc: &Scenario, playable: bool, targets: &[(System, Piece)], stranded: &[Stranded], seed: u64) -> Rig<CabalHero> {
    play_with(sc, playable, targets, stranded, &[], seed)
}

/// [`play`] with `extra` pieces added to the space area of their system first (ground forces cannot be put there by a scenario).
fn play_with(sc: &Scenario, playable: bool, targets: &[(System, Piece)], stranded: &[Stranded], extra: &[(System, Piece)], seed: u64) -> Rig<CabalHero> {
    let mut rig = carrion(sc.clone(), playable, targets.to_vec(), stranded.to_vec()).seeded(seed);
    for (system, piece) in extra {
        rig.state.board_mut().sys_mut(*system).units.push(*piece);
    }
    rig.run();
    while !rig.is_done() {
        // The soundness check replays from the scenario, which lacks the extra pieces.
        if extra.is_empty() {
            rig.assert_options_sound();
        }
        rig.pick(0);
    }
    rig
}

fn hub() -> (Scenario, System) {
    let [a, b] = two();
    let sc = Scenario::new(&[Faction::TheVuilraithCabal, Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_from(&[a.0, b.0]);
    (sc, a.0)
}

#[test]
fn each_ship_rolls_one_die_and_a_one_to_three_captures_it_from_the_board() {
    covers!("rules::hero_windows::CabalHero");
    let (sc, system) = hub();
    let sc = sc.unit(system, None, Unit::CruiserI, 1, 2);
    let ships: Vec<Piece> = vec![Piece::new(Unit::CruiserI, Seat(1)); 2];
    let targets: Vec<(System, Piece)> = ships.iter().map(|p| (system, *p)).collect();
    let (mut hit, mut miss) = (false, false);
    let mut faces_seen = std::collections::BTreeSet::new();
    for seed in 0..60 {
        // The two dice the window draws from the dice stream, read off a copy of the start position's streams.
        let mut probe = carrion(sc.clone(), true, Vec::new(), Vec::new()).state.rng().clone();
        let faces: Vec<u32> = (0..2).map(|_| probe.die(ti4_engine_core::state::RngDomain::Dice, &ti4_engine_core_tests::rig::KitSeeds(seed), 10)).collect();
        faces_seen.extend(faces.iter().copied());
        let rig = play(&sc, true, &targets, &[], seed);
        assert!(rig.driver.window.used);
        let taken = captured(&rig).len();
        assert_eq!(taken, faces.iter().filter(|face| **face <= 3).count(), "a 1 to 3 captures, a 4 or more does not: {faces:?}");
        assert_eq!(at(&rig, system, 1, Unit::CruiserI) + taken, 2, "a ship is on the board or in the holder's sheet");
        assert!(captured(&rig).iter().all(|c| *c == (Seat(1), Unit::CruiserI)));
        hit |= taken > 0;
        miss |= taken < 2;
    }
    assert!(hit && miss, "both outcomes are reached on some seed");
    assert!(faces_seen.contains(&3) && faces_seen.contains(&4), "the boundary faces were drawn: {faces_seen:?}");
    // The same seed gives the same outcome.
    assert_eq!(captured(&play(&sc, true, &targets, &[], 9)), captured(&play(&sc, true, &targets, &[], 9)));
}

#[test]
fn an_unplayable_card_rolls_nothing_and_changes_nothing() {
    covers!("rules::hero_windows::CabalHero");
    let (sc, system) = hub();
    let sc = sc.unit(system, None, Unit::CruiserI, 1, 1);
    let rig = play(&sc, false, &[(system, Piece::new(Unit::CruiserI, Seat(1)))], &[], 3);
    assert!(rig.is_done() && !rig.driver.window.used);
    assert_eq!(delta(rig.start_state(), &rig.state), "");
}

#[test]
fn the_cargo_a_captured_carrier_strands_is_removed_by_the_owner_and_captured_in_unit_order() {
    covers!("rules::hero_windows::CabalHero");
    let (sc, system) = hub();
    let sc = sc.unit(system, None, Unit::CarrierI, 1, 1);
    // The infantry stands before the fighters in the list: the captures follow the order of the unit ids, not the list.
    let infantry = [
        (system, Piece::new(Unit::InfantryI, Seat(1))),
        (system, Piece::new(Unit::InfantryI, Seat(1))),
        (system, Piece::new(Unit::FighterI, Seat(1))),
        (system, Piece::new(Unit::FighterI, Seat(1))),
    ];
    let target = (system, Piece::new(Unit::CarrierI, Seat(1)));
    let stranded = [Stranded { owner: Seat(1), system, limits: limits() }];
    let mut found = false;
    for seed in 0..60 {
        let rig = play_with(&sc, true, &[target], &stranded, &infantry, seed);
        if captured(&rig).is_empty() {
            assert_eq!(at(&rig, system, 1, Unit::FighterI), 2, "a carrier that was not captured keeps its cargo");
            continue;
        }
        found = true;
        assert_eq!(
            captured(&rig),
            vec![(Seat(1), Unit::CarrierI), (Seat(1), Unit::FighterI), (Seat(1), Unit::FighterI), (Seat(1), Unit::InfantryI), (Seat(1), Unit::InfantryI)],
            "the carrier, then what it carried, in the order of the unit ids"
        );
        assert!(rig.state.board().sys(system).units.as_slice().iter().all(|p| p.owner() != Seat(1)));
        break;
    }
    assert!(found, "a seed captures the carrier");
}

#[test]
fn the_owner_is_asked_which_units_go_when_the_survivors_cannot_hold_them_all() {
    covers!("rules::hero_windows::CabalHero");
    let (sc, system) = hub();
    let sc = sc.unit(system, None, Unit::CarrierI, 1, 2).unit(system, None, Unit::FighterI, 1, 8);
    let targets = [(system, Piece::new(Unit::CarrierI, Seat(1))), (system, Piece::new(Unit::CarrierI, Seat(1)))];
    let stranded = [Stranded { owner: Seat(1), system, limits: limits() }];
    for seed in 0..80 {
        let mut rig = carrion(sc.clone(), true, targets.to_vec(), stranded.to_vec()).seeded(seed);
        rig.run();
        if rig.is_done() {
            continue;
        }
        // One carrier left (capacity 4): four of the eight fighters must go, and the owner chooses.
        assert_eq!(rig.asked().actor, Seat(1), "the owner of the units answers");
        assert!(matches!(rig.asked().ask, Ask::Tactical(_)), "{:?}", rig.asked().ask);
        let mut questions = 0;
        while !rig.is_done() {
            rig.pick(0);
            questions += 1;
        }
        assert_eq!(at(&rig, system, 1, Unit::FighterI), 4);
        assert_eq!(questions, 4);
        assert_eq!(captured(&rig).iter().filter(|c| c.1 == Unit::FighterI).count(), 4, "each fighter that left is captured");
        return;
    }
    panic!("no seed captures exactly one of the two carriers");
}

#[test]
fn owners_and_systems_are_enforced_in_the_order_the_adapter_gives() {
    covers!("rules::hero_windows::CabalHero");
    let [(a, _), (b, _)] = two();
    let sc = Scenario::new(&[Faction::TheVuilraithCabal, Faction::TheArborec, Faction::TheEmbersOfMuaat])
        .map_from(&[a, b])
        .unit(a, None, Unit::CarrierI, 2, 1)
        .unit(a, None, Unit::FighterI, 2, 2)
        .unit(b, None, Unit::CarrierI, 1, 1)
        .unit(b, None, Unit::FighterI, 1, 2);
    // Seat 2 in the first system, seat 1 in the second; the adapter's order puts seat 2 first.
    let targets = [(a, Piece::new(Unit::CarrierI, Seat(2))), (b, Piece::new(Unit::CarrierI, Seat(1)))];
    let stranded = [Stranded { owner: Seat(2), system: a, limits: limits() }, Stranded { owner: Seat(1), system: b, limits: limits() }];
    for seed in 0..80 {
        let mut rig = carrion(sc.clone(), true, targets.to_vec(), stranded.to_vec()).seeded(seed);
        rig.run();
        if rig.is_done() {
            continue;
        }
        assert_eq!(rig.asked().actor, Seat(2), "the first pair of the list is enforced first when both lost a ship, or the only one that did");
        let both = {
            while !rig.is_done() {
                rig.pick(0);
            }
            captured(&rig).iter().any(|c| c.0 == Seat(1)) && captured(&rig).iter().any(|c| c.0 == Seat(2))
        };
        if both {
            let rows = captured(&rig);
            let first_seat1 = rows.iter().position(|c| c.0 == Seat(1)).unwrap();
            let last_seat2 = rows.iter().rposition(|c| c.0 == Seat(2)).unwrap();
            assert!(first_seat1 < last_seat2 || rows.len() >= 4, "{rows:?}");
            return;
        }
    }
    panic!("no seed captures both carriers");
}

// -- Hesh and Prit, Mathis Mathinus ------------------------------------------------------------------

use ti4_engine_core::kernel::fx_perform::{Carried, Facts as PerformFacts, Home};
use ti4_engine_core::rules::hero_windows::{NaazHero, WinnuHero};
use ti4_engine_core::tables::Scope;

fn perform_facts(carried: Carried) -> PerformFacts {
    PerformFacts { carried, replenish_events: false, rules_version: LATEST_RULES, roster: None, scope: Scope(u32::from(kernel_all().sources.0)), mahact_held: 0, home: Home::Unsupported, imperial: None, free: None, te_uses: None }
}

fn naaz(cards: &[StrategyCard], relics: &[Relic], tokens: (i16, i16, i16)) -> Rig<NaazHero> {
    let sc = Scenario::new(&[Faction::TheNaazRokhaAlliance, Faction::TheArborec]).kernel(kernel_all()).tokens(0, tokens.0, tokens.1, tokens.2).commodities(0, 0);
    let mut rig = Rig::new(sc, NaazHero::new(Seat(0), perform_facts(Carried { payments: true, ..Carried::default() }), cards.to_vec()));
    rig.state.decks_mut().relic.set(relics.to_vec());
    rig
}

fn relics_of(rig: &Rig<NaazHero>) -> Vec<Relic> {
    rig.state.players().by_seat(Seat(0)).unwrap().cards.relics.as_slice().to_vec()
}

#[test]
fn without_a_relic_in_the_deck_nothing_is_asked_and_the_card_is_not_used() {
    covers!("rules::hero_windows::NaazHero");
    let mut rig = naaz(&[StrategyCard::Trade], &[], (3, 3, 2));
    rig.run();
    assert!(rig.is_done() && !rig.driver.window.used && !rig.driver.window.unsupported);
    assert_eq!(delta(rig.start_state(), &rig.state), "");
}

#[test]
fn up_to_two_cards_are_chosen_the_relic_is_gained_and_each_secondary_is_performed_in_the_order_chosen() {
    covers!("rules::hero_windows::NaazHero");
    let relic = Relic::ALL.iter().copied().find(|r| *r != Relic::ShardOfTheThrone).unwrap();
    let mut rig = naaz(&[StrategyCard::Leadership, StrategyCard::Trade, StrategyCard::Imperial, StrategyCard::Warfare], &[relic], (3, 3, 2));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Leaders(LAsk::NaazCard)));
    assert_eq!(
        rig.opts(),
        vec![
            Opt::Leaders(LOpt::HeroCard(StrategyCard::Leadership)),
            Opt::Leaders(LOpt::HeroCard(StrategyCard::Trade)),
            Opt::Leaders(LOpt::HeroCard(StrategyCard::Imperial)),
            Opt::Leaders(LOpt::HeroCard(StrategyCard::Warfare)),
            Opt::Decline
        ]
    );
    let at_first = rig.state.clone();
    rig.pick_opt(Opt::Leaders(LOpt::HeroCard(StrategyCard::Trade)));
    assert_eq!(rig.opts().len(), 4, "the card chosen is not offered again");
    assert_eq!(delta(&at_first, &rig.state), "", "nothing changes before the last choice");
    rig.pick_opt(Opt::Leaders(LOpt::HeroCard(StrategyCard::Imperial)));
    // The relic, the replenishment of Trade's secondary and the secret of Imperial's secondary.
    while !rig.is_done() {
        rig.pick(0);
    }
    assert!(rig.driver.window.used && !rig.driver.window.unsupported, "{}", rig.transcript());
    assert_eq!(relics_of(&rig), vec![relic]);
    let p = rig.state.players().by_seat(Seat(0)).unwrap();
    assert!(p.econ.commodities > 0, "the Trade secondary replenished");
}

#[test]
fn declining_ends_the_choice_and_the_relic_is_still_gained() {
    covers!("rules::hero_windows::NaazHero");
    let relic = Relic::ALL.iter().copied().find(|r| *r != Relic::ShardOfTheThrone).unwrap();
    let mut rig = naaz(&[StrategyCard::Trade, StrategyCard::Leadership], &[relic], (3, 3, 2));
    rig.run();
    rig.decline();
    assert!(rig.is_done() && rig.driver.window.used);
    assert_eq!(relics_of(&rig), vec![relic]);
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.commodities, 0, "no secondary performed");
}

#[test]
fn a_card_that_costs_a_token_is_offered_only_while_the_reinforcements_hold_one() {
    covers!("rules::hero_windows::NaazHero");
    let relic = Relic::ALL.iter().copied().find(|r| *r != Relic::ShardOfTheThrone).unwrap();
    // 16 tokens on the sheet: none in the reinforcements.
    let mut rig = naaz(&[StrategyCard::Trade, StrategyCard::Leadership], &[relic], (8, 4, 4));
    rig.run();
    assert_eq!(rig.opts(), vec![Opt::Leaders(LOpt::HeroCard(StrategyCard::Leadership)), Opt::Decline], "Leadership's secondary costs influence, not a token");
    // Nothing else to offer: the choice ends and the relic is gained unasked.
    let mut none = naaz(&[StrategyCard::Trade, StrategyCard::Imperial], &[relic], (8, 4, 4));
    none.run();
    assert!(none.is_done() && none.driver.window.used);
}

#[test]
fn the_warfare_secondary_is_not_carried_and_makes_the_window_unsupported() {
    covers!("rules::hero_windows::NaazHero");
    let relic = Relic::ALL.iter().copied().find(|r| *r != Relic::ShardOfTheThrone).unwrap();
    let mut rig = naaz(&[StrategyCard::Warfare], &[relic], (3, 3, 2));
    rig.run();
    rig.pick_opt(Opt::Leaders(LOpt::HeroCard(StrategyCard::Warfare)));
    assert!(rig.is_done() && rig.driver.window.unsupported, "the string performs the home production");
}

#[test]
fn naaz_refuses_a_card_that_was_not_offered() {
    covers!("rules::hero_windows::NaazHero");
    let relic = Relic::ALL.iter().copied().find(|r| *r != Relic::ShardOfTheThrone).unwrap();
    let mut rig = naaz(&[StrategyCard::Trade], &[relic], (3, 3, 2));
    rig.run();
    rig.expect_illegal(Pick::Opt(Opt::Leaders(LOpt::HeroCard(StrategyCard::Politics))));
    rig.pick_opt(Opt::Leaders(LOpt::HeroCard(StrategyCard::Trade)));
    // One candidate left: nothing remains to choose, the window went on to its secondaries.
    assert!(rig.is_done());
}

fn winnu(cards: &[StrategyCard]) -> Rig<WinnuHero> {
    let sc = Scenario::new(&[Faction::TheWinnu, Faction::TheArborec, Faction::TheEmbersOfMuaat]).kernel(kernel_all()).commodities(0, 0).commodities(1, 0);
    Rig::new(sc, WinnuHero::new(Seat(0), perform_facts(Carried { payments: true, ..Carried::default() }), cards.to_vec()))
}

#[test]
fn the_card_is_asked_even_for_one_and_its_primary_is_performed_by_the_ability_window() {
    covers!("rules::hero_windows::WinnuHero");
    let mut rig = winnu(&[StrategyCard::Trade, StrategyCard::Imperial]);
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Leaders(LAsk::WinnuCard)));
    assert_eq!(rig.opts(), vec![Opt::Leaders(LOpt::HeroCard(StrategyCard::Trade)), Opt::Leaders(LOpt::HeroCard(StrategyCard::Imperial))], "no decline");
    rig.assert_options_sound();
    rig.pick_opt(Opt::Leaders(LOpt::HeroCard(StrategyCard::Trade)));
    while !rig.is_done() {
        rig.pick(0);
    }
    assert_eq!(rig.driver.window.chosen, Some(StrategyCard::Trade));
    assert!(!rig.driver.window.unsupported);
    assert!(rig.state.players().by_seat(Seat(0)).unwrap().econ.commodities > 0, "the Trade primary replenished the performer");
    assert!(!rig.state.pending().staged.is_empty(), "and announced the three trade goods");
}

#[test]
fn the_imperial_primary_is_not_carried_and_no_cards_means_no_question() {
    covers!("rules::hero_windows::WinnuHero");
    let mut rig = winnu(&[StrategyCard::Imperial]);
    rig.run();
    rig.pick(0);
    assert!(rig.is_done() && rig.driver.window.unsupported && rig.driver.window.chosen == Some(StrategyCard::Imperial));
    let mut none = winnu(&[]);
    none.run();
    assert!(none.is_done() && none.driver.window.chosen.is_none() && !none.driver.window.unsupported);
    assert_eq!(delta(none.start_state(), &none.state), "");
}

#[test]
fn winnu_refuses_a_card_that_was_not_offered() {
    covers!("rules::hero_windows::WinnuHero");
    let mut rig = winnu(&[StrategyCard::Trade]);
    rig.run();
    rig.expect_illegal(Pick::Opt(Opt::Leaders(LOpt::HeroCard(StrategyCard::Politics))));
    rig.expect_illegal(Pick::Opt(Opt::Decline));
}

#[test]
fn a_ship_that_is_not_there_is_not_captured_and_strands_nothing() {
    covers!("rules::hero_windows::CabalHero");
    let (sc, system) = hub();
    // Fighters without a carrier stand over the capacity already; no ship was captured, so nobody is enforced.
    let sc = sc.unit(system, None, Unit::FighterI, 1, 3);
    let target = (system, Piece::new(Unit::CarrierI, Seat(1)));
    let stranded = [Stranded { owner: Seat(1), system, limits: limits() }];
    for seed in 0..30 {
        let mut rig = carrion(sc.clone(), true, vec![target], stranded.to_vec()).seeded(seed);
        rig.run();
        assert!(rig.is_done(), "no question for a ship that was never there: {}", rig.transcript());
        assert!(captured(&rig).is_empty());
        assert_eq!(at(&rig, system, 1, Unit::FighterI), 3);
    }
}

// -- Flock Migration ------------------------------------------------------------------------------------

use ti4_engine_core::rules::hero_windows::{ArgentHero, MigrationFacts};

fn three() -> [(System, Planet); 3] {
    let mut found: Vec<(System, Planet)> = Vec::new();
    for s in plain_systems() {
        if let Some(p) = s.data().planets.first()
            && s.data().planets.len() == 1
        {
            found.push((s, *p));
        }
        if found.len() == 3 {
            break;
        }
    }
    found.sort_unstable();
    [found[0], found[1], found[2]]
}

fn facts(sys: &[System], enterable: &[System]) -> MigrationFacts {
    MigrationFacts { order: sys.to_vec(), on_map: sys.to_vec(), enterable: enterable.to_vec() }
}

/// Seat 0 (Argent) holds a carrier in the first system, command tokens on the other two.
fn flock(extra: impl FnOnce(Scenario, &[(System, Planet); 3]) -> Scenario, enterable: Option<Vec<System>>) -> (Rig<ArgentHero>, [(System, Planet); 3]) {
    let spots = three();
    let sys: Vec<System> = spots.iter().map(|s| s.0).collect();
    let sc = Scenario::new(&[Faction::TheArgentFlight, Faction::TheArborec])
        .kernel(kernel_all())
        .map_from(&sys)
        .unit(sys[0], None, Unit::CarrierI, 0, 1)
        .command_token(sys[1], 0)
        .command_token(sys[2], 0);
    let sc = extra(sc, &spots);
    let enterable = enterable.unwrap_or_else(|| sys.clone());
    (Rig::new(sc, ArgentHero::new(Seat(0), facts(&sys, &enterable))), spots)
}

#[test]
fn a_ship_moves_to_a_tokened_system_free_of_other_ships_and_is_not_offered_again() {
    covers!("rules::hero_windows::ArgentHero");
    let (mut rig, spots) = flock(|sc, s| sc.unit(s[2].0, None, Unit::CruiserI, 1, 1), None);
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Leaders(LAsk::FlockShip)));
    assert_eq!(rig.opts(), vec![Opt::Leaders(LOpt::FlockShip { index: 0, system: spots[0].0, unit: Unit::CarrierI }), Opt::Decline]);
    rig.assert_options_sound();
    rig.pick(0);
    assert!(matches!(rig.asked().ask, Ask::Leaders(LAsk::FlockTo)));
    assert_eq!(rig.opts(), vec![Opt::Leaders(LOpt::FlockTo(spots[1].0)), Opt::Decline], "the system with another player's ship is not offered");
    rig.pick(0);
    assert!(rig.is_done(), "the carrier that arrived is not offered again: {}", rig.transcript());
    assert!(rig.driver.window.moved);
    assert_eq!(at_seat(&rig, spots[0].0, 0, Unit::CarrierI), 0);
    assert_eq!(at_seat(&rig, spots[1].0, 0, Unit::CarrierI), 1);
}

fn at_seat<W: Window>(rig: &Rig<W>, system: System, seat: u8, unit: Unit) -> usize {
    rig.state.board().sys(system).units.as_slice().iter().filter(|p| p.owner() == Seat(seat) && p.kind() == unit).count()
}

#[test]
fn units_are_carried_from_the_space_and_the_planets_until_the_seat_stops_and_land_in_space() {
    covers!("rules::hero_windows::ArgentHero");
    let (mut rig, spots) = flock(|sc, s| sc.control(s[0].1, 0).unit(s[0].0, Some(s[0].1), Unit::InfantryI, 0, 2), None);
    rig.run();
    rig.pick(0);
    rig.pick(0);
    assert!(matches!(rig.asked().ask, Ask::Leaders(LAsk::FlockCargo)));
    assert_eq!(rig.opts(), vec![Opt::Leaders(LOpt::FlockCargo { index: 0, unit: Unit::InfantryI }), Opt::Leaders(LOpt::FlockCargo { index: 1, unit: Unit::InfantryI }), Opt::Decline]);
    rig.assert_options_sound();
    rig.pick(0);
    rig.decline();
    assert!(rig.is_done() && rig.driver.window.moved);
    assert_eq!(at_seat(&rig, spots[1].0, 0, Unit::InfantryI), 1, "the infantry that was taken is in the space area of the destination");
    assert_eq!(rig.state.board().planet(spots[0].1).units.as_slice().iter().filter(|p| p.owner() == Seat(0)).count(), 1, "the other stays on its planet");
}

#[test]
fn a_cargo_that_does_not_fit_is_dropped_and_the_ship_moves_alone() {
    covers!("rules::hero_windows::ArgentHero");
    // A carrier holds four; five ground forces are taken aboard.
    let (mut rig, spots) = flock(|sc, s| sc.control(s[0].1, 0).unit(s[0].0, Some(s[0].1), Unit::InfantryI, 0, 5), None);
    rig.run();
    rig.pick(0);
    rig.pick(0);
    for _ in 0..5 {
        rig.pick(0);
    }
    // Nothing is left to offer: the ship moves.
    assert!(rig.is_done() && rig.driver.window.moved);
    assert_eq!(at_seat(&rig, spots[1].0, 0, Unit::CarrierI), 1);
    assert_eq!(at_seat(&rig, spots[1].0, 0, Unit::InfantryI), 0, "the hold was over capacity: no cargo");
    assert_eq!(rig.state.board().planet(spots[0].1).units.as_slice().len(), 5);
}

#[test]
fn declining_any_question_ends_the_migration_and_a_destination_the_seat_cannot_enter_moves_nothing() {
    covers!("rules::hero_windows::ArgentHero");
    let (mut a, _) = flock(|sc, _| sc, None);
    a.run();
    a.decline();
    assert!(a.is_done() && !a.driver.window.moved);
    assert_eq!(delta(a.start_state(), &a.state), "");
    let (mut b, _) = flock(|sc, _| sc, None);
    b.run();
    b.pick(0);
    b.decline();
    assert!(b.is_done() && !b.driver.window.moved);
    // Neither destination can be entered: the relocation is refused and the card ends.
    let (mut c, _) = flock(|sc, _| sc, Some(Vec::new()));
    c.run();
    c.pick(0);
    c.pick(0);
    assert!(c.is_done() && !c.driver.window.moved);
    assert_eq!(delta(c.start_state(), &c.state), "");
}

#[test]
fn each_ship_moves_once_over_the_whole_migration() {
    covers!("rules::hero_windows::ArgentHero");
    let (mut rig, spots) = flock(|sc, s| sc.unit(s[0].0, None, Unit::DestroyerI, 0, 1), None);
    rig.run();
    // Two stacks of the first system: carrier, destroyer. Move the carrier, then the destroyer to the other tokened system.
    assert_eq!(rig.opts().len(), 3);
    rig.pick(0);
    rig.pick(1);
    let ships: Vec<Opt> = rig.opts();
    assert!(ships.contains(&Opt::Leaders(LOpt::FlockShip { index: 0, system: spots[0].0, unit: Unit::DestroyerI })), "{ships:?}");
    assert!(!ships.iter().any(|o| matches!(o, Opt::Leaders(LOpt::FlockShip { unit: Unit::CarrierI, .. }))), "the carrier that arrived is not offered: {ships:?}");
    rig.pick(0);
    rig.pick(0);
    assert!(rig.is_done() && rig.driver.window.moved, "{}", rig.transcript());
}

#[test]
fn the_cards_a_player_may_show_are_offered_in_the_order_of_the_names_not_of_the_hand() {
    covers!("rules::hero_windows::KyverHero");
    // A hand whose order differs from the order of the names.
    let (first, second) = {
        let mut found = None;
        for (i, a) in ActionCard::ALL.iter().enumerate() {
            for b in &ActionCard::ALL[i + 1..] {
                if a.name_rank() > b.name_rank() {
                    found = Some((*a, *b));
                }
            }
            if found.is_some() {
                break;
            }
        }
        found.expect("two cards out of name order")
    };
    let mut rig = kyver([&[], &[first, second], &[]], false);
    rig.run();
    assert_eq!(rig.opts(), vec![Opt::Cards(COpt::Card(second)), Opt::Cards(COpt::Card(first))]);
}

#[test]
fn the_third_card_is_never_asked_and_a_secondary_that_asks_nothing_does_not_stop_the_next() {
    covers!("rules::hero_windows::NaazHero");
    let relic = Relic::ALL.iter().copied().find(|r| *r != Relic::ShardOfTheThrone).unwrap();
    let mut rig = naaz(&[StrategyCard::Trade, StrategyCard::Imperial, StrategyCard::Leadership], &[relic], (3, 3, 2));
    rig.state.decks_mut().secret.set(vec![SecretObjective::ALL[0]]);
    rig.run();
    rig.pick_opt(Opt::Leaders(LOpt::HeroCard(StrategyCard::Trade)));
    rig.pick_opt(Opt::Leaders(LOpt::HeroCard(StrategyCard::Imperial)));
    assert!(rig.is_done() || !matches!(rig.asked().ask, Ask::Leaders(LAsk::NaazCard)), "two cards at most");
    while !rig.is_done() {
        rig.pick(0);
    }
    assert!(rig.state.players().by_seat(Seat(0)).unwrap().econ.commodities > 0, "Trade, which asks nothing");
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().cards.secrets.as_slice().len(), 1, "and then Imperial's secondary drew the secret");
}

#[test]
fn a_table_where_a_replenishment_causes_events_leaves_the_trade_secondary_to_the_string() {
    covers!("rules::hero_windows::NaazHero");
    let relic = Relic::ALL.iter().copied().find(|r| *r != Relic::ShardOfTheThrone).unwrap();
    let sc = Scenario::new(&[Faction::TheNaazRokhaAlliance, Faction::TheArborec]).kernel(kernel_all()).tokens(0, 3, 3, 2);
    let mut facts = perform_facts(Carried::default());
    facts.replenish_events = true;
    let mut rig = Rig::new(sc, NaazHero::new(Seat(0), facts, vec![StrategyCard::Trade]));
    rig.state.decks_mut().relic.set(vec![relic]);
    rig.run();
    rig.pick(0);
    assert!(rig.is_done() && rig.driver.window.unsupported);
}

/// A command token in a Mahact fleet pool is not in the reinforcements, which the core reads from the marks (`fx::tokens_held_by_mahact`):
/// the Leadership secondary is the core's at such a table since package W13 (it was the string's before).
#[test]
fn a_command_token_in_a_mahact_pool_does_not_make_the_leadership_secondary_the_strings() {
    covers!("rules::hero_windows::NaazHero");
    let relic = Relic::ALL.iter().copied().find(|r| *r != Relic::ShardOfTheThrone).unwrap();
    let sc = Scenario::new(&[Faction::TheNaazRokhaAlliance, Faction::TheArborec]).kernel(kernel_all()).tokens(0, 3, 3, 2);
    let mut facts = perform_facts(Carried { payments: true, ..Carried::default() });
    facts.mahact_held = 1;
    let mut rig = Rig::new(sc, NaazHero::new(Seat(0), facts, vec![StrategyCard::Leadership]));
    rig.state.decks_mut().relic.set(vec![relic]);
    rig.run();
    rig.pick(0);
    assert!(!rig.driver.window.unsupported, "{}", rig.transcript());
}

#[test]
fn the_diplomacy_primary_reports_the_tokens_it_placed() {
    covers!("rules::hero_windows::WinnuHero");
    let [(a, pa), _, _] = three();
    let sc = Scenario::new(&[Faction::TheWinnu, Faction::TheArborec, Faction::TheEmbersOfMuaat]).kernel(kernel_all()).map_from(&[a]).control(pa, 0);
    let mut rig = Rig::new(sc, WinnuHero::new(Seat(0), perform_facts(Carried::default()), vec![StrategyCard::DiplomacyCodex]));
    rig.run();
    rig.pick(0);
    while !rig.is_done() {
        rig.pick(0);
    }
    assert!(!rig.driver.window.unsupported);
    assert!(rig.driver.window.tokens.is_some(), "the system and the seats that got a token are kept for the announcement");
}

// -- more of Flock Migration ---------------------------------------------------------------------------

#[test]
fn a_ship_standing_in_the_only_destination_is_not_offered_and_a_destination_is_never_the_origin() {
    covers!("rules::hero_windows::ArgentHero");
    // One tokened system, with a ship of its own in it, and another system with a ship: only the second ship can move, and not to its own system.
    let spots = three();
    let sys: Vec<System> = spots.iter().map(|s| s.0).collect();
    let sc = Scenario::new(&[Faction::TheArgentFlight, Faction::TheArborec])
        .kernel(kernel_all())
        .map_from(&sys)
        .unit(sys[0], None, Unit::CarrierI, 0, 1)
        .unit(sys[1], None, Unit::DestroyerI, 0, 1)
        .command_token(sys[1], 0);
    let mut rig = Rig::new(sc, ArgentHero::new(Seat(0), facts(&sys, &sys)));
    rig.run();
    assert_eq!(rig.opts(), vec![Opt::Leaders(LOpt::FlockShip { index: 0, system: sys[0], unit: Unit::CarrierI }), Opt::Decline], "the destroyer already stands in the only destination");
    rig.pick(0);
    assert_eq!(rig.opts(), vec![Opt::Leaders(LOpt::FlockTo(sys[1])), Opt::Decline]);
    // With a token on the origin too, the origin is not a destination of its own ship.
    let sc = Scenario::new(&[Faction::TheArgentFlight, Faction::TheArborec])
        .kernel(kernel_all())
        .map_from(&sys)
        .unit(sys[0], None, Unit::CarrierI, 0, 1)
        .command_token(sys[0], 0)
        .command_token(sys[1], 0);
    let mut rig = Rig::new(sc, ArgentHero::new(Seat(0), facts(&sys, &sys)));
    rig.run();
    rig.pick(0);
    assert_eq!(rig.opts(), vec![Opt::Leaders(LOpt::FlockTo(sys[1])), Opt::Decline]);
}

#[test]
fn the_hold_is_asked_up_to_the_capacity_plus_eight_units() {
    covers!("rules::hero_windows::ArgentHero");
    let (mut rig, spots) = flock(|sc, s| sc.control(s[0].1, 0).unit(s[0].0, Some(s[0].1), Unit::InfantryI, 0, 14), None);
    rig.run();
    rig.pick(0);
    rig.pick(0);
    for _ in 0..12 {
        assert!(matches!(rig.asked().ask, Ask::Leaders(LAsk::FlockCargo)));
        rig.pick(0);
    }
    assert!(rig.is_done(), "a carrier holds 4, so 12 units are asked and the 13th is not: {}", rig.transcript());
    assert_eq!(at_seat(&rig, spots[1].0, 0, Unit::CarrierI), 1);
}

#[test]
fn a_destination_off_the_map_is_refused_though_the_seat_could_enter_it() {
    covers!("rules::hero_windows::ArgentHero");
    let spots = three();
    let sys: Vec<System> = spots.iter().map(|s| s.0).collect();
    let sc = Scenario::new(&[Faction::TheArgentFlight, Faction::TheArborec]).kernel(kernel_all()).map_from(&sys).unit(sys[0], None, Unit::CarrierI, 0, 1).command_token(sys[1], 0);
    let mut off = facts(&sys, &sys);
    off.on_map.retain(|s| *s != sys[1]);
    let mut rig = Rig::new(sc, ArgentHero::new(Seat(0), off));
    rig.run();
    rig.pick(0);
    rig.pick(0);
    assert!(rig.is_done() && !rig.driver.window.moved);
    assert_eq!(delta(rig.start_state(), &rig.state), "");
}

// -- Conservator Procyon ---------------------------------------------------------------------------------------

use ti4_engine_core::choice::strategy::{Ask as SAsk, Opt as SOpt, Pool};
use ti4_engine_core::rules::hero_windows::{EmpyreanHero, FrontierFacts, How};

const EMPTY: Code = Code(3);
const AGREEMENT: Code = Code(9);

fn frontier_facts() -> FrontierFacts {
    FrontierFacts { empty: EMPTY, rules_version: LATEST_RULES, mahact_held: 0, agreement: Some(AGREEMENT), draw_plain: true }
}

/// Seat 0 (an Empyrean) holds a cruiser and a frontier token in each of `tokens` systems; the frontier deck is `deck`; the window explores
/// them in the order of the systems returned.
fn empyrean(deck: &[Explore], tokens: usize, facts: FrontierFacts) -> (Rig<EmpyreanHero>, Vec<System>) {
    let (state, kernel, systems) = empyrean_parts(deck, tokens, facts);
    let window = EmpyreanHero::new(Seat(0), Vec::new(), systems.clone(), facts);
    (Rig::from_parts(state, kernel, window), systems)
}

fn empyrean_parts(deck: &[Explore], tokens: usize, facts: FrontierFacts) -> (CoreState, Kernel, Vec<System>) {
    let systems: Vec<System> = plain_systems().into_iter().take(tokens.max(1)).collect();
    let mut sc = Scenario::new(&[Faction::TheEmpyrean, Faction::TheArborec]).map_from(&systems).rules(facts.rules_version).commodities(0, 0);
    for system in &systems {
        sc = sc.unit(*system, None, Unit::CruiserI, 0, 1);
    }
    let (mut state, kernel) = sc.build();
    for system in systems.iter().take(tokens) {
        state.explore_mut().frontier.insert(*system);
    }
    state.decks_mut().explore[ExploreType::Frontier as usize].set(deck.to_vec());
    state.decks_mut().explore_present |= 1 << ExploreType::Frontier as u8;
    (state, kernel, systems)
}

fn goods_of(rig: &Rig<EmpyreanHero>) -> i16 {
    rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods
}

#[test]
fn a_fragment_and_an_entropic_field_are_explored_in_order_and_the_pool_is_asked() {
    covers!("rules::hero_windows::EmpyreanHero");
    let (mut rig, systems) = empyrean(&[Explore::UnknownRelicFragment1, Explore::EntropicField], 2, frontier_facts());
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Strategy(SAsk::GainPool { left: 1 })), "{}", rig.transcript());
    assert_eq!(rig.opts().len(), 3);
    rig.assert_options_sound();
    rig.pick_opt(Opt::Strategy(SOpt::Pool(Pool::Strategic)));
    assert!(rig.is_done() && rig.driver.window.used && !rig.driver.window.unsupported, "{}", rig.transcript());
    let p = rig.state.players().by_seat(Seat(0)).unwrap();
    assert_eq!(p.cards.fragments.get(ExploreType::Frontier), Some(1));
    assert_eq!(goods_of(&rig), 2);
    assert!(rig.state.explore().frontier.is_empty(), "both tokens are spent");
    let log: Vec<_> = rig.state.explore().log.iter().map(|r| (r.card, r.system, r.deck, r.planet)).collect();
    assert_eq!(
        log,
        vec![(Explore::UnknownRelicFragment1, Some(systems[0]), ExploreType::Frontier, None), (Explore::EntropicField, Some(systems[1]), ExploreType::Frontier, None)],
        "the rows are opened with the token's system, in the order of the systems"
    );
    assert!(rig.state.explore().log.iter().all(|r| r.outcome == EMPTY), "the words of a row are the adapter's");
    let rows = rig.driver.window.rows();
    assert_eq!(rows.len(), 2);
    assert_eq!((rows[0].how, rows[0].system), (How::Fragment(ExploreType::Frontier), systems[0]));
    assert_eq!((rows[1].how, rows[1].card), (How::Resolved, Explore::EntropicField));
    assert_eq!(rows[0].after.trade_goods, rows[0].before.trade_goods, "the fragment gives no goods");
    assert_eq!((rows[1].before.trade_goods, rows[1].after.trade_goods), (0, 2), "the holdings are read around each card");
}

#[test]
fn the_tokens_are_explored_in_the_order_the_adapter_gives_and_a_token_without_a_ship_is_left() {
    covers!("rules::hero_windows::EmpyreanHero");
    let (mut state, kernel, systems) = empyrean_parts(&[Explore::UnknownRelicFragment1, Explore::UnknownRelicFragment2, Explore::UnknownRelicFragment3], 3, frontier_facts());
    // The order reversed and the middle system without a ship.
    state.board_mut().sys_mut(systems[1]).units.clear();
    let mut rig = Rig::from_parts(state, kernel, EmpyreanHero::new(Seat(0), Vec::new(), vec![systems[2], systems[1], systems[0]], frontier_facts()));
    rig.run();
    assert!(rig.is_done() && rig.driver.window.used);
    let log: Vec<_> = rig.state.explore().log.iter().map(|r| (r.card, r.system)).collect();
    assert_eq!(log, vec![(Explore::UnknownRelicFragment1, Some(systems[2])), (Explore::UnknownRelicFragment2, Some(systems[0]))]);
    assert!(rig.state.explore().frontier.contains(systems[1]), "the token with no ship of the seat stays");
}

#[test]
fn the_placed_tokens_join_the_ones_on_the_board() {
    covers!("rules::hero_windows::EmpyreanHero");
    let (mut state, kernel, systems) = empyrean_parts(&[Explore::UnknownRelicFragment1, Explore::UnknownRelicFragment2], 2, frontier_facts());
    state.explore_mut().frontier.remove(systems[1]);
    let mut rig = Rig::from_parts(state, kernel, EmpyreanHero::new(Seat(0), vec![systems[1]], vec![systems[0], systems[1]], frontier_facts()));
    rig.run();
    assert!(rig.is_done() && rig.driver.window.used);
    assert_eq!(rig.state.explore().log.len(), 2, "the placed token under a ship is explored too");
    assert_eq!(rig.state.explore().log.iter().map(|r| r.system).collect::<Vec<_>>(), vec![Some(systems[0]), Some(systems[1])]);
    assert!(rig.state.explore().frontier.is_empty());
}

#[test]
fn a_merchant_station_replenishes_or_converts() {
    covers!("rules::hero_windows::EmpyreanHero");
    // Nothing to convert: the lone option is taken unasked and the commodities come to the printed value.
    let (mut rig, _) = empyrean(&[Explore::MerchantStation1], 1, frontier_facts());
    rig.run();
    assert!(rig.is_done() && rig.driver.window.used && !rig.driver.window.unsupported, "{}", rig.transcript());
    let limit = i16::from(Faction::TheEmpyrean.data().commodities);
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.commodities, limit);
    // Commodities in hand: asked; converted they become trade goods.
    let (mut rig, _) = empyrean(&[Explore::MerchantStation2], 1, frontier_facts());
    rig.state.players_mut().by_seat_mut(Seat(0)).unwrap().econ.commodities = 2;
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Explore(_)), "{}", rig.transcript());
    rig.pick_where(|o| matches!(o, Opt::Explore(ti4_engine_core::choice::explore::Opt::Convert)));
    assert!(rig.is_done() && rig.driver.window.used);
    assert_eq!((goods_of(&rig), rig.state.players().by_seat(Seat(0)).unwrap().econ.commodities), (2, 0));
    // Replenished with commodities in hand.
    let (mut rig, _) = empyrean(&[Explore::MerchantStation2], 1, frontier_facts());
    rig.state.players_mut().by_seat_mut(Seat(0)).unwrap().econ.commodities = 1;
    rig.run();
    rig.pick_where(|o| matches!(o, Opt::Explore(ti4_engine_core::choice::explore::Opt::Replenish)));
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.commodities, limit);
}

#[test]
fn a_replenish_the_core_does_not_follow_declines_the_run_but_the_conversion_is_carried() {
    covers!("rules::hero_windows::EmpyreanHero");
    let facts = FrontierFacts { agreement: None, ..frontier_facts() };
    let (mut rig, _) = empyrean(&[Explore::MerchantStation1], 1, facts);
    rig.run();
    assert!(rig.is_done() && rig.driver.window.unsupported && !rig.driver.window.used, "{}", rig.transcript());
    let (mut rig, _) = empyrean(&[Explore::MerchantStation1], 1, facts);
    rig.state.players_mut().by_seat_mut(Seat(0)).unwrap().econ.commodities = 1;
    rig.run();
    rig.pick_where(|o| matches!(o, Opt::Explore(ti4_engine_core::choice::explore::Opt::Convert)));
    assert!(rig.is_done() && !rig.driver.window.unsupported);
}

#[test]
fn a_deck_that_runs_out_undoes_the_run_after_the_questions_already_asked() {
    covers!("rules::hero_windows::EmpyreanHero");
    // The first token draws an entropic field (a pool question), the second finds the deck empty: everything is undone.
    let (mut rig, _) = empyrean(&[Explore::EntropicField], 2, frontier_facts());
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Strategy(SAsk::GainPool { .. })), "the first card's question is asked before the failure");
    rig.pick_opt(Opt::Strategy(SOpt::Pool(Pool::Tactic)));
    assert!(rig.is_done());
    assert!(!rig.driver.window.used && !rig.driver.window.unsupported);
    assert!(rig.driver.window.rows().is_empty());
    assert_eq!(delta(rig.start_state(), &rig.state), "", "the failed exploration is not a use and changes nothing");
    // No card at all.
    let (mut rig, _) = empyrean(&[], 1, frontier_facts());
    rig.run();
    assert!(rig.is_done() && !rig.driver.window.used);
    assert_eq!(delta(rig.start_state(), &rig.state), "");
}

#[test]
fn the_gamma_token_needs_an_active_system_and_the_ion_storm_a_planet() {
    covers!("rules::hero_windows::EmpyreanHero");
    let (mut rig, systems) = empyrean(&[Explore::GammaRelay, Explore::GammaWormhole, Explore::IonStorm], 3, frontier_facts());
    rig.state.meta_mut().active_system = Some(systems[2]);
    rig.run();
    assert!(rig.is_done() && rig.driver.window.used);
    assert_eq!(rig.state.explore().wormhole_tokens.get(&Wormhole::Gamma), Some(&systems[2]));
    let rows = rig.driver.window.rows();
    assert_eq!((rows[0].how, rows[0].gamma), (How::Resolved, Some(systems[2])), "the first card places the token");
    assert_eq!((rows[1].how, rows[1].gamma), (How::Resolved, None), "the second finds it in place");
    assert_eq!(rows[2].how, How::Unresolved, "the ion storm names its system through a planet");
    assert!(rig.state.meta().ion_storm.is_none());
    // Without an active system the card has nowhere to act.
    let (mut rig, _) = empyrean(&[Explore::GammaRelay], 1, frontier_facts());
    rig.state.meta_mut().active_system = None;
    rig.run();
    assert_eq!(rig.driver.window.rows()[0].how, How::Unresolved);
    assert!(rig.state.explore().wormhole_tokens.is_empty());
}

#[test]
fn the_mirage_is_placed_in_the_system_of_the_token_from_the_fix_on() {
    covers!("rules::hero_windows::EmpyreanHero");
    let (mut rig, systems) = empyrean(&[Explore::Mirage], 1, frontier_facts());
    rig.run();
    assert!(rig.is_done() && rig.driver.window.used);
    assert_eq!(rig.driver.window.rows()[0].how, How::Resolved);
    let cell = rig.state.board().planet(Planet::Mirage);
    assert_eq!((cell.placed_in, cell.control), (Some(systems[0]), Some(Seat(0))));
    // Before the fix the card named no system.
    let old = FrontierFacts { rules_version: 26, ..frontier_facts() };
    let (mut rig, _) = empyrean(&[Explore::Mirage], 1, old);
    rig.run();
    assert_eq!(rig.driver.window.rows()[0].how, How::Unresolved);
    assert!(rig.state.board().planet(Planet::Mirage).placed_in.is_none());
}

#[test]
fn a_relic_a_secret_a_device_and_command_tokens_without_a_pool_are_gained() {
    covers!("rules::hero_windows::EmpyreanHero");
    let relic = Relic::ALL.iter().copied().find(|r| *r != Relic::ShardOfTheThrone).unwrap();
    let secret = SecretObjective::ALL[0];
    let (mut rig, _) = empyrean(&[Explore::DeadWorld, Explore::DerelictVessel1, Explore::EnigmaticDevice1, Explore::KeleresShip1], 4, FrontierFacts { rules_version: 11, ..frontier_facts() });
    rig.state.decks_mut().relic.set(vec![relic]);
    rig.state.decks_mut().secret.set(vec![secret]);
    let tokens = rig.state.players().by_seat(Seat(0)).unwrap().econ.strategic_tokens;
    rig.run();
    assert!(rig.is_done() && rig.driver.window.used && !rig.driver.window.unsupported, "{}", rig.transcript());
    let p = rig.state.players().by_seat(Seat(0)).unwrap();
    assert_eq!(p.cards.relics.as_slice(), &[relic]);
    assert!(p.cards.secrets.as_slice().contains(&secret));
    assert_eq!(p.cards.explore.as_slice(), &[Explore::EnigmaticDevice1]);
    assert_eq!(p.econ.strategic_tokens, tokens + 2, "before version 12 the tokens go to the strategy pool unasked");
}

#[test]
fn a_secret_over_the_limit_asks_which_to_return() {
    covers!("rules::hero_windows::EmpyreanHero");
    let (mut rig, _) = empyrean(&[Explore::DerelictVessel1], 1, frontier_facts());
    let held: Vec<SecretObjective> = SecretObjective::ALL.iter().copied().take(3).collect();
    let next = SecretObjective::ALL[3];
    rig.state.players_mut().by_seat_mut(Seat(0)).unwrap().cards.secrets.extend(held);
    rig.state.decks_mut().secret.set(vec![next]);
    rig.run();
    assert!(!rig.is_done(), "a fourth secret is over the limit: the seat is asked");
    rig.pick(0);
    assert!(rig.is_done() && rig.driver.window.used);
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().cards.secrets.len(), 3);
}

#[test]
fn what_the_core_does_not_carry_declines_the_run() {
    covers!("rules::hero_windows::EmpyreanHero");
    // A card of another deck, a Lost Crew whose draw is not plain, an unlinked child.
    let (mut rig, _) = empyrean(&[Explore::DysonSphere], 1, frontier_facts());
    rig.run();
    assert!(rig.is_done() && rig.driver.window.unsupported && !rig.driver.window.used);
    let (mut rig, _) = empyrean(&[Explore::LostCrew1], 1, FrontierFacts { draw_plain: false, ..frontier_facts() });
    rig.run();
    assert!(rig.is_done() && rig.driver.window.unsupported);
    // (Command tokens while a Mahact fleet holds the seat's are the core's since package W13: the reinforcements read the marks.)
    let (mut rig, _) = empyrean(&[Explore::MinorEntropicField], 1, FrontierFacts { mahact_held: 1, ..frontier_facts() });
    rig.run();
    assert!(!rig.driver.window.unsupported, "{}", rig.transcript());
    let (mut rig, _) = empyrean(&[Explore::UnknownRelicFragment1], 1, FrontierFacts { mahact_held: 1, ..frontier_facts() });
    rig.run();
    assert!(rig.is_done() && !rig.driver.window.unsupported, "a card without command tokens does not care");
    // The Lost Crew's draw goes through the hand-limit window, which needs the printed names: a table without them declines.
    let (mut rig, _) = empyrean(&[Explore::LostCrew2], 1, frontier_facts());
    rig.state.decks_mut().action.set(vec![ActionCard::Spy, ActionCard::Rout]);
    for card in ActionCard::ALL.iter().copied().take(7) {
        rig.state.players_mut().by_seat_mut(Seat(0)).unwrap().cards.hand.push(card);
    }
    rig.run();
    assert!(rig.is_done() && rig.driver.window.unsupported, "{}", rig.transcript());
}

#[test]
fn an_answer_outside_the_pool_question_is_refused() {
    covers!("rules::hero_windows::EmpyreanHero");
    let (mut rig, _) = empyrean(&[Explore::MajorEntropicField], 1, frontier_facts());
    rig.run();
    rig.expect_illegal(Pick::Opt(Opt::Decline));
}

// -- The Warfare secondary and the Imperial primary as children -------------------------------------------------

use ti4_engine_core::choice::production::Ask as ProdAsk;
use ti4_engine_core::kernel::children::{HomeFacts, ImperialFacts, ProduceFacts};
use ti4_engine_core::kernel::fx_pay::PayEnv;
use ti4_engine_core_tests::scenario::plain_systems as plain;

fn with_home(home: Home) -> PerformFacts {
    PerformFacts { home, roster: Some(roster()), ..perform_facts(Carried { payments: true, structures: None, ..Carried::default() }) }
}

/// Seat 0 (a Naaz) holds a space dock on the first planet of the first plain system and `goods` trade goods; the home use is limited to
/// capacity of the dock.
fn naaz_home(cards: &[StrategyCard], goods: i16, home: impl Fn(System) -> Home) -> (Rig<NaazHero>, System) {
    let system = plain()[0];
    let planet = system.data().planets[0];
    let sc = Scenario::new(&[Faction::TheNaazRokhaAlliance, Faction::TheArborec])
        .kernel(kernel_all())
        .map_from(&[system])
        .control(planet, 0)
        .unit(system, Some(planet), Unit::SpaceDockI, 0, 1)
        .goods(0, goods)
        .tokens(0, 3, 3, 2)
        .commodities(0, 0);
    let (mut state, kernel) = sc.build();
    let relic = Relic::ALL.iter().copied().find(|r| *r != Relic::ShardOfTheThrone).unwrap();
    state.decks_mut().relic.set(vec![relic]);
    let window = NaazHero::new(Seat(0), with_home(home(system)), cards.to_vec());
    (Rig::from_parts(state, kernel, window), system)
}

fn use_at(system: System) -> Home {
    Home::At(system)
}

#[test]
fn the_warfare_secondary_is_a_use_of_production_run_as_a_child_and_reports_what_it_made() {
    covers!("rules::hero_windows::NaazHero");
    let (mut rig, system) = naaz_home(&[StrategyCard::Warfare], 6, use_at);
    rig.run();
    rig.pick_opt(Opt::Leaders(LOpt::HeroCard(StrategyCard::Warfare)));
    assert!(matches!(rig.asked().ask, Ask::Production(_)), "{}", rig.transcript());
    rig.pick_where(|o| matches!(o, Opt::Production(ti4_engine_core::choice::production::Opt::Build { unit: Unit::DestroyerI, .. })));
    while !rig.is_done() {
        if matches!(rig.asked().ask, Ask::Production(ti4_engine_core::choice::production::Ask::Pay(_))) {
            rig.pick_where(|o| matches!(o, Opt::Production(ti4_engine_core::choice::production::Opt::TradeGood { .. })));
        } else {
            rig.pick_where(|o| matches!(o, Opt::Production(ti4_engine_core::choice::production::Opt::Stop)));
        }
    }
    assert!(rig.driver.window.used && !rig.driver.window.unsupported, "{}", rig.transcript());
    let made = rig.driver.window.home_made.clone().expect("the use reports what it made");
    assert_eq!(made.as_slice().len(), 1);
    assert_eq!(made.as_slice()[0].0, Unit::DestroyerI);
    assert_eq!(rig.state.board().sys(system).units.as_slice().iter().filter(|p| p.kind() == Unit::DestroyerI).count(), 1);
}

#[test]
fn a_home_system_with_nothing_to_produce_with_does_nothing_and_a_table_the_core_does_not_carry_declines() {
    covers!("rules::hero_windows::NaazHero");
    let (mut rig, _) = naaz_home(&[StrategyCard::Warfare], 6, |_| Home::Nothing);
    rig.run();
    rig.pick_opt(Opt::Leaders(LOpt::HeroCard(StrategyCard::Warfare)));
    assert!(rig.is_done() && rig.driver.window.used && !rig.driver.window.unsupported && rig.driver.window.home_made.is_none());
    let (mut rig, _) = naaz_home(&[StrategyCard::Warfare], 6, |_| Home::Unsupported);
    rig.run();
    rig.pick_opt(Opt::Leaders(LOpt::HeroCard(StrategyCard::Warfare)));
    assert!(rig.is_done() && rig.driver.window.unsupported);
}

/// The roster of the faction units: none printed (the plain forms are built).
fn roster() -> &'static ti4_engine_core::query::tech::UnitRoster {
    Box::leak(Box::new(ti4_engine_core::query::tech::UnitRoster::default()))
}

fn structure_facts() -> ti4_engine_core::kernel::children::StructureFacts {
    ti4_engine_core::kernel::children::StructureFacts { pds: Some((Unit::PDSI, false)), dock: Some((Unit::SpaceDockI, false)), source: Code(3) }
}

fn capacity_of(state: &ti4_engine_core::state::CoreState, kernel: &ti4_engine_core::kernel::Kernel, system: System) -> i64 {
    let cx = ti4_engine_core::rules::production::Cx::new(state, kernel, Scope(u32::from(kernel.sources.0)), false);
    ti4_engine_core::rules::production::capacity(&cx, Seat(0), system)
}

#[test]
fn the_home_production_reads_the_position_when_it_starts_not_where_the_card_started() {
    covers!("rules::hero_windows::NaazHero");
    // A system with two planets: a dock on the first, the second controlled; the Construction secondary puts a second dock there, so the
    // production of the home system grows from the first dock's to both.
    let system = System::ALL.iter().copied().find(|s| plain().contains(s) && s.data().planets.len() >= 2).expect("a plain system with two planets");
    let (first, second) = (system.data().planets[0], system.data().planets[1]);
    let sc = Scenario::new(&[Faction::TheNaazRokhaAlliance, Faction::TheArborec])
        .kernel(kernel_all())
        .map_from(&[system])
        .control(first, 0)
        .control(second, 0)
        .unit(system, Some(first), Unit::SpaceDockI, 0, 1)
        .goods(0, 9)
        .tokens(0, 3, 3, 2)
        .commodities(0, 0);
    let (mut state, kernel) = sc.build();
    state.decks_mut().relic.set(vec![Relic::ALL.iter().copied().find(|r| *r != Relic::ShardOfTheThrone).unwrap()]);
    let before = capacity_of(&state, &kernel, system);
    let facts = PerformFacts { home: Home::At(system), roster: Some(roster()), carried: Carried { payments: true, structures: Some(structure_facts()), ..Carried::default() }, ..perform_facts(Carried::default()) };
    let mut rig = Rig::from_parts(state, kernel, NaazHero::new(Seat(0), facts, vec![StrategyCard::Construction, StrategyCard::Warfare]));
    rig.run();
    rig.pick_opt(Opt::Leaders(LOpt::HeroCard(StrategyCard::Construction)));
    rig.pick_opt(Opt::Leaders(LOpt::HeroCard(StrategyCard::Warfare)));
    rig.pick_where(|o| matches!(o, Opt::Strategy(SOpt::Structure { pds: false, planet, .. }) if *planet == second));
    let after = capacity_of(&rig.state, &rig.kernel, system);
    assert!(after > before, "the second dock adds its PRODUCTION: {before} then {after}");
    let Ask::Production(ProdAsk::Choose(choose)) = rig.asked().ask else { panic!("{}", rig.transcript()) };
    assert_eq!(i64::from(choose.limit), after, "the limit of the home production is the capacity after the Construction secondary, not before it: {}", rig.transcript());
    while !rig.is_done() {
        answer_somehow(&mut rig);
    }
    assert!(!rig.driver.window.unsupported, "{}", rig.transcript());
}

#[test]
fn the_announcement_of_the_home_production_goes_after_the_events_staged_before_it() {
    covers!("rules::hero_windows::NaazHero");
    // An event a secondary before the Warfare secondary staged: the units made are announced after it, so the adapter needs to know where.
    let (mut rig, _) = naaz_home(&[StrategyCard::Warfare], 6, use_at);
    ti4_engine_core::kernel::fx::stage(&mut rig.state, ti4_engine_core::kernel::fx::trade_goods_gained(Seat(0), 1, Code(1)));
    rig.run();
    rig.pick_opt(Opt::Leaders(LOpt::HeroCard(StrategyCard::Warfare)));
    rig.pick_where(|o| matches!(o, Opt::Production(ti4_engine_core::choice::production::Opt::Build { unit: Unit::DestroyerI, .. })));
    while !rig.is_done() {
        if matches!(rig.asked().ask, Ask::Production(ti4_engine_core::choice::production::Ask::Pay(_))) {
            rig.pick_where(|o| matches!(o, Opt::Production(ti4_engine_core::choice::production::Opt::TradeGood { .. })));
        } else {
            rig.pick_where(|o| matches!(o, Opt::Production(ti4_engine_core::choice::production::Opt::Stop)));
        }
    }
    assert!(rig.driver.window.home_made.is_some());
    assert_eq!(rig.driver.window.home_staged, 1, "one event was staged when the home production finished");
}

#[test]
fn a_home_use_needs_the_roster_of_the_faction_units_and_the_rules_version_decides_the_agent() {
    covers!("rules::hero_windows::NaazHero");
    // No roster: the forms that may be built cannot be read, the string performs the ability.
    let (mut rig, _) = naaz_home(&[StrategyCard::Warfare], 6, use_at);
    rig.driver.window = NaazHero::new(Seat(0), PerformFacts { roster: None, ..with_home(Home::At(plain()[0])) }, vec![StrategyCard::Warfare]);
    rig.run();
    rig.pick_opt(Opt::Leaders(LOpt::HeroCard(StrategyCard::Warfare)));
    assert!(rig.is_done() && rig.driver.window.unsupported);
    // Below the rules version of Berekar Berekon the use leaves the discount an earlier tactical action left unspent in the position.
    for (version, left) in [(LATEST_RULES, 0), (20, 5)] {
        let (mut rig, _) = naaz_home(&[StrategyCard::Warfare], 6, use_at);
        rig.driver.window = NaazHero::new(Seat(0), PerformFacts { rules_version: version, ..with_home(Home::At(plain()[0])) }, vec![StrategyCard::Warfare]);
        rig.state.transient_mut().production.discount_remaining = 5;
        rig.run();
        rig.pick_opt(Opt::Leaders(LOpt::HeroCard(StrategyCard::Warfare)));
        assert_eq!(rig.state.transient().production.discount_remaining, left, "rules version {version}");
    }
}

/// Decline the question, or take its first option where it cannot be declined.
fn answer_somehow<W: Window>(rig: &mut Rig<W>) {
    if rig.try_answer(Pick::Opt(Opt::Decline)).is_err() {
        rig.pick(0);
    }
}

fn imperial_facts(options: &[PublicObjective]) -> ImperialFacts {
    let mut scoreable = [None; 16];
    for (slot, o) in scoreable.iter_mut().zip(options) {
        *slot = Some(*o);
    }
    ImperialFacts { scoreable, env: PayEnv { scope: Scope(u32::from(kernel_all().sources.0)), blanked: false } }
}

#[test]
fn the_imperial_primary_is_a_child_that_reports_the_objective_scored() {
    covers!("rules::hero_windows::WinnuHero");
    let free = PublicObjective::ALL.iter().copied().find(|o| ti4_engine_core::query::objectives::cost_of(*o).is_none()).unwrap();
    let facts = PerformFacts { imperial: Some(imperial_facts(&[free])), ..perform_facts(Carried { payments: true, ..Carried::default() }) };
    let sc = Scenario::new(&[Faction::TheWinnu, Faction::TheArborec]).kernel(kernel_all()).commodities(0, 0);
    let mut rig = Rig::new(sc, WinnuHero::new(Seat(0), facts, vec![StrategyCard::Imperial]));
    rig.run();
    rig.pick(0);
    assert!(matches!(rig.asked().ask, Ask::Strategy(_)), "{}", rig.transcript());
    rig.pick_where(|o| matches!(o, Opt::Strategy(ti4_engine_core::choice::strategy::Opt::Objective(o)) if *o == free));
    while !rig.is_done() {
        rig.pick(0);
    }
    assert!(!rig.driver.window.unsupported && rig.driver.window.scored == Some(free), "{}", rig.transcript());
    // Declined: nothing scored; a secret objective instead of the Mecatol point.
    let facts = PerformFacts { imperial: Some(imperial_facts(&[free])), ..perform_facts(Carried { payments: true, ..Carried::default() }) };
    let sc = Scenario::new(&[Faction::TheWinnu, Faction::TheArborec]).kernel(kernel_all()).commodities(0, 0);
    let mut rig = Rig::new(sc, WinnuHero::new(Seat(0), facts, vec![StrategyCard::Imperial]));
    rig.run();
    rig.pick(0);
    rig.decline();
    while !rig.is_done() {
        rig.pick(0);
    }
    assert!(!rig.driver.window.unsupported && rig.driver.window.scored.is_none());
}

#[test]
fn the_pool_is_asked_from_version_twelve_and_a_minor_field_gives_one_good() {
    covers!("rules::hero_windows::EmpyreanHero");
    let (mut rig, _) = empyrean(&[Explore::MinorEntropicField], 1, FrontierFacts { rules_version: 12, ..frontier_facts() });
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Strategy(SAsk::GainPool { .. })), "version 12 asks");
    rig.pick_opt(Opt::Strategy(SOpt::Pool(Pool::Tactic)));
    assert_eq!(goods_of(&rig), 1, "a minor entropic field: one command token and one trade good");
}

#[test]
fn an_empty_secret_deck_draws_nothing_and_the_card_still_resolves() {
    covers!("rules::hero_windows::EmpyreanHero");
    let (mut rig, _) = empyrean(&[Explore::DerelictVessel1], 1, frontier_facts());
    rig.state.decks_mut().secret.set(vec![]);
    rig.run();
    assert!(rig.is_done() && rig.driver.window.used && !rig.driver.window.unsupported, "{}", rig.transcript());
    assert_eq!(rig.driver.window.rows()[0].how, How::Resolved);
}

#[test]
fn a_token_under_a_ground_force_alone_is_not_explored() {
    covers!("rules::hero_windows::EmpyreanHero");
    let (mut state, kernel, systems) = empyrean_parts(&[Explore::UnknownRelicFragment1], 1, frontier_facts());
    state.board_mut().sys_mut(systems[0]).units.clear();
    state.board_mut().sys_mut(systems[0]).units.push(Piece::new(Unit::InfantryI, Seat(0)));
    let mut rig = Rig::from_parts(state, kernel, EmpyreanHero::new(Seat(0), Vec::new(), systems.clone(), frontier_facts()));
    rig.run();
    assert!(rig.is_done() && rig.driver.window.rows().is_empty() && rig.state.explore().frontier.contains(systems[0]));
}

// -- The Thunder's Edge primaries as children of Mathis Mathinus (package W13) ---------------------------------------

use ti4_engine_core::kernel::children::{FreeSystems, TeUses};

fn winnu_te(card: StrategyCard, facts: PerformFacts) -> (Rig<WinnuHero>, System, Planet) {
    let system = plain()[0];
    let planet = system.data().planets[0];
    let sc = Scenario::new(&[Faction::TheWinnu, Faction::TheArborec])
        .kernel(kernel_all())
        .map_from(&[system, plain()[1]])
        .control(planet, 0)
        .unit(system, Some(planet), Unit::SpaceDockI, 0, 1)
        .goods(0, 6)
        .tokens(0, 3, 3, 2);
    (Rig::new(sc, WinnuHero::new(Seat(0), facts, vec![card])), system, planet)
}

#[test]
fn the_te_warfare_primary_redistributes_then_names_the_system_of_the_free_tactical_action() {
    covers!("rules::hero_windows::WinnuHero");
    let systems = [plain()[0], plain()[1]];
    let free = FreeSystems::new(Code(9), &systems).unwrap();
    let facts = PerformFacts { free: Some(free), ..perform_facts(Carried::default()) };
    let (mut rig, _, _) = winnu_te(StrategyCard::WarfareThundersEdge, facts);
    rig.run();
    rig.pick(0);
    assert!(matches!(rig.asked().ask, Ask::Strategy(ti4_engine_core::choice::strategy::Ask::TeWarfareRedistribute { .. })), "{}", rig.transcript());
    rig.pick(0);
    assert!(matches!(rig.asked().ask, Ask::Strategy(ti4_engine_core::choice::strategy::Ask::TeWarfareTactical)), "{}", rig.transcript());
    rig.pick_opt(Opt::Strategy(SOpt::System(systems[1])));
    assert!(rig.is_done() && !rig.driver.window.unsupported, "{}", rig.transcript());
    assert_eq!(rig.driver.window.free, Some(systems[1]), "the game plays the free action in the system chosen");
}

#[test]
fn the_te_warfare_primary_without_systems_resolves_and_without_facts_is_the_strings() {
    covers!("rules::hero_windows::WinnuHero");
    let none = FreeSystems::new(Code(9), &[]).unwrap();
    let (mut rig, _, _) = winnu_te(StrategyCard::WarfareThundersEdge, PerformFacts { free: Some(none), ..perform_facts(Carried::default()) });
    rig.run();
    rig.pick(0);
    assert!(rig.is_done() && !rig.driver.window.unsupported && rig.driver.window.free.is_none(), "{}", rig.transcript());
    let (mut rig, _, _) = winnu_te(StrategyCard::WarfareThundersEdge, perform_facts(Carried::default()));
    rig.run();
    rig.pick(0);
    assert!(rig.is_done() && rig.driver.window.unsupported, "a table whose systems the adapter could not give");
}

#[test]
fn too_many_systems_for_the_facts_are_refused() {
    covers!("rules::hero_windows::WinnuHero");
    let many = vec![plain()[0]; ti4_engine_core::kernel::children::FREE_SYSTEMS + 1];
    assert!(FreeSystems::new(Code(9), &many).is_none());
    assert_eq!(FreeSystems::new(Code(9), &many[..3]).unwrap().systems().len(), 3);
}

fn te_construction_facts(system: System, structures: bool) -> PerformFacts {
    let mut buildable = [None; 9];
    buildable[0] = Some(Unit::DestroyerI);
    let uses = [HomeFacts {
        produce: ProduceFacts { who: Seat(0), system, limit: 3, max_unit_cost: None, small_exemptions: 0, discount: 0, fleet_limit: 8, armada: 0, flagship_free: false, buildable },
        agent_rules: true,
    }];
    let place = Some((Unit::PDSI, false));
    let dock = Some((Unit::SpaceDockI, false));
    let structures = structures.then_some(ti4_engine_core::kernel::children::StructureFacts { pds: place, dock, source: Code(5) });
    PerformFacts { te_uses: TeUses::new(Code(8), &uses), ..perform_facts(Carried { structures, ..Carried::default() }) }
}

#[test]
fn the_te_construction_primary_runs_the_use_of_production_as_a_child_then_the_second_structure() {
    covers!("rules::hero_windows::WinnuHero");
    let system = plain()[0];
    let (mut rig, system, _) = winnu_te(StrategyCard::ConstructionThundersEdge, te_construction_facts(system, true));
    rig.run();
    rig.pick(0);
    assert!(matches!(rig.asked().ask, Ask::Strategy(ti4_engine_core::choice::strategy::Ask::ConstructionAbility)), "{}", rig.transcript());
    rig.pick_opt(Opt::Strategy(SOpt::Produce(system)));
    assert!(matches!(rig.asked().ask, Ask::Production(_)), "{}", rig.transcript());
    rig.pick_where(|o| matches!(o, Opt::Production(ti4_engine_core::choice::production::Opt::Build { unit: Unit::DestroyerI, .. })));
    while matches!(rig.asked().ask, Ask::Production(_)) {
        if matches!(rig.asked().ask, Ask::Production(ti4_engine_core::choice::production::Ask::Pay(_))) {
            rig.pick_where(|o| matches!(o, Opt::Production(ti4_engine_core::choice::production::Opt::TradeGood { .. })));
        } else {
            rig.pick_where(|o| matches!(o, Opt::Production(ti4_engine_core::choice::production::Opt::Stop)));
        }
    }
    assert!(matches!(rig.asked().ask, Ask::Strategy(ti4_engine_core::choice::strategy::Ask::PlaceStructure { step: 2, .. })), "the second structure follows the use: {}", rig.transcript());
    rig.pick_opt(Opt::Decline);
    assert!(rig.is_done() && !rig.driver.window.unsupported, "{}", rig.transcript());
    let (made_in, made) = rig.driver.window.produced.clone().expect("the use is reported");
    assert_eq!(made_in, system);
    assert_eq!(made.as_slice().len(), 1);
    assert_eq!(made.as_slice()[0].0, Unit::DestroyerI);
}

#[test]
fn the_te_construction_primary_chooses_a_structure_without_a_report_and_without_facts_is_the_strings() {
    covers!("rules::hero_windows::WinnuHero");
    let system = plain()[0];
    let (mut rig, _, _) = winnu_te(StrategyCard::ConstructionThundersEdge, te_construction_facts(system, true));
    rig.run();
    rig.pick(0);
    rig.pick_opt(Opt::Strategy(SOpt::Build));
    rig.pick_opt(Opt::Decline);
    rig.pick_opt(Opt::Decline);
    assert!(rig.is_done() && rig.driver.window.produced.is_none() && !rig.driver.window.unsupported, "{}", rig.transcript());
    let (mut rig, _, _) = winnu_te(StrategyCard::ConstructionThundersEdge, PerformFacts { te_uses: None, ..te_construction_facts(system, true) });
    rig.run();
    rig.pick(0);
    assert!(rig.is_done() && rig.driver.window.unsupported);
}

#[test]
fn a_use_of_production_that_needs_the_factions_link_declines_the_run_of_the_te_construction() {
    covers!("rules::hero_windows::WinnuHero");
    let system = plain()[0];
    let (mut rig, system, _) = winnu_te(StrategyCard::ConstructionThundersEdge, te_construction_facts(system, true));
    rig.state.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::SelfAssemblyRoutines);
    rig.run();
    rig.pick(0);
    rig.pick_opt(Opt::Strategy(SOpt::Produce(system)));
    rig.pick_where(|o| matches!(o, Opt::Production(ti4_engine_core::choice::production::Opt::Build { unit: Unit::DestroyerI, .. })));
    while !rig.is_done() {
        if matches!(rig.asked().ask, Ask::Production(ti4_engine_core::choice::production::Ask::Pay(_))) {
            rig.pick_where(|o| matches!(o, Opt::Production(ti4_engine_core::choice::production::Opt::TradeGood { .. })));
        } else {
            rig.pick_where(|o| matches!(o, Opt::Production(ti4_engine_core::choice::production::Opt::Stop)));
        }
    }
    assert!(rig.driver.window.unsupported, "Self-Assembly Routines is owned and the table has no link to its window: the string plays the card");
}
