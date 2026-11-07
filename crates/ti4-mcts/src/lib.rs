//! The MCTS core (lane L6 of `docs/MCTS_BOT_PLAN.md`; resume `docs/L6_RESUME.md`), on the search API `ti4-engine-search` (C6).
//!
//! * [`search`]: Gumbel AlphaZero at the root (Gumbel-top-k without replacement, sequential halving, the completed-Q improved policy),
//!   Gumbel (default) or PUCT selection below the root, chance nodes for the combat rolls by hit-count class, max^n backups of a 4-seat
//!   value vector, node and time bounds, accounting.
//! * [`tree`]: the arena; positions are a cache (trimmed ledgers, re-derived after a reuse).
//! * [`canon`]: the candidates of a tree question (the activation hard filter, the canonical move order).
//! * [`eval`]: the [`Evaluator`] trait (priors per candidate + an actor-relative 4-seat value) and its stubs: [`eval::Uniform`],
//!   [`eval::RuleBotPrior`], [`eval::Rollout`], and [`eval::FnEvaluator`] (the net hook).
//! * [`agent`]: [`MctsAgent`], a `SeatAgent`: searches its tree questions, the rule bot answers the rest; playout-cap randomisation, tree
//!   reuse, the determinisation hook ([`agent::Determinize`], lane C8).
//! * [`value`]: the value vector and its orders.
//!
//! What the search branches on (user decisions 2026-11-06): the strategic asks and the unit moves (the tree kinds of
//! `ti4_mcts_features::kind`, not an action card's question) and the combat dice; never on action cards or exploration draws (answered
//! once, from the search's stream). Production and the small asks stay with the rule bot. CPU only; one game per thread (a `Position` is
//! `!Send`).
//!
//! A dev tool: not in the production graph or the wasm.

pub mod agent;
pub mod canon;
pub mod eval;
pub mod gumbel;
pub mod play;
pub mod search;
pub mod tree;
pub mod value;

#[cfg(test)]
mod tests;

pub use agent::MctsAgent;
pub use eval::{Evaluation, Evaluator};
pub use search::{Config, NonRoot, SearchResult, Searcher, Stats};
pub use value::{SEATS, Value};
