//! Interactive branching game replayer primitives (`plans/R02_REPLAYER.md`).
//!
//! This crate is a sibling of `ti4-review`, never a replacement for it. It replays the same
//! checkpoint, map pool, seed, rotation, temperature, profiles and seating through the same
//! `ti4_engine::game::Game`, and adds the two things the reviewer deliberately does not have: a
//! human can take over any physical seat, and any historical frame can be replayed forward as a
//! new child branch while the original future stays intact.
//!
//! This package is the vocabulary only — seat modes, the pending manual choice and its
//! fingerprint, the replay record, provenance, and the project bounds those types must respect.
//! Stepping, reconstruction, persistence and the native app arrive in later packages, so nothing
//! here opens a window, runs a game, or edits `ti4-engine` or `ti4-review`.
//!
//! # Rules these types enforce
//!
//! Control is keyed by the *physical* [`ti4_model::id::PlayerId`], never by faction, because a
//! faction rotating around a seat must not silently inherit that seat's control mode. A manual
//! seat is shown exactly the options the engine offered, and a submission is accepted only when it
//! carries the fingerprint of the choice currently on screen and names an option actually in that
//! ordered list. Stale, changed, duplicate and invented answers are refused without consuming the
//! pending choice, so a click that arrived a step too late can never be applied to a new decision.
//!
//! Nothing here mutates game state. The engine consumes answers through its own
//! `Table::ask`/`settle` validation; these types only describe what was offered and what the
//! human chose, and record who answered.

pub mod control;

pub use control::BranchId;
pub use control::BranchIds;
pub use control::CHOICE_FINGERPRINT_VERSION;
pub use control::ChoiceFingerprint;
pub use control::ControlError;
pub use control::MAX_BRANCHES_PER_PROJECT;
pub use control::MAX_OFFERED_OPTIONS;
pub use control::ManualControl;
pub use control::ManualSubmission;
pub use control::ModeEffect;
pub use control::OfferedOption;
pub use control::PendingManualChoice;
pub use control::Provenance;
pub use control::ReplayRecord;
pub use control::SeatControl;
pub use control::SeatMode;
pub use control::SubmitOutcome;
