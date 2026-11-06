//! D4-1 low layer: `state/*` (Rng streams, Seats, Players, slices, Meta, the piece and the copy-on-write checkpoint of a whole state).

use proptest::prelude::*;
use ti4_engine_core::generated::Unit;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::state::{CoreState, Meta, MetaFlags, Phase, Piece, Players, Rng, RngDomain, Seats, Slice, SliceSet};
use ti4_engine_core_tests::prelude::*;

// ---- Rng --------------------------------------------------------------------------------------

#[test]
fn a_die_is_always_within_its_sides() {
    let mut r = Rng::new(1);
    for sides in [1u32, 2, 6, 10, 100] {
        for _ in 0..200 {
            let d = r.die(RngDomain::Dice, sides);
            assert!((1..=sides).contains(&d), "d{sides} gave {d}");
        }
    }
    assert_eq!(Rng::new(1).die(RngDomain::Dice, 1), 1);
}

#[test]
fn the_same_seed_gives_the_same_stream_and_another_seed_a_different_one() {
    let faces = |seed: u64| {
        let mut r = Rng::new(seed);
        (0..40).map(|_| r.die(RngDomain::Dice, 10)).collect::<Vec<_>>()
    };
    assert_eq!(faces(5), faces(5));
    assert_ne!(faces(5), faces(6));
    assert_eq!(Rng::new(5).seed(), 5);
}

#[test]
fn streams_of_different_domains_do_not_disturb_each_other() {
    let mut a = Rng::new(9);
    let mut b = Rng::new(9);
    for _ in 0..10 {
        a.die(RngDomain::Agendas, 10);
    }
    let from_a: Vec<u32> = (0..10).map(|_| a.die(RngDomain::Dice, 10)).collect();
    let from_b: Vec<u32> = (0..10).map(|_| b.die(RngDomain::Dice, 10)).collect();
    assert_eq!(from_a, from_b, "drawing agendas first leaves the dice stream where it was");
}

#[test]
fn positions_are_listed_in_domain_order_and_restore_exactly() {
    let mut r = Rng::new(4);
    r.die(RngDomain::Relics, 6);
    r.die(RngDomain::Dice, 6);
    r.die(RngDomain::Neuraloop, 6);
    let pos: Vec<_> = r.positions().collect();
    assert_eq!(pos.iter().map(|(d, _)| *d).collect::<Vec<_>>(), vec![RngDomain::Dice, RngDomain::Relics, RngDomain::Neuraloop], "sorted by domain");
    let next = r.die(RngDomain::Dice, 6);
    let mut other = Rng::new(4);
    other.die(RngDomain::Agendas, 6);
    other.set_positions(&pos);
    assert_eq!(other.positions().map(|(d, _)| d).collect::<Vec<_>>(), pos.iter().map(|(d, _)| *d).collect::<Vec<_>>(), "streams not listed are dropped");
    assert_eq!(other.die(RngDomain::Dice, 6), next);
}

proptest! {
    #![proptest_config(ti4_engine_core_tests::generators::config())]
    #[test]
    fn a_shuffle_is_a_permutation_and_repeatable(seed in any::<u64>(), len in 0usize..60) {
        let mut a: Vec<usize> = (0..len).collect();
        let mut b = a.clone();
        Rng::new(seed).shuffle(RngDomain::Agendas, &mut a);
        Rng::new(seed).shuffle(RngDomain::Agendas, &mut b);
        prop_assert_eq!(&a, &b);
        let mut sorted = a.clone();
        sorted.sort_unstable();
        prop_assert_eq!(sorted, (0..len).collect::<Vec<_>>());
    }

    #[test]
    fn a_clone_draws_what_its_original_would(seed in any::<u64>(), warm in 0usize..20, more in 1usize..20) {
        let mut a = Rng::new(seed);
        for _ in 0..warm { a.die(RngDomain::Dice, 10); }
        let mut b = a.clone();
        let x: Vec<u32> = (0..more).map(|_| a.die(RngDomain::Dice, 10)).collect();
        let y: Vec<u32> = (0..more).map(|_| b.die(RngDomain::Dice, 10)).collect();
        prop_assert_eq!(x, y);
        prop_assert!(a == b);
    }
}

