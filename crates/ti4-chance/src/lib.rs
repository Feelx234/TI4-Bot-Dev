//! The game's randomness as data: the bottom crate of docs/FORK_RNG_THEORY.md section 4.
//!
//! Today the core draws dice straight from `CoreState::rng` at about 35 sites. For a tree search every random event
//! must instead be a question the engine can stop at and resume from (a chance node), answered either by the game's own
//! streams (play) or by the search (any faces it likes). This crate holds what both sides need to name:
//!
//! * [`Domain`]: the independent streams (dice, Neuraloop, Yssaril hero, Fracture, the decks, setup and seating);
//! * [`Request`] / [`Kind`]: one roll group, and [`Faces`]: its answer (16 bytes, at most 15 dice);
//! * [`Streams`]: the seed and the ChaCha8 stream of each domain, with positions and the setup's
//!   deck [`Streams::shuffle`];
//! * [`answer`] / [`answer_mut`]: the one function that turns a request and the streams into faces. It draws exactly
//!   what today's `die(domain, sides)` calls draw, one die per face, in order, so play stays bit-identical;
//! * with feature `search`: [`search`] sampling, weights and outcome enumeration of a request.
//!
//! It names no `ti4-` crate (tools/engine_core/check_layers.py `chance`), so the core, the resolver, a search or an
//! offline odds tool can all depend on it. It is wasm-friendly: no clock, no threads, no OS entropy.

mod domain;
mod request;
mod streams;

#[cfg(feature = "search")]
pub mod search;

pub use domain::{Domain, derive_seed};
pub use request::{Faces, Kind, MAX_FACES, ModForm, Request};
pub use streams::{Refused, Streams, answer, answer_mut, try_answer_mut};
