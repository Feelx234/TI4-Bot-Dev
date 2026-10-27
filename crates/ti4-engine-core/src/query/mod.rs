//! L2: queries (docs/ENGINE_CORE_STATE.md 4.1). Pure reads over `&CoreState`: no mutation, no text.
//!
//! Package A1 (board queries). Every function names the string function it answers (`planets::
//! in_system`, `laws::active`, ...) and returns the same value in the same order; the dual run
//! (`Area::BoardQueries`) compares them on every golden position.
//!
//! * [`board`]: control, planets of a system, units and tokens, coexistence, neighbours.
//! * [`economy`]: planet resources/influence now, spendable planets, command token reinforcements.
//! * [`fleet`]: supply plastic (`supply::held/remaining`), fleet supply and transport (`fleet::standing`).
//! * [`seats`]: seating, speaker and initiative order.
//! * [`laws`]: laws in play and the standing rules they impose.
//! * [`relic`], [`promissory`], [`explore`] (package B5): relics held, notes in hand and play areas,
//!   exploration traits, fragments, legendary abilities and the Fracture.
//!
//! **What the core cannot see** is passed in by the caller, never guessed: the source scope
//! ([`Scope`], the game's `SourceSet`), the "all laws are blank this turn" flag (`blanked`: a
//! Keleres faction mark, text in the string state until package X1's typed marks land), and the
//! answers of faction hooks (`extra`, `half_weight`, `mahact_held`). A string caller computes them
//! with the string engine and the sum stays exact; the hook tables (A5) replace the arguments.

pub mod board;
pub mod economy;
pub mod fleet;
pub mod explore;
pub mod laws;
pub mod promissory;
pub mod relic;
pub mod seats;

pub use crate::tables::Scope;
