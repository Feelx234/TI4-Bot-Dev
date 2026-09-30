//! The remote side: one connection to a host, and what it has been told.
//!
//! A reader thread applies every message to a [`Remote`] behind a mutex; the window copies what it
//! needs out of it once per repaint. Writes (answers, pings) go straight to the socket from whoever
//! makes them, behind their own mutex, because they are small and rare.

use std::io;
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};

use ti4_model::id::PlayerId;
use ti4_review::{ReviewFrame, ReviewSession};

use super::protocol::{
    ClientMessage, PROTOCOL, SeatStatus, ServerMessage, read_message, write_message,
};
use super::{CLIENT_PING, HANDSHAKE_TIMEOUT};
use crate::control::{ChoiceFingerprint, ManualSubmission, PendingManualChoice};

/// Frames a client keeps. A game is well under this; a host sending more is misbehaving.
pub const MAX_CLIENT_FRAMES: usize = 20_000;

/// Where to connect and as whom.
#[derive(Clone, Debug)]
pub struct JoinRequest {
    /// `host:port`.
    pub address: String,
    pub code: String,
    pub seat: Option<PlayerId>,
    pub name: String,
    pub resume: Option<String>,
}

/// Everything the host has told this client.
#[derive(Debug, Default)]
pub struct Remote {
    pub seat: Option<PlayerId>,
    pub resume: Option<String>,
    pub header: Option<ReviewSession>,
    pub frames: Vec<ReviewFrame>,
    pub pending: Option<PendingManualChoice>,
    pub seats: Vec<SeatStatus>,
    pub state: String,
    pub waiting_on: Option<PlayerId>,
    /// The host's word on the last answer.
    pub answer: Option<(bool, String)>,
    /// Why the connection ended, once it has.
    pub closed: Option<String>,
    /// Bumped on every message, so a window knows to repaint.
    pub revision: u64,
}

/// A live connection to a host.
pub struct NetClient {
    remote: Arc<Mutex<Remote>>,
    writer: Arc<Mutex<TcpStream>>,
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

impl NetClient {
    /// Connect, say hello, and wait for the host to seat or refuse this client.
    ///
    /// # Errors
    /// The host could not be reached, refused the hello, or said something that is not an answer.
    pub fn join(request: &JoinRequest) -> io::Result<Self> {
        let address = request
            .address
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no such address"))?;
        let stream = TcpStream::connect_timeout(&address, HANDSHAKE_TIMEOUT)?;
        let _ = stream.set_nodelay(true);
        let mut reader = stream.try_clone()?;
        let mut writer = stream.try_clone()?;
        write_message(
            &mut writer,
            &ClientMessage::Hello {
                protocol: PROTOCOL.to_owned(),
                build: ti4_review::ENGINE_COMMIT.to_owned(),
                code: request.code.trim().to_owned(),
                seat: request.seat.clone(),
                name: request.name.clone(),
                resume: request.resume.clone(),
            },
        )?;
        reader.set_read_timeout(Some(HANDSHAKE_TIMEOUT))?;
        let mut remote = Remote::default();
        match read_message::<ServerMessage>(&mut reader)? {
            ServerMessage::Welcome { seat, resume } => {
                remote.seat = Some(seat);
                remote.resume = Some(resume);
            }
            ServerMessage::Refused { reason } => {
                return Err(io::Error::new(io::ErrorKind::PermissionDenied, reason));
            }
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("expected a welcome, got {}", kind(&other)),
                ));
            }
        }
        // The host speaks only when something changes, so the reader blocks without a timeout. A host
        // that goes away shows up as a closed or reset socket.
        reader.set_read_timeout(None)?;
        let remote = Arc::new(Mutex::new(remote));
        let writer = Arc::new(Mutex::new(writer));
        let stop = Arc::new(AtomicBool::new(false));
        let read = {
            let remote = Arc::clone(&remote);
            thread::Builder::new()
                .name("online-read".to_owned())
                .spawn(move || read_loop(&mut reader, &remote))?
        };
        let ping = {
            let writer = Arc::clone(&writer);
            let stop = Arc::clone(&stop);
            thread::Builder::new()
                .name("online-ping".to_owned())
                .spawn(move || {
                    let mut waited = std::time::Duration::ZERO;
                    let step = std::time::Duration::from_millis(100);
                    while !stop.load(Ordering::SeqCst) {
                        thread::sleep(step);
                        waited += step;
                        if waited >= CLIENT_PING {
                            waited = std::time::Duration::ZERO;
                            let mut stream = writer.lock().unwrap_or_else(PoisonError::into_inner);
                            if write_message(&mut *stream, &ClientMessage::Ping).is_err() {
                                break;
                            }
                        }
                    }
                })?
        };
        Ok(Self {
            remote,
            writer,
            stop,
            threads: vec![read, ping],
        })
    }

    /// What the host has said so far.
    pub fn remote(&self) -> MutexGuard<'_, Remote> {
        self.remote.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Answer the choice on screen.
    ///
    /// # Errors
    /// The connection is gone.
    pub fn submit(&self, fingerprint: ChoiceFingerprint, option_id: String) -> io::Result<()> {
        self.write(&ClientMessage::Submit(ManualSubmission {
            fingerprint,
            option_id,
        }))
    }

    /// Let the policy answer the choice on screen.
    ///
    /// # Errors
    /// The connection is gone.
    pub fn delegate(&self, fingerprint: ChoiceFingerprint) -> io::Result<()> {
        self.write(&ClientMessage::Delegate { fingerprint })
    }

    fn write(&self, message: &ClientMessage) -> io::Result<()> {
        let mut stream = self.writer.lock().unwrap_or_else(PoisonError::into_inner);
        write_message(&mut *stream, message)
    }
}

