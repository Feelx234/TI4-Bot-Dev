//! The closing menu of a turn (`Game::closing_options`): "end your turn" first, then what takes no action, and with Fleet Logistics a
//! second action. The assembly is `driver::menu::closing_menu`; the options of the sources it needs are the action menu's (lane F, with
//! the trade lane for the transactions).
//!
//! Native today: the closing menu that holds only "end your turn" (nothing else is free: no transaction partner, no leader printed
//! "During the action phase", no second action). Where any source could offer a free option the function stops with
//! [`TurnGap::MenuSources`] -- the typed options of those sources (`Opt::Open(partner)`, a leader's component action) are not the turn
//! flow's to invent -- and lane F replaces this gate with the menu assembly over the sources it ports.

use crate::choice::turn::Opt as TOpt;
use crate::driver::menu;
use crate::generated::{LeaderWindow, System};
use crate::ids::Seat;
use crate::kernel::turn_codes::TurnGap;
use crate::kernel::Kernel;
use crate::query::trade::{self, Cx as TradeCx, Limits};
use crate::query::reach;
use crate::rules::leaders;
use crate::rules::trade::KernelEnv;
use crate::state::CoreState;

/// `ti4_model::state::MAX_NEGOTIATIONS_PER_ACTION` and `_PER_ROUND`.
const LIMITS: Limits = Limits { per_action: 2, per_round: 6 };

/// Whether `seat` has a partner to open a transaction with (`transactions::available_actions` is not empty). Transactions need a map.
fn has_trade_partner(core: &CoreState, k: &Kernel, seat: Seat) -> bool {
    // A position lent without its map (the lockstep harness) has no map to trade over.
    if !core.loaded.has(crate::state::Slice::Map) {
        return false;
    }
    let Some(galaxy) = core.galaxy() else { return false };
    let adj = galaxy.adjacency();
    let mecatol = if galaxy.coord_of(System::MecatolRex112).is_some() { System::MecatolRex112 } else { System::MecatolRex18 };
    let env = KernelEnv { core, kernel: k };
    let cx = TradeCx { core, adj: &adj, mecatol, limits: LIMITS, env: &env };
    !trade::open_partners(&cx, seat).is_empty()
}

/// A leader the seat can use now whose printed window is the action phase, not the action: it leaves the turn's action untouched
/// (`Game::is_free_option`).
fn has_free_leader(core: &CoreState, seat: Seat) -> bool {
    leaders::usable(core, seat).as_slice().iter().any(|l| l.data().window == LeaderWindow::ActionPhase)
}

/// The options of the closing menu of `seat`, "end your turn" first.
///
/// # Errors
/// [`TurnGap::MenuSources`]: a source of the action menu could offer a free option and the core does not assemble it yet.
pub fn options(core: &CoreState, k: &Kernel, seat: Seat) -> Result<Vec<TOpt>, TurnGap> {
    let _ = reach::blanked(core);
    if menu::second_action(core, seat, core.pending().actions_this_turn) || has_trade_partner(core, k, seat) || has_free_leader(core, seat) {
        return Err(TurnGap::MenuSources);
    }
    Ok(vec![TOpt::EndTurn])
}
