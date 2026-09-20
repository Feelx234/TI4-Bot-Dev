//! In-memory mock client transport adapter for test harnesses and integration flows.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvError, TryRecvError};
use ti4_model::id::PlayerId;

use crate::protocol::server::{ActionAcceptedMsg, InitialSnapshotMsg, ServerMessage};
use crate::protocol::status::{RejectionReason, ViewerRole};
use crate::session::GameSession;

/// In-memory mock client connected to a `GameSession`.
pub struct MockClient {
    viewer: ViewerRole,
    receiver: Receiver<ServerMessage>,
    session: Arc<GameSession>,
}

impl MockClient {
    /// Connects a new mock client with the specified role.
    #[must_use]
    pub fn connect(session: Arc<GameSession>, viewer: ViewerRole) -> Self {
        let receiver = session.subscribe(viewer.clone());
        Self {
            viewer,
            receiver,
            session,
        }
    }

    /// Returns the role of this mock client.
    #[must_use]
    pub fn viewer(&self) -> &ViewerRole {
        &self.viewer
    }

    /// Fetches an initial snapshot from the session.
    #[must_use]
    pub fn snapshot(&self) -> InitialSnapshotMsg {
        self.session.get_snapshot(&self.viewer)
    }

    /// Blocks waiting for the next server message.
    ///
    /// # Errors
    ///
    /// Returns [`RecvError`] if the channel disconnected.
    pub fn recv(&self) -> Result<ServerMessage, RecvError> {
        self.receiver.recv()
    }

    /// Attempts to receive a message without blocking.
    ///
    /// # Errors
    ///
    /// Returns [`TryRecvError`] if empty or disconnected.
    pub fn try_recv(&self) -> Result<ServerMessage, TryRecvError> {
        self.receiver.try_recv()
    }

    /// Drains and returns all currently buffered messages.
    #[must_use]
    pub fn drain_messages(&self) -> Vec<ServerMessage> {
        let mut messages = Vec::new();
        while let Ok(msg) = self.receiver.try_recv() {
            messages.push(msg);
        }
        messages
    }

    /// Submits a choice for the client's seat.
    ///
    /// # Errors
    ///
    /// Returns [`RejectionReason`] if rejected.
    pub fn submit(
        &self,
        nonce: &str,
        expected_version: u64,
        option_id: &str,
    ) -> Result<ActionAcceptedMsg, RejectionReason> {
        let seat = match &self.viewer {
            ViewerRole::Player(p) => p.clone(),
            ViewerRole::Spectator => PlayerId::new("spectator"),
        };

        self.session
            .submit_choice(&seat, nonce, expected_version, option_id)
    }
}
