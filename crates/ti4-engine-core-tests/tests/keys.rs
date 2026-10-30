//! D4-1 low layer: `keys/*` (decimal text order, the prefix-aware id order against a text model, atoms, mark keys and the mark map).

use std::cmp::Ordering;

use proptest::prelude::*;
use ti4_engine_core::generated::{Faction, Planet, System, Unit};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::keys::order::{self, PrefixPairs, cmp_indexed, next_byte};
use ti4_engine_core::keys::tables;
use ti4_engine_core::keys::{Atom, AtomLike, DecimalTextOrd, Family, MarkKey, MarkVal, Word};
use ti4_engine_core::state::MarkMap;

// ---- DecimalTextOrd ---------------------------------------------------------------------------

fn text_cmp(a: u32, ta: u8, b: u32, tb: u8) -> Ordering {
    let mut x = a.to_string().into_bytes();
    let mut y = b.to_string().into_bytes();
    // A terminator of 0 is "end of key": it sorts before every byte, which is what the empty tail does.
    x.push(ta);
    y.push(tb);
    x.cmp(&y)
}

proptest! {
    #![proptest_config(ti4_engine_core_tests::generators::config())]
    #[test]
    fn decimal_cmp_followed_equals_the_text_comparison(a in 0u32..2000, b in 0u32..2000, ta in prop::sample::select(vec![0u8, b',', b':', b'|']), tb in prop::sample::select(vec![0u8, b',', b':', b'|'])) {
        prop_assert_eq!(DecimalTextOrd(a).cmp_followed(ta, DecimalTextOrd(b), tb), text_cmp(a, ta, b, tb));
    }

    #[test]
    fn decimal_order_is_a_total_order_that_is_the_string_order(a in any::<u32>(), b in any::<u32>()) {
        prop_assert_eq!(DecimalTextOrd(a).cmp(&DecimalTextOrd(b)), a.to_string().cmp(&b.to_string()));
        prop_assert_eq!(DecimalTextOrd(a).cmp(&DecimalTextOrd(b)), DecimalTextOrd(b).cmp(&DecimalTextOrd(a)).reverse());
    }
}

#[test]
fn decimal_digits_of_the_edge_numbers() {
    use ti4_engine_core::keys::decimal::digits;
    assert_eq!(digits(0), ([0; 10], 1));
    assert_eq!(digits(7), ([7, 0, 0, 0, 0, 0, 0, 0, 0, 0], 1));
    assert_eq!(digits(10).1, 2);
    assert_eq!(digits(999_999_999).1, 9);
    assert_eq!(digits(u32::MAX).1, 10);
}

// ---- cmp_indexed against a text model ---------------------------------------------------------

/// A synthetic id table: the decimal texts 1..=130, sorted as text (the way the generated enums are numbered), and the prefix pairs
/// computed from it exactly as `tables.rs` documents them.
fn model() -> (Vec<String>, PrefixPairs) {
    let mut ids: Vec<String> = (1..=130).map(|n: u32| n.to_string()).collect();
    ids.sort();
    let mut pairs = Vec::new();
    for (ia, a) in ids.iter().enumerate() {
        for (ib, b) in ids.iter().enumerate() {
            if ia != ib && b.starts_with(a.as_str()) {
                pairs.push((ia as u16, ib as u16, b.as_bytes()[a.len()]));
            }
        }
    }
    pairs.sort_unstable_by_key(|(a, b, _)| (*a, *b));
    (ids, Box::leak(pairs.into_boxed_slice()))
}

proptest! {
    #![proptest_config(ti4_engine_core_tests::generators::config())]
    #[test]
    fn cmp_indexed_equals_the_text_order_of_the_ids_with_their_followers(a in 0usize..130, b in 0usize..130, ta in prop::sample::select(vec![0u8, b',', b':', b'|']), tb in prop::sample::select(vec![0u8, b',', b':', b'|'])) {
        let (ids, pairs) = model();
        let mut x = ids[a].clone().into_bytes();
        let mut y = ids[b].clone().into_bytes();
        x.push(ta);
        y.push(tb);
        prop_assert_eq!(cmp_indexed(pairs, a, ta, b, tb), x.cmp(&y), "{} {} against {} {}", ids[a], ta, ids[b], tb);
    }
}

#[test]
fn next_byte_finds_the_byte_after_the_prefix() {
    let (ids, pairs) = model();
    let one = ids.iter().position(|s| s == "1").unwrap();
    let ten = ids.iter().position(|s| s == "10").unwrap();
    assert_eq!(next_byte(pairs, one, ten), Some(b'0'));
    assert_eq!(next_byte(pairs, ten, one), None, "not the other way round");
    assert_eq!(next_byte(pairs, one, one), None);
}