// ---- Seats, Players ---------------------------------------------------------------------------

#[test]
fn seats_map_both_ways_and_report_the_table_order() {
    let s = Seats::new(&[Seat(3), Seat(0), Seat(2)], &[Seat(2), Seat(3), Seat(0)], 5);
    assert_eq!((s.n_players(), s.n_owners()), (3, 5));
    assert_eq!(s.players(), &[Seat(3), Seat(0), Seat(2)]);
    assert_eq!(s.seat_of(0), Seat(3));
    assert_eq!(s.player_of(Seat(3)), Some(0));
    assert_eq!(s.player_of(Seat(1)), None);
    assert_eq!(s.seating_order(), &[Seat(2), Seat(3), Seat(0)]);
    assert_eq!(s.player_set().iter().collect::<Vec<_>>(), vec![Seat(0), Seat(2), Seat(3)]);
    assert_eq!(s.owners().collect::<Vec<_>>(), (0..5).map(Seat).collect::<Vec<_>>());
    assert_eq!(Seats::default().n_players(), 0);
}

#[test]
fn players_are_found_by_seat_and_a_write_copies_one_player() {
    let sc = Scenario::lobby(4).state();
    let mut b = sc.clone();
    assert_eq!(b.players().shared_with(sc.players()), 4);
    b.players_mut().by_seat_mut(Seat(2)).unwrap().econ.trade_goods = 5;
    assert_eq!(b.players().shared_with(sc.players()), 3);
    assert_eq!(sc.players().by_seat(Seat(2)).unwrap().econ.trade_goods, 0);
    assert!(sc.players().by_seat(Seat(7)).is_none());
    assert_eq!(sc.players().len(), 4);
    assert!(!sc.players().is_empty() && Players::default().is_empty());
    assert!(b.players_mut().by_seat_mut(Seat(7)).is_none());
    assert_eq!(sc.players().iter().map(|p| p.faction).collect::<Vec<_>>(), LOBBY[..4].to_vec());
}

// ---- Slices, Meta -----------------------------------------------------------------------------

#[test]
fn slice_sets_are_bitsets_over_the_seventeen_slices() {
    assert_eq!(Slice::ALL.len(), Slice::COUNT);
    for (i, s) in Slice::ALL.iter().enumerate() {
        assert_eq!(*s as usize, i, "ALL is in discriminant order");
    }
    let mut set = SliceSet::EMPTY;
    assert!(!set.has(Slice::Board));
    set.insert(Slice::Board);
    set.insert(Slice::Ledger);
    assert!(set.has(Slice::Board) && set.has(Slice::Ledger) && !set.has(Slice::Map));
    assert_eq!(SliceSet::of(&[Slice::Board, Slice::Ledger]), set);
    assert_eq!(SliceSet::of(&[Slice::Board]).union(SliceSet::of(&[Slice::Ledger])), set);
    assert!(SliceSet::ALL.contains_all(set) && !set.contains_all(SliceSet::ALL) && set.contains_all(SliceSet::EMPTY));
    for s in Slice::ALL {
        assert!(SliceSet::ALL.has(s));
    }
}

#[test]
fn a_state_with_a_slice_missing_refuses_to_read_it_in_debug() {
    let s = CoreState::with_loaded(SliceSet::of(&[Slice::Meta, Slice::Board]));
    assert_eq!(s.meta().round, 0);
    // The guard is a `debug_assert!` (trap T9): it exists in the profiles that keep debug assertions (dev, the D4 profile) and is compiled out of the
    // optimised ones (gate, lockstep, release), where the accessor just reads. So the refusal is asserted exactly where the guard exists; in the optimised
    // profiles the rest of the test (the loaded set as part of the state) still runs, and nothing is claimed about what an unguarded read returns.
    if cfg!(debug_assertions) {
        let r = std::panic::catch_unwind(|| {
            let s = CoreState::with_loaded(SliceSet::of(&[Slice::Meta]));
            let _ = s.board();
        });
        assert!(r.is_err(), "debug assertions are the unloaded-slice guard (trap T9): the D4 profile must keep them on");
    }
    let mut m = s.clone();
    m.mark_loaded(Slice::Rng);
    assert!(m.loaded.has(Slice::Rng) && !s.loaded.has(Slice::Rng));
    assert_ne!(m, s, "the loaded set is part of the state");
}

