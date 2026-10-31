//! Package W10: the command tokens in a Mahact fleet pool are not in the reinforcements (`kernel::fx::tokens_in_reinforcements`, read from the
//! marks when the window's footprint carries them).

use ti4_engine_core::choice::strategy::Pool;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::kernel::fx;
use ti4_engine_core_tests::prelude::*;

fn pool(owners: &[u8]) -> MarkVal {
    MarkVal::List(owners.iter().map(|s| MarkVal::Atom(Atom::Seat(Seat(*s)))).collect())
}

#[test]
fn a_token_in_a_mahact_fleet_pool_is_not_in_its_owners_reinforcements() {
    covers!("kernel::fx::tokens_held_by_mahact");
    let (mut s, _) = Scenario::lobby(3).build();
    let before = fx::tokens_in_reinforcements(&s, Seat(0));
    assert_eq!(fx::tokens_held_by_mahact(&s, Seat(0)), 0);
    // Seat 1 is the Mahact: its fleet pool holds two tokens of seat 0 and one of seat 2.
    s.marks_mut().tail.edit().insert(MarkKey::MahactFleet(Seat(1)), pool(&[0, 2, 0]));
    assert_eq!(fx::tokens_held_by_mahact(&s, Seat(0)), 2);
    assert_eq!(fx::tokens_held_by_mahact(&s, Seat(2)), 1);
    assert_eq!(fx::tokens_held_by_mahact(&s, Seat(1)), 0);
    assert_eq!(fx::tokens_in_reinforcements(&s, Seat(0)), before - 2);
    // Another Mahact's pool adds to it.
    s.marks_mut().tail.edit().insert(MarkKey::MahactFleet(Seat(2)), pool(&[0]));
    assert_eq!(fx::tokens_held_by_mahact(&s, Seat(0)), 3);
    assert_eq!(fx::tokens_in_reinforcements(&s, Seat(0)), before - 3);
    // A token gained is capped by what is left.
    let sheet = |s: &CoreState| {
        let p = s.players().by_seat(Seat(0)).unwrap();
        i32::from(p.econ.tactic_tokens) + i32::from(p.econ.fleet_tokens) + i32::from(p.econ.strategic_tokens)
    };
    let left = fx::tokens_in_reinforcements(&s, Seat(0));
    let got = fx::gain_token(&mut s, Seat(0), Pool::Tactic, left + 5);
    assert_eq!(got, left, "no more than the reinforcements hold");
    assert!(sheet(&s) >= got);
}

#[test]
fn a_window_that_does_not_carry_the_marks_counts_none_held() {
    covers!("kernel::fx::tokens_held_by_mahact");
    use ti4_engine_core::state::{Slice, SliceSet};
    let mut s = CoreState::with_loaded(SliceSet::of(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board]));
    let _ = &mut s;
    assert_eq!(fx::tokens_held_by_mahact(&s, Seat(0)), 0, "the marks are not part of the lend: nothing is read");
}
