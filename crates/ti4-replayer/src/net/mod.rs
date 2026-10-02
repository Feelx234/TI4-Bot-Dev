//! Online play: the replayer's live table, served to remote seats over TCP.
//!
//! - [`host`] runs inside the replayer window. It serves the branch the window is running, relays
//!   each remote seat's pending choice to that seat alone, and passes answers back through the gate.
//! - [`client`] is one connection to a host; [`remote_gui`] is the window a remote player sees.
//! - [`redact`] is the line between them: nothing leaves the host for a seat without passing it.
//! - [`protocol`] is the bounded wire format.
//!
//! The connection is plain TCP. The join code keeps strangers out of the seats but is sent in the
//! clear, so over the internet run it inside a VPN (Tailscale, `WireGuard`) or an SSH tunnel.

use std::time::Duration;

pub mod client;
#[cfg(feature = "host")]
pub mod host;
pub mod protocol;
pub mod redact;
pub mod remote_gui;

pub use client::{JoinRequest, NetClient, Remote};
#[cfg(feature = "host")]
pub use host::NetHost;

/// Port the host listens on unless told otherwise.
pub const DEFAULT_PORT: u16 = 47_474;

/// Seats remote players can hold at once. A table has six.
pub const MAX_CLIENTS: usize = 6;

/// Connections allowed to be mid-hello at once.
pub const MAX_HANDSHAKES: usize = 8;

/// How long a new connection has to say hello.
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// How often a client says it is still there.
pub const CLIENT_PING: Duration = Duration::from_secs(5);

/// How long the host waits to hear from a client before dropping it.
pub const CLIENT_SILENCE: Duration = Duration::from_secs(30);

/// How often the host relays new frames, choices and seat changes.
pub const PUMP_TICK: Duration = Duration::from_millis(50);
