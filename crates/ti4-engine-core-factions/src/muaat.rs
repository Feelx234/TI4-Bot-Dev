//! The Embers of Muaat: Gashlai Physiology and Magmus Reactor.

use ti4_engine_core::generated::{Faction, Technology};
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::state::CoreState;

use crate::common::{has_tech_text, is_faction, player};

pub fn register(t: &mut HookTable) {
    t.movement.may_enter_supernova.push(may_enter_supernova);
    t.movement.may_pass_through_supernova.push(may_pass_through_supernova);
}

fn is_muaat(s: &CoreState, who: Seat) -> bool {
    is_faction(s, who, Faction::TheEmbersOfMuaat)
}

/// `holds_reactor`: the technology, or a Valefar Assimilator standing for it (any value of the map).
fn holds_reactor(s: &CoreState, who: Seat) -> bool {
    has_tech_text(s, who, Technology::MagmusReactor)
        || player(s, who).is_some_and(|p| p.tech.assimilated.values().any(|t| t == Technology::MagmusReactor))
}

fn may_enter_supernova(s: &CoreState, _: &Kernel, who: Seat) -> bool {
    is_muaat(s, who) && holds_reactor(s, who)
}

fn may_pass_through_supernova(s: &CoreState, _: &Kernel, who: Seat) -> bool {
    is_muaat(s, who)
}
