//! `ti4-server` provides authoritative server logic, wire protocol data transfer objects,
//! session workers, and redacted game projections for online multiplayer Twilight Imperium 4.

#![allow(clippy::missing_panics_doc)]

pub mod fixtures;
pub mod projection;
pub mod protocol;
pub mod session;

pub use protocol::*;
pub use session::{
    GameRegistry, GameSession, MockClient, RemoteHumanDecider, SeatController, SessionConfig,
};
