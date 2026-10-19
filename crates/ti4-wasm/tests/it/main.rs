//! The hosted game, natively: one test crate, so the `slow_` tier is found by module path
//! (`cargo t-web-slow`). The exports keep one game in statics: every test takes `common::SERIAL`.

mod catalog_fixture;
mod common;
mod feedback_offline;
mod host_batch;
mod host_history;
mod host_local;
mod host_odds;
mod host_preview;
mod host_scenario;
mod host_script;
mod host_session;
mod host_setup;
mod host_staged;
mod host_thunders_edge;
mod protocol_fixtures;
mod replay_export;
mod server_parity;
mod wire_types;
