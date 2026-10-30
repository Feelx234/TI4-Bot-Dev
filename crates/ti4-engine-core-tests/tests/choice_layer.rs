//! D4-1 low layer: `choice/*` (the decision model: options, picks, areas, the order bots and renderers rely on).

use ti4_engine_core::choice::generic::{Ask as GAsk, Rule, Subtype as GSub};
use ti4_engine_core::choice::strategy::Opt as SOpt;
use ti4_engine_core::choice::{Area, Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::ids::Seat;

fn ctx() -> Ctx {
    Ctx {
        origin: Origin::Rule(Rule::R22),
        subtype: Subtype::Generic(GSub::Unclassified),
        optional: false,
        target: None,
        outstanding: SmallList::new(),
        space_battle: false,
        invasion_seq: None,
    }
}

#[test]
fn the_generic_options_sort_before_every_area_and_in_a_fixed_order() {
    let mut v = vec![Opt::Strategy(SOpt::Top), Opt::Number(3), Opt::Done, Opt::Number(-1), Opt::Decline, Opt::Strategy(SOpt::Bottom)];
    v.sort();
    assert_eq!(v, vec![Opt::Decline, Opt::Done, Opt::Number(-1), Opt::Number(3), Opt::Strategy(SOpt::Top), Opt::Strategy(SOpt::Bottom)]);
}

#[test]
fn every_option_names_its_area_and_the_generic_exits_are_exactly_decline_and_done() {
    assert_eq!(Opt::Decline.area(), Area::Generic);
    assert_eq!(Opt::Done.area(), Area::Generic);
    assert_eq!(Opt::Number(4).area(), Area::Generic);
    assert_eq!(Opt::Strategy(SOpt::Top).area(), Area::Strategy);
    assert!(Opt::Decline.is_decline() && !Opt::Done.is_decline());
    assert!(Opt::Decline.is_generic_exit() && Opt::Done.is_generic_exit() && !Opt::Number(0).is_generic_exit());
    assert_eq!(Area::ALL[0], Area::Generic);
    assert_eq!(Area::ALL.len(), 20, "the generic area and nineteen rule areas (D1b2 added Turn)");
    let mut seen = Area::ALL.to_vec();
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), Area::ALL.len());
}

#[test]
fn an_ask_reports_the_area_of_its_family() {
    assert_eq!(Ask::Generic(GAsk::Confirm).area(), Area::Generic);
    assert_eq!(Ask::Strategy(ti4_engine_core::choice::strategy::Ask::ImperialScore).area(), Area::Strategy);
}

#[test]
fn a_choice_indexes_its_options_in_order_and_finds_the_decline() {
    let opts: Opts = [Opt::Strategy(SOpt::Top), Opt::Strategy(SOpt::Bottom), Opt::Decline].into_iter().collect();
    let c = Choice { actor: Seat(1), ask: Ask::Generic(GAsk::Confirm), ctx: ctx(), opts };
    assert_eq!(c.pick(0), Some(Pick::Opt(Opt::Strategy(SOpt::Top))));
    assert_eq!(c.pick(2), Some(Pick::Opt(Opt::Decline)));
    assert_eq!(c.pick(3), None);
    assert!(c.can_decline());
    assert_eq!(c.decline_index(), Some(2));
    assert_eq!(c.opts.index_of(&Opt::Strategy(SOpt::Bottom)), Some(1));
    assert_eq!(c.opts.index_of(&Opt::Done), None);
    assert_eq!(c.opts.len(), 3);
    assert!(!c.opts.is_empty());
    let none = Choice { opts: Opts::default(), ..c.clone() };
    assert!(!none.can_decline() && none.decline_index().is_none() && none.opts.is_empty());
}

#[test]
fn options_collect_push_and_iterate_in_insertion_order() {
    let mut o = Opts::default();
    o.push(Opt::Done);
    o.push(Opt::Number(2));
    o.push(Opt::Decline);
    assert_eq!(o.iter().copied().collect::<Vec<_>>(), vec![Opt::Done, Opt::Number(2), Opt::Decline]);
    assert_eq!(o.as_slice().len(), 3);
    let again: Opts = o.iter().copied().collect();
    assert_eq!(again, o);
}

#[test]
fn a_pick_made_from_an_option_equals_the_option_it_came_from() {
    let o = Opt::Strategy(SOpt::Top);
    assert_eq!(Pick::from(o), Pick::Opt(o));
    assert_ne!(Pick::Number(1), Pick::Opt(Opt::Number(1)), "a typed number and a count answer are different picks");
}

#[test]
fn an_option_stays_small_and_copy() {
    assert!(std::mem::size_of::<Opt>() <= 16);
    fn copy<T: Copy + Eq + Ord + std::hash::Hash>() {}
    copy::<Opt>();
    copy::<Ask>();
}
