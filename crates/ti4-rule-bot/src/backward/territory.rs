//! Territory goals: control N planets, planets of a trait or with specialties, outside home,
//! on the rim, attached planets, legendary planets.
//!
//! Backward: the goal is met when `threshold` planets of the right kind are controlled. The
//! cheapest route is the fewest activations (at most three, within the tactic tokens) whose
//! landings take enough of them, judged by the engine's own requirement functions on the
//! imagined planets ([`PlanCtx::route`]). Only steps that take a planet qualify; the probability
//! is the product of the steps' fight and landing odds. Nothing within three activations: no
//! plan (the default rules play).

use ti4_engine::choice::SeatObservation;

use super::{ObjectivePlanner, Plan, PlanCtx};

/// The territory group.
pub struct Territory;

impl ObjectivePlanner for Territory {
    fn objective_ids(&self) -> &[&str] {
        &[
            "non_home",
            "on_the_rim",
            "same_trait",
            "tech_specialties",
            "planets_of_trait_cultural",
            "planets_of_trait_hazardous",
            "planets_of_trait_industrial",
            "attached_planets",
            "legendary_planets",
            "rival_home_planets",
            "shared_planet_systems",
            "notable_systems",
        ]
    }

    fn plan(&self, obs: &SeatObservation<'_>, ctx: &PlanCtx) -> Option<Plan> {
        // A bought-cost goal never reaches here; a goal already within one planet is cheapest.
        ctx.route_plan(obs, 3, |step| !step.planets.is_empty(), "take planets")
    }
}
