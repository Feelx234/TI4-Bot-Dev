//! Interactive branching game replayer primitives (`plans/R02_REPLAYER.md`).
//!
//! This crate is a sibling of `ti4-review`, never a replacement for it. It replays the same
//! checkpoint, map pool, seed, rotation, temperature, profiles and seating through the same
//! `ti4_engine::game::Game`, and adds the two things the reviewer deliberately does not have: a
//! human can take over any physical seat, and any historical frame can be replayed forward as a
//! new child branch while the original future stays intact.
//!
//! Layers, bottom up:
//!
//! - [`control`] — the vocabulary: seat modes, the pending manual choice and its fingerprint, the
//!   replay record, provenance, and the project bounds those types must respect.
//! - [`decider`] — the decorator that answers for one seat, sitting under the reviewer's trace.
//! - [`live`] — one running branch: a thread that owns the game, and the gate every command passes
//!   through, so a manual seat can park inside the engine's own `ask` without mutating anything.
//! - [`rebuild`] — replay a prefix and prove the result, frame by frame, or refuse.
//! - [`project`] — the branch tree and what a branch may be played for.
//! - [`persistence`] — the atomic bounded project file that holds the tree.
//!
//! The native app arrives in a later package, so nothing here opens a window or edits `ti4-engine`;
//! `ti4-review` is used as a library through the seam R02-002 added.
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

pub mod app;
pub mod control;
pub mod decider;
pub mod fingerprint;
pub mod live;
pub mod persistence;
pub mod project;
pub mod rebuild;

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
pub use decider::AnswerLog;
pub use decider::AnsweredDecision;
pub use decider::ControlledDecider;
pub use decider::ManualFallbacks;
pub use decider::ManualInbox;
pub use decider::QueuedAnswer;
pub use fingerprint::FRAME_FINGERPRINT_VERSION;
pub use fingerprint::FrameFingerprint;
pub use fingerprint::first_difference;
pub use fingerprint::state_fingerprint;
pub use live::AdvanceGoal;
pub use live::FrameTick;
pub use live::Gate;
pub use live::LiveBranch;
pub use live::LiveError;
pub use live::LiveEvent;
pub use live::LiveState;
pub use live::MAX_QUEUED_EVENTS;
pub use live::Snapshot;
pub use persistence::MAX_PROJECT_BYTES;
pub use persistence::load_project;
pub use persistence::save_project;
pub use persistence::sha256_file;
pub use project::Branch;
pub use project::MAX_TOTAL_FRAMES;
pub use project::Origin;
pub use project::PROJECT_SCHEMA;
pub use project::PROJECT_VERSION;
pub use project::ProjectError;
pub use project::ReplayInputs;
pub use project::ReplayerProject;
pub use project::SeatSetting;
pub use project::SourceTimeline;
pub use project::Verification;
pub use rebuild::Mismatch;
pub use rebuild::MismatchKind;
pub use rebuild::RebuildBounds;
pub use rebuild::RebuildError;
pub use rebuild::RebuildTarget;
pub use rebuild::Rebuilt;
pub use rebuild::ReplayScript;
pub use rebuild::describe_frame_difference;
pub use rebuild::rebuild;
