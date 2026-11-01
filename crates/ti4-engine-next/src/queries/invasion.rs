//! `invasion.rs` of the legacy crate, as far as the product surface uses it: whether a planet can be bombarded (the core's
//! `rules::invasion::bombardable` over the kernel with every faction module). A position the core cannot hold answers `false`.

use ti4_content::ContentStore;
use ti4_engine_core::generated::{Leader, Planet, System};
use ti4_engine_core::rules::invasion::Rd;
use ti4_engine_core::state::{Slice, SliceSet};
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlanetId, PlayerId, SystemId};
use ti4_model::state::GameState;

use super::view::Loaded;
use crate::ties::core_adapter::typed;

/// 15.1f: Planetary Shield makes a planet immune to bombardment entirely, but a war sun ignores it, and Arc Secundus takes the shields of the
/// others in its system away; Conventions of War forbids bombardment of a cultural planet outright.
#[must_use]
pub fn bombardable(state: &GameState, _content: &ContentStore, sources: SourceSet, system: &SystemId, planet: &PlanetId, invader: &PlayerId) -> bool {
    bombardable_in(state, sources, system, planet, invader).unwrap_or(false)
}

fn bombardable_in(state: &GameState, sources: SourceSet, system: &SystemId, planet: &PlanetId, invader: &PlayerId) -> Option<bool> {
    let view = Loaded::of(state, None, sources, SliceSet::of_without(SliceSet::ALL, Slice::Map))?;
    let (sys, pl, seat) = (typed::<System>(system.as_str())?, typed::<Planet>(planet.as_str())?, view.seat(invader)?);
    // The L1Z1X commander's ability (the invader's, with the Mahact Imperia and the Alliance notes).
    let held = ti4_engine_core_factions::cc_ground::borrowed::query::has_commander_ability(view.core(), view.kernel(), seat, typed::<Leader>("l1z1xcommander")?);
    let rd = Rd::new(view.core(), view.kernel()).with_blanked(view.blanked()).with_l1z1x_commander(held);
    Some(ti4_engine_core::rules::invasion::bombardable(rd, sys, pl, seat))
}
