//! The tactical tail (`Game::close_tactical`): what is decided from the position when the action ends (does the Crown of Emphidia explore,
//! does Dark Energy Tap, does anyone listen for the end of the action), what is announced (`TACTICAL_ACTION_ENDED`, which no other core
//! window emits) and written (no active system, the legacy pending tag cleared), and where the turn goes next (`driver::tactical_tail`).
//!
//! Not carried (counted gaps): the explorations the Crown and the frontier token start (no native planet exploration window yet), the
//! agents a Yssaril seat borrows with Ssruu, Thunder's Edge Warfare's follower window and a hero's strategy primary waiting for the end of
//! the free tactical action. The end of the action itself (`finish_action`) is the turn flow's.

use super::{TacticalAction, TacticalGap};
use crate::generated::{Faction, Leader};
use crate::kernel::fx;
use crate::kernel::stage::Sx;
use crate::kernel::Next;
use crate::query::reach;
use crate::state::{CoreState, EventKind, Key, LeaderStatus, Payload, PlainKind, PlainRec, Val, WindowKind};

use crate::driver::tactical_tail as tail;

/// A Yssaril seat with a readied Ssruu while another seat plays the Sardakk (whose agent T'ro it could copy): the string's `borrowable_agents`
/// would list it, and the core does not know which agents Ssruu can borrow.
fn ssruu_lends(core: &CoreState) -> bool {
    let ssruu = core.players().iter().any(|p| p.faction == Faction::TheYssarilTribes && p.leaders.status.get(&Leader::Ssruu) == Some(&LeaderStatus::Readied));
    ssruu && core.players().iter().any(|p| p.faction == Faction::SardakkNorr)
}

pub(super) fn run(a: &mut TacticalAction, sx: &mut Sx<'_>) -> Next {
    let active = sx.state.meta().active;
    let system = sx.state.meta().active_system;
    if let (Some(player), Some(system)) = (active, system) {
        let scope = reach::scope_of(sx.kernel);
        if tail::crown_offers(sx.state, player, scope) || tail::det_fires(sx.state, player, system) {
            return TacticalAction::bail(sx, TacticalGap::ExplorationAtTheEnd);
        }
    }
    let lends = ssruu_lends(sx.state);
    if lends {
        return TacticalAction::bail(sx, TacticalGap::SsruuLends);
    }
    if tail::tro_window(sx.state, lends)
        && let (Some(player), Some(system)) = (active, system)
    {
        let payload = Payload::new().with(Key::Player, Val::Seat(player)).with(Key::System, Val::System(system));
        // An error here rolls the step back: the driver restores the position, as the string saves and restores it.
        let _ = sx.emit_mirrored(fx::event(EventKind::TacticalActionEnded, payload));
    }
    tail::close_writes(sx.state);
    sx.plain(PlainRec::bare(PlainKind::TacticalActionComplete));
    let pending = sx.state.pending();
    match tail::then(pending.window(WindowKind::LeaderStrategy).is_some(), pending.window(WindowKind::StrategySecondaryAfterTactical).is_some()) {
        tail::Then::LeaderStrategy => TacticalAction::bail(sx, TacticalGap::LeaderStrategy),
        tail::Then::Secondary => TacticalAction::bail(sx, TacticalGap::WarfareSecondary),
        tail::Then::FinishAction => {
            let _ = a;
            Next::Done
        }
    }
}
