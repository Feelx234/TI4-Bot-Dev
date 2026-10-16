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

use std::collections::BTreeMap;

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

    /// [`Self::works`] for a fleet that is assumed to be able to get there.
    #[must_use]
    pub fn works_if_reached(
        &self,
        seen: &Observed<'_>,
        ground: usize,
        own: &[(String, usize)],
    ) -> bool {
        !own.is_empty() && ground >= self.need && self.matchup_of(seen, own) >= self.margin
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
pub struct Boost {
    /// Units bought as (type, count).
    pub units: Vec<(String, usize)>,
    pub resources: i64,
    /// Fleet counts and ground forces the act step then has.
    pub fleet: Vec<(String, usize)>,
    pub ground: usize,
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
) -> Option<Boost> {
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
    Some(Boost {
        units,
        resources: spent,
        fleet,
        ground,
    })
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
    let Boost {
        units,
        resources,
        fleet,
        ..
    } = build_boost(seen, me, ctx, case)?;
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
            ..Steer::default()
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
    if !(tuning::bw_mr_build() || tuning::bw_mr_warfare()) {
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
    if tuning::bw_mr_warfare() {
        plans.extend(warfare_and_act(observed, me, ctx, &case));
    }
    plans.into_iter().max_by(|a, b| {
        let key = |p: &Plan| {
            p.probability * p.points - tuning::bw_token_cost() * f64::from(p.cost.tokens)
        };
        key(a).total_cmp(&key(b))
    })
}

// -- Warfare ------------------------------------------------------------------------------------

/// What the seat's Warfare card gives this turn (shared by the build and move variants).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarfareAccess {
    /// The seat's own ready Warfare card.
    pub own: Option<String>,
    /// Playing it gives one more activation: Thunder's Edge's free tactical action, or the
    /// printed card's recalled token (needs a token on the board).
    pub extra_activation: bool,
    /// Another seat's Warfare is still to come: its secondary produces at home.
    pub secondary: bool,
}

/// The seat's Warfare options.
#[must_use]
pub fn warfare(seen: &Observed<'_>, me: &PlayerId) -> WarfareAccess {
    let own = crate::ready_card(seen, me, "Warfare");
    let extra_activation = own
        .as_ref()
        .is_some_and(|c| c.starts_with("te") || !seen.systems_with_token(me).is_empty());
    WarfareAccess {
        secondary: crate::secondary_to_come(seen, me, "Warfare"),
        extra_activation,
        own,
    }
}

/// Whether gathering at `gather` (an activation next to Mecatol Rex) makes the act step work, and
/// with what ground and fleet.
#[must_use]
pub fn gathered(
    seen: &Observed<'_>,
    me: &PlayerId,
    case: &Case,
    gather: &SystemId,
) -> Option<(usize, Vec<(String, usize)>)> {
    if gather == &case.system || crate::has_hostile_ships(seen, me, gather) {
        return None;
    }
    let mut set: BTreeMap<(SystemId, usize), ti4_engine::tactical::Movable> = BTreeMap::new();
    for m in seen
        .movable_into(me, gather)
        .into_iter()
        .chain(seen.movable_into(me, &case.system))
    {
        set.entry((m.origin.clone(), m.index)).or_insert(m);
    }
    let carriers: Vec<_> = set
        .values()
        .filter(|m| m.capacity > 0 && crate::is_ship(seen, &m.unit))
        .collect();
    let capacity: i64 = carriers.iter().map(|m| m.capacity).sum();
    let origins: std::collections::BTreeSet<&SystemId> =
        carriers.iter().map(|m| &m.origin).collect();
    let loadable: usize = origins
        .into_iter()
        .map(|o| {
            seen.loadable(me, o)
                .iter()
                .filter(|c| crate::is_ground(seen, &c.unit))
                .count()
        })
        .sum();
    let board = seen.system(&case.system);
    let there = board
        .units
        .iter()
        .filter(|u| &u.owner == me && crate::is_ground(seen, u))
        .count();
    let ground = loadable.min(usize::try_from(capacity).unwrap_or(0)) + there;
    let present = board
        .units
        .iter()
        .filter(|u| &u.owner == me && crate::is_ship(seen, u));
    let (own, _) = crate::tally(present.chain(set.values().map(|m| &m.unit)));
    case.works_if_reached(seen, ground, &own)
        .then_some((ground, own))
}

/// Variant 2: gather beside Mecatol Rex (Warfare's extra activation if tokens are short), then act.
fn warfare_and_act(seen: &Observed<'_>, me: &PlayerId, ctx: &PlanCtx, case: &Case) -> Option<Plan> {
    let access = warfare(seen, me);
    let use_warfare = ctx.tactic_tokens < 2;
    if use_warfare && !(access.extra_activation && ctx.tactic_tokens >= 1) {
        return None;
    }
    if ctx.tactic_tokens < 1 {
        return None;
    }
    let galaxy = seen.galaxy()?;
    let tokened = seen.systems_with_token(me);
    let best = galaxy
        .adjacent(case.system.as_str())
        .into_iter()
        .map(SystemId::new)
        .filter(|x| !tokened.contains(x))
        .filter_map(|x| gathered(seen, me, case, &x).map(|(_, own)| (x, own)))
        .map(|(x, own)| {
            let p = case.act_success(seen, &own);
            (x, p)
        })
        .max_by(|a, b| a.1.total_cmp(&b.1).then_with(|| b.0.cmp(&a.0)))?;
    let (gather, act) = best;
    let odds = if use_warfare { 0.95 } else { 1.0 };
    let steer = Steer {
        systems: vec![gather.clone(), case.system.clone()],
        cards: if use_warfare {
            vec!["Warfare".to_owned()]
        } else {
            Vec::new()
        },
        play: if use_warfare {
            vec!["Warfare".to_owned()]
        } else {
            Vec::new()
        },
        ..Steer::default()
    };
    Some(Plan {
        objective: "mecatol".to_owned(),
        points: tuning::bw_mecatol_weight()
            * (if ctx.mecatol.custodians_removed {
                0.0
            } else {
                1.0
            } + 0.6),
        probability: odds * 0.85 * act * 0.9,
        cost: Cost {
            tokens: if use_warfare { 1 } else { 2 },
            resources: 0,
            activations: 2,
        },
        steer,
        reason: format!(
            "warfare move + act: gather in {gather}{}, then take Mecatol Rex",
            if use_warfare {
                " (Warfare gives the activation)"
            } else {
                " (tokens enough, no Warfare)"
            }
        ),
        payment: false,
        secret: false,
    })
}

/// While a gather step of the Warfare variant is under way (the active system is the one the
/// plan gathers in), the movement option to take: carriers and the strongest ships first.
#[must_use]
pub fn gather_active(seen: &Observed<'_>, me: &PlayerId) -> bool {
    if !tuning::bw_enabled() || !tuning::bw_mr_warfare() || seen.round() < tuning::bw_round() {
        return false;
    }
    let Some(active) = seen.active_system() else {
        return false;
    };
    let mut ctx = PlanCtx::blank(me);
    ctx.mecatol = super::mecatol::state(seen, me);
    let Some(case) = case(seen, me, &ctx) else {
        return false;
    };
    if case.works(seen, case.ground, &case.own) {
        return false;
    }
    seen.galaxy().is_some_and(|g| {
        g.adjacent(case.system.as_str())
            .iter()
            .any(|s| *s == active.as_str())
    }) && gathered(seen, me, &case, active).is_some()
}

/// The movement option of a gather step: carriers (most capacity) first, then the strongest
/// ships; `done` once nothing useful is left to move.
#[must_use]
pub fn gather_move(choice: &ti4_engine::choice::Choice, seen: &Observed<'_>) -> Option<usize> {
    let done = choice
        .options
        .iter()
        .position(ti4_engine::choice::ChoiceOption::is_decline)?;
    let best = choice
        .options
        .iter()
        .enumerate()
        .filter(|(_, o)| o.kind == "move")
        .filter_map(|(i, o)| {
            let kind = crate::unit_type(seen, crate::payload_str(o, "unit")?)?;
            (kind.is_ship() && !kind.is_fighter()).then_some((
                i,
                kind.capacity(),
                kind.combat_dice(),
            ))
        })
        .max_by_key(|&(i, cap, dice)| (cap, dice, std::cmp::Reverse(i)))
        .map(|(i, _, _)| i);
    Some(best.unwrap_or(done))
}

/// The load option of a gather step: ground forces while any are offered.
#[must_use]
pub fn gather_load(choice: &ti4_engine::choice::Choice, seen: &Observed<'_>) -> Option<usize> {
    let done = choice
        .options
        .iter()
        .position(ti4_engine::choice::ChoiceOption::is_decline)?;
    let ground = choice.options.iter().position(|o| {
        o.kind == "load"
            && crate::payload_str(o, "unit")
                .and_then(|id| crate::unit_type(seen, id))
                .is_some_and(|t| t.is_ground_force())
    });
    Some(ground.unwrap_or(done))
}