#[test]
fn meta_flags_set_and_clear_independently() {
    let mut f = MetaFlags::default();
    f.set(MetaFlags::THUNDERS_EDGE, true);
    f.set(MetaFlags::NEXUS_UNLOCKED, true);
    assert!(f.has(MetaFlags::THUNDERS_EDGE) && f.has(MetaFlags::NEXUS_UNLOCKED) && !f.has(MetaFlags::FRACTURE_IN_PLAY));
    f.set(MetaFlags::THUNDERS_EDGE, false);
    assert!(!f.has(MetaFlags::THUNDERS_EDGE) && f.has(MetaFlags::NEXUS_UNLOCKED));
    f.set(MetaFlags::NEXUS_UNLOCKED, false);
    assert_eq!(f, MetaFlags::default());
}

#[test]
fn meta_rules_gate_is_inclusive_and_phases_know_who_orders_them() {
    let mut m = Meta::default();
    m.rules_version = 23;
    assert!(m.rules_at_least(23) && m.rules_at_least(0) && !m.rules_at_least(24));
    assert!(Phase::Strategy.uses_speaker_order() && Phase::Agenda.uses_speaker_order());
    assert!(!Phase::Action.uses_speaker_order() && !Phase::Status.uses_speaker_order());
    assert_eq!(Meta::default().phase, Phase::Strategy);
}

// ---- Piece ------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ti4_engine_core_tests::generators::config())]
    #[test]
    fn a_piece_keeps_every_field_independent(kind in 0usize..Unit::COUNT, kind2 in 0usize..Unit::COUNT, owner in 0u8..16, owner2 in 0u8..16, sus in any::<bool>(), gal in any::<bool>()) {
        let (k, k2) = (Unit::from_index(kind).unwrap(), Unit::from_index(kind2).unwrap());
        let p = Piece::new(k, Seat(owner)).with_sustained(sus).with_galvanized(gal);
        prop_assert_eq!((p.kind(), p.owner(), p.sustained(), p.galvanized()), (k, Seat(owner), sus, gal));
        let q = p.with_owner(Seat(owner2));
        prop_assert_eq!((q.kind(), q.owner(), q.sustained(), q.galvanized()), (k, Seat(owner2), sus, gal));
        let r = p.with_kind(k2);
        prop_assert_eq!((r.kind(), r.owner(), r.sustained(), r.galvanized()), (k2, Seat(owner), sus, gal));
        prop_assert_eq!(p.with_sustained(!sus).sustained(), !sus);
        prop_assert_eq!(p.with_galvanized(!gal).galvanized(), !gal);
    }
}

// ---- A whole state ----------------------------------------------------------------------------

#[test]
fn a_cloned_state_shares_everything_and_a_write_copies_only_what_it_touches() {
    let sc = Scenario::lobby(3).map_ring(7);
    let (sys, planet) = ti4_engine_core_tests::scenario::first_planet();
    let a = sc.unit(sys, Some(planet), Unit::InfantryI, 0, 2).state();
    let mut b = a.clone();
    assert!(a == b && a.board.shared_chunks(&b.board) > 0);
    assert!(a.rng.shares_streams_with(&b.rng));
    let before = a.board.shared_chunks(&b.board);
    b.board_mut().planet_mut(planet).units.push(Piece::new(Unit::InfantryI, Seat(1)));
    assert_eq!(a.board.shared_chunks(&b.board), before - 1, "one chunk of planets copied");
    assert_eq!(a.board().planet(planet).units.len(), 2);
    assert_eq!(b.board().planet(planet).units.len(), 3);
    assert!(a != b);
}