#[test]
fn the_shipped_prefix_tables_are_sorted_and_forward() {
    for (name, t) in [("SYSTEM", tables::SYSTEM), ("PLANET", tables::PLANET), ("UNIT", tables::UNIT), ("TECHNOLOGY", tables::TECHNOLOGY), ("ACTION_CARD", tables::ACTION_CARD), ("PROMISSORY_NOTE", tables::PROMISSORY_NOTE), ("LEADER", tables::LEADER), ("WORD", tables::WORD)] {
        assert!(t.windows(2).all(|w| (w[0].0, w[0].1) < (w[1].0, w[1].1)), "{name} is sorted by (a, b) (binary search needs it)");
        assert!(t.iter().all(|(a, b, c)| a < b && c.is_ascii_graphic()), "{name}: a prefix has the smaller index, the follower is a visible byte");
    }
    assert!(order::NONE.is_empty() && order::AGENDA.is_empty() && order::FACTION.is_empty());
}

#[test]
fn the_note_rank_is_a_permutation() {
    let mut r = tables::NOTE_RANK.to_vec();
    r.sort_unstable();
    assert_eq!(r, (0..40).collect::<Vec<u16>>());
}

#[test]
fn the_shipped_system_table_orders_a_prefix_id_by_its_follower() {
    // Pair (9, 10, '0'): system 9's text is a prefix of system 10's text, whose next byte is '0'.
    assert_eq!(cmp_indexed(tables::SYSTEM, 9, b':', 10, b':'), Ordering::Greater, "':' sorts after '0'");
    assert_eq!(cmp_indexed(tables::SYSTEM, 9, b',', 10, b','), Ordering::Less, "',' sorts before '0'");
    assert_eq!(cmp_indexed(tables::SYSTEM, 9, 0, 10, 0), Ordering::Less);
    assert_eq!(cmp_indexed(tables::SYSTEM, 10, b':', 9, b':'), Ordering::Less);
    assert_eq!(cmp_indexed(tables::SYSTEM, 3, b':', 4, b':'), Ordering::Less, "unrelated ids order by index");
}

// ---- Atom, Word -------------------------------------------------------------------------------

#[test]
fn words_are_numbered_in_their_declared_order() {
    for (i, w) in Word::ALL.iter().enumerate() {
        assert_eq!(w.index(), i);
    }
    assert!(Word::ALL.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(Word::ALL.len(), Word::COUNT);
}

#[test]
fn atom_round_trips_through_its_typed_form() {
    assert_eq!(<Seat as AtomLike>::from_atom(Seat(3).to_atom()), Some(Seat(3)));
    assert_eq!(<u32 as AtomLike>::from_atom(Atom::Num(9)), Some(9));
    assert_eq!(<Seat as AtomLike>::from_atom(Atom::Num(3)), None, "a number is not a seat");
    assert_eq!(<Option<Planet> as AtomLike>::from_atom(Atom::Absent), Some(None));
    assert_eq!(<Option<Planet> as AtomLike>::from_atom(Planet::Abyz.to_atom()), Some(Some(Planet::Abyz)));
    assert_eq!(<Option<Planet> as AtomLike>::from_atom(Atom::Seat(Seat(1))), None);
    assert_eq!(None::<Planet>.to_atom(), Atom::Absent);
    assert_eq!(Word::Yes.to_atom(), Atom::Word(Word::Yes));
    assert_eq!(<Word as AtomLike>::from_atom(Atom::Word(Word::Space)), Some(Word::Space));
}

#[test]
fn atom_kinds_are_distinct_and_cross_kind_comparison_is_by_kind() {
    let atoms = [
        Atom::Absent,
        Atom::Num(1),
        Atom::Dec(DecimalTextOrd(1)),
        Atom::Seat(Seat(1)),
        Atom::System(System::ALL[1]),
        Atom::Planet(Planet::Abyz),
        Atom::Unit(Unit::ALL[1]),
        Atom::Faction(Faction::TheArborec),
        Atom::Word(Word::Yes),
        Atom::Blob(1),
    ];
    let kinds: Vec<u8> = atoms.iter().map(Atom::kind).collect();
    let mut sorted = kinds.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), atoms.len(), "{kinds:?}");
    for a in &atoms {
        for b in &atoms {
            if a.kind() != b.kind() {
                assert_eq!(a.cmp_followed(0, *b, 0), a.kind().cmp(&b.kind()));
            }
        }
    }
}

#[test]
fn atoms_of_one_kind_order_numerically_or_by_id_with_the_follower_deciding_a_tie() {
    assert_eq!(Atom::Num(9).cmp_followed(0, Atom::Num(10), 0), Ordering::Less, "plain counts are numeric");
    assert_eq!(Atom::Num(9).cmp_followed(b':', Atom::Num(9), b','), Ordering::Greater);
    assert_eq!(Atom::Dec(DecimalTextOrd(9)).cmp_followed(0, Atom::Dec(DecimalTextOrd(10)), 0), Ordering::Greater, "decimals are textual");
    assert_eq!(Atom::Seat(Seat(2)).cmp_followed(0, Atom::Seat(Seat(10)), 0), Ordering::Less);
    assert_eq!(Atom::Absent.cmp_followed(0, Atom::Absent, b'|'), Ordering::Less);
    assert_eq!(Atom::Word(Word::Action).cmp_followed(0, Atom::Word(Word::Action), 0), Ordering::Equal);
    // Word 0 ("action") is a prefix of word 1 ("action_cards"): the byte after it decides.
    assert_eq!(Atom::Word(Word::Action).cmp_followed(b':', Atom::Word(Word::ActionCards), b':'), Ordering::Less, "':' (58) sorts before '_' (95)");
    assert_eq!(Atom::Word(Word::Action).cmp_followed(b'|', Atom::Word(Word::ActionCards), b'|'), Ordering::Greater, "'|' (124) sorts after '_'");
}

