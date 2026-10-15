//! The hosted game, natively: one test crate, so the `slow_` tier is found by module path
//! (`cargo t-web-slow`). The exports keep one game in statics: every test takes `common::SERIAL`.

mod common;
mod feedback_offline;
mod host_batch;
mod host_history;
mod host_local;
mod host_preview;
mod host_scenario;
mod host_script;
mod host_session;
mod host_setup;
mod host_staged;
mod server_parity;
