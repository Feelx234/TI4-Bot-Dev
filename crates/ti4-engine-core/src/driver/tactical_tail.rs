//! The close of a tactical action (package D1b2; `Game::close_tactical`): what is decided from the position (does the Crown of
//! Emphidia explore, does Dark Energy Tap, does anyone listen for the end of the action), what is written (the aftermath
//! window is gone, the active system is cleared, the legacy pending tag is cleared) and where the turn goes next.
//!
//! The explorations themselves (the Crown's, the frontier token's) are the exploration area's and stay lent; the adapter
//! bails when one fires (`D1b2-bail-03`). The `TACTICAL_ACTION_ENDED` event reaches the reaction table through the string
//! `emit_typed` (lent; compared by the A5 shim).
//!
//! L5: reads `rules::explore_windows` (the Crown's options) beside the technologies and the leaders.

use crate::generated::{Leader, System, Technology};
use crate::ids::Seat;
use crate::query::tech;
use crate::rules::explore_windows::CrownExplore;
use crate::state::{CoreState, LeaderStatus, WindowKind};
use crate::tables::Scope;

/// Where the turn goes after the action has closed (the tail of `close_tactical`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Then {
    /// A hero's strategy primary is resolving (Thunder's Edge Warfare): redistribute the tokens, then select the followers.
    LeaderStrategy,
    /// Thunder's Edge Warfare's follower window resumes after the free tactical action.
    Secondary,
    /// A plain tactical action: `finish_action`.
    FinishAction,
}

/// The continuation: a hero's strategy first, then the follower window held for after the action, else the end of the action.
#[must_use]
pub const fn then(leader_strategy: bool, secondary_after_tactical: bool) -> Then {
    if leader_strategy {
        Then::LeaderStrategy
    } else if secondary_after_tactical {
        Then::Secondary
    } else {
        Then::FinishAction
    }
}

/// Dark Energy Tap: "after you perform a tactical action in a system that contains a frontier token, if you have 1 or more ships
/// in that system, explore that token." The trigger is the action ending, not a move inside it.
#[must_use]
pub fn det_fires(core: &CoreState, seat: Seat, system: System) -> bool {
    core.explore().frontier.contains(system)
        && tech::owns(core, seat, Technology::DarkEnergyTap)
        && core.board().sys(system).units.iter().any(|u| u.owner() == seat)
}

/// The Crown of Emphidia: a ready Crown and a planet the seat controls that has a trait to explore.
#[must_use]
pub fn crown_offers(core: &CoreState, seat: Seat, scope: Scope) -> bool {
    CrownExplore::offers(core, seat, scope)
}

/// T'ro (the Sardakk agent) and Field Marshal Mercer read the end of the tactical action: a readied copy anywhere. The agents a
/// Yssaril seat borrows with Ssruu are the adapter's (`ssruu_lends`: a readied Ssruu with another seat's agent to copy).
#[must_use]
pub fn tro_window(core: &CoreState, ssruu_lends: bool) -> bool {
    ssruu_lends
        || core.players().iter().any(|p| {
            [Leader::Tro, Leader::FieldMarshalMercer].iter().any(|agent| p.leaders.status.get(agent) == Some(&LeaderStatus::Readied))
        })
}

/// The writes of the close: the aftermath window is gone, no system is active, the legacy pending tag is cleared.
pub fn close_writes(core: &mut CoreState) {
    core.pending_mut().close(WindowKind::Aftermath);
    core.pending_mut().tag = None;
    core.meta_mut().active_system = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_continuation_prefers_a_heros_strategy_then_the_held_followers_then_the_end_of_the_action() {
        assert_eq!(then(true, true), Then::LeaderStrategy);
        assert_eq!(then(false, true), Then::Secondary);
        assert_eq!(then(false, false), Then::FinishAction);
    }
}
