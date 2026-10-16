//! Backward planning for the objectives that are bought: "spend N resources / influence / trade
//! goods / command tokens" (Erect a Monument, Found a Golden Age, Sway the Council, Manipulate
//! Galactic Law, Negotiate Trade Routes, Centralize Galactic Trade, Lead From the Front,
//! Galvanize the People, Amass Wealth, Hold Vast Reserves).
//!
//! These are the objectives the rule bot scores most (93% of the seats for the stage I ones)
//! without any steering, because the price is paid from the whole empire and needs no board
//! state. The planner therefore models them honestly as cheap: a bought objective that the seat
//! can pay now is reachable with probability close to one, and the only steer is to **keep the
//! price unspent until the status phase** (the [`Reserve`]); one that is a little short may be
//! helped by a strategy card (Trade for trade goods, Leadership for tokens). Each plan carries
//! the reserve of its own price; the weighing between plans (and the payment discount) is the
//! framework's.

use ti4_engine::choice::SeatObservation;
use ti4_engine::objectives::Cost as Price;
use ti4_model::id::ObjectiveId;

use super::{Cost, ObjectivePlanner, Plan, PlanCtx, Steer};
use crate::objectives::{Goal, Reserve, pool_tokens, trade_goods};

/// The bought objectives of the corpus.
pub const IDS: &[&str] = &[
    "monument",
    "golden_age",
    "sway_council",
    "manipulate_law",
    "trade_routes",
    "centralize_trade",
    "lead",
    "galvanize",
    "amass_wealth",
    "vast_reserves",
];

/// Probability that a price payable now is still payable in the status phase: the planets and
/// goods are only at risk from forced payments (production beyond the reserve, a secondary).
pub const HELD_P: f64 = 0.95;
/// A strategy card that closes a gap of at most this many price units (trade goods or tokens).
pub const CARD_REACH: f64 = 3.0;
/// Probability the card closes the gap once drafted (others may take it first).
pub const CARD_P: f64 = 0.45;

/// What the seat holds that a price draws on.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Holdings {
    pub trade_goods: i64,
    pub tokens: i64,
}

/// The plan of one bought goal, or `None` when it is no bought objective of the seat's, already
/// scored, or out of reach for good.
#[must_use]
pub fn plan_for(goal: &Goal, hold: Holdings) -> Option<Plan> {
    if !goal.bought || goal.secret {
        return None;
    }
    let price = ti4_engine::objectives::cost_of(&ObjectiveId::new(&goal.alias))?;
    let reserve = Reserve::for_cost(price);
    #[expect(clippy::cast_precision_loss, reason = "a handful of units")]
    let (tokens, resources) = match price {
        Price::Tokens(n) => (f64::from(n), 0.0),
        Price::Spend { amount, .. } => (0.0, amount as f64),
        Price::TradeGoods(n) => (0.0, f64::from(n)),
        Price::AllThree(n) => (0.0, 3.0 * n as f64),
    };
    let cost = Cost {
        tokens,
        resources,
        activations: 0.0,
    };
    if goal.satisfied {
        return Some(Plan {
            objective: goal.alias.clone(),
            vp: goal.points,
            p_this_turn: HELD_P,
            cost,
            steer: Steer {
                reserve,
                ..Steer::default()
            },
            reason: format!("{} is payable now: keep the price until the status phase", goal.alias),
        });
    }
    let gap = goal.gap();
    // A card closes a small gap of trade goods or tokens.
    let card = match price {
        Price::TradeGoods(_) | Price::AllThree(_) if gap <= CARD_REACH => Some("Trade"),
        Price::Tokens(_) if gap <= CARD_REACH => Some("Leadership"),
        _ => None,
    };
    let card = card?;
    let _ = hold;
    Some(Plan {
        objective: goal.alias.clone(),
        vp: goal.points,
        p_this_turn: CARD_P,
        cost,
        steer: Steer {
            reserve: Reserve::default(),
            cards: vec![(card.to_owned(), goal.points)],
            ..Steer::default()
        },
        reason: format!("{} is {gap:.0} short: {card} would close it", goal.alias),
    })
}

/// The spend planner.
pub struct SpendPlanner;

impl SpendPlanner {
    /// Every plan, best first.
    #[must_use]
    pub fn plans(&self, obs: &SeatObservation<'_>, ctx: &PlanCtx) -> Vec<Plan> {
        let observed = obs.observed();
        let hold = Holdings {
            trade_goods: trade_goods(observed, &ctx.me),
            tokens: i64::from(pool_tokens(observed, &ctx.me)),
        };
        let mut plans: Vec<Plan> = ctx.goals.iter().filter_map(|g| plan_for(g, hold)).collect();
        plans.sort_by(|a, b| (b.vp * b.p_this_turn).total_cmp(&(a.vp * a.p_this_turn)));
        plans
    }
}

impl ObjectivePlanner for SpendPlanner {
    fn objective_ids(&self) -> &[&str] {
        IDS
    }

    fn plan(&self, obs: &SeatObservation<'_>, ctx: &PlanCtx) -> Option<Plan> {
        self.plans(obs, ctx).into_iter().next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn goal(alias: &str, points: f64, have: f64, threshold: f64) -> Goal {
        Goal {
            alias: alias.to_owned(),
            secret: false,
            points,
            bought: true,
            have,
            threshold,
            satisfied: have >= threshold,
            family: String::new(),
        }
    }

    #[test]
    fn payable_now_is_reachable_and_reserves_its_price() {
        let p = plan_for(&goal("monument", 1.0, 8.0, 8.0), Holdings::default()).unwrap();
        assert!(p.p_this_turn > 0.9);
        assert_eq!(p.steer.reserve.resources, 8);
        assert!((p.cost.resources - 8.0).abs() < 1e-9);
    }

    #[test]
    fn a_small_goods_or_token_gap_is_helped_by_a_card() {
        let p = plan_for(&goal("trade_routes", 1.0, 3.0, 5.0), Holdings::default()).unwrap();
        assert_eq!(p.steer.cards[0].0, "Trade");
        assert!(p.p_this_turn < 0.9 && p.steer.reserve.is_empty());
        let p = plan_for(&goal("lead", 1.0, 1.0, 3.0), Holdings::default()).unwrap();
        assert_eq!(p.steer.cards[0].0, "Leadership");
    }

    #[test]
    fn a_planet_spend_gap_or_a_big_gap_is_not_planned() {
        // Resources and influence are not fetched by a card: fallback.
        assert!(plan_for(&goal("sway_council", 1.0, 3.0, 8.0), Holdings::default()).is_none());
        assert!(plan_for(&goal("centralize_trade", 2.0, 2.0, 10.0), Holdings::default()).is_none());
    }

    #[test]
    fn requirement_objectives_are_not_bought() {
        let mut g = goal("corner", 1.0, 4.0, 4.0);
        g.bought = false;
        assert!(plan_for(&g, Holdings::default()).is_none());
    }
}
