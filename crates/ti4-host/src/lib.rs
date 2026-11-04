//! `ti4-host`: everything between the engine and a client that does not depend on how the
//! process is hosted. The online server (`ti4-server`, threads and tokio) and the browser
//! (`ti4-wasm`, a worker) both build on it, so the wire messages, the event log and the
//! history rules cannot drift apart.
//!
//! No tokio, no threads spawned, no system clock: time and nonces are injected ([`clock`]).
//! `cargo check -p ti4-host --target wasm32-unknown-unknown` is part of the contract.

#![allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]

/// A wire value of the legacy crate read as the same value of the thin one (one serde shape, `ti4-engine/tests/wire_parity.rs`): test help only.
#[cfg(test)]
pub(crate) fn from_legacy_wire<A: serde::Serialize, B: serde::de::DeserializeOwned>(a: &A) -> B {
    serde_json::from_value(serde_json::to_value(a).expect("a wire value serialises")).expect("the thin crate reads the same wire value")
}

pub mod batch;
pub mod clock;
pub mod disabled;
pub mod export;
pub mod history;
pub mod journal;
pub mod map;
pub mod maps;
pub mod planning;
pub mod preset;
pub mod preview;
pub mod projection;
pub mod protocol;
pub mod replay;
pub mod rng_force;
pub mod setup;
pub mod staged;
pub mod state_update;
pub mod step_snapshot;
pub mod view;
pub mod wire;

pub use protocol::*;
