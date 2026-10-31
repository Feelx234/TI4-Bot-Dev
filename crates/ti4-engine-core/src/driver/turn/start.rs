//! The start of a turn (the `turn_prelude` block of `Game::step_flow`). Placeholder until KP1.
use super::inline::Flow;
use crate::kernel::stage::Sx;

pub fn run(_: &mut Sx<'_>) -> Flow {
    Flow::Done
}
