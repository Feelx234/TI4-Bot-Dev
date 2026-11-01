//! From `production_live.rs` of the legacy crate: the texts the core production composer needs from outside, and the words of the positions
//! it does not run (copy; the lockstep tests stay in the legacy crate).

use ti4_engine_core::kernel::ability::ProductionGap;
use ti4_engine_core::kernel::stage::AbilityTable;

use crate::render::Render;

/// Fill the production texts of `table`: the `source` of `UNITS_PRODUCED` and the payload key of its `units`, the adapter's event texts.
pub fn fill_codes(table: &mut AbilityTable, r: &Render) {
    let c = &mut table.codes.production;
    c.ability = r.code("ability");
    c.production = r.code("production");
    c.units = r.code("units");
}

/// The words of a position the native production step does not run (`Unknown::UnportedProduction`).
#[must_use]
pub const fn gap_text(why: ProductionGap) -> &'static str {
    match why {
        ProductionGap::NoRoster => "the table has no faction unit roster, so the buildable units cannot be named",
        ProductionGap::NoFacts => "the facts of the use of PRODUCTION cannot be read from the position",
        ProductionGap::NoLink => "the table has no link to Self-Assembly Routines' window",
        ProductionGap::ChildPassed => "a window of the start or the end of a use of PRODUCTION asked the driver for something that is no question",
        ProductionGap::ChildStuck => "a window of the start or the end of a use of PRODUCTION asks a question its answer does not settle",
        ProductionGap::NoProgress => "the production step made no progress (a stage that neither asks nor moves on)",
    }
}
