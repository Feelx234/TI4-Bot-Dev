//! Decision types of the `objectives` area: scoring and the secret objective hand limit.
//!
//! Owner: package A4 (docs/ENGINE_CORE_STATE.md 4.4). Ported: "score an objective" (rule 61.6, the status
//! window and the action- and agenda-timed secret windows) and "return a secret objective to the deck" (rule
//! 45.4). The adapter side is `ti4-engine/src/render/objectives.rs`.

use crate::generated::SecretObjective;
use crate::state::ObjectiveRef;

/// What the area asks. Parameters only; the option list is derived by `opts(state, ask)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// "Score an objective": the seat's victory points now (an option previews one more).
    Score { vp: u8 },
    /// "Return a secret objective to the deck": secrets held plus scored (an option previews one less).
    ReturnSecret { held: u8 },
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// Score this objective (a public one, or a secret: in hand, or made public by Classified Document Leaks).
    Score(ObjectiveRef),
    /// Return this secret objective.
    Return(SecretObjective),
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    /// The status window offers publics and secrets in one list.
    ScoreObjective,
    /// An action- or agenda-timed window offers only the secret its event satisfied.
    ScoreSecretObjective,
    ReturnOverSecretHandLimit,
}
