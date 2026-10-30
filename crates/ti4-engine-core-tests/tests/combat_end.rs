//! D4-2d: how a space combat ends (`rules::combat::end`: winner, reason, ships lost) and which feats it earns (`rules::combat::feats`, the secret
//! objectives that ask about a fight). Pure functions of a position: scenarios are built by hand from the rules text of the objectives
//! ("Destroy Their Greatest Ship", "Demonstrate Your Power", ...).

use ti4_engine_core::containers::SmallList;
use ti4_engine_core::rules::combat::end::{end_report, finished, winner, EndReason, EndReport};
use ti4_engine_core::rules::combat::feats::{feats, record, Outcome};
use ti4_engine_core::rules::combat::Rd;
use ti4_engine_core::state::{Before, Feat};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn sys() -> System {
    plain_systems()[0]
}

fn arena() -> Scenario {
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys()])
}

fn rd<'a>(s: &'a CoreState, k: &'a Kernel) -> Rd<'a> {
    Rd::new(s, k)
}

// ---- finished / winner -------------------------------------------------------------------------

#[test]
fn a_combat_is_finished_when_a_side_has_no_ship_and_the_other_side_wins() {
    covers!("rules::combat::end");
    let both = arena().unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), None, Unit::CruiserI, 1, 1).build();
    assert!(!finished(rd(&both.0, &both.1), sys(), Seat(0), Seat(1)));
    assert_eq!(winner(rd(&both.0, &both.1), sys(), Seat(0), Seat(1)), None, "both sides still stand: no winner");
    let only_a = arena().unit(sys(), None, Unit::CruiserI, 0, 1).build();
    assert!(finished(rd(&only_a.0, &only_a.1), sys(), Seat(0), Seat(1)));
    assert_eq!(winner(rd(&only_a.0, &only_a.1), sys(), Seat(0), Seat(1)), Some(Seat(0)));
    let only_d = arena().unit(sys(), None, Unit::CruiserI, 1, 1).build();
    assert_eq!(winner(rd(&only_d.0, &only_d.1), sys(), Seat(0), Seat(1)), Some(Seat(1)));
    let nobody = arena().build();
    assert!(finished(rd(&nobody.0, &nobody.1), sys(), Seat(0), Seat(1)));
    assert_eq!(winner(rd(&nobody.0, &nobody.1), sys(), Seat(0), Seat(1)), None, "both sides wiped out: a draw");
}

#[test]
fn ground_forces_in_space_are_not_ships_and_do_not_keep_a_side_in_the_fight() {
    covers!("rules::combat::end");
    let (mut state, kernel) = arena().unit(sys(), None, Unit::CruiserI, 0, 1).build();
    state.board_mut().sys_mut(sys()).units.push(ti4_engine_core::state::Piece::new(Unit::InfantryI, Seat(1)));
    assert!(finished(rd(&state, &kernel), sys(), Seat(0), Seat(1)));
    assert_eq!(winner(rd(&state, &kernel), sys(), Seat(0), Seat(1)), Some(Seat(0)));
}

// ---- end_report --------------------------------------------------------------------------------

fn start(a: &[Unit], d: &[Unit]) -> Vec<(Seat, Vec<Unit>)> {
    vec![(Seat(0), a.to_vec()), (Seat(1), d.to_vec())]
}

#[test]
fn a_destroyed_fleet_is_listed_side_by_side_attacker_first_in_the_order_of_the_opening_fleet() {
    covers!("rules::combat::end");
    let (state, kernel) = arena().unit(sys(), None, Unit::CruiserI, 0, 1).build();
    let at_start = start(&[Unit::CruiserI, Unit::DestroyerI], &[Unit::DreadnoughtI, Unit::FighterI, Unit::CruiserI]);
    let report = end_report(rd(&state, &kernel), sys(), [Seat(0), Seat(1)], Some(Seat(0)), false, &[], &at_start);
    assert_eq!(
        report,
        EndReport {
            reason: EndReason::Destruction,
            losers: [Seat(1)].into_iter().collect(),
            retreated: SmallList::new(),
            destroyed: vec![(Seat(0), Unit::DestroyerI), (Seat(1), Unit::DreadnoughtI), (Seat(1), Unit::FighterI), (Seat(1), Unit::CruiserI)],
        }
    );
}

#[test]
fn a_retreat_names_who_left_and_the_ships_that_arrived_are_not_counted_as_lost() {
    covers!("rules::combat::end");
    let (state, kernel) = arena().unit(sys(), None, Unit::CruiserI, 0, 1).build();
    let at_start = start(&[Unit::CruiserI], &[Unit::DreadnoughtI, Unit::CruiserI, Unit::FighterI]);
    let fled = vec![(Seat(1), vec![Unit::DreadnoughtI, Unit::CruiserI])];
    let report = end_report(rd(&state, &kernel), sys(), [Seat(0), Seat(1)], Some(Seat(0)), false, &fled, &at_start);
    assert_eq!(report.reason, EndReason::Retreat);
    assert_eq!(report.retreated.as_slice(), &[Seat(1)]);
    assert_eq!(report.destroyed, vec![(Seat(1), Unit::FighterI)], "the fighter did not make it out");
    assert_eq!(report.losers.as_slice(), &[Seat(1)]);
}

