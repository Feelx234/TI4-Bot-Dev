//! D4-1 low layer: `containers/*` (Chunked, Cow, SmallList, SortedMap, Set, SeatSet, Deck, Log, Opt32, DecimalTextOrd).

use proptest::prelude::*;
use ti4_engine_core::containers::{Chunked, Cow, DecimalTextOrd, Deck, Log, Opt32, SeatSet, Set, SmallList, SortedMap};
use ti4_engine_core::generated::{Faction, Planet};
use ti4_engine_core::ids::Seat;

#[test]
fn cow_copies_only_on_the_first_write_after_a_clone() {
    let mut a = Cow::new(vec![1, 2, 3]);
    let b = a.clone();
    assert!(Cow::ptr_eq(&a, &b));
    assert_eq!(a.share_count(), 2);
    a.edit().push(4);
    assert!(!Cow::ptr_eq(&a, &b));
    assert_eq!(*b, vec![1, 2, 3]);
    assert_eq!(*a, vec![1, 2, 3, 4]);
    assert_eq!(a.share_count(), 1);
    let before = a.share_count();
    a.edit().push(5);
    assert_eq!(a.share_count(), before, "an unshared edit does not copy");
}

#[test]
fn cow_equality_is_by_value_not_by_allocation() {
    assert_eq!(Cow::new(5), Cow::new(5));
    assert_ne!(Cow::new(5), Cow::new(6));
    assert!(!Cow::ptr_eq(&Cow::new(5), &Cow::new(5)));
}

#[test]
fn chunked_edit_copies_one_chunk_and_leaves_the_clone_alone() {
    let mut a: Chunked<u8, 4, 3> = Chunked::new(&0);
    assert_eq!(Chunked::<u8, 4, 3>::CAPACITY, 12);
    let b = a.clone();
    assert_eq!(a.shared_chunks(&b), 3);
    *a.edit(5) = 9;
    assert_eq!(*a.get(5), 9);
    assert_eq!(*b.get(5), 0);
    assert_eq!(a.shared_chunks(&b), 2);
    assert!(a.chunk_shared_with(&b, 0) && !a.chunk_shared_with(&b, 4) && !a.chunk_shared_with(&b, 7), "chunk 1 holds cells 4..8");
    assert!(a.chunk_shared_with(&b, 8));
    assert_eq!(a.iter().copied().collect::<Vec<_>>(), vec![0, 0, 0, 0, 0, 9, 0, 0, 0, 0, 0, 0]);
    assert_ne!(a, b);
    *a.edit(5) = 0;
    assert_eq!(a, b, "equal by value after the edit is undone");
}

#[test]
fn small_list_keeps_order_across_the_spill_and_back() {
    let mut l: SmallList<u8, 3> = SmallList::new();
    assert!(l.is_empty() && !l.spilled());
    for x in 1..=3 {
        l.push(x);
    }
    assert!(!l.spilled(), "N elements stay inline");
    l.push(4);
    assert!(l.spilled(), "N+1 elements spill");
    assert_eq!(l.as_slice(), &[1, 2, 3, 4]);
    assert_eq!(l.pop(), Some(4));
    assert_eq!(l.len(), 3);
    l.insert(1, 9);
    assert_eq!(l.as_slice(), &[1, 9, 2, 3]);
    assert_eq!(l.remove(0), 1);
    assert_eq!(l.as_slice(), &[9, 2, 3]);
    l.retain(|x| *x != 2);
    assert_eq!(l.as_slice(), &[9, 3]);
    l.truncate(1);
    assert_eq!(l.as_slice(), &[9]);
    l.clear();
    assert!(l.is_empty());
    assert_eq!(l.pop(), None);
}

#[test]
fn small_list_inline_and_spilled_with_equal_elements_are_equal() {
    let mut a: SmallList<u8, 2> = SmallList::from_slice(&[1, 2, 3]);
    assert!(a.spilled());
    a.truncate(2);
    let b: SmallList<u8, 2> = SmallList::from_slice(&[1, 2]);
    assert_eq!(a, b);
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
}

