//! Atomic payments (LRR 34, 75, 47).
//!
//! Production, voting and objective costs all spend the same way: exhaust planets, spend trade
//! goods. Each had grown its own loop. This enumerates *plans* instead, so a caller can weigh
//! whole payments rather than being walked through one exhaust at a time — which is what
//! M06-001 asks for, and what a scorer needs to compare "pay with Jord" against "pay with two
//! trade goods".

use ti4_model::id::{PlanetId, PlayerId};
use ti4_model::state::GameState;

use ti4_content::ContentStore;
use ti4_model::content_types::SourceSet;

use crate::production::{Spend, planet_value_now, spendable_planets};

/// One complete way to meet a cost.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Plan {
    /// Planets to exhaust, in a stable order.
    pub planets: Vec<PlanetId>,
    /// Trade goods to spend.
    pub trade_goods: i32,
}

impl Plan {
    /// What this plan is worth towards the cost.
    #[must_use]
    pub fn worth(
        &self,
        state: &GameState,
        content: &ContentStore,
        sources: SourceSet,
        kind: Spend,
    ) -> i64 {
        let from_planets: i64 = self
            .planets
            .iter()
            .map(|planet| planet_value_now(state, content, sources, planet, kind))
            .sum();
        from_planets + i64::from(self.trade_goods)
    }

    /// [`Self::worth`] for a payer whose trade goods may be worth more than one each (Mentak
    /// Mirror Computing). Identical to `worth` when they are worth one.
    #[must_use]
    pub fn worth_for(
        &self,
        state: &GameState,
        content: &ContentStore,
        sources: SourceSet,
        player: &PlayerId,
        kind: Spend,
    ) -> i64 {
        let from_planets: i64 = self
            .planets
            .iter()
            .map(|planet| planet_value_now(state, content, sources, planet, kind))
            .sum();
        from_planets
            + i64::from(self.trade_goods)
                * crate::factions::hooks_economy::trade_good_worth(state, player, 1)
    }
}

/// How many planets a single plan will consider. A cost of ten from a dozen readied planets
/// has thousands of subsets, and nothing in the game needs the exhaustive list — it needs a
/// handful of sensible ones.
pub const MAX_PLANETS_PER_PLAN: usize = 6;

/// Distinct ways to pay `cost`, cheapest first.
///
/// Each plan is *minimal*: dropping any part of it would leave the cost unmet. A plan that
/// exhausts a planet it did not need is not a different way to pay, it is the same way plus
/// waste, and offering it would make a decider likelier to overpay purely because more
/// wasteful plans exist.
#[must_use]
pub fn plans(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    cost: i64,
    kind: Spend,
) -> Vec<Plan> {
    if cost <= 0 {
        return vec![Plan::default()];
    }
    // What one held trade good pays *as modules change it*: one with no module acting. Not
    // `production::trade_good_worth`, which also carries the hard-coded `mc` technology that
    // this path has never honoured (kept neutral; the Mentak package moves `mc` onto the hook).
    let per_good = crate::factions::hooks_economy::trade_good_worth(state, player, 1).max(1);
    let goods = state
        .player(player)
        .map_or(0, |seat| i64::from(seat.trade_goods))
        * per_good;

    // Planets worth nothing towards this cost cannot help, and including them would generate
    // plans that differ only by a planet that paid nothing.
    let mut useful: Vec<(PlanetId, i64)> = spendable_planets(state, player)
        .into_iter()
        .map(|planet| {
            let worth = planet_value_now(state, content, sources, &planet, kind);
            (planet, worth)
        })
        .filter(|(_, worth)| *worth > 0)
        .collect();
    useful.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    useful.truncate(MAX_PLANETS_PER_PLAN);

    let mut found: Vec<Plan> = Vec::new();
    let count = useful.len();
    for mask in 0u32..(1u32 << count) {
        let chosen: Vec<&(PlanetId, i64)> = (0..count)
            .filter(|index| mask & (1 << index) != 0)
            .map(|index| &useful[index])
            .collect();
        let from_planets: i64 = chosen.iter().map(|(_, worth)| *worth).sum();
        if from_planets >= cost + chosen.last().map_or(0, |(_, worth)| *worth) && !chosen.is_empty()
        {
            continue; // the last planet was not needed
        }
        let shortfall = (cost - from_planets).max(0);
        if shortfall > goods {
            continue;
        }
        found.push(Plan {
            planets: chosen.iter().map(|(planet, _)| planet.clone()).collect(),
            // Whole goods, rounded up: two goods worth two each pay a bill of three.
            trade_goods: i32::try_from((shortfall + per_good - 1) / per_good).unwrap_or(i32::MAX),
        });
    }

    // Cheapest first: fewest trade goods, then fewest planets, then stable by name.
    found.sort_by(|a, b| {
        a.trade_goods
            .cmp(&b.trade_goods)
            .then_with(|| a.planets.len().cmp(&b.planets.len()))
            .then_with(|| a.planets.cmp(&b.planets))
    });
    found.dedup();
    found
}

