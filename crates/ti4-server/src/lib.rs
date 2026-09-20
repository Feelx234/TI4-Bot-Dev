//! `ti4-server` provides authoritative server logic, wire protocol data transfer objects,
//! and redacted game projections for online multiplayer Twilight Imperium 4.

pub mod fixtures;
pub mod projection;
pub mod protocol;

pub use protocol::*;
