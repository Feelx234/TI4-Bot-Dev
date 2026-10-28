//! Decision types of the `strategy` area: strategy phase, strategy cards, status phase, draft.
//!
//! Owner: package B8 (docs/ENGINE_CORE_STATE.md 4.4). Created empty by F4: the three enums below are
//! uninhabited placeholders. The owner replaces them by real variants carrying what the option needs
//! to be applied (typed ids, counts, seats), never text; every type stays `Copy`. The adapter side is
//! `ti4-engine/src/render/strategy.rs`.

/// What the area asks. Parameters only; the option list is derived by `opts(state, ask)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// Trade primary: "let another player replenish commodities" (one seat per question, until none is
    /// left that can or the holder is done).
    TradeReplenish,
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// Trade primary: this seat replenishes (the end of the list is the generic `Done`).
    Replenish(crate::ids::Seat),
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    TradeReplenish,
}
