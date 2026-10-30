//! `Seen`: what the rule bot may know about a position, typed (docs/todo/D3-bots.md 2.2, package D3-1).
//!
//! The typed counterpart of `ti4_engine::choice::Observed` (public facts) plus the bound seat's own hand
//! (`SeatObservation`'s private facts), over a [`CoreState`]. The accessor list is the list of what the string
//! bot really calls (docs/todo/D3-bots.md 1.2), not "everything":
//!
//! * **Hidden information is a type-level contract.** There is no `Deref` to the state, no accessor that returns
//!   it, no public field; the state is private. Deck order, the RNG streams, the ledger, other seats' hands
//!   (action cards, secrets, promissory notes in hand) and the pending machine are not reachable. The private
//!   accessors (`my_*`) take no seat: they answer for the bound seat [`Seen::me`] only.
//! * Layer: this module reads queries (L2), the hook table (L3) and rule areas (L4) and so sits at L5
//!   (`tools/engine_core/check_layers.py`); the doc's "L2" was not reachable for `available_spend`
//!   (`rules::production::available`) and the movement reach (`query::reach`, L3).
//! * Every slice the accessors read is listed in [`READS`]; a loader must load at least those.
//!
//! ```compile_fail
//! // `Seen` does not dereference to the core state.
//! fn leak(seen: &ti4_engine_core::seen::Seen<'_>) -> &ti4_engine_core::CoreState {
//!     &**seen
//! }
//! ```
//!
//! ```compile_fail
//! // The state is a private field, not an accessor.
//! fn leak<'a>(seen: &ti4_engine_core::seen::Seen<'a>) -> &'a ti4_engine_core::CoreState {
//!     seen.state
//! }
//! ```
//!
//! ```compile_fail
//! // There is no `core()` accessor either.
//! fn leak(seen: &ti4_engine_core::seen::Seen<'_>) {
//!     let _ = seen.core();
//! }
//! ```

mod agenda;
mod board;
mod facts;
mod hand;
mod objectives;
mod reach;

pub use agenda::{AgendaFacts, InvasionFacts};
pub use board::{PlanetView, SystemView};
pub use facts::SeatView;
pub use objectives::{ProgressRow, RowFamily};
pub use reach::MoveCache;

use std::cell::RefCell;

use crate::ids::Seat;
use crate::kernel::Kernel;
use crate::state::{CoreState, Slice, SliceSet};
use crate::tables::Scope;

/// The slices the accessors of [`Seen`] read: a loader loads at least these (the decks, the RNG streams and
/// the ledger are deliberately not among them).
pub const READS: SliceSet = SliceSet::of(&[
    Slice::Meta,
    Slice::Seats,
    Slice::Players,
    Slice::Board,
    Slice::Map,
    Slice::Cards,
    Slice::Objectives,
    Slice::Agenda,
    Slice::Strategy,
    Slice::Explore,
    Slice::Trade,
    Slice::Marks,
    Slice::Transient,
]);

/// The bot's view of one position, for one bound seat.
pub struct Seen<'a> {
    state: &'a CoreState,
    kernel: &'a Kernel,
    me: Seat,
    blanked: bool,
    memo: RefCell<Memo>,
}

/// What a position computes once: the objectives scan per seat (and, with the movement accessors, the reach).
#[derive(Default)]
pub(crate) struct Memo {
    pub(crate) facts: objectives::FactsMemo,
    pub(crate) moves: reach::MoveCache,
}

impl<'a> Seen<'a> {
    /// The view of `state` for the bound seat `me`. `kernel` carries the hook table (faction terms) and the
    /// source scope.
    #[must_use]
    pub fn new(state: &'a CoreState, kernel: &'a Kernel, me: Seat) -> Self {
        Self { state, kernel, me, blanked: false, memo: RefCell::new(Memo::default()) }
    }

    /// The same view with "all laws are blank this turn" (Law's Order) as the caller computes it.
    #[must_use]
    pub fn with_blanked(mut self, blanked: bool) -> Self {
        self.blanked = blanked;
        self
    }

    /// The bound seat: the private accessors answer for it.
    #[must_use]
    pub const fn me(&self) -> Seat {
        self.me
    }

    pub(crate) const fn st(&self) -> &'a CoreState {
        self.state
    }

    pub(crate) const fn kernel(&self) -> &'a Kernel {
        self.kernel
    }

    pub(crate) const fn blanked(&self) -> bool {
        self.blanked
    }

    pub(crate) fn memo(&self) -> &RefCell<Memo> {
        &self.memo
    }

    pub(crate) fn scope(&self) -> Scope {
        Scope(u32::from(self.kernel.sources.0))
    }
}
