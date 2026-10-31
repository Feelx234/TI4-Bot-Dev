//! Phase changes (`Game::step_phase`). Placeholder until KP3.
use super::inline::Flow;
use crate::kernel::stage::Sx;

pub fn run_phase(_: &mut Sx<'_>) -> Flow {
    Flow::Done
}

pub fn run_announce(_: &mut Sx<'_>) -> Flow {
    Flow::Done
}
