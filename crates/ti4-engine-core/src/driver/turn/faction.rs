//! The faction component actions of the action menu (`faction_abilities::perform_component` and the modules' `perform_component`): each
//! code of [`crate::kernel::hooks::components`] is routed to the rule or the window that runs it, the facts computed from the core. An
//! action whose window is not native yet is the counted gap [`MenuGap::Unrouted`] (named by the option's key).

use super::component::{conclude, Verdict};
use super::inline::{gap_flow, run as run_window, Flow};
use crate::choice::turn::{ComponentAction, Opt};
use crate::generated::UnitBaseType;
use crate::ids::Seat;
use crate::kernel::hooks::components as c;
use crate::kernel::stage::Sx;
use crate::kernel::turn_codes::{MenuGap, TurnGap};
use crate::kernel::fx_strategy;
use crate::query::reach;

/// The unit roster of the table's play facts (the forms a seat places).
fn roster(sx: &Sx<'_>) -> Option<&'static crate::query::tech::UnitRoster> {
    sx.table.facts.plays.as_ref().and_then(|p| p.roster)
}

/// Run the faction component action `action` chosen as option `key`.
pub fn run(sx: &mut Sx<'_>, who: Seat, key: u32, action: ComponentAction) -> Flow {
    match action.code {
        c::LAZAX_GATE_FOLDING => lazax_gate_folding(sx, who, key),
        c::ORBITAL_DROP => orbital_drop(sx, who, key),
        _ => gap_flow(sx, TurnGap::Menu(MenuGap::Unrouted(super::action::gap_code(&crate::driver::dispatch::Dispatch::Faction(action)), Opt::Faction(action).key().unwrap_or(0)))),
    }
}

/// Lazax Gate Folding (Winnu): the infantry goes to Mecatol Rex (`winnu::lazax_gate_folding_string`).
fn lazax_gate_folding(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    let Some(roster) = roster(sx) else { return gap_flow(sx, TurnGap::NoRoster) };
    let scope = reach::scope_of(sx.kernel);
    let form = fx_strategy::placed_unit(sx.state, sx.kernel, scope, roster, who, UnitBaseType::Infantry)
        .filter(|unit| !sx.kernel.hooks.economy.effect_placement_forbidden(sx.state, sx.kernel, who, *unit));
    let mecatol = crate::query::objectives::mecatol_on(sx.state);
    let done = crate::rules::faction_component::lazax_gate_folding(sx.state, scope, who, mecatol, form);
    conclude(sx, who, key, if done { Verdict::Resolved } else { Verdict::Failed { withhold: true } }, true)
}

/// Orbital Drop (Sol; `faction_abilities::orbital_drop`): the core window asks the planet, spends the token, places the infantry and offers the mech.
/// It asks through the tape; a table without a link to the payment window is the counted gap `NoLink`.
fn orbital_drop(sx: &mut Sx<'_>, who: Seat, key: u32) -> Flow {
    use crate::generated::Unit;
    use crate::rules::faction_component::{OrbitalDrop, OrbitalFacts};
    let Some(roster) = roster(sx) else { return gap_flow(sx, TurnGap::NoRoster) };
    let scope = reach::scope_of(sx.kernel);
    let infantry = fx_strategy::placed_unit(sx.state, sx.kernel, scope, roster, who, UnitBaseType::Infantry)
        .filter(|unit| !sx.kernel.hooks.economy.effect_placement_forbidden(sx.state, sx.kernel, who, *unit));
    let mech = scope.unit(Unit::ZSThunderboltM2).map(|_| Unit::ZSThunderboltM2);
    let can_pay = crate::driver::menu_cards::resources(sx.state, sx.kernel, who) >= 3;
    let facts = OrbitalFacts { infantry, mech, can_pay, scope, source: sx.table.codes.turn.orbital_drop };
    let mut window = OrbitalDrop::new(who, facts);
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    if window.unsupported {
        return gap_flow(sx, TurnGap::NoLink);
    }
    // (The token the window spent is announced at once by the string.)
    if super::component::flush_staged_events(sx) == Flow::Halt {
        return Flow::Halt;
    }
    conclude(sx, who, key, if window.done { Verdict::Resolved } else { Verdict::Failed { withhold: true } }, true)
}
