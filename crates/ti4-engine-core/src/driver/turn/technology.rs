//! The technology component actions of the action menu (`technology::perform_component`): Sling Relay, the use of PRODUCTION as an ability. The
//! core's `SlingRelay` window asks the dock system, the ship and the payment through the tape; the technology exhausts when a ship was produced. The
//! technology arm of the string announces nothing after its body (`conclude`: no staged announcements).

use super::component::{conclude_quiet, verdict};
use super::inline::{gap_flow, run as run_window, Flow};
use crate::generated::Technology;
use crate::ids::Seat;
use crate::kernel::fx_strategy;
use crate::kernel::stage::Sx;
use crate::kernel::turn_codes::{MenuGap, TurnGap};
use crate::query::reach;
use crate::rules::production::triggered::SlingRelay;

/// Whether the technology's component action has a native body ([`run`] never ends it in the counted gap).
#[must_use]
pub const fn routed(tech: Technology) -> bool {
    matches!(tech, Technology::SlingRelay)
}

/// Run the component action of `tech` chosen as option `key`.
pub fn run(sx: &mut Sx<'_>, who: Seat, key: u32, tech: Technology) -> Flow {
    match tech {
        Technology::SlingRelay => sling_relay(sx, who, key),
        other => gap_flow(sx, TurnGap::Menu(MenuGap::Unrouted(6, crate::choice::turn::Opt::Tech(other).key().unwrap_or(0)))), // mutants:skip (the key default, and the kind): every option on the menu has a key; Sling Relay is the only technology the menu lists
    }
}

/// Sling Relay: "ACTION: Exhaust this card to produce 1 ship at one of your space docks" (the dock's PRODUCTION value is not consumed).
fn sling_relay(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let Some(roster) = sx.table.facts.plays.as_ref().and_then(|p| p.roster) else { return gap_flow(sx, TurnGap::NoRoster) };
    let Some(buildable) = crate::rules::production::probe::buildable_for(sx.state, sx.kernel, reach::scope_of(sx.kernel), roster, who) else {
        return gap_flow(sx, TurnGap::NoRoster);
    };
    let mut window = SlingRelay::new(who, buildable);
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    if window.produced {
        fx_strategy::exhaust_technology_text(sx.state, who, Technology::SlingRelay);
    }
    conclude_quiet(sx, who, key, verdict(window.produced))
}
