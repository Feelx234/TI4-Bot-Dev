//! The Deepwrought Scholarate: D.W.S. Luminous moves through its own units and earns steps.

use ti4_engine_core::generated::Unit;
use ti4_engine_core::hooks::{HookTable, PassSite};
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::state::CoreState;

use crate::common::flagship_has_text;

pub fn register(t: &mut HookTable) {
    t.movement.own_unit_passage.push(own_unit_passage);
}

/// Does not read the active system.
fn own_unit_passage(s: &CoreState, _: &Kernel, site: PassSite) -> bool {
    flagship_has_text(s, site.player, site.ship, Unit::DWSLuminous)
}
