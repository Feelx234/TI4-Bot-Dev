//! What a decider may see (8b3 of docs/todo/D2-8-wrapper.md): the public position beside a choice, and the view bound to one seat.
//! The type is declared here with the fields the wire code needs; the methods follow in the 8b3 commits.
use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_model::content_types::SourceSet;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

/// The public position, offered to a decider alongside the choice (see the legacy `choice::Observed` for the full account).
#[allow(dead_code, reason = "the methods follow in the 8b3 commits")]
pub struct Observed<'a> {
    pub(crate) state: &'a GameState,
    pub(crate) content: &'a ContentStore,
    pub(crate) sources: SourceSet,
    pub(crate) galaxy: Option<&'a Galaxy>,
}

impl<'a> Observed<'a> {
    /// Wrap a position. Public so tests and sibling crates can build one.
    #[must_use]
    pub fn new(state: &'a GameState, content: &'a ContentStore, sources: SourceSet, galaxy: Option<&'a Galaxy>) -> Self {
        Self { state, content, sources, galaxy }
    }
}

/// An [`Observed`] bound to one seat (the seat of the choice being asked).
pub struct SeatObservation<'a> {
    observed: &'a Observed<'a>,
    acting_seat: PlayerId,
}

impl<'a> SeatObservation<'a> {
    /// Engine-internal binding. The seat is authenticated by the caller's context, never by data a policy-side caller supplies.
    pub(crate) fn bind(observed: &'a Observed<'a>, acting_seat: PlayerId) -> Self {
        Self { observed, acting_seat }
    }

    /// The public position this view is bound to. Public facts only.
    #[must_use]
    pub const fn observed(&self) -> &'a Observed<'a> {
        self.observed
    }

    /// The seat this private observation is bound to.
    #[must_use]
    pub const fn bound_seat(&self) -> &PlayerId {
        &self.acting_seat
    }
}

impl<'a> std::ops::Deref for SeatObservation<'a> {
    type Target = Observed<'a>;

    fn deref(&self) -> &Self::Target {
        self.observed
    }
}
