//! The starting technology picks (KP4). Placeholder.
use super::inline::Flow;
use crate::kernel::stage::Sx;

/// `starting_tech::settle`: give out the picks that have nothing to choose.
pub fn settle(_: &mut Sx<'_>) -> Flow {
    Flow::Done
}
