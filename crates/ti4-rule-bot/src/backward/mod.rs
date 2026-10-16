//! LOCAL STUB of agent A's framework (`backward-a`): replaced by A's `mod.rs` on merge.
//!
//! Spec used: `ObjectivePlanner { objective_ids, plan }`, `Plan` (objective id, VP value,
//! probability reachable this turn, cost, concrete steer, reason), `PlanCtx`.

use ti4_engine::choice::SeatObservation;
use ti4_model::id::{PlayerId, SystemId};

use crate::objectives::{Goal, Reserve};

pub mod spend;

/// What the seat looks at when planning.
pub struct PlanCtx {
    pub me: PlayerId,
    pub round: u32,
    pub goals: Vec<Goal>,
}

/// What a plan costs.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Cost {
    pub tokens: f64,
    pub resources: f64,
    pub activations: f64,
}

/// The concrete steer of a plan.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Steer {
    pub reserve: Reserve,
    pub cards: Vec<(String, f64)>,
    pub techs: Vec<(String, f64)>,
    pub activate: Vec<(SystemId, f64)>,
    pub build: Vec<(String, f64)>,
}

/// One objective's plan.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub objective: String,
    pub vp: f64,
    pub p_this_turn: f64,
    pub cost: Cost,
    pub steer: Steer,
    pub reason: String,
}

pub trait ObjectivePlanner {
    fn objective_ids(&self) -> &[&str];
    fn plan(&self, obs: &SeatObservation<'_>, ctx: &PlanCtx) -> Option<Plan>;
}
