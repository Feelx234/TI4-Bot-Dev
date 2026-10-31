//! The sources of the action menu the core lists itself (package F2, KM1) behind the string interface: the typed options of the strategic
//! action and of the leaders' component actions of a string position, read by the core from the position and the live hook table
//! (`ti4_engine_core::driver::menu_sources`). `menu_dual` builds the action menu from these instead of from the ids the string lists, and
//! compares the result with the string's own question; the bridge compares the lists themselves over positions
//! (`check_menu_sources`).

use ti4_engine_core::choice::turn::Opt as TOpt;
use ti4_engine_core::driver::menu_sources;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::state::{Slice, SliceSet};
use ti4_model::content_types::SourceSet;
use ti4_model::state::GameState;

use crate::core_sync::{CoreView, Ctx, Sources};

/// What the core lists of the action menu's sources for one seat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Native {
    /// The strategic action options, or the lone "pass".
    pub strategic: Vec<TOpt>,
    /// The leaders' component actions (own, then Ssruu's copies).
    pub leaders: Vec<TOpt>,
    /// The technologies' component actions (Sling Relay).
    pub technology: Vec<TOpt>,
}

/// Every slice but the random streams and the map (the hooks the sources ask read most of the position; none needs the galaxy, which the
/// caller of a dual check does not hold).
fn reads() -> SliceSet {
    let all: Vec<Slice> = Slice::ALL.iter().copied().filter(|s| !matches!(s, Slice::Rng | Slice::Map)).collect();
    SliceSet::of(&all)
}

/// The sources of `seat`'s action menu, read by the core. `None`: the position is not one the core can hold (the string's lists stand).
#[must_use]
pub fn native(gs: &GameState, cx: &mut Ctx, sources: SourceSet, seat: Seat) -> Option<Native> {
    // A table with a switched-off faction (Ral Nel, Crimson ...) offers agents the core has no hook for: the string's lists stand.
    if crate::core_sync::unsupported::find(gs).is_some() {
        return None;
    }
    let view = CoreView::of(gs, cx, Sources { galaxy: None, rng: None }, reads()).ok()?;
    let kernel = crate::live::kernel::kernel_for(sources);
    let roster = ti4_engine_core_factions::cd_registry::yin::timing::roster(ti4_engine_core::query::reach::scope_of(kernel));
    Some(Native {
        strategic: menu_sources::strategic(&view, seat),
        leaders: menu_sources::leaders(&view, kernel, seat),
        technology: menu_sources::technology(&view, kernel, &roster, seat),
    })
}

/// The sources of `player`'s action menu as the options the table renders (`ChoiceOption`: id, kind, label): `(strategic, leaders, technology)`.
/// `None`: the position is not one the core holds.
#[must_use]
pub fn rendered(gs: &GameState, sources: SourceSet, player: &ti4_model::id::PlayerId) -> Option<[Vec<crate::choice::ChoiceOption>; 3]> {
    use ti4_engine_core::driver::menu::{self, Source};
    let (mut cx, r) = crate::dual_window::tables(gs)?;
    let seat = r.seat(player.as_str())?;
    let found = native(gs, &mut cx, sources, seat)?;
    let render = |part: Vec<TOpt>, source: Source| {
        let choice = menu::action_menu(seat, &[(source, part)], false, &[]);
        crate::render::Typed::new(&r, choice, crate::render::generic::core_phase(gs.phase), gs.round).rendered.options
    };
    Some([render(found.strategic, Source::Strategic), render(found.leaders, Source::Leaders), render(found.technology, Source::Technology)])
}