impl Drop for NetClient {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = self
            .writer
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .shutdown(Shutdown::Both);
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

fn read_loop(reader: &mut TcpStream, remote: &Mutex<Remote>) {
    loop {
        let message = read_message::<ServerMessage>(reader);
        let mut remote = remote.lock().unwrap_or_else(PoisonError::into_inner);
        remote.revision += 1;
        match message {
            Ok(message) => {
                if let Some(reason) = apply(&mut remote, message) {
                    remote.closed = Some(reason);
                    return;
                }
            }
            Err(error) => {
                if remote.closed.is_none() {
                    remote.closed = Some(format!("connection lost: {error}"));
                }
                return;
            }
        }
    }
}

/// Fold one message into what the client knows. `Some(reason)` ends the connection.
pub fn apply(remote: &mut Remote, message: ServerMessage) -> Option<String> {
    match message {
        ServerMessage::Header(header) => {
            remote.header = Some(*header);
            remote.frames.clear();
            remote.pending = None;
        }
        ServerMessage::Frames(frames) => {
            for frame in frames {
                // Frames arrive in order; anything else is a repeat and is ignored.
                if frame.index == remote.frames.len() {
                    if remote.frames.len() >= MAX_CLIENT_FRAMES {
                        return Some("the host sent more frames than a game has".to_owned());
                    }
                    remote.frames.push(frame);
                }
            }
        }
        ServerMessage::Pending(pending) => remote.pending = pending.map(|pending| *pending),
        ServerMessage::Answer { accepted, message } => remote.answer = Some((accepted, message)),
        ServerMessage::Table {
            seats,
            state,
            waiting_on,
        } => {
            remote.seats = seats;
            remote.state = state;
            remote.waiting_on = waiting_on;
        }
        ServerMessage::Closed { reason } | ServerMessage::Refused { reason } => {
            return Some(reason);
        }
        ServerMessage::Welcome { seat, resume } => {
            remote.seat = Some(seat);
            remote.resume = Some(resume);
        }
    }
    None
}

fn kind(message: &ServerMessage) -> &'static str {
    match message {
        ServerMessage::Welcome { .. } => "welcome",
        ServerMessage::Refused { .. } => "refusal",
        ServerMessage::Header(_) => "header",
        ServerMessage::Frames(_) => "frames",
        ServerMessage::Pending(_) => "pending choice",
        ServerMessage::Answer { .. } => "answer",
        ServerMessage::Table { .. } => "table status",
        ServerMessage::Closed { .. } => "goodbye",
    }
}