#[test]
fn sorted_map_iterates_in_key_order_whatever_the_insertion_order() {
    let mut m: SortedMap<u8, u8, 2> = SortedMap::new();
    assert_eq!(m.insert(5, 50), None);
    assert_eq!(m.insert(1, 10), None);
    assert_eq!(m.insert(3, 30), None);
    assert_eq!(m.insert(3, 31), Some(30));
    assert_eq!(m.keys().collect::<Vec<_>>(), vec![1, 3, 5]);
    assert_eq!(m.values().collect::<Vec<_>>(), vec![10, 31, 50]);
    assert_eq!(m.first(), Some((1, 10)));
    assert!(m.contains_key(&5) && !m.contains_key(&4));
    assert_eq!(m.remove(&1), Some(10));
    assert_eq!(m.remove(&1), None);
    *m.get_or_insert_with(7, || 70) += 1;
    assert_eq!(m.get(&7), Some(&71));
    m.retain(|k, _| *k > 3);
    assert_eq!(m.len(), 2);
    let n: SortedMap<u8, u8, 2> = [(7, 71), (5, 50)].into_iter().collect();
    assert_eq!(m, n);
}

#[test]
fn set_iterates_in_index_order_and_insert_reports_novelty() {
    let mut s: Set<Faction, 1> = Set::new();
    assert!(s.insert(Faction::TheYssarilTribes));
    assert!(s.insert(Faction::TheArborec));
    assert!(!s.insert(Faction::TheArborec));
    assert_eq!(s.iter().collect::<Vec<_>>(), vec![Faction::TheArborec, Faction::TheYssarilTribes]);
    assert_eq!(s.first(), Some(Faction::TheArborec));
    assert_eq!(s.len(), 2);
    assert!(s.remove(Faction::TheArborec) && !s.remove(Faction::TheArborec));
    assert!(s.contains(Faction::TheYssarilTribes) && !s.contains(Faction::TheArborec));
    s.clear();
    assert!(s.is_empty());
}

#[test]
fn set_algebra_matches_the_obvious_definitions() {
    let a: Set<Planet, 3> = [Planet::Abyz, Planet::Accoen, Planet::Acheron].into_iter().collect();
    let b: Set<Planet, 3> = [Planet::Accoen, Planet::Acheron, Planet::Ang].into_iter().collect();
    let mut u = a;
    u.union_with(&b);
    assert_eq!(u.len(), 4);
    let mut i = a;
    i.intersect_with(&b);
    assert_eq!(i.iter().collect::<Vec<_>>(), vec![Planet::Accoen, Planet::Acheron]);
    let mut d = a;
    d.subtract(&b);
    assert_eq!(d.iter().collect::<Vec<_>>(), vec![Planet::Abyz]);
    assert!(a.intersects(&b));
    assert!(i.is_subset(&a) && i.is_subset(&b) && !a.is_subset(&b));
    let none: Set<Planet, 3> = Set::new();
    assert!(!a.intersects(&none) && none.is_subset(&a));
}

#[test]
fn seat_set_is_a_bitset_of_seats_in_seat_order() {
    let mut s = SeatSet::EMPTY;
    assert!(s.insert(Seat(5)) && s.insert(Seat(0)) && !s.insert(Seat(5)));
    assert_eq!(s.iter().collect::<Vec<_>>(), vec![Seat(0), Seat(5)]);
    assert_eq!(s.len(), 2);
    let t: SeatSet = [Seat(5), Seat(7)].into_iter().collect();
    assert_eq!(s.union(t).len(), 3);
    assert_eq!(s.intersection(t).iter().collect::<Vec<_>>(), vec![Seat(5)]);
    assert_eq!(s.without(t).iter().collect::<Vec<_>>(), vec![Seat(0)]);
    assert!(s.remove(Seat(0)) && !s.remove(Seat(0)));
    assert!(SeatSet::EMPTY.is_empty() && !s.is_empty());
    assert!(s.contains(Seat(5)) && !s.contains(Seat(6)));
    assert!(SeatSet(0x8000).contains(Seat(15)));
}

