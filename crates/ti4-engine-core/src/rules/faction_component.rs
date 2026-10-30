//! The faction component actions that the core owns (package W11, lent-bodies lane 5): each is a function over the state or a
//! [`crate::kernel::stage::Window`] in its own file section. The listing of the actions and the dispatch (`perform_component`)
//! are the driver's (D1b); the adapter calls the bodies from the string dispatch and compares.

use crate::generated::{System, Technology, Unit};
use crate::ids::Seat;
use crate::kernel::{fx, fx_strategy};
use crate::query::{board, fleet};
use crate::state::CoreState;
use crate::tables::Scope;

/// Lazax Gate Folding (Winnu): "ACTION: Exhaust this card to place 1 infantry from your reinforcements on Mecatol Rex" (when the
/// seat controls it). No question: the planet is the first the seat controls in the Mecatol system (`mecatol`), the unit is the
/// form the seat places (`None`: it places nothing, or a faction rule forbids it). Returns whether the action resolved: the card is
/// ready, the seat controls a planet there and the box holds an infantry for it; the card is exhausted even when the unit then
/// cannot be placed (the string exhausts first), in which case the action did not resolve.
pub fn lazax_gate_folding(core: &mut CoreState, scope: Scope, who: Seat, mecatol: System, form: Option<Unit>) -> bool {
    let ready = core.players().by_seat(who).is_some_and(|p| p.tech.owned.contains(Technology::LazaxGateFolding) && !p.tech.exhausted.contains(Technology::LazaxGateFolding));
    if !ready {
        return false;
    }
    let Some(planet) = board::controlled_planets(core, who).as_slice().iter().find(|(system, _)| *system == mecatol).map(|(_, planet)| *planet) else {
        return false;
    };
    let Some(unit) = form else { return false };
    if fleet::remaining_all(core, scope, who, unit) <= 0 {
        return false;
    }
    fx_strategy::exhaust_technology(core, who, Technology::LazaxGateFolding);
    fx::place_unit(core, who, mecatol, Some(planet), unit);
    true
}
