//! D4-2c: `kernel::next` (the continuation algebra) and `kernel::read::Rd` (the read bundle of the combat and invasion rules).

use proptest::prelude::*;
use ti4_engine_core::kernel::next::Next;
use ti4_engine_core::kernel::read::Rd;
use ti4_engine_core::state::{Window as StateWindow, WindowKind};
use ti4_engine_core_tests::prelude::*;

fn open(kind: WindowKind) -> Next {
    Next::Open(StateWindow::Untyped { kind, actor: None, data: [0; 3] })
}

fn arb_next() -> impl Strategy<Value = Next> {
    let kinds = [WindowKind::Tokens, WindowKind::Trade, WindowKind::Voting, WindowKind::Tactical];
    let leaf = prop_oneof![Just(Next::Done), (0..kinds.len()).prop_map(move |i| open(kinds[i]))];
    leaf.prop_recursive(3, 12, 3, |inner| prop::collection::vec(inner, 0..4).prop_map(Next::Chain))
}

proptest! {
    #![proptest_config(ti4_engine_core_tests::generators::config())]

    #[test]
    fn then_is_concatenation_of_the_flattened_continuations(a in arb_next(), b in arb_next()) {
        let want: Vec<Next> = a.clone().flatten().into_iter().chain(b.clone().flatten()).collect();
        prop_assert_eq!(a.then(b).flatten(), want);
    }

    #[test]
    fn done_is_the_identity_and_is_done_means_nothing_left_to_do(a in arb_next()) {
        prop_assert_eq!(Next::Done.then(a.clone()).flatten(), a.clone().flatten());
        prop_assert_eq!(a.clone().then(Next::Done).flatten(), a.clone().flatten());
        prop_assert_eq!(a.is_done(), a.flatten().is_empty());
    }
}

#[test]
fn a_chain_of_chains_flattens_in_order_and_done_is_dropped() {
    covers!("kernel::next::Next");
    let n = Next::Chain(vec![open(WindowKind::Tokens), Next::Chain(vec![Next::Done, open(WindowKind::Trade)]), open(WindowKind::Voting)]);
    assert_eq!(n.flatten(), vec![open(WindowKind::Tokens), open(WindowKind::Trade), open(WindowKind::Voting)]);
    assert!(Next::Done.is_done() && Next::Chain(vec![]).is_done() && !open(WindowKind::Tokens).is_done());
    assert_eq!(open(WindowKind::Tokens).then(open(WindowKind::Trade)), Next::Chain(vec![open(WindowKind::Tokens), open(WindowKind::Trade)]));
}

#[test]
fn the_read_bundle_exposes_the_scope_the_unit_rows_and_the_overrides() {
    covers!("kernel::read::Rd");
    let (s, k) = Scenario::lobby(2).build();
    let rd = Rd::new(&s, &k);
    assert!(!rd.blanked && rd.l1z1x_commander.is_none());
    assert!(rd.with_blanked(true).blanked);
    assert_eq!(rd.with_l1z1x_commander(true).l1z1x_commander, Some(true));
    assert_eq!(rd.scope().0, u32::from(k.sources.0));
    assert!(rd.is_ship(Unit::CruiserI) && !rd.is_ship(Unit::InfantryI));
    assert!(rd.is_fighter(Unit::FighterI) && !rd.is_fighter(Unit::CruiserI));
    assert_eq!(rd.data(Unit::CruiserI).map(|d| d.ship), Some(true));
    let other = Scenario::lobby(3).state();
    assert_eq!(rd.over(&other).s.players().len(), 3, "the read over another state");
    assert_eq!(rd.q().s.players().len(), 2);
}
