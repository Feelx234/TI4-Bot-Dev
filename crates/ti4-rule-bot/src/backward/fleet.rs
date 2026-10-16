//! Fleet and ship placement goals: ships in N systems, planetless systems, next to Mecatol Rex,
//! an anomaly or a rival's home, rival docks, capital ships.
//!
//! Backward: the goal counts systems holding a ship of the seat. The cheapest route is the fewest
//! activations (at most three) that put a ship in a system where the seat has none, judged by the
//! engine's requirement functions on the imagined systems ([`PlanCtx::route`]). A step whose
//! ship would have to leave a system with only that ship counts less ([`super::Step::spare_ship`]).
//! Goals that need a bigger fleet in one place (production) are left to
//! [`crate::planner::Bias`].

use ti4_engine::choice::SeatObservation;

use super::{ObjectivePlanner, Plan, PlanCtx};

/// The fleet group.
pub struct Fleet;

impl ObjectivePlanner for Fleet {
    fn objective_ids(&self) -> &[&str] {
        &[
            "ships_in_systems",
            "planetless_systems",
            "ship_systems_beside_anomaly",
            "ship_systems_beside_rival_home",
            "ships_adjacent_to_mecatol",
            "rival_dock_systems_with_ships",
            "mecatol_ships_while_controlling",
            "distinct_rival_home_reaches",
            "weaker_neighbours",
        ]
    }

    fn plan(&self, obs: &SeatObservation<'_>, ctx: &PlanCtx) -> Option<Plan> {
        ctx.route_plan(obs, 3, |step| step.ship, "put a ship in")
    }
}
