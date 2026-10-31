//! `fx_pay`: the deterministic payments (package A4b-1): what a bought objective, a priced secret objective
//! and any other "pay the first plan" site charges, as writes on the state.
//!
//! The question-driven payment (the player picks the planets) is `rules::production::pay`. These are the
//! payments the string engine takes without asking: [`objectives::pay_for`] pays the first plan
//! `payment::plans` lists, spending trade goods with the commodities a Keleres agent made spendable first,
//! and [`secrets::pay_for`] takes two relic fragments or five action cards. Every function answers `false`
//! with nothing changed when the price cannot be met (an all-or-nothing payment: the sites that pay and
//! then score check the answer before they score).
//!
//! The terms the price reads (what a trade good pays, spendable commodities, Deepwrought ocean cards, the
//! Helios dock bonus) come from the kernel's hook table, so a window that pays runs under the production
//! kernel. What the table cannot hold, the content scope and Law's Order, is the caller's [`PayEnv`].
//!
//! Copied on purpose (pure refactor): the five cards of Form a Spy Network are dropped, not discarded;
//! the fragments are taken in the key order of the string map (cultural, frontier, hazardous, industrial)
//! and every key left at zero leaves the map.

use crate::generated::{ExploreType, SecretObjective};
use crate::ids::Seat;
use crate::kernel::{fx, Kernel};
use crate::query::board;
use crate::query::economy::Spend;
use crate::query::objectives::{self as q, Cost, PlanC, SeatFacts, Terms};
use crate::state::CoreState;
use crate::tables::Scope;

/// What a payment reads that is neither in the state nor in the hook table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PayEnv {
    /// The content scope of the game (`core_adapter::scope(sources)`).
    pub scope: Scope,
    /// Law's Order (`laws::blanked`), computed by the caller.
    pub blanked: bool,
}

/// The seat's payment facts: the board scan with the hook terms of a payment.
#[must_use]
pub fn pay_facts(core: &CoreState, k: &Kernel, env: PayEnv, seat: Seat) -> SeatFacts {
    let mut terms = Terms { blanked: env.blanked, ..Terms::default() };
    terms.per_good = k.hooks.economy.trade_good_worth(core, seat, 1).max(1);
    terms.commodities = k.hooks.direct.keleres_spendable_commodities(core, seat);
    terms.oceans = !k.hooks.direct.deepwrought_spendable_oceans(core, seat).is_empty();
    for (_, planet) in board::controlled_planets(core, seat).as_slice() {
        let bonus = k.hooks.direct.bastion_units_resource_bonus(core, *planet);
        if bonus != 0 {
            terms.helios.push((*planet, bonus));
        }
    }
    SeatFacts::of(core, env.scope, seat, terms)
}

/// Whether `amount` resources or influence can be paid now (`production::available` at least `amount`): a plan of `payment::plans` exists.
#[must_use]
pub fn can_afford(core: &CoreState, k: &Kernel, env: PayEnv, seat: Seat, kind: Spend, amount: i64) -> bool {
    let f = pay_facts(core, k, env, seat);
    q::first_plan(core, &f, kind, amount).is_some()
}

/// `payment::apply` of a plan: commodities first, then trade goods, then the planets are exhausted. `false`,
/// changing nothing, when the seat cannot cover the goods or a planet of the plan is already exhausted.
fn apply_plan(core: &mut CoreState, f: &SeatFacts, plan: &PlanC) -> bool {
    let commodities = i32::try_from(f.terms.commodities).unwrap_or(0);
    let Some(p) = core.players().by_seat(f.seat) else { return false };
    let payable = i32::from(p.econ.trade_goods) + commodities >= plan.trade_goods
        && plan.planets().iter().all(|planet| !board::exhausted(core, *planet));
    if !payable {
        return false;
    }
    let from_commodities = plan.trade_goods.min(commodities);
    if let Some(p) = core.players_mut().by_seat_mut(f.seat) {
        p.econ.commodities = p.econ.commodities.saturating_sub(i16::try_from(from_commodities).unwrap_or(i16::MAX));
        p.econ.trade_goods = p.econ.trade_goods.saturating_sub(i16::try_from(plan.trade_goods - from_commodities).unwrap_or(i16::MAX));
    }
    for planet in plan.planets() {
        fx::exhaust_planet(core, *planet);
    }
    true
}

/// `objectives::pay_for` of `Cost::Spend`: pay `amount` resources or influence with the first plan `payment::plans`
/// lists. `false` (nothing changed) when no plan pays it, and while the seat holds ocean cards.
pub fn pay_first_plan(core: &mut CoreState, k: &Kernel, env: PayEnv, seat: Seat, kind: Spend, amount: i64) -> bool {
    let f = pay_facts(core, k, env, seat);
    first_plan_with(core, &f, kind, amount)
}

