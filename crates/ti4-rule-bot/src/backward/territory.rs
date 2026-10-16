//! Territory goals: control N planets, planets of a trait or with specialties, outside home,
//! rim, attached planets. (Skeleton: planned in the territory step.)

use ti4_engine::choice::SeatObservation;

use super::{ObjectivePlanner, Plan, PlanCtx};

/// The territory group.
pub struct Territory;

impl ObjectivePlanner for Territory {
    fn objective_ids(&self) -> &[&str] {
        &[]
    }
    fn plan(&self, _obs: &SeatObservation<'_>, _ctx: &PlanCtx) -> Option<Plan> {
        None
    }
}