/// Whether this player can meet a cost at all.
#[must_use]
pub fn affordable(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    cost: i64,
    kind: Spend,
) -> bool {
    !plans(state, content, sources, player, cost, kind).is_empty()
}

/// Spend a plan. All of it lands or none of it does.
///
/// Returns `false` without changing anything when the plan is no longer payable — a plan built
/// against an older state must not half-apply.
pub fn apply(state: &mut GameState, player: &PlayerId, plan: &Plan) -> bool {
    let payable = state.player(player).is_some_and(|seat| {
        seat.trade_goods >= plan.trade_goods
            && plan
                .planets
                .iter()
                .all(|planet| !state.exhausted_planets.contains(planet))
    });
    if !payable {
        return false;
    }
    if let Some(seat) = state.player_mut(player) {
        seat.trade_goods -= plan.trade_goods;
    }
    for planet in &plan.planets {
        state.exhaust_planet(planet.clone());
    }
    true
}

#[cfg(test)]
mod tests {
    use ti4_model::content_types::POK;

    use super::*;
    use crate::fixtures::{a_placed_planet, game};

    fn player() -> PlayerId {
        PlayerId::new("a")
    }

    /// Give the player a planet worth something, and report what.
    fn give_planet(state: &mut GameState) -> (PlanetId, i64) {
        let (system, planet) = a_placed_planet();
        state
            .system_mut(&system)
            .set_control(planet.clone(), player());
        let worth = crate::production::planet_value(
            ContentStore::embedded(),
            POK,
            &planet,
            Spend::Resources,
        );
        (planet, worth)
    }

    #[test]
    fn a_free_cost_has_one_empty_plan() {
        let state = game(&["a"]);
        assert_eq!(
            plans(
                &state,
                ContentStore::embedded(),
                POK,
                &player(),
                0,
                Spend::Resources
            ),
            vec![Plan::default()]
        );
    }

    #[test]
    fn trade_goods_alone_can_pay() {
        let mut state = game(&["a"]);
        state.player_mut(&player()).unwrap().trade_goods = 3;

        let found = plans(
            &state,
            ContentStore::embedded(),
            POK,
            &player(),
            2,
            Spend::Resources,
        );

        assert!(!found.is_empty());
        assert_eq!(found[0].trade_goods, 2);
        assert!(found[0].planets.is_empty());
    }

    #[test]
    fn an_unaffordable_cost_has_no_plans() {
        let state = game(&["a"]);
        assert!(
            plans(
                &state,
                ContentStore::embedded(),
                POK,
                &player(),
                99,
                Spend::Resources
            )
            .is_empty()
        );
        assert!(!affordable(
            &state,
            ContentStore::embedded(),
            POK,
            &player(),
            99,
            Spend::Resources
        ));
    }

    #[test]
    fn a_planet_worth_nothing_towards_the_cost_is_not_offered() {
        // Otherwise every plan would have a twin differing only by a planet that paid nothing.
        let mut state = game(&["a"]);
        state.player_mut(&player()).unwrap().trade_goods = 2;
        let (planet, worth) = give_planet(&mut state);
        if worth > 0 {
            return; // this fixture planet does pay, so the case does not arise here
        }

        let found = plans(
            &state,
            ContentStore::embedded(),
            POK,
            &player(),
            1,
            Spend::Resources,
        );
        assert!(found.iter().all(|plan| !plan.planets.contains(&planet)));
    }

    #[test]
    fn plans_are_minimal() {
        // Dropping any part of a plan would leave the cost unmet. A plan that exhausts a planet
        // it did not need is the same payment plus waste, and offering it makes a sampling
        // decider likelier to overpay purely because wasteful plans outnumber lean ones.
        let mut state = game(&["a"]);
        state.player_mut(&player()).unwrap().trade_goods = 5;
        let (_, worth) = give_planet(&mut state);
        if worth == 0 {
            return;
        }

        for plan in plans(
            &state,
            ContentStore::embedded(),
            POK,
            &player(),
            1,
            Spend::Resources,
        ) {
            let total = plan.worth(&state, ContentStore::embedded(), POK, Spend::Resources);
            assert!(total >= 1, "it pays the bill");
            if let Some(last) = plan.planets.last() {
                let without = total
                    - crate::production::planet_value(
                        ContentStore::embedded(),
                        POK,
                        last,
                        Spend::Resources,
                    );
                assert!(without < 1, "the last planet was needed");
            }
        }
    }