fn first_plan_with(core: &mut CoreState, f: &SeatFacts, kind: Spend, amount: i64) -> bool {
    match q::first_plan(core, f, kind, amount) {
        Some(plan) => apply_plan(core, f, &plan),
        None => false,
    }
}

/// `objectives::pay_for` of `Cost::AllThree`: `amount` resources, `amount` influence from other planets, and
/// `amount` trade goods besides, or nothing.
pub fn pay_all_three(core: &mut CoreState, k: &Kernel, env: PayEnv, seat: Seat, amount: i64) -> bool {
    let f = pay_facts(core, k, env, seat);
    all_three_with(core, &f, amount)
}

fn all_three_with(core: &mut CoreState, f: &SeatFacts, amount: i64) -> bool {
    let Some((resources, influence)) = q::all_three_plan(core, f, amount) else { return false };
    if !apply_plan(core, f, &resources) || !apply_plan(core, f, &influence) {
        return false;
    }
    if let Some(p) = core.players_mut().by_seat_mut(f.seat) {
        p.econ.trade_goods = p.econ.trade_goods.saturating_sub(i16::try_from(amount).unwrap_or(i16::MAX));
    }
    true
}

/// `objectives::pay_for` of `Cost::Tokens`: strategy tokens first, then tactic tokens, never the fleet pool.
pub fn pay_tokens(core: &mut CoreState, seat: Seat, amount: i32) -> bool {
    if core.players().by_seat(seat).is_none() || q::token_pool(core, seat) < amount {
        return false;
    }
    let Some(p) = core.players_mut().by_seat_mut(seat) else { return false };
    let mut owed = amount;
    for pool in [&mut p.econ.strategic_tokens, &mut p.econ.tactic_tokens] {
        let take = i32::from(*pool).min(owed);
        *pool -= i16::try_from(take).unwrap_or(0);
        owed -= take;
    }
    owed == 0
}

/// `objectives::pay_for` of `Cost::TradeGoods`: plain trade goods, no commodities.
pub fn pay_trade_goods(core: &mut CoreState, seat: Seat, amount: i32) -> bool {
    match core.players_mut().by_seat_mut(seat) {
        Some(p) if i32::from(p.econ.trade_goods) >= amount => {
            p.econ.trade_goods = p.econ.trade_goods.saturating_sub(i16::try_from(amount).unwrap_or(i16::MAX));
            true
        }
        _ => false,
    }
}

/// `objectives::pay_for`: the price of a bought objective.
pub fn pay_cost(core: &mut CoreState, k: &Kernel, env: PayEnv, seat: Seat, cost: Cost) -> bool {
    match cost {
        Cost::Spend { amount, kind } => pay_first_plan(core, k, env, seat, kind, amount),
        Cost::TradeGoods(amount) => pay_trade_goods(core, seat, amount),
        Cost::AllThree(amount) => pay_all_three(core, k, env, seat, amount),
        Cost::Tokens(amount) => pay_tokens(core, seat, amount),
    }
}

/// `secrets::pay_for`: Destroy Heretical Works takes two relic fragments, Form a Spy Network the first five
/// action cards of the hand (dropped, not discarded). Every other secret is free. `false`, nothing changed,
/// when the seat cannot pay.
pub fn pay_secret(core: &mut CoreState, seat: Seat, secret: SecretObjective) -> bool {
    match secret {
        SecretObjective::DestroyHereticalWorks => {
            let Some(p) = core.players_mut().by_seat_mut(seat) else { return false };
            let frags = &mut p.cards.fragments;
            let held: i32 = ExploreType::ALL.iter().filter_map(|t| frags.get(*t)).filter(|n| *n > 0).map(i32::from).sum();
            if held < 2 {
                return false;
            }
            let mut owed = 2i16;
            for kind in ExploreType::ALL {
                if let Some(n) = frags.get(kind) {
                    let take = n.max(0).min(owed);
                    frags.set(kind, n - take);
                    owed -= take;
                }
            }
            // `retain(|_, held| *held > 0)`: a key at zero leaves the map, even one the payment did not touch.
            for kind in ExploreType::ALL {
                if frags.get(kind).is_some_and(|n| n <= 0) {
                    frags.remove(kind);
                }
            }
            owed == 0
        }
        SecretObjective::FormASpyNetwork => {
            let Some(p) = core.players_mut().by_seat_mut(seat) else { return false };
            if p.cards.hand.len() < 5 {
                return false;
            }
            for _ in 0..5 {
                p.cards.hand.remove(0);
            }
            true
        }
        _ => true,
    }
}