#[test]
fn deck_draws_from_the_top_without_copying_and_edits_copy() {
    let mut d = Deck::new(vec![1u8, 2, 3, 4]);
    assert_eq!((d.top(), d.len(), d.is_empty()), (Some(1), 4, false));
    let snapshot = d.clone();
    assert_eq!(d.draw(), Some(1));
    assert!(d.shares_order_with(&snapshot), "a draw moves the head only");
    d.put_top(9);
    assert_eq!(d.cards(), &[9, 2, 3, 4]);
    d.put_top(8);
    assert_eq!(d.cards(), &[8, 9, 2, 3, 4], "putting back on an untouched head inserts");
    assert_eq!(d.remove(2), 2);
    d.put_bottom(7);
    assert_eq!(d.cards(), &[8, 9, 3, 4, 7]);
    d.set(vec![5]);
    assert_eq!((d.top(), d.len()), (Some(5), 1));
    assert_eq!(d.draw(), Some(5));
    assert_eq!(d.draw(), None);
    assert!(d.is_empty());
    assert_eq!(snapshot.cards(), &[1, 2, 3, 4], "the clone never changed");
}

#[test]
fn deck_equality_ignores_how_the_cards_got_there() {
    let mut a = Deck::new(vec![1u8, 2, 3]);
    a.draw();
    assert_eq!(a, Deck::new(vec![2u8, 3]));
}

#[test]
fn log_appends_in_order_and_clones_share_the_sealed_prefix() {
    let mut l: Log<u32, 4> = Log::new();
    assert!(l.is_empty() && l.last().is_none() && l.get(0).is_none());
    for i in 0..10 {
        l.push(i);
    }
    assert_eq!(l.len(), 10);
    assert_eq!(l.iter().copied().collect::<Vec<_>>(), (0..10).collect::<Vec<_>>());
    assert_eq!(l.get(0), Some(&0));
    assert_eq!(l.get(5), Some(&5));
    assert_eq!(l.get(9), Some(&9));
    assert_eq!(l.get(10), None);
    assert_eq!(l.last(), Some(&9));
    let c = l.clone();
    assert!(c.shares_prefix_with(&l) && c.shares_tail_with(&l));
    l.push(10);
    assert!(c.shares_prefix_with(&l), "an append that seals nothing keeps the prefix");
    assert!(!c.shares_tail_with(&l), "the first append copies the tail");
    for i in 11..16 {
        l.push(i);
    }
    assert!(!c.shares_prefix_with(&l), "sealing a chunk moves the prefix");
    assert_eq!(c.len(), 10);
    assert_eq!(c.to_vec(), (0..10).collect::<Vec<_>>());
}

#[test]
fn log_get_mut_edits_in_place_without_touching_a_clone() {
    let mut l: Log<u32, 4> = Log::new();
    for i in 0..9 {
        l.push(i);
    }
    let c = l.clone();
    *l.get_mut(1).unwrap() = 100;
    *l.get_mut(8).unwrap() = 800;
    assert!(l.get_mut(9).is_none());
    assert_eq!(l.get(1), Some(&100));
    assert_eq!(l.get(8), Some(&800));
    assert_eq!(c.get(1), Some(&1));
    assert_eq!(c.get(8), Some(&8));
    assert_eq!(l.get(5), Some(&5));
}

#[test]
fn opt32_stores_zero_and_everything_but_the_maximum() {
    assert_eq!(Opt32::NONE.get(), None);
    assert!(!Opt32::NONE.is_some());
    assert_eq!(Opt32::some(0).get(), Some(0));
    assert!(Opt32::some(0).is_some());
    assert_eq!(Opt32::some(u32::MAX - 1).get(), Some(u32::MAX - 1));
    assert!(Opt32::some(7).is(7) && !Opt32::some(7).is(8) && !Opt32::NONE.is(0));
    let mut o = Opt32::new(Some(3));
    assert_eq!(o.take(), Some(3));
    assert_eq!(o, Opt32::NONE);
    o.set(Some(4));
    assert_eq!(o.get(), Some(4));
    o.set(None);
    assert_eq!(o.take(), None);
    assert_eq!(Opt32::from(Some(2)), Opt32::some(2));
    assert_eq!(Opt32::default(), Opt32::NONE);
}

