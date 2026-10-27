//! Package X1 `core-x1-key-codecs`: the text of the `String`-typed keys and values of
//! `GameState` (docs/ENGINE_CORE_STATE.md, table 1.8), as pure `parse` / `render` functions over
//! the typed enums of `ti4_engine_core::keys`.
//!
//! * [`Codec`] holds what the text depends on: the owner table ([`Owners`]) and the table of
//!   uninterpreted value texts ([`Blobs`]).
//! * Marks: `Codec::{parse,render}_mark{,_key,_value}`; the families and their value shapes are
//!   `marks.rs` and `mark_values.rs`.
//! * Outcomes, ledger reasons, destruction causes and the deck / slice keys: `other.rs`.
//!
//! Every parse is `None` for text it cannot type (a test key, an id the corpus does not have, an
//! owner outside the table). The caller keeps the string for that game (the strangler's escape
//! hatch), exactly as `core_adapter::Unmapped` does. For every text that parses,
//! `render(parse(t)) == t`; the corpus test (`tests.rs`, `corpus/`) checks it for every key and
//! value seen in the golden games, the fixtures and the engine tests.

pub mod mark_values;
pub mod marks;
pub mod other;
pub mod text;
pub mod word_text;

#[cfg(test)]
mod tests;

pub use other::{
    parse_destroy, parse_expedition, parse_explore_deck, parse_ledger, render_destroy,
    render_expedition, render_explore_deck, render_ledger,
};
pub use text::{Blobs, Codec, K, Owners};
