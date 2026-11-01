//! Lane E2: the two secret objectives the core did not answer (`ScoringSecret`, the first gap of 23 games of the native sweep): Strengthen Bonds
//! ("Have another player's promissory note in your play area") and Produce en Masse ("Have at least 8 PRODUCTION in one system" -- the
//! printed threshold of the table). The lockstep against the string is `ti4-engine`'s `objectives_core` dual (the typed view answers them now).

use ti4_engine_core::generated::{Faction, PromissoryNote, SecretObjective as S, System, Unit};
use ti4_engine_core::query::objectives::{self as q, Family, Imagined, Pos, SeatFacts, Terms};
use ti4_engine_core::seen::Seen;
use ti4_engine_core::state::NoteKey;
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn table() -> Scenario {
    Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat, Faction::TheYssarilTribes])
}

fn issuers(s: &CoreState, seat: u8) -> usize {
    q::rival_note_issuers(s, Seat(seat))
}

fn put(s: &mut CoreState, note: PromissoryNote, issuer: Faction, holder: u8, faceup: bool) {
    let key = NoteKey { note, faction: issuer };
    s.cards_mut().promissory.edit().insert(key, Seat(holder));
    if faceup {
        s.cards_mut().promissory_faceup.edit().insert(key, ());
    }
}

#[test]
fn strengthen_bonds_counts_the_other_issuers_with_a_note_in_the_play_area_once_each() {
    let mut s = table().state();
    assert_eq!(issuers(&s, 0), 0);
    // A note in hand is not in the play area.
    put(&mut s, PromissoryNote::Ceasefire, Faction::TheEmbersOfMuaat, 0, false);
    assert_eq!(issuers(&s, 0), 0, "a note in hand is not a bond");
    // The same note placed faceup is; two notes of one issuer are one bond.
    put(&mut s, PromissoryNote::Ceasefire, Faction::TheEmbersOfMuaat, 0, true);
    assert_eq!(issuers(&s, 0), 1);
    put(&mut s, PromissoryNote::TradeAgreement, Faction::TheEmbersOfMuaat, 0, true);
    assert_eq!(issuers(&s, 0), 1, "one issuer, one bond");
    // Your own faction's note is not another's.
    put(&mut s, PromissoryNote::Ceasefire, Faction::TheArborec, 0, true);
    assert_eq!(issuers(&s, 0), 1, "the seat's own note is not a bond");
    // Another seat's holdings are its own.
    assert_eq!(issuers(&s, 1), 0);
    assert_eq!(issuers(&s, 2), 0);
    // The Support for the Throne lent by seat 2 is a bond of the holder, never of its owner; one lent to the seat itself does not count.
    s.trade_mut().support_holders.insert(Seat(2), Seat(0));
    assert_eq!(issuers(&s, 0), 2, "the Support of another seat counts for that seat");
    s.trade_mut().support_holders.insert(Seat(0), Seat(0));
    assert_eq!(issuers(&s, 0), 2, "the seat's own Support in its own area is not a bond");
    // The Support and a faceup note of the SAME issuer are one bond.
    put(&mut s, PromissoryNote::TradeAgreement, Faction::TheYssarilTribes, 0, true);
    assert_eq!(issuers(&s, 0), 2);
    // A note of a faction nobody plays is a bond of that faction.
    put(&mut s, PromissoryNote::Ceasefire, Faction::TheNaazRokhaAlliance, 0, true);
    assert_eq!(issuers(&s, 0), 3);
}

#[test]
fn strengthen_bonds_is_satisfied_by_one_bond_and_the_scoring_view_agrees() {
    let mut s = table().state();
    let f = SeatFacts::of(&s, ALL_SOURCES, Seat(0), Terms::default());
    let p = q::secret_progress(&Pos::new(&s, &f, true, Imagined::NONE), S::StrengthenBonds).unwrap().unwrap();
    assert_eq!((p.family, p.have, p.threshold, p.satisfied()), (Family::RivalNoteIssuers, 0, 1, false));
    put(&mut s, PromissoryNote::Ceasefire, Faction::TheEmbersOfMuaat, 0, true);
    let f = SeatFacts::of(&s, ALL_SOURCES, Seat(0), Terms::default());
    let p = q::secret_progress(&Pos::new(&s, &f, true, Imagined::NONE), S::StrengthenBonds).unwrap().unwrap();
    assert!(p.satisfied() && p.have == 1);
    // The seat's hand: `scoreable_secrets` offers it in the status window.
    let (mut s, k) = table().build();
    s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.secrets.push(S::StrengthenBonds);
    let seen = Seen::new(&s, &k, Seat(0));
    let status = ti4_engine_core::generated::ObjectivePhase::Status;
    assert_eq!(seen.scoreable_secrets(Seat(0), status, None).expect("answered").as_slice(), &[] as &[S]);
    put(&mut s, PromissoryNote::Ceasefire, Faction::TheEmbersOfMuaat, 0, true);
    let seen = Seen::new(&s, &k, Seat(0));
    assert_eq!(seen.scoreable_secrets(Seat(0), status, None).expect("answered").as_slice(), &[S::StrengthenBonds]);
}

