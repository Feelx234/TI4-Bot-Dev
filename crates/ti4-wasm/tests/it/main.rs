//! The hosted game, natively: one test crate, so the `slow_` tier is found by module path
//! (`cargo t-web-slow`). The exports keep one game in statics: every test takes `common::SERIAL`.

mod catalog_fixture;
mod common;
mod feedback_offline;
mod host_batch;
mod host_free_form;
mod host_history;
mod host_local;
mod host_neutral_opponent;
mod host_odds;
mod host_preview;
mod host_reaction_followup;
mod host_scenario;
mod host_script;
mod host_session;
mod host_setup;
mod host_staged;
mod host_thunders_edge;
mod protocol_fixtures;
mod replay_export;
mod server_parity;
// Needs `ti4_wasm::wire_ts`, which exists only with the `ts` feature (`cargo wire-web` sets it); without the gate `cargo test -p ti4-wasm` does not compile.
#[cfg(feature = "ts")]
mod wire_types;
