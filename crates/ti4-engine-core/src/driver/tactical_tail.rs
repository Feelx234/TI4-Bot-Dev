//! The close of a tactical action (package D1b2; `Game::close_tactical`): what is decided from the position (does the Crown of
//! Emphidia explore, does Dark Energy Tap, does anyone listen for the end of the action), what is written (the aftermath
//! window is gone, the active system is cleared, the legacy pending tag is cleared) and where the turn goes next.
//!
//! The explorations themselves (the Crown's, the frontier token's) are the exploration area's window since package X
//! (`rules::explore_windows::Exploring`, lockstep behind the string's `exploration::explore_with` / `explore_frontier`, which the
//! string `close_tactical` calls); this file only decides that one fires. The `TACTICAL_ACTION_ENDED` event reaches the reaction table
//! through the string `emit_typed` (lent; compared by the A5 shim).
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

    fn three_seats() -> CoreState {
        use crate::generated::Faction;
        use crate::state::{Player, Players};
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec((0..3).map(|i| Player::new(Seat(i), Faction::ALL[usize::from(i)])).collect());
        s
    }

    #[test]
    fn dark_energy_tap_needs_a_frontier_token_the_technology_and_a_ship_of_the_seat() {
        let system = System::ALL[0];
        let mut s = three_seats();
        assert!(!det_fires(&s, Seat(0), system));
        s.explore.frontier.insert(system);
        s.players.edit(0).tech.owned.insert(Technology::DarkEnergyTap);
        assert!(!det_fires(&s, Seat(0), system), "no ship there");
        s.board.sys_mut(system).units.push(crate::state::Piece::new(crate::generated::Unit::ALL[0], Seat(1)));
        assert!(!det_fires(&s, Seat(0), system), "somebody else's ship");
        s.board.sys_mut(system).units.push(crate::state::Piece::new(crate::generated::Unit::ALL[0], Seat(0)));
        assert!(det_fires(&s, Seat(0), system));
        s.explore.frontier.remove(system);
        assert!(!det_fires(&s, Seat(0), system), "no token");
        s.explore.frontier.insert(system);
        s.players.edit(0).tech.owned.remove(Technology::DarkEnergyTap);
        assert!(!det_fires(&s, Seat(0), system), "no technology");
    }

    #[test]
    fn the_tro_window_is_open_for_a_readied_agent_anywhere_or_a_borrowed_one() {
        let mut s = three_seats();
        assert!(!tro_window(&s, false));
        assert!(tro_window(&s, true), "Ssruu lends one");
        s.players.edit(2).leaders.status.insert(Leader::Tro, LeaderStatus::Exhausted);
        assert!(!tro_window(&s, false), "an exhausted agent does not listen");
        s.players.edit(2).leaders.status.insert(Leader::Tro, LeaderStatus::Readied);
        assert!(tro_window(&s, false));
        s.players.edit(2).leaders.status.insert(Leader::Tro, LeaderStatus::Exhausted);
        s.players.edit(1).leaders.status.insert(Leader::FieldMarshalMercer, LeaderStatus::Readied);
        assert!(tro_window(&s, false), "Field Marshal Mercer reads the same moment");
    }

    #[test]
    fn the_crown_offers_only_with_a_ready_crown_and_a_planet_with_a_trait() {
        let scope = Scope(u32::MAX);
        let mut s = three_seats();
        assert!(!crown_offers(&s, Seat(0), scope), "no Crown");
        s.players.edit(0).cards.relics.push(crate::generated::Relic::TheCrownOfEmphidia);
        assert!(!crown_offers(&s, Seat(0), scope), "no planet to explore");
        let planet = crate::generated::Planet::ALL.iter().copied().find(|p| !p.data().traits.is_empty()).expect("a planet with a trait");
        s.board.planet_mut(planet).control = Some(Seat(0));
        assert!(crown_offers(&s, Seat(0), scope));
        assert!(!crown_offers(&s, Seat(1), scope), "not the holder");
        s.players.edit(0).cards.relics_exhausted.insert(crate::generated::Relic::TheCrownOfEmphidia);
        assert!(!crown_offers(&s, Seat(0), scope), "exhausted");
    }

    #[test]
    fn the_close_clears_the_aftermath_the_system_and_the_pending_tag() {
        let mut s = three_seats();
        s.meta.active_system = Some(System::ALL[0]);
        s.pending.tag = Some(crate::state::PendingTag::Move);
        s.pending.stack.push(crate::state::Window::Untyped { kind: WindowKind::Aftermath, actor: None, data: [0; 3] });
        s.pending.stack.push(crate::state::Window::Untyped { kind: WindowKind::Trade, actor: None, data: [0; 3] });
        close_writes(&mut s);
        assert_eq!((s.meta.active_system, s.pending.tag), (None, None));
        assert_eq!(s.pending.stack.len(), 1, "only the aftermath goes");
        assert_eq!(s.pending.stack[0].kind(), WindowKind::Trade);
    }
}