#[test]
fn a_declared_draw_or_the_round_limit_are_their_own_reasons_and_have_no_losers() {
    covers!("rules::combat::end");
    let (state, kernel) = arena().unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), None, Unit::CruiserI, 1, 1).build();
    let at_start = start(&[Unit::CruiserI], &[Unit::CruiserI]);
    let draw = end_report(rd(&state, &kernel), sys(), [Seat(0), Seat(1)], None, true, &[], &at_start);
    assert_eq!(draw.reason, EndReason::Draw);
    assert!(draw.losers.is_empty() && draw.retreated.is_empty() && draw.destroyed.is_empty());
    let limit = end_report(rd(&state, &kernel), sys(), [Seat(0), Seat(1)], None, false, &[], &at_start);
    assert_eq!(limit.reason, EndReason::RoundLimit, "both still stand and nobody declared a draw");
    let wiped = arena().build();
    let both_dead = end_report(rd(&wiped.0, &wiped.1), sys(), [Seat(0), Seat(1)], None, false, &[], &at_start);
    assert_eq!(both_dead.reason, EndReason::Destruction);
    assert_eq!(both_dead.destroyed, vec![(Seat(0), Unit::CruiserI), (Seat(1), Unit::CruiserI)]);
}

// ---- feats -------------------------------------------------------------------------------------

fn before(sides: &[u8], capitals: &[(u8, u16)]) -> Before {
    Before { sides: sides.iter().map(|s| Seat(*s)).collect(), capitals: capitals.iter().map(|(s, n)| (Seat(*s), *n)).collect(), notes: SmallList::new() }
}

fn won(winner: u8) -> Outcome {
    Outcome { winner: Some(Seat(winner)), rounds: 2 }
}

#[test]
fn a_fight_of_no_rounds_earns_nothing() {
    covers!("rules::combat::feats");
    let (state, kernel) = arena().unit(sys(), None, Unit::WarSun, 0, 1).build();
    let b = before(&[0, 1], &[(1, 1)]);
    assert_eq!(feats(rd(&state, &kernel), sys(), &b, Outcome { winner: Some(Seat(0)), rounds: 0 }), vec![]);
}

#[test]
fn losing_a_war_sun_or_flagship_is_the_other_sides_feat() {
    covers!("rules::combat::feats");
    let (state, kernel) = arena().unit(sys(), None, Unit::CruiserI, 0, 1).build();
    let b = before(&[0, 1], &[(1, 1)]);
    let got = feats(rd(&state, &kernel), sys(), &b, won(0));
    assert!(got.contains(&(Seat(0), Feat::DestroyedACapitalShip)), "{got:?}");
    // The war sun survived: nothing.
    let (alive, kernel) = arena().unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), None, Unit::WarSun, 1, 1).build();
    let got = feats(rd(&alive, &kernel), sys(), &b, Outcome { winner: None, rounds: 3 });
    assert!(!got.iter().any(|(_, f)| *f == Feat::DestroyedACapitalShip), "{got:?}");
    // Both lost one: both score.
    let (none, kernel) = arena().build();
    let both = before(&[0, 1], &[(0, 1), (1, 2)]);
    let got = feats(rd(&none, &kernel), sys(), &both, Outcome { winner: None, rounds: 1 });
    assert_eq!(got, vec![(Seat(1), Feat::DestroyedACapitalShip), (Seat(0), Feat::DestroyedACapitalShip)], "in side order");
}

#[test]
fn three_non_fighter_ships_left_after_the_fight_hold_the_feat_and_fighters_do_not_count() {
    covers!("rules::combat::feats");
    let (state, kernel) = arena().unit(sys(), None, Unit::CruiserI, 0, 2).unit(sys(), None, Unit::DestroyerI, 0, 1).unit(sys(), None, Unit::FighterI, 1, 6).unit(sys(), None, Unit::CruiserI, 1, 2).build();
    let got = feats(rd(&state, &kernel), sys(), &before(&[0, 1], &[]), Outcome { winner: None, rounds: 1 });
    assert_eq!(got, vec![(Seat(0), Feat::HeldThreeShipsAfterASpaceCombat)], "seat 1 has two non-fighters and six fighters");
}

