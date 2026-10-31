//! The closing menu of a turn (`Game::closing_options`): "end your turn" first, then what takes no action, and with Fleet Logistics a
//! second action. The assembly is `driver::menu::closing_menu`; the sources it needs are the action menu's (lane F).

use crate::choice::turn::Opt as TOpt;
use crate::driver::menu;
use crate::ids::Seat;
use crate::kernel::turn_codes::TurnGap;
use crate::kernel::Kernel;
use crate::state::CoreState;

/// The options of the closing menu of `seat`, "end your turn" first.
///
/// # Errors
/// [`TurnGap::MenuSources`]: a source of the action menu the core does not hold yet could offer a free option.
pub fn options(core: &CoreState, k: &Kernel, seat: Seat) -> Result<Vec<TOpt>, TurnGap> {
    let _ = (core, k, seat, menu::ORDER);
    Err(TurnGap::MenuSources)
}
