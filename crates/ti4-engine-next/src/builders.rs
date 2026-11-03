//! Position builders (D2-8 lane C, class M of `docs/todo/D2-8-wrapper.md`).
//!
//! The mutating helpers on a [`ti4_model::state::GameState`] that a game is *set up* and a start
//! position is *built* with: the seating of a faction, the dealing of notes and leaders, a
//! technology gained outright, an objective turned up, a fragment gained, a map edit replayed. The
//! host presets and scenarios (`ti4-host`: `preset`, `view::scenario`, `map`) call them, never the
//! game loop, so they are plain functions over the position and the content.
//!
//! Each body is the **string engine's own text body** (the `*_text` half of the legacy dual
//! functions), copied, not wrapped: this crate must build a game without the legacy engine.
//! `crates/ti4-engine-core-bridge/tests/setup_parity.rs` compares every builder with the legacy
//! one on the same positions, field by field. A legacy `*_text` body that changes (it is frozen)
//! would show there first.
//!
//! The modules are named like the legacy ones they stand for, so the host's call sites read
//! `ti4_engine::builders::promissory::take` where they read `ti4_engine::promissory::take`.
//! Lane B's query modules (`promissory`, `technology`, ... at the crate root) are a different
//! thing: they only read.

pub mod exploration;
pub mod factions;
pub mod leaders;
pub mod movement;
pub mod objectives;
pub mod phase;
pub mod promissory;
pub mod support;
pub mod technology;
