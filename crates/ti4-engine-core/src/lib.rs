//! PLACEHOLDER for the engine core.
//!
//! The real `ti4-engine-core` (no JSON, no id strings, no card text: id enums, typed gameplay
//! tables and hand-written `match` effects) is designed on branch `engine-raw-core-design`. Until
//! it exists, this crate only holds what `cargo xtask gen-cards` generates for it, so that the
//! output compiles and its tests run. The core's crate absorbs `src/generated/` and `names/`
//! (the frozen variant names) unchanged; nothing in the workspace depends on this crate.
//!
//! See docs/CONTENT_GENERATOR.md.

#[rustfmt::skip]
pub mod generated;
