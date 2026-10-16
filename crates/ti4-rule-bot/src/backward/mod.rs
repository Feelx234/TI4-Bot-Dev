//! Backward planning (bot version 4, experimental; every knob `bw_*` in [`crate::tuning`], default off).
//!
//! From round [`tuning::bw_round`] on, the seat looks at the objectives it can still score (the
//! revealed publics and, where cheap, its held secrets) and plans its turn *backwards* from
//! each: what must be true, what is the cheapest route to it this turn (activations, which
//! planets or systems, which fleet), how likely is that route to work, and what does it cost.
//! One [`ObjectivePlanner`] per objective group answers that for the goals it knows, each in its
//! own source file (see [`registry`]):
//!
//! | file | goals |
//! |---|---|
//! | [`territory`] | control N planets, trait / specialty / outside-home / rim / attached planets |
//! | [`fleet`] | ships in N systems, planetless systems, next to Mecatol Rex / an anomaly / a rival home, fleet size |
//! | `payment` (Agent B) | spend resources, influence, trade goods, tokens |
//! | `technology` (Agent B) | techs, colours, unit upgrades |
//! | `combat` (Agent B) | kills, rival systems, everything else |
//! | [`mecatol`] | Mecatol Rex: the custodians' point and Imperial's primary point |
//!
//! [`verdict`] collects the plans, weighs them ([`weigh`]) and returns a [`Verdict`]:
//!
//! - **expected VP** of a plan = probability this turn x marginal points - the tokens and
//!   resources it costs; a public objective beyond the one the seat could already score this
//!   round only counts a quarter unless Imperial's extra scoring is in play (as
//!   [`crate::objectives::gain`]);
//! - **Imperial**: when at least [`tuning::bw_likely`]-probable plans for two public objectives
//!   exist, the Imperial card is wanted (it scores a second public objective) and each such plan
//!   gains [`tuning::bw_imperial_bonus`];
//! - **payment objectives** ("spend N ...") are discounted by [`tuning::bw_payment_discount`]:
//!   they are usually scoreable whatever the board looks like and need no steering;
//! - the Mecatol Rex / Imperial plan competes as one more candidate;
//! - **fallback**: when the best plan's expected VP is below [`tuning::bw_threshold`] the verdict
//!   is empty and every default rule plays unchanged.
//!
//! The chosen plans steer the *activation* (a system in a plan's route gets
//! `bw_steer x expected VP` extra worth, [`Verdict::bonus`]) and the strategy-card choice
//! (Imperial). Nothing here ever picks an option that is not offered.

use std::collections::BTreeMap;

use ti4_engine::choice::{Observed, SeatObservation};
use ti4_engine::fleet_strength::matchup;
use ti4_engine::objectives::Imagined;
use ti4_model::id::{PlanetId, PlayerId, SystemId};

use crate::objectives::{self, Goal};
use crate::tuning;

pub mod fleet;
pub mod mecatol;
pub mod payment;
pub mod territory;

/// What a plan costs this turn.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Cost {
    /// Command tokens (tactic tokens for activations, others spent).
    pub tokens: i32,
    /// Resources spent (production, payments).
    pub resources: i64,
    /// Tactical activations.
    pub activations: u32,
}

/// What the plan wants the seat to do.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Steer {
    /// Systems to activate (and move or land in).
    pub systems: Vec<SystemId>,
    /// Unit type ids worth producing for it.
    pub builds: Vec<String>,
    /// Strategy card names worth taking or playing (e.g. "Imperial", "Technology").
    pub cards: Vec<String>,
}

/// One plan: the cheapest route found to score one objective, and how it weighs.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// The objective alias (or `"mecatol"` for the Mecatol / Imperial plan).
    pub objective: String,
    /// Victory points it is worth.
    pub points: f64,
    /// Probability that the steer is carried out and the objective is met by the end of this
    /// round's action phase.
    pub probability: f64,
    /// What it costs.
    pub cost: Cost,
    /// What to do.
    pub steer: Steer,
    /// For logs and tests.
    pub reason: String,
    /// A "spend N ..." objective (discounted in [`weigh`]).
    pub payment: bool,
    /// A held secret rather than a revealed public.
    pub secret: bool,
}

