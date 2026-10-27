//! The string <-> typed seam of the core-state migration (docs/ENGINE_CORE_STATE.md, section 3).
//!
//! `keys` (package X1) is the text codec of every `String`-typed key and value of `GameState`:
//! pure `parse` / `render`, no game state involved. The footprint and lend machinery (package F2)
//! lives in sibling modules.

pub mod keys;
