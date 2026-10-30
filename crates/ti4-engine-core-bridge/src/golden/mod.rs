//! Golden games: frozen per-`RULES_VERSION` replays of recorded games (docs/todo/D4-core-tests.md 4, package D4-4).
//! The CLI is `bin/golden.rs`; the committed data lives in `tools/engine_core/golden/` (see its README).

pub mod bless;
pub mod corpus;
pub mod format;
pub mod record;
pub mod replay;
pub mod sets;