#[test]
fn decimal_text_order_is_the_string_order_of_the_numbers() {
    assert!(DecimalTextOrd(10) < DecimalTextOrd(9));
    assert!(DecimalTextOrd(100) < DecimalTextOrd(11));
    assert!(DecimalTextOrd(1) < DecimalTextOrd(10));
    assert!(DecimalTextOrd(0) < DecimalTextOrd(1));
    assert_eq!(DecimalTextOrd(77).cmp(&DecimalTextOrd(77)), std::cmp::Ordering::Equal);
    assert!(DecimalTextOrd(u32::MAX) > DecimalTextOrd(4_000_000_000));
}

proptest! {
    #![proptest_config(ti4_engine_core_tests::generators::config())]
    #[test]
    fn decimal_text_order_equals_string_order(a in any::<u32>(), b in any::<u32>()) {
        prop_assert_eq!(DecimalTextOrd(a).cmp(&DecimalTextOrd(b)), a.to_string().cmp(&b.to_string()));
    }

    #[test]
    fn small_list_behaves_like_a_vec(ops in proptest::collection::vec((0u8..4, any::<u8>()), 0..40)) {
        let mut l: SmallList<u8, 3> = SmallList::new();
        let mut v: Vec<u8> = Vec::new();
        for (op, x) in ops {
            match op {
                0 => { l.push(x); v.push(x); }
                1 => { prop_assert_eq!(l.pop(), v.pop()); }
                2 if !v.is_empty() => { let i = usize::from(x) % v.len(); prop_assert_eq!(l.remove(i), v.remove(i)); }
                _ => { let i = usize::from(x) % (v.len() + 1); l.insert(i, x); v.insert(i, x); }
            }
            prop_assert_eq!(l.as_slice(), v.as_slice());
        }
    }

    #[test]
    fn sorted_map_behaves_like_a_btreemap(ops in proptest::collection::vec((any::<bool>(), 0u8..12, any::<u8>()), 0..60)) {
        let mut m: SortedMap<u8, u8, 3> = SortedMap::new();
        let mut r = std::collections::BTreeMap::new();
        for (put, k, v) in ops {
            if put { prop_assert_eq!(m.insert(k, v), r.insert(k, v)); } else { prop_assert_eq!(m.remove(&k), r.remove(&k)); }
            prop_assert_eq!(m.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>(), r.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>());
        }
    }

    #[test]
    fn set_behaves_like_a_btreeset(ops in proptest::collection::vec((any::<bool>(), 0usize..34), 0..60)) {
        let mut s: Set<Faction, 1> = Set::new();
        let mut r = std::collections::BTreeSet::new();
        for (put, i) in ops {
            let f = Faction::ALL[i];
            if put { prop_assert_eq!(s.insert(f), r.insert(i)); } else { prop_assert_eq!(s.remove(f), r.remove(&i)); }
            prop_assert_eq!(s.iter().map(|f| f.index()).collect::<Vec<_>>(), r.iter().copied().collect::<Vec<_>>());
        }
    }

    #[test]
    fn log_behaves_like_a_vec_and_clones_never_change(items in proptest::collection::vec(any::<u16>(), 0..90), cut in 0usize..90) {
        let mut l: Log<u16, 8> = Log::new();
        let mut snapshot = None;
        for (i, x) in items.iter().enumerate() {
            if i == cut { snapshot = Some((l.clone(), items[..i].to_vec())); }
            l.push(*x);
        }
        prop_assert_eq!(l.to_vec(), items);
        if let Some((c, expect)) = snapshot { prop_assert_eq!(c.to_vec(), expect); }
    }
}
