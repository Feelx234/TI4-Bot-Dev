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
//! With default features off, only [`control`], [`fingerprint`] and the client half of [`net`]
//! remain: the remote online seat (`ti4-join`), which builds without the reviewer's simulation and
//! so without libtorch.
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

#[cfg(feature = "host")]
pub mod app;
pub mod control;
#[cfg(feature = "host")]
pub mod decider;
pub mod fingerprint;
#[cfg(feature = "host")]
pub mod gui;
#[cfg(feature = "host")]
pub mod live;
pub mod net;
#[cfg(feature = "host")]
pub mod persistence;
#[cfg(feature = "host")]
pub mod project;
#[cfg(feature = "host")]
pub mod rebuild;
#[cfg(feature = "host")]
pub mod store;

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
#[cfg(feature = "host")]
pub use decider::AnswerLog;
#[cfg(feature = "host")]
pub use decider::AnsweredDecision;
#[cfg(feature = "host")]
pub use decider::ControlledDecider;
#[cfg(feature = "host")]
pub use decider::ManualFallbacks;
#[cfg(feature = "host")]
pub use decider::ManualInbox;
#[cfg(feature = "host")]
pub use decider::QueuedAnswer;
pub use fingerprint::FRAME_FINGERPRINT_VERSION;
pub use fingerprint::FrameFingerprint;
pub use fingerprint::first_difference;
pub use fingerprint::state_fingerprint;
#[cfg(feature = "host")]
pub use live::AdvanceGoal;
#[cfg(feature = "host")]
pub use live::Feed;
#[cfg(feature = "host")]
pub use live::FrameTick;
#[cfg(feature = "host")]
pub use live::Gate;
#[cfg(feature = "host")]
pub use live::LiveBranch;
#[cfg(feature = "host")]
pub use live::LiveError;
#[cfg(feature = "host")]
pub use live::LiveEvent;
#[cfg(feature = "host")]
pub use live::LiveState;
#[cfg(feature = "host")]
pub use live::MAX_QUEUED_EVENTS;
#[cfg(feature = "host")]
pub use live::ReplayRequest;
#[cfg(feature = "host")]
pub use live::Snapshot;
#[cfg(feature = "host")]
pub use persistence::MAX_PROJECT_BYTES;
#[cfg(feature = "host")]
pub use persistence::load_project;
#[cfg(feature = "host")]
pub use persistence::save_project;
#[cfg(feature = "host")]
pub use persistence::{sha256_file, sha256_path};
#[cfg(feature = "host")]
pub use project::Branch;
#[cfg(feature = "host")]
pub use project::MAX_TOTAL_FRAMES;
#[cfg(feature = "host")]
pub use project::Origin;
#[cfg(feature = "host")]
pub use project::PROJECT_SCHEMA;
#[cfg(feature = "host")]
pub use project::PROJECT_VERSION;
#[cfg(feature = "host")]
pub use project::ProjectError;
#[cfg(feature = "host")]
pub use project::ReplayInputs;
#[cfg(feature = "host")]
pub use project::ReplayerProject;
#[cfg(feature = "host")]
pub use project::SeatSetting;
#[cfg(feature = "host")]
pub use project::SourceTimeline;
#[cfg(feature = "host")]
pub use project::Verification;
#[cfg(feature = "host")]
pub use rebuild::Mismatch;
#[cfg(feature = "host")]
pub use rebuild::MismatchKind;
#[cfg(feature = "host")]
pub use rebuild::RebuildBounds;
#[cfg(feature = "host")]
pub use rebuild::RebuildError;
#[cfg(feature = "host")]
pub use rebuild::RebuildTarget;
#[cfg(feature = "host")]
pub use rebuild::Rebuilt;
#[cfg(feature = "host")]
pub use rebuild::ReplayScript;
#[cfg(feature = "host")]
pub use rebuild::describe_frame_difference;
#[cfg(feature = "host")]
pub use rebuild::rebuild;
#[cfg(feature = "host")]
pub use rebuild::rebuild_with_gate;