#[test]
fn the_winner_scores_for_an_anomaly_a_rival_home_system_a_surviving_flagship_and_the_leader() {
    covers!("rules::combat::feats");
    let anomaly = System::ALL.iter().copied().find(|s| s.data().anomalies.asteroid_field).expect("an asteroid field tile");
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[anomaly]).unit(anomaly, None, Unit::CruiserI, 0, 1);
    let (mut state, kernel) = sc.build();
    let got = feats(rd(&state, &kernel), anomaly, &before(&[0, 1], &[]), won(0));
    assert!(got.contains(&(Seat(0), Feat::WonInAnAnomaly)));
    assert!(!got.iter().any(|(_, f)| matches!(f, Feat::WonInARivalHome | Feat::WonBesideASurvivingFlagship)));
    // A plain system is no anomaly.
    let (plain, k2) = arena().unit(sys(), None, Unit::CruiserI, 0, 1).build();
    assert!(!feats(rd(&plain, &k2), sys(), &before(&[0, 1], &[]), won(0)).iter().any(|(_, f)| *f == Feat::WonInAnAnomaly));
    // Rival home: the loser's home system.
    state.players_mut().by_seat_mut(Seat(1)).unwrap().home_system = Some(anomaly);
    let got = feats(rd(&state, &kernel), anomaly, &before(&[0, 1], &[]), won(0));
    assert!(got.contains(&(Seat(0), Feat::WonInARivalHome)));
    // The winner's own home is no rival home.
    state.players_mut().by_seat_mut(Seat(1)).unwrap().home_system = None;
    state.players_mut().by_seat_mut(Seat(0)).unwrap().home_system = Some(anomaly);
    assert!(!feats(rd(&state, &kernel), anomaly, &before(&[0, 1], &[]), won(0)).iter().any(|(_, f)| *f == Feat::WonInARivalHome));
}

#[test]
fn a_surviving_flagship_and_the_points_leader_and_a_note_holder_are_the_winners_feats() {
    covers!("rules::combat::feats");
    let flagship = Unit::TheInferno;
    let (mut state, kernel) = arena().unit(sys(), None, flagship, 0, 1).build();
    state.players_mut().by_seat_mut(Seat(1)).unwrap().econ.victory_points = 4;
    state.players_mut().by_seat_mut(Seat(0)).unwrap().econ.victory_points = 2;
    let mut b = before(&[0, 1], &[(0, 1)]);
    let got = feats(rd(&state, &kernel), sys(), &b, won(0));
    assert!(got.contains(&(Seat(0), Feat::WonBesideASurvivingFlagship)), "{got:?}");
    assert!(got.contains(&(Seat(0), Feat::WonAgainstThePointsLeader)), "the loser has the most points: {got:?}");
    assert!(!got.contains(&(Seat(0), Feat::WonAgainstANoteHolder)));
    b.notes.push((Seat(0), Seat(1)));
    assert!(feats(rd(&state, &kernel), sys(), &b, won(0)).contains(&(Seat(0), Feat::WonAgainstANoteHolder)), "the note (winner, loser) was held");
    let mut reversed = before(&[0, 1], &[]);
    reversed.notes.push((Seat(1), Seat(0)));
    assert!(!feats(rd(&state, &kernel), sys(), &reversed, won(0)).contains(&(Seat(0), Feat::WonAgainstANoteHolder)), "the pair is ordered");
    // A tie for the most points still counts; fewer than the leader does not.
    state.players_mut().by_seat_mut(Seat(1)).unwrap().econ.victory_points = 2;
    assert!(feats(rd(&state, &kernel), sys(), &b, won(0)).contains(&(Seat(0), Feat::WonAgainstThePointsLeader)), "the most victory points includes a tie");
    state.players_mut().by_seat_mut(Seat(0)).unwrap().econ.victory_points = 6;
    assert!(!feats(rd(&state, &kernel), sys(), &b, won(0)).contains(&(Seat(0), Feat::WonAgainstThePointsLeader)));
    // Without a winner the win-based feats are not earned.
    let none = feats(rd(&state, &kernel), sys(), &b, Outcome { winner: None, rounds: 2 });
    assert!(!none.iter().any(|(_, f)| matches!(f, Feat::WonBesideASurvivingFlagship | Feat::WonAgainstThePointsLeader | Feat::WonAgainstANoteHolder)));
}

#[test]
fn recording_feats_is_idempotent_per_occurrence_and_sorted() {
    covers!("rules::combat::feats");
    let mut state = arena().state();
    record(&mut state, &[(Seat(0), Feat::WonInAnAnomaly), (Seat(0), Feat::DestroyedACapitalShip), (Seat(1), Feat::HeldThreeShipsAfterASpaceCombat)], 5);
    record(&mut state, &[(Seat(0), Feat::WonInAnAnomaly)], 5);
    record(&mut state, &[(Seat(0), Feat::WonInAnAnomaly)], 6);
    let mine = state.players().by_seat(Seat(0)).unwrap().misc.event_feats.as_slice().to_vec();
    assert_eq!(mine, vec![(Feat::DestroyedACapitalShip, 5), (Feat::WonInAnAnomaly, 5), (Feat::WonInAnAnomaly, 6)]);
    assert_eq!(state.players().by_seat(Seat(1)).unwrap().misc.event_feats.as_slice(), &[(Feat::HeldThreeShipsAfterASpaceCombat, 5)]);
    record(&mut state, &[(Seat(7), Feat::WonInAnAnomaly)], 5);
}
