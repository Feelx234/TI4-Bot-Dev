//! Decision types of the `tactical` area: movement, activation, tactical action, transit.
//!
//! Owner: package A2 (docs/ENGINE_CORE_STATE.md 4.4). Created empty by F4: the three enums below are
//! uninhabited placeholders. The owner replaces them by real variants carrying what the option needs
//! to be applied (typed ids, counts, seats), never text; every type stays `Copy`. The adapter side is
//! `ti4-engine/src/render/tactical.rs`.

/// What the area asks. Parameters only; the option list is derived by `opts(state, ask)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {}
