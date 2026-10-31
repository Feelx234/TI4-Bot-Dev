//! The facts the turn-flow jobs build from the position alone (the string adapters `tactical_dual::limits_of` and the Minister's
//! system list read the string `GameState` and the content store).
//!
//! L5: reads the fleet, the laws, the abilities.

use crate::driver::kernel_env::KernelEnv;
use crate::generated::{System, Unit};
use crate::ids::Seat;
use crate::kernel::Kernel;
use crate::query::{board, fleet, laws, reach};
use crate::rules::abilities;
use crate::rules::tactical::fleet::Limits;
use crate::state::CoreState;

/// The rules version from which an unlimited fleet supply (Darktalon Treilla's hero) lifts the enforcement too
/// (`ti4_model::state::RULES_FACTION_BUGFIXES`).
const RULES_FACTION_BUGFIXES: u32 = 17;

/// The fleet supply of a seat as the rules compute it: the pool (with the tokens other seats hold in it), capped by Fleet Regulations,
/// lifted by Armada and the faction modules (`fleet::limit`).
#[must_use]
pub fn fleet_limit(core: &CoreState, k: &Kernel, seat: Seat) -> i32 {
    let foreign = k.hooks.direct.mahact_foreign_tokens(core, seat);
    let base = fleet::pool_base(core, seat, foreign);
    let capped = laws::fleet_pool_cap(core, reach::blanked(core), base);
    let env = KernelEnv::new(core, k);
    abilities::fleet_supply(&abilities::Cx { core, env: &env }, seat, capped)
}

/// The limits the enforcement of `seat` in `system` judges by (`tactical_dual::limits_of`). At most two half-weight fighter forms are
/// kept (the core `Limits` holds two; no table has more).
#[must_use]
pub fn limits(core: &CoreState, k: &Kernel, seat: Seat, system: System) -> Limits {
    let unlimited = fleet::is_unlimited(core, seat);
    let bill = fleet::UNLIMITED_FLEET_BILL;
    let limit = i64::from(fleet_limit(core, k, seat)).max(0);
    let enforce = if core.meta().rules_at_least(RULES_FACTION_BUGFIXES) && unlimited { bill } else { limit };
    let shown = if unlimited { bill } else { limit };
    let mut forms: Vec<Unit> = board::space_units(core, system).iter().filter(|p| p.owner() == seat).map(|p| p.kind()).collect();
    forms.sort_unstable_by_key(|u| *u as usize);
    forms.dedup();
    let mut halves = [None, None];
    let mut found = 0;
    for unit in forms {
        if k.hooks.combat.fighter_fleet_weight_halves(core, k, seat, unit) {
            if found >= halves.len() {
                break;
            }
            halves[found] = Some(unit);
            found += 1;
        }
    }
    Limits { enforce, shown, halves }
}

/// The `(seat, system, limits)` pairs of `fleet::enforce_everywhere`: every seat in table order, every system in board order where the
/// seat has a unit in the space area.
#[must_use]
pub fn enforce_pairs(core: &CoreState, k: &Kernel) -> Vec<(Seat, System, Limits)> {
    let seats: Vec<Seat> = core.players().iter().map(|p| p.seat).collect();
    let mut out = Vec::new();
    for seat in seats {
        for (system, cell) in core.board().systems() {
            if cell.units.as_slice().iter().any(|u| u.owner() == seat) {
                out.push((seat, system, limits(core, k, seat, system)));
            }
        }
    }
    out
}
