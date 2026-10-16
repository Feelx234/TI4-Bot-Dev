//! Two-step plans for Mecatol Rex: a prep step that makes the Mecatol activation succeed where
//! the one-step plan ([`super::mecatol`]) fails, then the act step (activating Mecatol Rex).
//!
//! Four variants, each behind its own knob (default off):
//!
//! 1. [`tuning::bw_mr_build`]: production adds what the act step lacks (ground forces to land, a
//!    carrier to bring them, warships to win the fight), by a home-dock activation, Construction's
//!    production (Thunder's Edge card set), or Warfare's secondary (home production).
//! 2. [`tuning::bw_mr_warfare`]: Warfare's extra activation or token lets a ship gather next to
//!    Mecatol Rex first (the *prep* activation), so the units that could not reach Mecatol Rex
//!    now do.
//! 3. [`tuning::bw_mr_tech_range`]: a movement technology (Gravity Drive, a faster unit upgrade)
//!    brings the ships into reach.
//! 4. [`tuning::bw_mr_tech_strength`]: a combat or unit-upgrade technology lifts the fleet's
//!    strength enough to win the fight.
//!
//! Every variant is judged on the position the engine reports ([`crate::objectives::reach`],
//! `movable_into`, `can_reach`, [`ti4_engine::fleet_strength`]) with the prep's effect added to
//! it. A variant only produces a plan when the one-step plan does not already reach
//! [`tuning::bw_likely`]. The plan's probability is prep feasibility x act success x 0.9 for the
//! extra step; it competes in [`super::weigh`] as the Mecatol plan.

use std::collections::BTreeSet;

use ti4_engine::choice::{Observed, SeatObservation};
use ti4_engine::fleet_strength::{Descriptors, descriptors, matchup};
use ti4_model::id::{PlayerId, SystemId};

use super::{Cost, Plan, PlanCtx, Steer};
use crate::tuning;

/// Mecatol Rex's activation as it stands: what it needs and what the seat can bring now.
#[derive(Debug, Clone)]
pub struct Case {
    pub system: SystemId,
    /// Ground forces needed to take the planet.
    pub need: usize,
    /// Ground forces the carriers that can reach it bring now (plus those already there).
    pub ground: usize,
    pub hostile: bool,
    /// The fleet that could fight there now, as unit counts.
    pub own: Vec<(String, usize)>,
    pub enemy: Descriptors,
    pub margin: f64,
    pub faction: String,
    /// Whether any ship can reach it at all.
    pub ships: bool,
}

impl Case {
    /// The strength matchup of the fleet `own` against the defence.
    #[must_use]
    pub fn matchup_of(&self, seen: &Observed<'_>, own: &[(String, usize)]) -> f64 {
        if !self.hostile {
            return f64::INFINITY;
        }
        let me = descriptors(seen.content(), seen.sources(), &self.faction, own, &[]);
        matchup(&me, &self.enemy)
    }

    /// Whether the activation would succeed with `ground` and fleet `own`.
    #[must_use]
    pub fn works(&self, seen: &Observed<'_>, ground: usize, own: &[(String, usize)]) -> bool {
        self.ships && ground >= self.need && self.matchup_of(seen, own) >= self.margin
    }

    /// Probability that the fight and the landing go well with fleet `own`.
    #[must_use]
    pub fn act_success(&self, seen: &Observed<'_>, own: &[(String, usize)]) -> f64 {
        let fight = if self.hostile {
            1.0 / (1.0 + (-2.0 * self.matchup_of(seen, own)).exp())
        } else {
            1.0
        };
        fight * 0.9
    }
}

/// Mecatol Rex's case, or `None` when the seat cannot take it whatever it prepares (held already,
/// the custodians out of reach for lack of influence, no map).
#[must_use]
pub fn case(seen: &Observed<'_>, me: &PlayerId, ctx: &PlanCtx) -> Option<Case> {
    let system = ctx.mecatol.system.clone()?;
    if ctx.mecatol.held {
        return None;
    }
    let target = crate::targets(seen, me, &system)
        .into_iter()
        .find(|(planet, _, _)| planet.as_str() == crate::MECATOL)?;
    let (ships, ground) = crate::objectives::reach(seen, me, &system);
    let movable = seen.movable_into(me, &system);
    let board = seen.system(&system);
    let present = board
        .units
        .iter()
        .filter(|u| &u.owner == me && crate::is_ship(seen, u));
    let (own, _) = crate::tally(present.chain(movable.iter().map(|m| &m.unit)));
    Some(Case {
        hostile: crate::has_hostile_ships(seen, me, &system),
        enemy: crate::hostile_space(seen, me, &system),
        margin: crate::fight_margin(seen, &system),
        faction: crate::faction(seen, me),
        need: target.2,
        ground,
        own,
        ships,
        system,
    })
}

