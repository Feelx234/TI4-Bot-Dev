//! Oracle-free tests of the typed core (docs/todo/D4-core-tests.md).
//!
//! This crate holds no production code. `src/` is the **kit**, the typed fixtures every rule test shares:
//!
//! * [`scenario`]: `Scenario`, a builder of a real `CoreState` (real factions only, a map, pieces, tokens, hands);
//! * [`kernel`]: `kernel_all()` (the faction hook table filled) and the deliberately named `kernel_plain()`;
//! * [`rig`]: `Rig`, a driver wrapper that records the questions, probes every option and checks the contract;
//! * [`delta`]: the canonical text of what a run changed;
//! * [`snap`]: file snapshots (`BLESS=1` rewrites them; a missing or stale one fails);
//! * [`inv`]: the invariants (conservation) the properties check;
//! * [`generators`]: proptest strategies and the deterministic scenario generator;
//! * [`props`]: the core-level properties P1-P6 as plain functions over a window factory;
//! * [`smoke`]: the standard rule test of one window position (four answer policies, contract and conservation at every step, snapshot);
//! * [`covers`]: the `covers!` registry the coverage report reads.
//!
//! Tests run in the default (dev) profile, with debug assertions on: the `CoreState` slice accessors
//! `debug_assert!` that the slice is loaded, which is what makes a window that reads an unloaded slice fail here.

pub mod covers;
pub mod delta;
pub mod generators;
pub mod inv;
pub mod kernel;
pub mod props;
pub mod rig;
pub mod scenario;
pub mod smoke;
pub mod snap;

pub use snap::assert_snapshot;

/// What nearly every test imports.
pub mod prelude {
    pub use crate::delta::delta;
    pub use crate::kernel::{kernel_all, kernel_plain};
    pub use crate::rig::Rig;
    pub use crate::scenario::{LATEST_RULES, LOBBY, Scenario};
    pub use crate::smoke::{smoke, smoke_quiet};
    pub use crate::snap::assert_snapshot;
    pub use crate::covers_slot;
    pub use crate::{covers, covers_card};
    pub use ti4_engine_core::choice::{Ask, Choice, Opt, Pick};
    pub use ti4_engine_core::driver::{Driver, DriverError, Status};
    pub use ti4_engine_core::generated::*;
    pub use ti4_engine_core::ids::Seat;
    pub use ti4_engine_core::kernel::stage::Window;
    pub use ti4_engine_core::kernel::Kernel;
    pub use ti4_engine_core::state::{CoreState, Phase};
}
