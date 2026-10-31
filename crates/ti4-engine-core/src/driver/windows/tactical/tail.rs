//! The tactical tail (`Game::close_tactical`): what is decided from the position when the action ends (does the Crown of Emphidia explore,
//! does Dark Energy Tap, does anyone listen for the end of the action), what is announced (`TACTICAL_ACTION_ENDED`, which no other core
//! window emits) and written (no active system, the legacy pending tag cleared), and where the turn goes next (`driver::tactical_tail`).
//!
//! Not carried (counted gaps): the explorations the Crown and the frontier token start (no native planet exploration window yet), the
//! agents a Yssaril seat borrows with Ssruu, Thunder's Edge Warfare's follower window and a hero's strategy primary waiting for the end of
//! the free tactical action. The end of the action itself (`finish_action`) is the turn flow's.

use super::{TacticalAction, TacticalGap};
use crate::driver::turn::inline::{halted, run as run_window, Flow};
use crate::generated::{Faction, Leader, System, Unit};
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::Sx;
use crate::kernel::Next;
use crate::query::{board, fleet, reach};
use crate::rules::explore_windows::{CrownExplore, Exploring};
use crate::state::{CoreState, EDeck, EventKind, Key, LeaderStatus, Payload, PlainKind, PlainRec, Val, WindowKind};

use crate::driver::tactical_tail as tail;

/// A Yssaril seat with a readied Ssruu while another seat holds an agent the string's `borrowable_agents` would list: the agents that listen
/// to the end of the action are T'ro (Sardakk) and Field Marshal Mercer (Nomad), readied or exhausted.
fn ssruu_lends(core: &CoreState) -> bool {
    let ssruu = core.players().iter().any(|p| p.faction == Faction::TheYssarilTribes && p.leaders.status.get(&Leader::Ssruu) == Some(&LeaderStatus::Readied));
    ssruu && core.players().iter().any(|p| matches!(p.leaders.status.get(&Leader::Tro), Some(LeaderStatus::Readied | LeaderStatus::Exhausted)))
}

/// Whether T'ro (a readied agent of a Sardakk seat, or a copy a Yssaril Ssruu makes) or Field Marshal Mercer would be offered at the end of the
/// action: neither is a rule of the core table yet, so an offer would be missing silently. T'ro needs an infantry in the actor's reinforcements and
/// a planet the actor controls in the active system (`sardakk::agent_event_window`); Mercer is named whenever it is readied.
fn agent_listens(core: &CoreState, scope: crate::tables::Scope, actor: Seat, system: System, lends: bool) -> bool {
    let tro_readied = core.players().iter().any(|p| p.leaders.status.get(&Leader::Tro) == Some(&LeaderStatus::Readied));
    let mercer = core.players().iter().any(|p| p.leaders.status.get(&Leader::FieldMarshalMercer) == Some(&LeaderStatus::Readied));
    let lands = board::planets_of(core, system).any(|p| board::controls(core, actor, p));
    // Mercer: "up to two ground forces" of the actor to a planet it controls in the active system (`nomad_agents::mercer_window`).
    let ground_force = |u: &crate::state::Piece| u.owner() == actor && matches!(u.kind().data().base_type, crate::generated::UnitBaseType::Infantry | crate::generated::UnitBaseType::Mech);
    let has_ground_forces = core.board().systems().any(|(sys, cell)| cell.units.as_slice().iter().any(ground_force) || board::planets_of(core, sys).any(|p| board::planet_units(core, p).iter().any(ground_force)));
    if mercer && lands && has_ground_forces {
        return true;
    }
    if !(tro_readied || lends) {
        return false;
    }
    fleet::remaining_all(core, scope, actor, Unit::InfantryI) > 0 && lands
}

/// The Crown of Emphidia and Dark Energy Tap (`close_tactical`): the Crown's holder may explore a planet it controls (the deck is the planet's
/// printed type), then a frontier token in the active system is explored when the seat has Dark Energy Tap and a ship there.
fn explorations(sx: &mut Sx<'_>, player: Seat, system: System) -> Flow {
    let scope = reach::scope_of(sx.kernel);
    if tail::crown_offers(sx.state, player, scope) {
        let mut crown = CrownExplore::new(player, scope);
        if run_window(sx, &mut crown) == Flow::Halt {
            return Flow::Halt;
        }
        if let Some(planet) = crown.planet {
            let Some(deck) = crate::query::explore::traits_of(scope, planet).first().copied() else { return Flow::Done };
            let mut window = Exploring::planet(player, planet, EDeck::Fixed(deck), true);
            if run_window(sx, &mut window) == Flow::Halt {
                return Flow::Halt;
            }
        }
    }
    if tail::det_fires(sx.state, player, system) {
        let mut window = Exploring::frontier(player, system, true, true);
        if run_window(sx, &mut window) == Flow::Halt {
            return Flow::Halt;
        }
        if window.explored() {
            sx.plain(PlainRec::bare(PlainKind::FrontierExplored));
        }
    }
    Flow::Done
}

pub(super) fn run(a: &mut TacticalAction, sx: &mut Sx<'_>) -> Next {
    let active = sx.state.meta().active;
    let system = sx.state.meta().active_system;
    if let (Some(player), Some(system)) = (active, system) {
        if explorations(sx, player, system) == Flow::Halt {
            return Next::Done;
        }
    }
    if halted(sx) {
        return Next::Done;
    }
    let lends = ssruu_lends(sx.state);
    if let (Some(player), Some(system)) = (active, system)
        && agent_listens(sx.state, reach::scope_of(sx.kernel), player, system, lends)
    {
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
    crate::kernel::fx_move::clear_conduit_links(sx.state);
    sx.plain(PlainRec::bare(PlainKind::TacticalActionComplete));
    let pending = sx.state.pending();
    match tail::then(pending.window(WindowKind::LeaderStrategy).is_some(), pending.window(WindowKind::StrategySecondaryAfterTactical).is_some()) {
        tail::Then::LeaderStrategy => TacticalAction::bail(sx, TacticalGap::LeaderStrategy),
        tail::Then::Secondary => TacticalAction::bail(sx, TacticalGap::WarfareSecondary),
        tail::Then::FinishAction => {
            let _ = a;
            // `finish_action`: `ACTION_COMPLETED`, then the closing menu or the end of the turn, is the turn flow's job.
            crate::driver::turn::open_finish(sx.state);
            Next::Done
        }
    }
}
