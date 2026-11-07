//! The search API (lane C6 of `docs/FORK_RNG_THEORY.md`, the interface of `docs/MCTS_BOT_PLAN.md` 2.1 and 4.4; resume
//! `docs/C6_RESUME.md`).
//!
//! A [`Position`] is a game cut at its next stop: the typed core's `CoreState`, the state of the step it is in (which reconcile run or
//! which arm, the step's `Env`, the run's combat texts, the run's question), a warm copy of the renderer (the code table and the `Env`
//! cache) and the counters a caller keeps. It is driven by pulling:
//!
//! * [`Position::node`]: what the game waits for: a question of a seat ([`Node::Decide`], the core `Choice`), a chance request
//!   ([`Node::Chance`], only in [`Mode::Search`]), or the end ([`Node::End`], with [`Position::outcome`]);
//! * [`Position::answer`] (a core `Pick`), [`Position::answer_id`] (the wire option id, the product's path), [`Position::answer_chance`]
//!   (faces): resume the kernel at the ask and run on to the next stop;
//! * [`Position::fork`]: an independent copy (a `CoreState` clone, the renderer's warm copy, the rest shared by `Rc`).
//!
//! The step is the product's (`ti4_engine::native::Game::step` -> `native_step`, cut at its questions like the thin crate's
//! `live::pull`): the same reconcile runs, arm, hand-overs, code table and glue flag, minus the wrapper's presentation (no string mirror,
//! no event text, no journal). In [`Mode::Play`] every chance request is answered from the position's own streams through
//! `ti4_engine_chance` (exactly as the product); in [`Mode::Search`] nothing is answered from them: every request stops the run and the
//! caller answers it with faces of its own. A position has no accessor for its streams ([`Position::stream_positions`] reports how far
//! each stream was drawn, never its keys).
//!
//! On top: [`advance::advance_to_tree_decision`] (plays on through the asks the search does not branch on), [`chance`] (hit-count
//! outcome classes, the search-only random stream), [`agent`] (the [`agent::SeatAgent`] trait, the rule bot, a policy hook, a random
//! seat) and [`driver`] (a whole game with one agent per seat: the selfplay core driver).
//!
//! Hidden information (lane C8, `docs/C8_RESUME.md`): [`determinize`] makes the copy a search starts from at its root: every card the
//! searching seat has not seen (other seats' secret objectives, the secret, public objective, agenda, relic and exploration decks) is
//! drawn anew from the search's own stream, consistently with what the seat has seen ([`seen_digest`] is the same).
//!
//! A dev tool: no production crate depends on this one (`tools/engine_core/check_production_graph.sh`), and it is not in the wasm.
//! `tools/engine_core/check_layers.py search` checks its rules (no stream read, no draw, no legacy crate).

pub mod advance;
pub mod agent;
pub mod chance;
pub mod determinize;
pub mod driver;
mod position;
mod table;
#[cfg(test)]
mod c8_tests;
#[cfg(test)]
mod tests;

pub use determinize::{determinize, seen_digest};
pub use position::{Node, Position, SearchError};
pub use table::{MCTS_FACTIONS, Table, lobby_spec, mcts_spec, quick_spec, start};
pub use ti4_engine_chance::Mode;

/// The rule bot's movement memory (`ti4_rule_bot::backward`): the one piece of state the rule bot carries between decisions, kept per
/// position (saved and restored around every rule-bot answer by [`agent::RuleBotAgent`]).
pub type BotMemory = ti4_rule_bot::backward::Memory;

/// How a game ended (or stands): victory points in seat order, the winner (the most points, ties broken by the initiative order, as
/// the product's `objectives::leader`) once the game is over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub finished: bool,
    pub round: u32,
    pub vp: Vec<i16>,
    pub winner: Option<usize>,
}