/// One activation the seat could make this turn, as far as an objective cares.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub system: SystemId,
    /// Planets the activation would take (greedy, as the activation value counts them).
    pub planets: Vec<PlanetId>,
    /// A ship of the seat would be in the system afterwards where none is now.
    pub ship: bool,
    /// Whether a ship could come from a system that keeps another ship.
    pub spare_ship: bool,
    /// Probability that the step does what it says (fight won, landing succeeds, ship comes).
    pub success: f64,
}

/// Mecatol Rex as the planners see it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MecatolState {
    pub system: Option<SystemId>,
    pub held: bool,
    pub custodians_removed: bool,
    pub takeable: bool,
    pub in_reach: bool,
    pub rival_holds: bool,
}

/// Everything a planner needs about the position and the seat's means this turn.
pub struct PlanCtx {
    pub me: PlayerId,
    pub round: u32,
    /// The goal being planned (set by the dispatcher before each call of
    /// [`ObjectivePlanner::plan`]; planners plan exactly this goal).
    pub goal: Goal,
    /// Every goal of the seat (publics, secrets), unmet and met.
    pub goals: Vec<Goal>,
    pub tactic_tokens: i32,
    pub strategic_tokens: i32,
    pub fleet_tokens: i32,
    pub resources: i64,
    pub influence: i64,
    pub trade_goods: i64,
    pub mecatol: MecatolState,
    /// Every activation worth considering (own-token systems and unwinnable fights left out).
    pub steps: Vec<Step>,
}

/// The cheapest set of steps found for a goal.
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    /// Indices into [`PlanCtx::steps`].
    pub steps: Vec<usize>,
    /// Product of the steps' success, with a small discount per extra step.
    pub probability: f64,
}

/// A planner for one objective group.
pub trait ObjectivePlanner {
    /// The objective aliases or family tokens (see `ti4_engine::objectives::family_token`) it
    /// plans. The dispatcher calls [`Self::plan`] once for each of the seat's goals that matches.
    fn objective_ids(&self) -> &[&str];
    /// The plan for `ctx.goal`, or `None` when the group has nothing to propose (met already,
    /// out of reach, no way to steer).
    fn plan(&self, obs: &SeatObservation<'_>, ctx: &PlanCtx) -> Option<Plan>;
}

/// Every planner. Agent B registers its groups here, one line each.
#[must_use]
pub fn registry() -> Vec<Box<dyn ObjectivePlanner>> {
    let mut out: Vec<Box<dyn ObjectivePlanner>> = Vec::new();
    if tuning::bw_territory() {
        out.push(Box::new(territory::Territory));
    }
    if tuning::bw_fleet() {
        out.push(Box::new(fleet::Fleet));
    }
    if tuning::bw_payment() {
        out.push(Box::new(payment::Payment));
    }
    out
}

impl Goal {
    /// Whether `ids` names this goal by alias or by family token.
    #[must_use]
    pub fn matches_any(&self, ids: &[&str]) -> bool {
        ids.iter().any(|id| *id == self.alias || *id == self.family)
    }
}

impl PlanCtx {
    /// Build the context for the seat.
    #[must_use]
    pub fn new(seen: &SeatObservation<'_>, me: &PlayerId) -> Self {
        let observed = seen.observed();
        let seat = observed.seat(me);
        let mecatol = mecatol::state(observed, me);
        Self {
            me: me.clone(),
            round: observed.round(),
            goal: Goal {
                alias: String::new(),
                secret: false,
                points: 0.0,
                bought: false,
                have: 0.0,
                threshold: 1.0,
                satisfied: false,
                family: String::new(),
            },
            goals: objectives::goals(seen, me, &Imagined::NONE),
            tactic_tokens: seat.as_ref().map_or(0, |s| s.tactic_tokens),
            strategic_tokens: seat.as_ref().map_or(0, |s| s.strategic_tokens),
            fleet_tokens: seat.as_ref().map_or(0, |s| s.fleet_tokens),
            resources: observed.available_spend(me, ti4_engine::production::Spend::Resources),
            influence: observed.available_spend(me, ti4_engine::production::Spend::Influence),
            trade_goods: objectives::trade_goods(observed, me),
            mecatol,
            steps: steps(seen, me),
        }
    }

