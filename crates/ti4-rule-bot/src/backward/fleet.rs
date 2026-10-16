//! Fleet and ship placement goals. (Skeleton: planned in the fleet step.)

use ti4_engine::choice::SeatObservation;

use super::{ObjectivePlanner, Plan, PlanCtx};

/// The fleet group.
pub struct Fleet;

impl ObjectivePlanner for Fleet {
    fn objective_ids(&self) -> &[&str] {
        &[]
    }
    fn plan(&self, _obs: &SeatObservation<'_>, _ctx: &PlanCtx) -> Option<Plan> {
        None
    }
}
