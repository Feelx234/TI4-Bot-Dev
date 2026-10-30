//! D4-2c: the uniform seams of the kernel (`kernel::seam`): the modifier stack, the predicate, the contribution list, the permutation of the
//! order audit. The providers are plain functions; the sites are test types.

use proptest::prelude::*;
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::kernel::seam::{Clamp, Combine, Delta, Gather, Layer, Pass, Permute, Pred, Q, Seams, SetWhy, Site, Source, Stack, Verdict};
use ti4_engine_core_tests::prelude::*;

struct S;
impl Site for S {}

fn q_of<R>(f: impl FnOnce(Q<'_>) -> R) -> R {
    let (s, k) = (Scenario::lobby(2).state(), kernel_plain());
    f(Q::new(&s, &k))
}

fn plus_one(_: Q<'_>, _: &S, _: i64) -> Delta {
    Delta::Add(1)
}
fn plus_ten(_: Q<'_>, _: &S, _: i64) -> Delta {
    Delta::Add(10)
}
fn minus_two(_: Q<'_>, _: &S, _: i64) -> Delta {
    Delta::Add(-2)
}
fn nothing(_: Q<'_>, _: &S, _: i64) -> Delta {
    Delta::None
}
fn double(_: Q<'_>, _: &S, v: i64) -> Delta {
    Delta::Set { to: v * 2, why: SetWhy::Instead }
}
fn to_five(_: Q<'_>, _: &S, _: i64) -> Delta {
    Delta::Set { to: 5, why: SetWhy::Instead }
}

#[test]
fn an_empty_stack_is_its_base_and_the_clamp_bounds_the_result() {
    covers!("kernel::seam::stack::Stack");
    q_of(|q| {
        let st = Stack::<S>::new();
        assert!(st.is_empty());
        assert_eq!(st.eval(q, &S, 7, Clamp::None).value, 7);
        assert_eq!(st.eval(q, &S, -3, Clamp::Min(0)).value, 0);
        assert_eq!(st.eval(q, &S, 12, Clamp::Range(1, 10)).value, 10);
        assert_eq!(st.eval(q, &S, 0, Clamp::Range(1, 10)).value, 1);
        assert_eq!(st.eval(q, &S, 5, Clamp::Range(1, 10)).value, 5);
        assert_eq!(st.eval(q, &S, 5, Clamp::Min(5)).value, 5, "the bound itself is allowed");
    });
}

#[test]
fn layers_order_the_terms_and_a_set_in_an_earlier_layer_is_built_on() {
    covers!("kernel::seam::stack::Stack");
    q_of(|q| {
        let mut st = Stack::<S>::new();
        st.push_set(Source::Printed, Layer::Card, to_five);
        st.push_add(Source::Printed, Layer::Technology, plus_ten);
        st.push_add(Source::Printed, Layer::Printed, plus_one);
        assert_eq!(st.len(), 3);
        // 0 +1 (printed) +10 (technology), then the card sets 5
        assert_eq!(st.eval(q, &S, 0, Clamp::None).value, 5);
        let mut after = Stack::<S>::new();
        after.push_add(Source::Printed, Layer::Leader, plus_one);
        after.push_set(Source::Printed, Layer::Faction, double);
        assert_eq!(after.eval(q, &S, 4, Clamp::None).value, 9, "double (faction) comes before the leader's +1");
    });
}

#[test]
fn the_parts_list_the_adds_and_the_effect_of_each_set_and_skip_zero_effects() {
    covers!("kernel::seam::stack::Stack");
    q_of(|q| {
        let mut st = Stack::<S>::new();
        st.push_add(Source::Printed, Layer::Printed, plus_one);
        st.push_add(Source::Printed, Layer::Technology, nothing);
        st.push_set(Source::Printed, Layer::Law, to_five);
        st.push_set(Source::Printed, Layer::Faction, to_five);
        let e = st.eval(q, &S, 2, Clamp::None);
        assert_eq!(e.value, 5);
        let deltas: Vec<i64> = e.parts.as_slice().iter().map(|(_, d)| *d).collect();
        assert_eq!(deltas, vec![1, 2], "+1, then the set from 3 to 5; the second set changes nothing; None adds nothing");
    });
}

proptest! {
    #[test]
    fn adds_commute_under_every_audit_permutation(base in -20_i64..20, seed in any::<u64>()) {
        q_of(|q| {
            let mut st = Stack::<S>::new();
            for f in [plus_one, plus_ten, minus_two, plus_one, plus_ten] {
                st.push_add(Source::Printed, Layer::Faction, f);
            }
            let want = st.eval(q, &S, base, Clamp::None).value;
            for p in [Permute::Reverse, Permute::Shuffle(seed)] {
                let mut permuted = st.clone();
                permuted.permute_commutative(p);
                prop_assert_eq!(permuted.eval(q, &S, base, Clamp::None).value, want);
            }
            Ok(())
        })?;
    }
}

#[test]
fn a_layer_with_a_set_keeps_its_order_under_the_audit() {
    covers!("kernel::seam::stack::Stack");
    q_of(|q| {
        let mut st = Stack::<S>::new();
        st.push_add(Source::Printed, Layer::Faction, plus_ten);
        st.push_set(Source::Printed, Layer::Faction, double);
        let want = st.eval(q, &S, 1, Clamp::None).value;
        assert_eq!(want, 22);
        st.permute_commutative(Permute::Reverse);
        assert_eq!(st.eval(q, &S, 1, Clamp::None).value, want);
    });
}

fn forbid(_: Q<'_>, _: &u8) -> Verdict {
    Verdict::Forbid
}
fn allow(_: Q<'_>, _: &u8) -> Verdict {
    Verdict::Allow
}
fn abstain(_: Q<'_>, _: &u8) -> Verdict {
    Verdict::Abstain
}
fn allow_even(_: Q<'_>, n: &u8) -> Verdict {
    if n % 2 == 0 { Verdict::Allow } else { Verdict::Abstain }
}

#[test]
fn forbid_wins_is_an_all_and_allow_wins_an_any_and_the_default_decides_when_all_abstain() {
    covers!("kernel::seam::pred::Pred");
    q_of(|q| {
        let mut all = Pred::<u8>::new(Combine::ForbidWins);
        assert!(all.is_empty());
        assert!(all.decide(q, &0, true) && !all.decide(q, &0, false), "no rule: the default");
        all.push(Source::Printed, abstain);
        all.push(Source::Printed, allow);
        assert!(all.decide(q, &0, true) && !all.decide(q, &0, false), "an Allow does not override the default of a forbid-wins");
        all.push(Source::Printed, forbid);
        assert!(!all.decide(q, &0, true), "one forbid is enough");
        assert_eq!(all.len(), 3);
        assert_eq!(all.why(q, &0).len(), 1, "only the forbidding rule is named");
        let mut any = Pred::<u8>::new(Combine::AllowWins);
        any.push(Source::Printed, forbid);
        any.push(Source::Printed, abstain);
        assert!(!any.decide(q, &0, false) && any.decide(q, &0, true));
        any.push(Source::Printed, allow_even);
        assert!(any.decide(q, &2, false));
        assert!(!any.decide(q, &3, false), "the rule abstains on odd numbers");
        assert_eq!(any.why(q, &2).len(), 1);
        assert!(any.why(q, &3).is_empty());
    });
}

proptest! {
    #[test]
    fn a_predicate_does_not_depend_on_the_order_of_its_rules(n in 0_u8..10, seed in any::<u64>(), forbid_wins in any::<bool>(), default in any::<bool>()) {
        q_of(|q| {
            let combine = if forbid_wins { Combine::ForbidWins } else { Combine::AllowWins };
            let mut p = Pred::<u8>::new(combine);
            for f in [abstain, allow_even, forbid, allow, abstain] {
                p.push(Source::Printed, f);
            }
            let want = p.decide(q, &n, default);
            for perm in [Permute::Reverse, Permute::Shuffle(seed)] {
                let mut permuted = p.clone();
                permuted.permute(perm);
                prop_assert_eq!(permuted.decide(q, &n, default), want);
            }
            Ok(())
        })?;
    }
}

type G = Gather<u8, u8, 6>;

#[test]
fn a_gather_runs_every_add_before_every_edit_and_sorts_and_dedups_only_when_asked() {
    covers!("kernel::seam::gather::Gather");
    q_of(|q| {
        let mut g = G::new(ti4_engine_core::kernel::seam::gather::Order::Registration);
        assert!(g.is_empty());
        g.push(Source::Printed, Pass::Edit, |_, _, out| out.retain(|x| *x != 2));
        g.push(Source::Printed, Pass::Add, |_, _, out| out.extend([3, 2]));
        g.push(Source::Printed, Pass::Add, |_, _, out| out.extend([1, 3]));
        assert_eq!(g.len(), 3);
        assert_eq!(g.collect(q, &0).as_slice(), &[3, 1, 3]);
        let mut sorted = G::new(ti4_engine_core::kernel::seam::gather::Order::Sorted);
        sorted.push(Source::Printed, Pass::Add, |_, _, out| out.extend([3, 1]));
        sorted.push(Source::Printed, Pass::Add, |_, _, out| out.extend([2, 3, 1]));
        assert_eq!(sorted.collect(q, &0).as_slice(), &[1, 2, 3]);
        let mut with_edit = G::new(ti4_engine_core::kernel::seam::gather::Order::Sorted);
        with_edit.push(Source::Printed, Pass::Add, |_, _, out| out.extend([3, 1]));
        with_edit.push(Source::Printed, Pass::Edit, |_, _, out| out.retain(|x| *x != 3));
        let want = with_edit.collect(q, &0);
        with_edit.permute_if_sorted(Permute::Reverse);
        assert_eq!(with_edit.collect(q, &0).as_slice(), want.as_slice(), "an edit pass is never reordered");
        let _ = SmallList::<u8, 1>::new();
    });
}

proptest! {
    #[test]
    fn a_permutation_is_a_permutation_and_deterministic(len in 0_usize..30, seed in any::<u64>()) {
        for p in [Permute::Reverse, Permute::Shuffle(seed)] {
            let mut a: Vec<usize> = (0..len).collect();
            let mut b = a.clone();
            p.apply(&mut a);
            p.apply(&mut b);
            prop_assert_eq!(&a, &b);
            a.sort_unstable();
            prop_assert_eq!(a, (0..len).collect::<Vec<_>>());
        }
    }
}

#[test]
fn a_seam_table_with_nothing_registered_is_neutral_and_counts_its_slots() {
    covers!("kernel::seam::Seams");
    let seams = Seams::default();
    assert_eq!(seams.registered(), 0);
    assert!(Seams::SLOTS > 17, "5 stacks, 7 predicates, 3 gathers, 2 replacements and the observer and offer points");
    let plain = kernel_plain();
    assert_eq!(plain.hooks.seam.registered(), 0, "the plain kernel has no providers");
    let all = kernel_all();
    assert!(all.hooks.seam.registered() > 0, "the factions register providers");
}

mod p4_p6 {
    use super::*;
    use ti4_engine_core::kernel::seam::{Offer, OfferSite, Offers, Point, Replacement, Replacements, Step};
    use ti4_engine_core::kernel::Next;

    #[test]
    fn the_first_replacement_that_offers_wins_and_none_means_the_step_is_played_as_printed() {
        covers!("kernel::seam::replace::Replacements");
        q_of(|q| {
            let mut r = Replacements::<u8, u8>::new();
            assert!(r.is_empty() && r.offered(q, &0).is_none());
            r.push(Replacement { source: Source::Printed, step: Step::UnitForm, offer: |_, x| (*x > 5).then_some(1), perform: |_, _, _| Next::Done });
            r.push(Replacement { source: Source::Printed, step: Step::PdsPlacement, offer: |_, x| (*x > 2).then_some(2), perform: |_, _, _| Next::Done });
            assert_eq!(r.len(), 2);
            assert_eq!(r.offered(q, &9), Some(1));
            assert_eq!(r.offered(q, &3), Some(2));
            assert_eq!(r.offered(q, &1), None);
            let (which, offer) = r.first(q, &3).expect("an offer");
            assert_eq!((which.step, offer), (Step::PdsPlacement, 2), "the provider comes with its offer");
        });
    }

    fn site() -> OfferSite {
        OfferSite { who: Seat(0), system: None, planet: None, round: 1 }
    }

    fn silent() -> Offer {
        Offer { source: Source::Printed, ask: |_, _| None, apply: |_, _, _| Next::Done }
    }

    fn asking() -> Offer {
        Offer { source: Source::Printed, ask: |_, _| Some(Ask::Generic(ti4_engine_core::choice::generic::Ask::Confirm)), apply: |_, _, _| Next::Done }
    }

    #[test]
    fn offers_belong_to_their_point_and_are_asked_in_registration_order_from_an_index() {
        covers!("kernel::seam::offer::Offers");
        q_of(|q| {
            let mut o = Offers::default();
            assert!(o.is_empty());
            o.push(Point::ProductionStart, silent());
            o.push(Point::ProductionStart, asking());
            o.push(Point::ProductionStart, asking());
            o.push(Point::RoundEnd, asking());
            assert_eq!(o.len(), 4);
            assert_eq!(o.next_ask(q, Point::ProductionStart, 0, &site()).map(|(i, _)| i), Some(1), "the silent one is skipped");
            assert_eq!(o.next_ask(q, Point::ProductionStart, 2, &site()).map(|(i, _)| i), Some(2), "resume after the answered one");
            assert!(o.next_ask(q, Point::ProductionStart, 3, &site()).is_none());
            assert!(o.next_ask(q, Point::CombatStart, 0, &site()).is_none(), "another point has no offers");
            assert_eq!(o.next_ask(q, Point::RoundEnd, 0, &site()).map(|(i, _)| i), Some(0));
        });
    }

    #[test]
    fn every_point_has_its_own_slot() {
        covers!("kernel::seam::offer::Offers");
        let points = [
            Point::PaymentWindow,
            Point::ProductionStart,
            Point::ProductionEnd,
            Point::CombatStart,
            Point::AfterRollDice,
            Point::AfterBarrage,
            Point::RoundEnd,
            Point::BeforeCommit,
            Point::CommitToll,
            Point::AfterActivation,
        ];
        assert_eq!(points.len(), Point::COUNT);
        let mut o = Offers::default();
        for p in points {
            o.push(p, asking());
        }
        assert_eq!(o.len(), Point::COUNT);
        q_of(|q| {
            for p in points {
                assert_eq!(o.next_ask(q, p, 0, &site()).map(|(i, _)| i), Some(0), "{p:?} holds exactly its own offer");
                assert!(o.next_ask(q, p, 1, &site()).is_none(), "{p:?}");
            }
        });
    }
}