    /// Whether the seat still holds a public objective it could score this round already.
    #[must_use]
    pub fn public_ready(&self) -> f64 {
        self.goals
            .iter()
            .filter(|g| !g.secret && g.satisfied)
            .map(|g| g.points)
            .fold(0.0, f64::max)
    }

    /// The cheapest route (fewest steps, then best probability) of at most `max_steps` of the
    /// steps `useful` accepts that makes the goal `alias` met, judged by the engine's own
    /// requirement functions on the imagined position.
    pub fn route(
        &self,
        seen: &SeatObservation<'_>,
        alias: &str,
        max_steps: usize,
        useful: impl Fn(&Step) -> bool,
    ) -> Option<Route> {
        let max_steps = max_steps
            .min(usize::try_from(self.tactic_tokens).unwrap_or(0))
            .min(3);
        if max_steps == 0 {
            return None;
        }
        let mut pool: Vec<usize> = (0..self.steps.len())
            .filter(|&i| useful(&self.steps[i]))
            .collect();
        pool.sort_by(|&a, &b| {
            self.steps[b]
                .success
                .total_cmp(&self.steps[a].success)
                .then(a.cmp(&b))
        });
        pool.truncate(7);
        for size in 1..=max_steps.min(pool.len()) {
            let mut best: Option<Route> = None;
            for combo in combinations(&pool, size) {
                if self.meets(seen, alias, &combo) {
                    let p = combo
                        .iter()
                        .map(|&i| self.steps[i].success)
                        .product::<f64>()
                        * 0.9_f64.powi(i32::try_from(size).unwrap_or(1) - 1);
                    if best.as_ref().is_none_or(|b| p > b.probability) {
                        best = Some(Route {
                            steps: combo,
                            probability: p,
                        });
                    }
                }
            }
            if best.is_some() {
                return best;
            }
        }
        None
    }

    /// Whether goal `alias` is met after the steps `combo` all happened.
    #[must_use]
    pub fn meets(&self, seen: &SeatObservation<'_>, alias: &str, combo: &[usize]) -> bool {
        let planets: Vec<PlanetId> = combo
            .iter()
            .flat_map(|&i| self.steps[i].planets.iter().cloned())
            .collect();
        let systems: Vec<String> = combo
            .iter()
            .filter(|&&i| self.steps[i].ship)
            .map(|&i| self.steps[i].system.to_string())
            .collect();
        objectives::goals(
            seen,
            &self.me,
            &Imagined {
                planets: &planets,
                systems: &systems,
                ..Imagined::default()
            },
        )
        .iter()
        .any(|g| g.alias == alias && g.satisfied)
    }

