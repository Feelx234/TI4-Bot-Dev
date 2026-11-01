//! The product-facing engine crate. Facade stage: everything not declared here is re-exported from
//! `ti4-engine-legacy` (the directory `crates/ti4-engine`, docs/todo/D2-7-legacy-split.md). A module declared in this
//! file shadows the glob re-export of the same name; the crate is filled module by module (docs/todo/D2-8-wrapper.md).
//!
//! One section per lane (section 6.2 of the plan): a lane edits only its own section.

pub use ti4_engine_legacy::*;

// ---- lane A (foundation, 8b1-8b3): generated tables, wire types, glue, Observed ----
/// The generated adapter tables: one source file, included from the legacy directory (not copied; plan 1.4).
#[path = "../../ti4-engine/src/generated/mod.rs"]
pub mod generated;
/// The wire and format types (copies; `tests/wire_parity.rs`).
pub mod wire;
/// The items of the legacy crate that the glue names outside itself (Appendix A of the plan): copies.
pub mod ties;
/// What a decider may see (8b3).
pub mod observed;

// ---- lane B (surface, 8d): constants, queries, previews ----

// ---- lane C (setup, 8c): setup, seating, deck, opening, starting_tech, map_feasibility, fixtures ----

// ---- lane D (wrapper, 8e/8f): game ----

// ---- lane E (product edges) and lane F (gates) add no module here ----