/// How the seat gets a production this turn, besides activating its dock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Activate the home dock (one more command token).
    Dock,
    /// The seat's own Construction card, whose primary offers a production (Thunder's Edge).
    Construction,
    /// Another seat's Warfare secondary, which produces at home for a strategy token.
    WarfareSecondary,
}

impl Source {
    fn label(self) -> &'static str {
        match self {
            Self::Dock => "home dock activation",
            Self::Construction => "Construction production",
            Self::WarfareSecondary => "Warfare secondary production",
        }
    }
}

/// What a build adds.
#[derive(Debug, Clone, PartialEq)]
pub struct Build {
    pub source: Source,
    /// Units bought as (type, count).
    pub units: Vec<(String, usize)>,
    pub resources: i64,
    /// Fleet counts and ground forces the act step then has.
    pub fleet: Vec<(String, usize)>,
    pub ground: usize,
    pub odds: f64,
}

fn with_added(fleet: &[(String, usize)], id: &str, n: usize) -> Vec<(String, usize)> {
    let mut out = fleet.to_vec();
    match out.iter_mut().find(|(k, _)| k == id) {
        Some((_, c)) => *c += n,
        None => out.push((id.to_owned(), n)),
    }
    out
}

/// Production slots of the home system (docks' production values, as the engine counts them).
fn home_slots(seen: &Observed<'_>, me: &PlayerId, home: &SystemId) -> i64 {
    let board = seen.system(home);
    board
        .planet_units
        .iter()
        .flat_map(|(planet, units)| {
            let res = ti4_content::galaxy::planet(seen.content(), planet.as_str(), seen.sources())
                .map_or(0, |p| p.resources());
            units
                .iter()
                .filter(|u| &u.owner == me)
                .filter_map(move |u| crate::unit_type(seen, u.type_id.as_str()))
                .map(move |t| t.production(res))
        })
        .sum::<i64>()
        .max(0)
}

/// What a production at home could add so the Mecatol activation works, if anything can.
#[must_use]
pub fn build_boost(
    seen: &Observed<'_>,
    me: &PlayerId,
    ctx: &PlanCtx,
    case: &Case,
) -> Option<(Vec<(String, usize)>, i64, Vec<(String, usize)>, usize)> {
    let home = crate::home_system(seen, me)?;
    let slots = home_slots(seen, me, &home);
    if slots <= 0 {
        return None;
    }
    let carrier = crate::unit_type(seen, "carrier")?;
    let cruiser = crate::unit_type(seen, "cruiser")?;
    let infantry = crate::unit_type(seen, "infantry")?;
    // New units must be able to get from home to Mecatol Rex.
    let range = i32::try_from(carrier.move_value()).unwrap_or(1)
        + i32::from(crate::carrier_reach(seen, me) > 1);
    if home != case.system && !seen.can_reach(me, &home, &case.system, range) {
        return None;
    }
    // Carry capacity free at home now.
    let movable = seen.movable_into(me, &case.system);
    let cap_home: i64 = movable
        .iter()
        .filter(|m| m.origin == home && m.capacity > 0 && crate::is_ship(seen, &m.unit))
        .map(|m| m.capacity)
        .sum();
    let waiting = i64::try_from(
        seen.loadable(me, &home)
            .iter()
            .filter(|c| crate::is_ground(seen, &c.unit))
            .count(),
    )
    .unwrap_or(0);
    let spare = (cap_home - waiting).max(0);
    let sg = i64::try_from(case.need.saturating_sub(case.ground)).unwrap_or(0);
    let mut units: Vec<(String, usize)> = Vec::new();
    let mut slots_left = slots;
    let mut budget = ctx.resources as f64;
    let mut fleet = case.own.clone();
    let mut ground = case.ground;
    if sg > 0 {
        let carriers =
            ((sg - spare).max(0) + carrier.capacity().max(1) - 1) / carrier.capacity().max(1);
        let need_slots = sg + carriers;
        #[expect(clippy::cast_precision_loss, reason = "small counts")]
        let need_cost = sg as f64 * infantry.cost() + carriers as f64 * carrier.cost();
        if need_slots > slots_left || need_cost > budget {
            return None;
        }
        slots_left -= need_slots;
        budget -= need_cost;
        units.push((infantry.id().to_owned(), usize::try_from(sg).unwrap_or(0)));
        ground += usize::try_from(sg).unwrap_or(0);
        if carriers > 0 {
            let n = usize::try_from(carriers).unwrap_or(0);
            units.push((carrier.id().to_owned(), n));
            fleet = with_added(&fleet, carrier.id(), n);
        }
    }
    // Warships until the fight is won (cruisers).
    let mut added = 0usize;
    while case.hostile
        && case.matchup_of(seen, &fleet) < case.margin
        && slots_left > 0
        && budget >= cruiser.cost()
        && added < 4
    {
        fleet = with_added(&fleet, cruiser.id(), 1);
        slots_left -= 1;
        budget -= cruiser.cost();
        added += 1;
    }
    if added > 0 {
        units.push((cruiser.id().to_owned(), added));
    }
    if units.is_empty() || !case.works(seen, ground, &fleet) {
        return None;
    }
    #[expect(clippy::cast_possible_truncation, reason = "a handful of resources")]
    let spent = (ctx.resources as f64 - budget).ceil() as i64;
    Some((units, spent, fleet, ground))
}