    /// The route as a plan skeleton: cost, steer and the steps' probability.
    #[must_use]
    pub fn plan_of(&self, route: &Route, reason: String) -> Plan {
        let systems: Vec<SystemId> = route
            .steps
            .iter()
            .map(|&i| self.steps[i].system.clone())
            .collect();
        Plan {
            objective: self.goal.alias.clone(),
            points: self.goal.points,
            probability: route.probability,
            cost: Cost {
                tokens: i32::try_from(route.steps.len()).unwrap_or(1),
                resources: 0,
                activations: u32::try_from(route.steps.len()).unwrap_or(1),
            },
            steer: Steer {
                systems,
                ..Steer::default()
            },
            reason,
            payment: self.goal.bought,
            secret: self.goal.secret,
        }
    }
}

/// All `size`-subsets of `pool`, in index order.
fn combinations(pool: &[usize], size: usize) -> Vec<Vec<usize>> {
    fn go(
        pool: &[usize],
        size: usize,
        from: usize,
        cur: &mut Vec<usize>,
        out: &mut Vec<Vec<usize>>,
    ) {
        if cur.len() == size {
            out.push(cur.clone());
            return;
        }
        for i in from..pool.len() {
            cur.push(pool[i]);
            go(pool, size, i + 1, cur, out);
            cur.pop();
        }
    }
    let mut out = Vec::new();
    go(pool, size, 0, &mut Vec::new(), &mut out);
    out
}

/// Probability that a fleet wins the fight in `system` (1 when nobody is there to fight).
fn fight_probability(seen: &Observed<'_>, me: &PlayerId, system: &SystemId) -> f64 {
    if !crate::has_hostile_ships(seen, me, system) {
        return 1.0;
    }
    let movable = seen.movable_into(me, system);
    let board = seen.system(system);
    let own = crate::fleet(
        seen,
        me,
        board
            .units
            .iter()
            .filter(|u| &u.owner == me && crate::is_ship(seen, u))
            .chain(movable.iter().map(|m| &m.unit)),
    );
    let enemy = crate::hostile_space(seen, me, system);
    1.0 / (1.0 + (-2.0 * matchup(&own, &enemy)).exp())
}

/// Every activation the seat could make this turn that does something, with its odds.
fn steps(seen: &SeatObservation<'_>, me: &PlayerId) -> Vec<Step> {
    let observed = seen.observed();
    let Some(galaxy) = observed.galaxy() else {
        return Vec::new();
    };
    let tokened = observed.systems_with_token(me);
    let mut out = Vec::new();
    for id in galaxy.system_ids() {
        let system = SystemId::new(id);
        if tokened.contains(&system) {
            continue;
        }
        let base = crate::activation_value(observed, me, &system);
        if base < 0.0 {
            continue;
        }
        let (ships, ground) = objectives::reach(observed, me, &system);
        let present = observed
            .system(&system)
            .units
            .iter()
            .any(|u| &u.owner == me && crate::is_ship(observed, u));
        let planets = if ground > 0 {
            objectives::taken_planets(observed, me, &system, ground)
        } else {
            Vec::new()
        };
        let ship = ships && !present;
        if planets.is_empty() && !ship {
            continue;
        }
        let spare_ship = observed.movable_into(me, &system).iter().any(|m| {
            crate::is_ship(observed, &m.unit)
                && observed
                    .system(&m.origin)
                    .units
                    .iter()
                    .filter(|u| &u.owner == me && crate::is_ship(observed, u))
                    .count()
                    >= 2
        });
        let mut success = fight_probability(observed, me, &system);
        if !planets.is_empty() {
            success *= 0.9;
        }
        if ship && !spare_ship {
            success *= 0.6;
        }
        out.push(Step {
            system,
            planets,
            ship,
            spare_ship,
            success,
        });
    }
    out
}

/// A plan with its weighed expected victory points.
#[derive(Debug, Clone, PartialEq)]
pub struct Weighed {
    pub plan: Plan,
    pub expected: f64,
}

/// The outcome of planning: weighed plans, the ones chosen and what they steer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Verdict {
    /// Every plan found, best first.
    pub plans: Vec<Weighed>,
    /// Indices into [`Self::plans`] of the plans followed this turn.
    pub chosen: Vec<usize>,
    /// Imperial is wanted (two public objectives look likely, or Mecatol Rex is in play).
    pub imperial: bool,
    /// Nothing reaches the threshold: every default rule plays as before.
    pub fallback: bool,
}