    #[test]
    fn the_cheapest_plan_comes_first() {
        let mut state = game(&["a"]);
        state.player_mut(&player()).unwrap().trade_goods = 5;
        let (_, worth) = give_planet(&mut state);
        if worth == 0 {
            return;
        }

        let found = plans(
            &state,
            ContentStore::embedded(),
            POK,
            &player(),
            worth,
            Spend::Resources,
        );
        assert!(!found.is_empty());
        assert_eq!(
            found[0].trade_goods, 0,
            "paying with the planet beats spending goods"
        );
    }

    #[test]
    fn applying_a_plan_exhausts_and_spends() {
        let mut state = game(&["a"]);
        state.player_mut(&player()).unwrap().trade_goods = 2;
        let (planet, worth) = give_planet(&mut state);
        if worth == 0 {
            return;
        }
        let plan = Plan {
            planets: vec![planet.clone()],
            trade_goods: 1,
        };

        assert!(apply(&mut state, &player(), &plan));
        assert!(state.exhausted_planets.contains(&planet));
        assert_eq!(state.player(&player()).unwrap().trade_goods, 1);
    }

    #[test]
    fn a_stale_plan_applies_nothing() {
        // A plan built against an older state must not half-apply.
        let mut state = game(&["a"]);
        state.player_mut(&player()).unwrap().trade_goods = 2;
        let (planet, _) = give_planet(&mut state);
        state.exhaust_planet(planet.clone()); // somebody else spent it first
        let before = state.clone();

        let plan = Plan {
            planets: vec![planet],
            trade_goods: 1,
        };
        assert!(!apply(&mut state, &player(), &plan));
        assert!(state.identical(&before), "nothing was spent");
    }

    #[test]
    fn a_plan_beyond_the_purse_applies_nothing() {
        let mut state = game(&["a"]);
        state.player_mut(&player()).unwrap().trade_goods = 1;
        let before = state.clone();

        let plan = Plan {
            planets: Vec::new(),
            trade_goods: 5,
        };
        assert!(!apply(&mut state, &player(), &plan));
        assert!(state.identical(&before));
    }
    /// Mentak Mirror Computing reaches votes and objective costs, which pay through `plans`: with
    /// each good worth two, three influence costs two goods, and three goods can pay six.
    #[test]
    fn a_trade_good_worth_hook_changes_what_goods_cost_in_a_plan() {
        use crate::factions::hooks_economy::{EconomyHooks, with_test_hooks};
        let mut state = game(&["a"]);
        state.player_mut(&player()).unwrap().trade_goods = 2;
        let content = ContentStore::embedded();
        let ask = |state: &GameState, cost: i64| {
            plans(state, content, POK, &player(), cost, Spend::Influence)
        };
        assert!(
            ask(&state, 3).is_empty(),
            "two goods of one cannot pay three"
        );
        let doubled = EconomyHooks {
            trade_good_worth: Some(|_, _, worth| worth * 2),
            ..EconomyHooks::NONE
        };
        with_test_hooks(doubled, || {
            let found = ask(&state, 3);
            assert_eq!(found.len(), 1);
            assert_eq!(found[0].trade_goods, 2, "rounded up to whole goods");
            assert_eq!(
                found[0].worth_for(&state, content, POK, &player(), Spend::Influence),
                4
            );
            assert_eq!(ask(&state, 4)[0].trade_goods, 2);
            assert_eq!(ask(&state, 2)[0].trade_goods, 1);
            assert!(ask(&state, 5).is_empty());
        });
    }
    /// `plans` behaves exactly as before while no module acts: the hard-coded `mc` technology is
    /// honoured by production payment only, not here (the Mentak package moves it onto the hook).
    #[test]
    fn mirror_computing_doubles_trade_goods_in_every_payment() {
        // Mirror Computing (Mentak): "When you spend trade goods, each trade good is worth 2
        // resources or influence instead of 1." Implemented once, by the Mentak module through the
        // `trade_good_worth` hook, so production and every other payment agree.
        let mut state = game(&["a"]);
        {
            let seat = state.player_mut(&player()).unwrap();
            seat.trade_goods = 2;
            seat.technologies
                .insert(ti4_model::id::TechnologyId::new("mc"));
        }
        let content = ContentStore::embedded();
        let found = plans(&state, content, POK, &player(), 3, Spend::Influence);
        assert!(!found.is_empty(), "two goods now pay up to four");
        assert!(found.iter().all(|plan| plan.trade_goods <= 2));
        let two = plans(&state, content, POK, &player(), 4, Spend::Influence);
        assert!(
            two.iter().any(|plan| plan.trade_goods == 2
                && plan.worth_for(&state, content, POK, &player(), Spend::Influence) >= 4),
            "two goods are worth four"
        );
        state
            .player_mut(&player())
            .unwrap()
            .technologies
            .remove(&ti4_model::id::TechnologyId::new("mc"));
        assert!(
            plans(&state, content, POK, &player(), 3, Spend::Influence).is_empty(),
            "without the technology two goods pay two"
        );
    }
}
