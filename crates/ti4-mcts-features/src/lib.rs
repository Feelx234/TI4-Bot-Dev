//! Features for the MCTS bot (docs/MCTS_BOT_PLAN.md 3, lane L1).
//!
//! * [`kind`]: which asks are tree decisions, a stable name per ask kind.
//! * [`filter`]: the hard filter on the activations.
//! * [`encode`]: the actor-relative encoding of one decision (fixed token sets plus a ragged option block).
//! * [`schema`], [`shard`], [`npy`]: the training shards the Python trainer (`tools/mcts`) reads.
//!
//! Everything reads [`ti4_engine_core::seen::Seen`] for the actor and the core `Choice`: hidden information cannot reach a
//! feature, because `Seen` has no accessor for it.

pub mod encode;
pub mod filter;
pub mod kind;
pub mod npy;
pub mod schema;
pub mod shard;