/// The ways the seat can get a production before the act step.
fn sources(seen: &Observed<'_>, me: &PlayerId, ctx: &PlanCtx) -> Vec<(Source, i32, f64)> {
    let mut out = Vec::new();
    if let Some(home) = crate::home_system(seen, me)
        && ctx.tactic_tokens >= 2
        && !seen.systems_with_token(me).contains(&home)
        && crate::has_own_production(seen, me, &home)
    {
        out.push((Source::Dock, 1, 0.95));
    }
    if crate::ready_card(seen, me, "Construction").is_some_and(|c| c.starts_with("te"))
        && ctx.tactic_tokens >= 1
    {
        out.push((Source::Construction, 0, 0.95));
    }
    if ctx.strategic_tokens >= 1
        && ctx.tactic_tokens >= 1
        && crate::secondary_to_come(seen, me, "Warfare")
    {
        out.push((Source::WarfareSecondary, 0, 0.6));
    }
    out
}

/// Variant 1: build, then act.
fn build_and_act(seen: &Observed<'_>, me: &PlayerId, ctx: &PlanCtx, case: &Case) -> Option<Plan> {
    let (units, resources, fleet, _ground) = build_boost(seen, me, ctx, case)?;
    let (source, prep_tokens, odds) = sources(seen, me, ctx)
        .into_iter()
        .max_by(|a, b| a.2.total_cmp(&b.2).then(b.1.cmp(&a.1)))?;
    let probability = odds * case.act_success(seen, &fleet) * 0.9;
    let mut systems = vec![case.system.clone()];
    if source == Source::Dock {
        systems.insert(0, crate::home_system(seen, me)?);
    }
    let cards = match source {
        Source::Construction => vec!["Construction".to_owned()],
        _ => Vec::new(),
    };
    Some(Plan {
        objective: "mecatol".to_owned(),
        points: tuning::bw_mecatol_weight()
            * (if ctx.mecatol.custodians_removed {
                0.0
            } else {
                1.0
            } + 0.6),
        probability,
        cost: Cost {
            tokens: prep_tokens + 1,
            resources,
            activations: u32::try_from(prep_tokens + 1).unwrap_or(1),
        },
        steer: Steer {
            systems,
            builds: units.iter().map(|(u, _)| u.clone()).collect(),
            cards,
        },
        reason: format!(
            "build + act: {} adds {:?}, then take Mecatol Rex",
            source.label(),
            units
        ),
        payment: false,
        secret: false,
    })
}

/// All enabled two-step plans for Mecatol Rex that beat nothing the one-step plan offers; the
/// best by probability x points less its tokens.
#[must_use]
pub fn plan(seen: &SeatObservation<'_>, ctx: &PlanCtx) -> Option<Plan> {
    if !(tuning::bw_mr_build()) {
        return None;
    }
    let observed = seen.observed();
    let me = &ctx.me;
    let one_step = ctx
        .steps
        .iter()
        .find(|s| Some(&s.system) == ctx.mecatol.system.as_ref() && !s.planets.is_empty())
        .map_or(0.0, |s| s.success);
    if one_step >= tuning::bw_likely() {
        return None;
    }
    let case = case(observed, me, ctx)?;
    let mut plans: Vec<Plan> = Vec::new();
    if tuning::bw_mr_build() {
        plans.extend(build_and_act(observed, me, ctx, &case));
    }
    let _ = BTreeSet::<()>::new();
    plans.into_iter().max_by(|a, b| {
        let key = |p: &Plan| {
            p.probability * p.points - tuning::bw_token_cost() * f64::from(p.cost.tokens)
        };
        key(a).total_cmp(&key(b))
    })
}
