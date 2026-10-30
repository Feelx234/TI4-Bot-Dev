//! D4-2c: `kernel::fx_pay` (the price of a bought objective or a secret: tokens, goods, planets, fragments, cards).

use ti4_engine_core::kernel::fx_pay::{pay_cost, pay_secret, pay_tokens, pay_trade_goods, PayEnv};
use ti4_engine_core::query::economy::Spend;
use ti4_engine_core::query::objectives::Cost;
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{planets_with_influence, systems_of};

fn env() -> PayEnv {
    PayEnv { scope: ALL_SOURCES, blanked: false }
}

fn econ(s: &CoreState) -> (i16, i16, i16, i16) {
    let e = s.players().by_seat(Seat(0)).unwrap().econ;
    (e.strategic_tokens, e.tactic_tokens, e.fleet_tokens, e.trade_goods)
}

#[test]
fn tokens_are_paid_from_the_strategy_pool_first_then_the_tactic_pool_and_never_the_fleet_pool() {
    covers!("kernel::fx_pay::pay_tokens");
    let mut s = Scenario::lobby(2).tokens(0, 3, 9, 2).state();
    assert!(pay_tokens(&mut s, Seat(0), 4));
    assert_eq!(econ(&s), (0, 1, 9, 0));
    let before = s.clone();
    assert!(!pay_tokens(&mut s, Seat(0), 2), "one token left in the two pools");
    assert!(s == before, "a refused payment changes nothing");
    assert!(pay_tokens(&mut s, Seat(0), 0));
    assert!(!pay_tokens(&mut s, Seat(9), 1));
}

#[test]
fn trade_goods_pay_exactly_what_is_held() {
    covers!("kernel::fx_pay::pay_trade_goods");
    let mut s = Scenario::lobby(2).goods(0, 3).commodities(0, 4).state();
    assert!(!pay_trade_goods(&mut s, Seat(0), 4), "commodities do not count");
    assert!(pay_trade_goods(&mut s, Seat(0), 3));
    assert_eq!(econ(&s).3, 0);
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.commodities, 4);
    assert!(!pay_trade_goods(&mut s, Seat(9), 0));
}

#[test]
fn a_cost_is_dispatched_to_its_kind() {
    covers!("kernel::fx_pay::pay_cost");
    let k = kernel_all();
    let mut s = Scenario::lobby(2).tokens(0, 2, 2, 2).goods(0, 5).state();
    assert!(pay_cost(&mut s, &k, env(), Seat(0), Cost::TradeGoods(2)));
    assert!(pay_cost(&mut s, &k, env(), Seat(0), Cost::Tokens(3)));
    assert_eq!(econ(&s), (0, 1, 2, 3));
    assert!(!pay_cost(&mut s, &k, env(), Seat(0), Cost::TradeGoods(4)));
    assert!(!pay_cost(&mut s, &k, env(), Seat(0), Cost::Tokens(2)));
}

#[test]
fn a_spend_cost_exhausts_planets_and_uses_goods_and_refuses_what_cannot_be_covered() {
    covers!("kernel::fx_pay::pay_first_plan");
    let k = kernel_all();
    let planets = planets_with_influence(&[2, 1]);
    let base = Scenario::lobby(2).map_from(&systems_of(&planets)).control(planets[0], 0).control(planets[1], 0);
    let mut s = base.clone().state();
    assert!(pay_cost(&mut s, &k, env(), Seat(0), Cost::Spend { amount: 3, kind: Spend::Influence }), "2 + 1 influence from both planets");
    for p in &planets {
        assert!(s.board().planet(*p).flags.has(ti4_engine_core::state::PlanetFlags::EXHAUSTED), "{p:?}");
    }
    let mut poor = base.clone().state();
    let before = poor.clone();
    assert!(!pay_cost(&mut poor, &k, env(), Seat(0), Cost::Spend { amount: 9, kind: Spend::Influence }));
    assert!(poor == before, "nothing is spent when the amount cannot be covered");
}

#[test]
fn destroy_heretical_works_takes_two_fragments_and_drops_spent_kinds() {
    covers!("kernel::fx_pay::pay_secret");
    let mut s = Scenario::lobby(2).state();
    {
        let f = &mut s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.fragments;
        f.set(ExploreType::Cultural, 1);
        f.set(ExploreType::Hazardous, 2);
        f.set(ExploreType::Industrial, 0);
    }
    assert!(pay_secret(&mut s, Seat(0), SecretObjective::DestroyHereticalWorks));
    let f = s.players().by_seat(Seat(0)).unwrap().cards.fragments;
    assert_eq!(f.get(ExploreType::Cultural), None, "spent");
    assert_eq!(f.get(ExploreType::Hazardous), Some(1));
    assert_eq!(f.get(ExploreType::Industrial), None, "a kind at zero leaves the map even when untouched");
    let mut few = Scenario::lobby(2).state();
    few.players_mut().by_seat_mut(Seat(0)).unwrap().cards.fragments.set(ExploreType::Cultural, 1);
    let before = few.clone();
    assert!(!pay_secret(&mut few, Seat(0), SecretObjective::DestroyHereticalWorks));
    assert!(few == before);
}

#[test]
fn form_a_spy_network_drops_the_first_five_cards_and_every_other_secret_is_free() {
    covers!("kernel::fx_pay::pay_secret");
    let mut s = Scenario::lobby(2).hand(0, &[ActionCard::Spy, ActionCard::Sabotage1, ActionCard::Sabotage2, ActionCard::Sabotage3, ActionCard::Sabotage4, ActionCard::Spy]).state();
    assert!(pay_secret(&mut s, Seat(0), SecretObjective::FormASpyNetwork));
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice(), &[ActionCard::Spy]);
    assert!(s.cards().discarded_action.as_slice().is_empty(), "dropped, not discarded");
    assert!(!pay_secret(&mut s, Seat(0), SecretObjective::FormASpyNetwork), "one card left");
    let free = SecretObjective::ALL.into_iter().find(|o| !matches!(o, SecretObjective::DestroyHereticalWorks | SecretObjective::FormASpyNetwork)).unwrap();
    assert!(pay_secret(&mut s, Seat(0), free), "every other secret is free");
    assert!(!pay_secret(&mut s, Seat(9), SecretObjective::FormASpyNetwork));
}
