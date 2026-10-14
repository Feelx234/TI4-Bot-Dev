//! The hosted game, natively: one test crate, so the `slow_` tier is found by module path
//! (`cargo t-web-slow`). The exports keep one game in statics: every test takes `common::SERIAL`.

mod common;
mod host_batch;
mod host_history;
mod host_session;
mod server_parity;
mod host_setup;
