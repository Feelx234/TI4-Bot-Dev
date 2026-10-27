//! Typed homes of the string keys and values of `GameState` (docs/ENGINE_CORE_STATE.md, 1.8; package X1).
//!
//! Enums and orderings only. The text of every key (parse / render, the corpus test) is the
//! adapter's: `ti4_engine::core_sync::keys`. Nothing here knows a string.
//!
//! The one rule that makes the typed maps a drop-in for the string-keyed maps they replace is
//! that **typed order is text order**: iterating a map keyed by [`MarkKey`] visits the keys in the order
//! the rendered texts have. Three things make that true:
//!
//! * the families of [`MarkKey`] are declared in the text order of their literal prefixes;
//! * an id enum is numbered in text order (generated; `ids_are_numbered_in_text_order`);
//! * a variable-length field followed by more text (`bastion:lost:10:a:cruiser` against
//!   `bastion:lost:100:a:cruiser`) compares with the byte that follows it: see [`order`].

pub mod atom;
pub mod decimal;
pub mod decks;
pub mod destroy;
pub mod ledger;
pub mod mark;
pub mod order;
pub mod outcome;
pub mod tables;
pub mod word;

pub use atom::{Atom, AtomLike};
pub use decimal::DecimalTextOrd;
pub use decks::{Expedition, ExploreDeck};
pub use destroy::{ActionCardCause, DestroyReason, FactionAbilityCause, UnitAbilityCause};
pub use ledger::LedgerReason;
pub use mark::{Family, MarkKey, MarkVal};
pub use outcome::Outcome;
pub use word::Word;
