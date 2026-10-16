//! The Mecatol Rex / Imperial plan: one more candidate beside the objective plans.
//!
//! Taking Mecatol Rex scores the custodians' point (while the token stands, six influence) and,
//! with the Imperial card, Imperial's primary point; holding it scores Imperial's point each
//! round the seat plays Imperial. The plan's probability is the odds that an activation of
//! Mecatol Rex's system succeeds (a fight won, ground forces to land); its points are
//! `custodians + imperial_share`, scaled by [`tuning::bw_mecatol_weight`].

use ti4_engine::choice::{Observed, SeatObservation};
use ti4_model::id::{PlanetId, PlayerId};

use super::{Cost, MecatolState, Plan, PlanCtx, Steer};
use crate::tuning;

/// Share of Imperial's primary point counted for a Mecatol plan (the card must still be had).
const IMPERIAL_SHARE: f64 = 0.6;

/// Mecatol Rex as the planners see it.
#[must_use]
pub fn state(seen: &Observed<'_>, me: &PlayerId) -> MecatolState {
    let system = crate::mecatol_system(seen);
    let rival_holds = system.as_ref().is_some_and(|s| {
        seen.system(s)
            .planet_control
            .get(&PlanetId::new(crate::MECATOL))
            .is_some_and(|owner| owner != me)
    });
    MecatolState {
        held: crate::holds_mecatol(seen, me),
        custodians_removed: seen.custodians_removed(),
        takeable: crate::mecatol_takeable(seen, me),
        in_reach: crate::mecatol_in_reach(seen, me),
        rival_holds,
        system,
    }
}

/// The plan for Mecatol Rex, if the seat can do anything about it this turn.
#[must_use]
pub fn plan(_seen: &SeatObservation<'_>, ctx: &PlanCtx) -> Option<Plan> {
    let m = &ctx.mecatol;
    let system = m.system.clone()?;
    let weight = tuning::bw_mecatol_weight();
    let custodians = if m.custodians_removed { 0.0 } else { 1.0 };
    if m.held {
        // Hold it and play Imperial: the point comes with the card.
        return Some(Plan {
            objective: "mecatol".to_owned(),
            points: weight * IMPERIAL_SHARE,
            probability: 1.0,
            cost: Cost::default(),
            steer: Steer {
                cards: vec!["Imperial".to_owned()],
                ..Steer::default()
            },
            reason: "hold Mecatol Rex, play Imperial".to_owned(),
            payment: false,
            secret: false,
        });
    }
    let step = ctx
        .steps
        .iter()
        .find(|s| s.system == system && !s.planets.is_empty())?;
    if !m.takeable && !m.rival_holds {
        return None;
    }
    Some(Plan {
        objective: "mecatol".to_owned(),
        points: weight * (custodians + IMPERIAL_SHARE),
        probability: step.success,
        cost: Cost {
            tokens: 1,
            resources: 0,
            activations: 1,
        },
        steer: Steer {
            systems: vec![system],
            cards: vec!["Imperial".to_owned()],
            ..Steer::default()
        },
        reason: "take Mecatol Rex (custodians + Imperial)".to_owned(),
        payment: false,
        secret: false,
    })
}
