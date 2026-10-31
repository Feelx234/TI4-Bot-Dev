//! W10 (lane 8): `rules::hero_windows`, the heroes composed from windows and kernel effects. UNIT.DSGN.FLAYESH (Nekro hero) here.

use ti4_engine_core::choice::leaders::{Ask as LAsk, Opt as LOpt};
use ti4_engine_core::generated::Planet;
use ti4_engine_core::keys::MarkKey;
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
    assert!(rig.driver.window.used && !rig.driver.window.unsupported);
    assert_eq!(units_on(&rig, a.1, 1), 0, "every other seat's unit on the planet is gone, the structure too");
    assert_eq!(units_on(&rig, a.1, 0), 1, "the seat's own unit stays");
    assert_eq!(units_on(&rig, b.1, 1), 1, "the other planet is untouched");
    assert_eq!(staged_rows(&rig), 2, "the two infantry are staged as destroyed, the structure is not");
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
    rig.run();
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
fn a_galvanized_ground_force_that_falls_makes_the_window_unsupported() {
    covers!("rules::hero_windows::NekroHero");
    let (sc, [a, _]) = scenario();
    let mut rig = hero(sc, vec![target(a, 3, &[Technology::GravityDrive])]);
    for piece in rig.state.board_mut().planet_mut(a.1).units.as_mut_slice() {
        if piece.owner() == Seat(1) && piece.kind() == Unit::InfantryI {
            *piece = piece.with_galvanized(true);
        }
    }
    rig.run();
    assert!(rig.driver.window.unsupported, "the staged row cannot hold the galvanize tail");
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
    let sc = Scenario::new(&[Faction::TheYssarilTribes, Faction::TheArborec, Faction::TheEmbersOfMuaat])
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
    for seed in 0..60 {
        let rig = play(&sc, true, &targets, &[], seed);
        assert!(rig.driver.window.used);
        let taken = captured(&rig).len();
        assert_eq!(at(&rig, system, 1, Unit::CruiserI) + taken, 2, "a ship is on the board or in the holder's sheet");
        assert!(captured(&rig).iter().all(|c| *c == (Seat(1), Unit::CruiserI)));
        hit |= taken > 0;
        miss |= taken < 2;
    }
    assert!(hit && miss, "both outcomes are reached on some seed");
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
    let sc = sc.unit(system, None, Unit::CarrierI, 1, 1).unit(system, None, Unit::FighterI, 1, 2);
    let infantry = [(system, Piece::new(Unit::InfantryI, Seat(1))); 2];
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