#[test]
fn produce_en_masse_reads_the_most_production_in_one_system_once_the_caller_filled_it() {
    let s = table().state();
    let mut terms = Terms::default();
    let f = SeatFacts::of(&s, ALL_SOURCES, Seat(0), terms.clone());
    assert_eq!(q::secret_progress(&Pos::new(&s, &f, true, Imagined::NONE), S::ProduceEnMasse), Err(q::Unsupported));
    terms.capacity_known = true;
    let f = SeatFacts::of(&s, ALL_SOURCES, Seat(0), terms.clone());
    let p = q::secret_progress(&Pos::new(&s, &f, true, Imagined::NONE), S::ProduceEnMasse).unwrap().unwrap();
    assert_eq!((p.family, p.have, p.threshold), (Family::ProductionInOneSystem, 0, 8), "nothing filled: no production anywhere");
    terms.capacity.push((System::JordSol, 5));
    terms.capacity.push((plain_systems()[0], 8));
    terms.capacity.push((plain_systems()[1], 3));
    let f = SeatFacts::of(&s, ALL_SOURCES, Seat(0), terms);
    let p = q::secret_progress(&Pos::new(&s, &f, true, Imagined::NONE), S::ProduceEnMasse).unwrap().unwrap();
    assert_eq!((p.have, p.satisfied()), (8, true), "the largest system, not the sum");
}

#[test]
fn the_scoring_view_fills_the_production_of_a_seat_that_holds_or_sees_produce_en_masse() {
    // A space dock and other producers: the seat's PRODUCTION in the system is what `rules::production::capacity` says.
    let sys = plain_systems()[0];
    let planet = sys.data().planets[0];
    let base = table().map_ring(7).control(planet, 0).unit(sys, Some(planet), Unit::SpaceDockI, 0, 1);
    let (mut s, k) = base.build();
    let seen = Seen::new(&s, &k, Seat(0));
    // Without the secret the scan is not paid for (and the progress is the hand's: no secret, no row).
    assert!(seen.my_secret_progress(Imagined::NONE).expect("answered").is_empty());
    s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.secrets.push(S::ProduceEnMasse);
    let seen = Seen::new(&s, &k, Seat(0));
    let rows = seen.my_secret_progress(Imagined::NONE).expect("answered, not unsupported");
    assert_eq!(rows.len(), 1);
    let cx = ti4_engine_core::rules::production::Cx::new(&s, &k, ti4_engine_core::query::reach::scope_of(&k), false);
    let want = ti4_engine_core::rules::production::capacity(&cx, Seat(0), sys);
    assert!(want > 0, "a space dock produces");
    assert_eq!((rows[0].have, rows[0].threshold), (want, 8));
    // Another seat's hand is its own: seat 1 sees nothing and holds nothing.
    assert!(q::wants_capacity(&s, Seat(0)) && !q::wants_capacity(&s, Seat(1)));
    // A revealed copy (leaked) is scored by whoever meets it: every seat then pays for the scan.
    let mut leaked = s.clone();
    leaked.players_mut().by_seat_mut(Seat(0)).unwrap().cards.secrets.clear();
    leaked.objectives_mut().revealed.push(ti4_engine_core::state::ObjectiveRef::Secret(S::ProduceEnMasse));
    assert!(q::wants_capacity(&leaked, Seat(0)) && q::wants_capacity(&leaked, Seat(1)));
}

#[test]
fn a_production_of_one_counts_in_the_scoring_view() {
    // The Arborec's Letani Warrior I has PRODUCTION 1: the smallest capacity that is a producer at all.
    let sys = plain_systems()[0];
    let planet = sys.data().planets[0];
    let (mut s, k) = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_ring(7).control(planet, 0).unit(sys, Some(planet), Unit::LetaniWarriorI, 0, 1).build();
    s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.secrets.push(S::ProduceEnMasse);
    let rows = Seen::new(&s, &k, Seat(0)).my_secret_progress(Imagined::NONE).expect("answered");
    assert_eq!((rows[0].have, rows[0].threshold), (1, 8), "one PRODUCTION is one");
}