// ---- MarkKey ----------------------------------------------------------------------------------

#[test]
fn mark_keys_round_trip_through_their_atoms() {
    let keys = [
        MarkKey::AllowPass(),
        MarkKey::ArborecLw2(Seat(2)),
        MarkKey::ArgentProduction(Seat(1), System::ALL[5]),
        MarkKey::CabalDevour(System::ALL[3], None),
        MarkKey::CabalDevour(System::ALL[3], Some(Planet::Abyz)),
        MarkKey::CardsReveal(Word::Yes, Word::Space, Seat(0), Seat(1), Word::Open),
        MarkKey::PrivateMoved(DecimalTextOrd(12)),
        MarkKey::YssarilBt(Word::Yes, Seat(0), Seat(1)),
    ];
    for k in keys {
        let (atoms, n) = k.atoms();
        assert_eq!(MarkKey::from_atoms(k.family(), &atoms[..n]), Some(k), "{k:?}");
        // Wrong arity and wrong kind are refused.
        if n > 0 {
            assert_eq!(MarkKey::from_atoms(k.family(), &atoms[..n - 1]), None, "{k:?}: one atom too few");
        }
        let mut longer = atoms[..n].to_vec();
        longer.push(Atom::Num(0));
        assert_eq!(MarkKey::from_atoms(k.family(), &longer), None, "{k:?}: one atom too many");
    }
    assert_eq!(MarkKey::from_atoms(Family::ArborecLw2, &[Atom::Num(2)]), None, "a number is not a seat");
}

#[test]
fn mark_key_order_is_family_order_then_atoms() {
    assert!(Family::ALL.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(Family::ALL.len(), Family::COUNT);
    for (i, f) in Family::ALL.iter().enumerate() {
        assert_eq!(f.index(), i);
    }
    assert!(MarkKey::AllowPass() < MarkKey::ArborecLw2(Seat(0)));
    assert!(MarkKey::ArborecLw2(Seat(1)) < MarkKey::ArborecLw2(Seat(2)));
    assert!(MarkKey::ArborecLw2(Seat(9)) < MarkKey::ArborecPsychosporeExhausted(Seat(0)));
    // The decimal sequence of a family sorts as text: 10 before 9.
    assert!(MarkKey::PrivateMoved(DecimalTextOrd(10)) < MarkKey::PrivateMoved(DecimalTextOrd(9)));
    // An absent optional tail sorts before a present one ("cabal:devour:S" against "cabal:devour:S|P").
    assert!(MarkKey::CabalDevour(System::ALL[3], None) < MarkKey::CabalDevour(System::ALL[3], Some(Planet::Abyz)));
    // Fixed width sequences are numeric.
    assert!(MarkKey::PrivateStagedEvent(9) < MarkKey::PrivateStagedEvent(10));
}

#[test]
fn a_family_lists_one_terminator_per_field() {
    assert_eq!(Family::ArgentProduction.terms(), &[b':', 0]);
    assert_eq!(Family::AllowPass.terms(), &[] as &[u8]);
    assert_eq!(Family::CardsReveal.terms().len(), 5);
}

#[test]
fn the_mark_map_stays_sorted_by_key_order_and_replaces_in_place() {
    let mut m = MarkMap::new();
    assert!(m.is_empty());
    let hi = MarkKey::PrivateMoved(DecimalTextOrd(9));
    let lo = MarkKey::PrivateMoved(DecimalTextOrd(10));
    let first = MarkKey::AllowPass();
    assert_eq!(m.insert(hi, MarkVal::Atom(Atom::Num(1))), None);
    assert_eq!(m.insert(first, MarkVal::List(vec![])), None);
    assert_eq!(m.insert(lo, MarkVal::Atom(Atom::Num(2))), None);
    assert_eq!(m.insert(hi, MarkVal::Atom(Atom::Num(3))), Some(MarkVal::Atom(Atom::Num(1))));
    assert_eq!(m.len(), 3);
    assert_eq!(m.iter().map(|(k, _)| *k).collect::<Vec<_>>(), vec![first, lo, hi]);
    assert_eq!(m.get(&hi), Some(&MarkVal::Atom(Atom::Num(3))));
    assert!(m.contains_key(&lo));
    assert_eq!(m.remove(&lo), Some(MarkVal::Atom(Atom::Num(2))));
    assert_eq!(m.remove(&lo), None);
    assert!(!m.contains_key(&lo));
    let rebuilt: MarkMap = m.iter().map(|(k, v)| (*k, v.clone())).collect();
    assert_eq!(rebuilt, m);
}