impl Verdict {
    /// Extra activation worth per system from the chosen plans.
    #[must_use]
    pub fn bonus(&self) -> BTreeMap<SystemId, f64> {
        let mut out: BTreeMap<SystemId, f64> = BTreeMap::new();
        if self.fallback {
            return out;
        }
        for &i in &self.chosen {
            let w = &self.plans[i];
            for system in &w.plan.steer.systems {
                *out.entry(system.clone()).or_default() += tuning::bw_steer() * w.expected;
            }
        }
        out
    }
}

/// Weigh `plans` (see the module docs) and choose.
#[must_use]
pub fn weigh(ctx: &PlanCtx, plans: Vec<Plan>) -> Verdict {
    let likely = tuning::bw_likely();
    let ready = ctx.public_ready();
    // Two public objectives look likely: Imperial scores the second.
    let likely_public = plans
        .iter()
        .filter(|p| !p.secret && p.objective != "mecatol" && p.probability >= likely)
        .count()
        + usize::from(ready > 0.0);
    let imperial = likely_public >= 2;
    let mut weighed: Vec<Weighed> = plans
        .into_iter()
        .map(|plan| {
            let mut value = if plan.objective == "mecatol" {
                plan.points
            } else if !plan.secret && !imperial {
                // One public scoring per round: only what beats the one already in hand.
                (plan.points - ready).max(0.25 * plan.points)
            } else {
                plan.points
            };
            if plan.payment {
                value *= 1.0 - tuning::bw_payment_discount();
            }
            let mut expected = plan.probability * value
                - tuning::bw_token_cost() * f64::from(plan.cost.tokens)
                - tuning::bw_resource_cost() * plan.cost.resources as f64;
            if imperial && !plan.secret && plan.probability >= likely && plan.objective != "mecatol"
            {
                expected += tuning::bw_imperial_bonus();
            }
            Weighed { plan, expected }
        })
        .collect();
    weighed.sort_by(|a, b| {
        b.expected
            .total_cmp(&a.expected)
            .then_with(|| a.plan.objective.cmp(&b.plan.objective))
    });
    let fallback = weighed
        .first()
        .is_none_or(|w| w.expected < tuning::bw_threshold());
    // Follow the best plan and every other that fits the tactic tokens left.
    let mut chosen = Vec::new();
    let mut tokens = ctx.tactic_tokens;
    if !fallback {
        for (i, w) in weighed.iter().enumerate() {
            if w.expected < tuning::bw_threshold() {
                break;
            }
            if w.plan.cost.tokens <= tokens || chosen.is_empty() {
                tokens -= w.plan.cost.tokens;
                chosen.push(i);
            }
        }
    }
    let imperial = !fallback
        && (imperial
            || chosen
                .iter()
                .any(|&i| weighed[i].plan.steer.cards.iter().any(|c| c == "Imperial")));
    Verdict {
        plans: weighed,
        chosen,
        imperial,
        fallback,
    }
}

/// Plan the seat's turn. `None` when the layer is off or it is too early.
#[must_use]
pub fn verdict(seen: &SeatObservation<'_>, me: &PlayerId) -> Option<Verdict> {
    if !tuning::bw_enabled() || seen.observed().round() < tuning::bw_round() {
        return None;
    }
    let mut ctx = PlanCtx::new(seen, me);
    let planners = registry();
    let mut plans = Vec::new();
    for goal in ctx.goals.clone() {
        if goal.satisfied {
            continue;
        }
        ctx.goal = goal.clone();
        for planner in &planners {
            if goal.matches_any(planner.objective_ids())
                && let Some(plan) = planner.plan(seen, &ctx)
            {
                plans.push(plan);
                break;
            }
        }
    }
    if tuning::bw_mecatol() {
        plans.extend(mecatol::plan(seen, &ctx));
    }
    Some(weigh(&ctx, plans))
}
