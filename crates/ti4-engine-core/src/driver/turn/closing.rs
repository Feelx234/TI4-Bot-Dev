//! The closing menu of a turn (`Game::closing_options`): "end your turn" first, then what takes no action, and with Fleet Logistics a
//! second action. The assembly is `driver::menu::closing_menu` over the entries of the action menu ([`super::action_menu`]); its sources
//! are listed by the core (lane F), so the menu is always the core's. Where a source cannot be listed for the position the function
//! stops with the counted gap [`TurnGap::Menu`].

use crate::choice::turn::Opt as TOpt;
use crate::ids::Seat;
use crate::kernel::turn_codes::TurnGap;
use crate::kernel::Kernel;
use crate::query::tech::UnitRoster;
use crate::state::CoreState;

/// The options of the closing menu of `seat`, "end your turn" first.
///
/// # Errors
/// [`TurnGap::Menu`]: a source of the action menu cannot be listed for the position.
pub fn options(core: &CoreState, k: &Kernel, roster: &UnitRoster, seat: Seat) -> Result<Vec<TOpt>, TurnGap> {
    super::action_menu::closing(core, k, roster, seat).map_err(TurnGap::Menu)
}
