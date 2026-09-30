//! The host side: the replayer window's live branch, served to remote seats.
//!
//! The host stays authoritative. The game runs where it always ran, on the branch thread behind the
//! [`Gate`]; this module only relays. A remote seat is a manual seat whose panel is on another
//! machine: its answer arrives over TCP and goes through [`Gate::submit`], so the same fingerprint
//! check refuses a stale, invented or duplicate click from across the network as it does from the
//! window.
//!
//! Threads, and who holds the lock:
//!
//! - **accept** takes connections and gives each a handler thread.
//! - **handler** (one per connection) reads the hello, claims a seat, then reads submissions until
//!   the connection goes quiet or closes.
//! - **writer** (one per connection) drains that connection's outbox onto the socket, so a slow
//!   client never blocks anyone but itself.
//! - **pump** is the only thread that sends frames. Every tick it copies what each client is owed
//!   out from under the lock, redacts it without the lock, and queues it; then it relays the pending
//!   choice to the seat it belongs to and the table status to everybody.
//!
//! The window calls [`NetHost::sync`] from its repaint. That call only appends shared pointers, so
//! the window never waits on redaction or on the network.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io;
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use rand::Rng;
use ti4_model::id::PlayerId;
use ti4_review::{ReviewFrame, ReviewSession};

use super::protocol::{
    ClientMessage, FRAMES_PER_MESSAGE, Holder, MAX_NAME_CHARS, PROTOCOL, SeatStatus, ServerMessage,
    read_message, write_message,
};
use super::redact::{frame_for, header_for};
use super::{CLIENT_SILENCE, HANDSHAKE_TIMEOUT, MAX_CLIENTS, MAX_HANDSHAKES, PUMP_TICK};
use crate::control::{ChoiceFingerprint, SeatMode, SubmitOutcome};
use crate::live::Gate;

/// Shortest join code the host accepts when one is given to it.
pub const MIN_CODE_CHARS: usize = 16;

/// Messages a connection may have queued before the host gives up on it.
const OUTBOX: usize = 64;

/// Frame messages in flight to one client before the pump waits for it to catch up.
const FRAMES_IN_FLIGHT: usize = 4;

/// A table served to the network. Dropping it closes every connection.
pub struct NetHost {
    shared: Arc<Shared>,
    address: SocketAddr,
    threads: Vec<JoinHandle<()>>,
}

struct Shared {
    code: String,
    table: Mutex<Table>,
    shutdown: AtomicBool,
    handshakes: AtomicUsize,
}

#[derive(Default)]
struct Table {
    gate: Option<Arc<Gate>>,
    /// Already redacted: the header does not depend on the seat.
    header: Option<Arc<ReviewSession>>,
    /// Unredacted; the pump redacts per seat on the way out.
    frames: Vec<Arc<ReviewFrame>>,
    /// Bumped whenever the timeline is replaced, so work prepared for the old one is discarded.
    generation: u64,
    clients: BTreeMap<u64, Client>,
    claims: BTreeMap<PlayerId, Claim>,
    next_client: u64,
    last_status: Option<ServerMessage>,
}

struct Client {
    seat: PlayerId,
    outbox: SyncSender<ServerMessage>,
    stream: TcpStream,
    in_flight: Arc<AtomicUsize>,
    generation: u64,
    header_sent: bool,
    frames_sent: usize,
    pending_sent: Option<ChoiceFingerprint>,
}

struct Claim {
    name: String,
    resume: String,
    client: Option<u64>,
}

/// Frames one client is owed, copied out from under the lock.
struct Owed {
    client: u64,
    seat: PlayerId,
    generation: u64,
    header: Option<Arc<ReviewSession>>,
    from: usize,
    frames: Vec<Arc<ReviewFrame>>,
}

impl NetHost {
    /// Listen on `bind` with a fresh join code.
    ///
    /// # Errors
    /// The address could not be bound.
    pub fn start(bind: SocketAddr) -> io::Result<Self> {
        Self::start_with_code(bind, None)
    }

    /// Listen on `bind` with the given join code, or a fresh one. A given code must be at least
    /// [`MIN_CODE_CHARS`] letters and digits: it is the only thing keeping strangers out of a seat.
    ///
    /// # Errors
    /// The code is too weak, or the address could not be bound.
    pub fn start_with_code(bind: SocketAddr, code: Option<String>) -> io::Result<Self> {
        let code = match code {
            Some(code)
                if code.len() >= MIN_CODE_CHARS
                    && code.chars().all(|ch| ch.is_ascii_alphanumeric()) =>
            {
                code
            }
            Some(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("a join code needs at least {MIN_CODE_CHARS} letters and digits"),
                ));
            }
            None => random_token(),
        };
        let listener = TcpListener::bind(bind)?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let shared = Arc::new(Shared {
            code,
            table: Mutex::new(Table::default()),
            shutdown: AtomicBool::new(false),
            handshakes: AtomicUsize::new(0),
        });
        let accept = {
            let shared = Arc::clone(&shared);
            thread::Builder::new()
                .name("online-accept".to_owned())
                .spawn(move || accept_loop(&shared, &listener))?
        };
        let pump = {
            let shared = Arc::clone(&shared);
            thread::Builder::new()
                .name("online-pump".to_owned())
                .spawn(move || pump_loop(&shared))?
        };
        Ok(Self {
            shared,
            address,
            threads: vec![accept, pump],
        })
    }

    #[must_use]
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// The code a remote player must present to join.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.shared.code
    }

    /// Tell the host what the window is showing. Cheap: it appends pointers and returns.
    ///
    /// A different gate, a header arriving, or fewer frames than before all mean a new timeline, and
    /// every client is sent it from the start.
    pub fn sync(&self, gate: &Arc<Gate>, header: Option<&ReviewSession>, frames: &[ReviewFrame]) {
        let mut table = self.shared.lock();
        let same_gate = table
            .gate
            .as_ref()
            .is_some_and(|held| Arc::ptr_eq(held, gate));
        let reset = !same_gate
            || table.header.is_none() != header.is_none()
            || frames.len() < table.frames.len();
        if reset {
            table.gate = Some(Arc::clone(gate));
            table.header = header.map(|header| Arc::new(header_for(header)));
            table.frames = frames.iter().cloned().map(Arc::new).collect();
            table.generation += 1;
            // A seat a remote player holds is manual on every timeline the host shows them.
            let claimed: Vec<PlayerId> = table.claims.keys().cloned().collect();
            for seat in claimed {
                gate.set_mode(&seat, SeatMode::Manual);
            }
        } else if frames.len() > table.frames.len() {
            let known = table.frames.len();
            table
                .frames
                .extend(frames[known..].iter().cloned().map(Arc::new));
        }
    }

    /// Seats remote players hold: seat, name, whether they are connected right now.
    #[must_use]
    pub fn remote_seats(&self) -> Vec<(PlayerId, String, bool)> {
        self.shared
            .lock()
            .claims
            .iter()
            .map(|(seat, claim)| (seat.clone(), claim.name.clone(), claim.client.is_some()))
            .collect()
    }

    /// Whether a remote player holds this seat, connected or not.
    #[must_use]
    pub fn is_remote(&self, seat: &PlayerId) -> bool {
        self.shared.lock().claims.contains_key(seat)
    }

    /// Give a seat back to the host: disconnect its player and forget the claim. The seat keeps
    /// whatever mode it has; the host decides whether the bot or the window answers it next.
    pub fn release(&self, seat: &PlayerId) {
        let mut table = self.shared.lock();
        if let Some(claim) = table.claims.remove(seat)
            && let Some(client) = claim.client
        {
            drop_client(&mut table, client, "the host took the seat back");
        }
    }
}

impl Drop for NetHost {
    fn drop(&mut self) {
        self.shared.shutdown.store(true, Ordering::SeqCst);
        {
            let mut table = self.shared.lock();
            let ids: Vec<u64> = table.clients.keys().copied().collect();
            for id in ids {
                drop_client(&mut table, id, "the host stopped hosting");
            }
        }
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Table> {
        self.table.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// 128 bits from the operating system's generator, as hex.
fn random_token() -> String {
    let bytes: [u8; 16] = rand::rng().random();
    bytes
        .iter()
        .fold(String::with_capacity(32), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}

/// Compare without stopping at the first difference, so response time says nothing about the code.
fn same_secret(left: &str, right: &str) -> bool {
    left.len() == right.len()
        && left
            .bytes()
            .zip(right.bytes())
            .fold(0_u8, |diff, (a, b)| diff | (a ^ b))
            == 0
}

fn accept_loop(shared: &Arc<Shared>, listener: &TcpListener) {
    let mut handlers: Vec<JoinHandle<()>> = Vec::new();
    while !shared.shutdown.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                handlers.retain(|handler| !handler.is_finished());
                if shared.handshakes.load(Ordering::SeqCst) >= MAX_HANDSHAKES {
                    let _ = stream.shutdown(Shutdown::Both);
                    continue;
                }
                shared.handshakes.fetch_add(1, Ordering::SeqCst);
                let handler_shared = Arc::clone(shared);
                match thread::Builder::new()
                    .name("online-client".to_owned())
                    .spawn(move || serve_connection(&handler_shared, &stream))
                {
                    Ok(handler) => handlers.push(handler),
                    Err(_) => {
                        // The closure never ran, so it cannot release its handshake slot.
                        shared.handshakes.fetch_sub(1, Ordering::SeqCst);
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(PUMP_TICK);
            }
            Err(_) => thread::sleep(PUMP_TICK),
        }
    }
    for handler in handlers {
        let _ = handler.join();
    }
}

/// One connection, from hello to hang-up.
fn serve_connection(shared: &Arc<Shared>, stream: &TcpStream) {
    let admitted = handshake(shared, stream);
    shared.handshakes.fetch_sub(1, Ordering::SeqCst);
    let Some((id, seat, mut reader)) = admitted else {
        let _ = stream.shutdown(Shutdown::Both);
        return;
    };
    let _ = reader.set_read_timeout(Some(CLIENT_SILENCE));
    while !shared.shutdown.load(Ordering::SeqCst) {
        let Ok(message) = read_message::<ClientMessage>(&mut reader) else {
            break;
        };
        match message {
            ClientMessage::Ping => {}
            ClientMessage::Hello { .. } => break,
            ClientMessage::Submit(submission) => {
                let mut table = shared.lock();
                let answer = match table.gate.clone() {
                    Some(gate) if pending_actor(&gate).as_ref() == Some(&seat) => {
                        describe(&gate.submit(&submission))
                    }
                    Some(_) => (false, "your seat is not being asked anything".to_owned()),
                    None => (false, "no table is running".to_owned()),
                };
                send(
                    &mut table,
                    id,
                    ServerMessage::Answer {
                        accepted: answer.0,
                        message: answer.1,
                    },
                );
            }
            ClientMessage::Delegate { fingerprint } => {
                let mut table = shared.lock();
                let answer = match table.gate.clone().and_then(|gate| {
                    gate.pending()
                        .filter(|pending| {
                            pending.actor == seat && pending.fingerprint == fingerprint
                        })
                        .map(|_| gate)
                }) {
                    Some(gate) => match gate.delegate_pending() {
                        Ok(_) => (true, "the policy answered this one".to_owned()),
                        Err(error) => (false, format!("{error:?}")),
                    },
                    None => (false, "that choice is no longer on screen".to_owned()),
                };
                send(
                    &mut table,
                    id,
                    ServerMessage::Answer {
                        accepted: answer.0,
                        message: answer.1,
                    },
                );
            }
        }
    }
    let mut table = shared.lock();
    drop_client(&mut table, id, "connection closed");
}

fn pending_actor(gate: &Gate) -> Option<PlayerId> {
    gate.pending().map(|pending| pending.actor)
}

fn describe(outcome: &SubmitOutcome) -> (bool, String) {
    match outcome {
        SubmitOutcome::Accepted { option_id } => (true, format!("played {option_id}")),
        SubmitOutcome::NoPendingChoice => (false, "nothing is waiting for an answer".to_owned()),
        SubmitOutcome::Stale { .. } => (
            false,
            "that choice changed before your answer arrived".to_owned(),
        ),
        SubmitOutcome::NotOffered { .. } => (false, "that option was not offered".to_owned()),
        SubmitOutcome::Duplicate => (false, "that choice was already answered".to_owned()),
    }
}

/// Read the hello and seat the client, or refuse it. On success the client is registered and its
/// writer is running; the returned stream is the read half.
fn handshake(shared: &Arc<Shared>, stream: &TcpStream) -> Option<(u64, PlayerId, TcpStream)> {
    stream.set_nonblocking(false).ok()?;
    stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT)).ok()?;
    let _ = stream.set_nodelay(true);
    let mut reader = stream.try_clone().ok()?;
    let refuse = |reason: &str| refuse(stream, reason);
    let Ok(ClientMessage::Hello {
        protocol,
        build,
        code,
        seat,
        name,
        resume,
    }) = read_message::<ClientMessage>(&mut reader)
    else {
        return refuse("expected a hello");
    };
    if protocol != PROTOCOL {
        return refuse(&format!("this host speaks {PROTOCOL}, not {protocol}"));
    }
    if build != ti4_review::ENGINE_COMMIT {
        return refuse(&format!(
            "the host runs build {}, this client is build {build}; both need the same one",
            ti4_review::ENGINE_COMMIT
        ));
    }
    if !same_secret(&code, &shared.code) {
        // Slows a guesser down; a 128-bit code is the real defence.
        thread::sleep(Duration::from_millis(500));
        return refuse("wrong join code");
    }
    let name: String = name.trim().chars().take(MAX_NAME_CHARS).collect();
    let name = if name.is_empty() {
        "player".to_owned()
    } else {
        name
    };

    let mut table = shared.lock();
    let seat = match choose_seat(&table, seat, resume.as_deref()) {
        Ok(seat) => seat,
        Err(reason) => {
            drop(table);
            return refuse(&reason);
        }
    };
    let (outbox, inbox) = mpsc::sync_channel(OUTBOX);
    let in_flight = Arc::new(AtomicUsize::new(0));
    let writer_stream = stream.try_clone().ok()?;
    spawn_writer(writer_stream, inbox, Arc::clone(&in_flight))?;
    let id = table.next_client;
    table.next_client += 1;
    let resume = match table.claims.get(&seat) {
        Some(claim) => claim.resume.clone(),
        None => random_token(),
    };
    table.claims.insert(
        seat.clone(),
        Claim {
            name,
            resume: resume.clone(),
            client: Some(id),
        },
    );
    if let Some(gate) = &table.gate {
        gate.set_mode(&seat, SeatMode::Manual);
    }
    table.clients.insert(
        id,
        Client {
            seat: seat.clone(),
            outbox,
            stream: stream.try_clone().ok()?,
            in_flight,
            generation: 0,
            header_sent: false,
            frames_sent: 0,
            pending_sent: None,
        },
    );
    send(
        &mut table,
        id,
        ServerMessage::Welcome {
            seat: seat.clone(),
            resume,
        },
    );
    // Everybody hears about the new seat, the newcomer included.
    table.last_status = None;
    Some((id, seat, reader))
}

/// Tell a client why it was not seated. Always `None`, so a refusal reads as a return.
fn refuse<T>(stream: &TcpStream, reason: &str) -> Option<T> {
    if let Ok(mut writer) = stream.try_clone() {
        let _ = write_message(
            &mut writer,
            &ServerMessage::Refused {
                reason: reason.to_owned(),
            },
        );
    }
    None
}

fn choose_seat(
    table: &Table,
    wanted: Option<PlayerId>,
    resume: Option<&str>,
) -> Result<PlayerId, String> {
    let (Some(gate), Some(latest)) = (&table.gate, table.frames.last()) else {
        return Err("the host has no table running yet".to_owned());
    };
    let seats: Vec<PlayerId> = latest.state.players.iter().map(|p| p.id.clone()).collect();
    if table.clients.len() >= MAX_CLIENTS {
        return Err("the table is full".to_owned());
    }
    let modes = gate.snapshot().seats;
    let free = |seat: &PlayerId| !table.claims.contains_key(seat) && !modes.is_manual(seat);
    match wanted {
        Some(seat) if !seats.contains(&seat) => Err(format!("there is no seat {seat}")),
        Some(seat) => match table.claims.get(&seat) {
            Some(claim) if claim.client.is_some() => Err(format!("{seat} is taken")),
            Some(claim) if resume.is_some_and(|token| same_secret(token, &claim.resume)) => {
                Ok(seat)
            }
            Some(_) => Err(format!("{seat} is held for a player who disconnected")),
            None if free(&seat) => Ok(seat),
            None => Err(format!("the host is playing {seat}")),
        },
        None => seats
            .into_iter()
            .find(|seat| free(seat))
            .ok_or_else(|| "no seat is free".to_owned()),
    }
}

fn spawn_writer(
    mut stream: TcpStream,
    inbox: Receiver<ServerMessage>,
    in_flight: Arc<AtomicUsize>,
) -> Option<()> {
    thread::Builder::new()
        .name("online-writer".to_owned())
        .spawn(move || {
            for message in inbox {
                let frames = matches!(message, ServerMessage::Frames(_) | ServerMessage::Header(_));
                let written = write_message(&mut stream, &message);
                if frames {
                    in_flight.fetch_sub(1, Ordering::SeqCst);
                }
                if written.is_err() || matches!(message, ServerMessage::Closed { .. }) {
                    break;
                }
            }
            let _ = stream.shutdown(Shutdown::Both);
        })
        .ok()
        .map(|_| ())
}

/// Queue a message for one client. A client whose outbox is full is dropped: it has stopped reading,
/// and the host will not hold memory for it.
fn send(table: &mut Table, id: u64, message: ServerMessage) {
    let Some(client) = table.clients.get(&id) else {
        return;
    };
    let frames = matches!(message, ServerMessage::Frames(_) | ServerMessage::Header(_));
    if frames {
        client.in_flight.fetch_add(1, Ordering::SeqCst);
    }
    match client.outbox.try_send(message) {
        Ok(()) => {}
        Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
            if frames {
                client.in_flight.fetch_sub(1, Ordering::SeqCst);
            }
            drop_client(table, id, "fell too far behind");
        }
    }
}

/// Forget a connection. Its claim stays, so the player can come back to the same seat.
fn drop_client(table: &mut Table, id: u64, reason: &str) {
    let Some(client) = table.clients.remove(&id) else {
        return;
    };
    let _ = client.outbox.try_send(ServerMessage::Closed {
        reason: reason.to_owned(),
    });
    // The writer sends the goodbye and then shuts the socket; shutting the read half here unblocks
    // the handler now rather than at its next timeout.
    let _ = client.stream.shutdown(Shutdown::Read);
    if let Some(claim) = table.claims.get_mut(&client.seat)
        && claim.client == Some(id)
    {
        claim.client = None;
    }
    table.last_status = None;
}

fn pump_loop(shared: &Arc<Shared>) {
    while !shared.shutdown.load(Ordering::SeqCst) {
        pump_frames(shared);
        pump_status(shared);
        thread::sleep(PUMP_TICK);
    }
}

/// Send every client the frames it is owed, redacted for its seat.
fn pump_frames(shared: &Arc<Shared>) {
    let owed: Vec<Owed> = {
        let mut table = shared.lock();
        let generation = table.generation;
        let header = table.header.clone();
        let total = table.frames.len();
        let mut owed = Vec::new();
        for (id, client) in &mut table.clients {
            if client.generation != generation {
                client.generation = generation;
                client.header_sent = false;
                client.frames_sent = 0;
            }
            if header.is_none() || client.in_flight.load(Ordering::SeqCst) >= FRAMES_IN_FLIGHT {
                continue;
            }
            let from = client.frames_sent;
            let until = total.min(from + FRAMES_PER_MESSAGE * 2);
            if client.header_sent && from >= until {
                continue;
            }
            owed.push(Owed {
                client: *id,
                seat: client.seat.clone(),
                generation,
                header: (!client.header_sent).then(|| header.clone()).flatten(),
                from,
                frames: Vec::new(),
            });
        }
        for item in &mut owed {
            let until = total.min(item.from + FRAMES_PER_MESSAGE * 2);
            item.frames = table.frames[item.from..until].to_vec();
        }
        owed
    };
    for item in owed {
        let redacted: Vec<ReviewFrame> = item
            .frames
            .iter()
            .map(|frame| frame_for(frame, &item.seat))
            .collect();
        let mut table = shared.lock();
        if table.generation != item.generation {
            return;
        }
        let Some(client) = table.clients.get(&item.client) else {
            continue;
        };
        if client.frames_sent != item.from {
            continue;
        }
        let count = redacted.len();
        if let Some(header) = item.header {
            send(
                &mut table,
                item.client,
                ServerMessage::Header(Box::new((*header).clone())),
            );
        }
        let mut rest = redacted;
        while !rest.is_empty() {
            let tail = rest.split_off(rest.len().min(FRAMES_PER_MESSAGE));
            send(&mut table, item.client, ServerMessage::Frames(rest));
            rest = tail;
        }
        if let Some(client) = table.clients.get_mut(&item.client) {
            client.header_sent = true;
            client.frames_sent = item.from + count;
        }
    }
}

/// Relay the pending choice to the seat it is for, and the table status to everybody.
fn pump_status(shared: &Arc<Shared>) {
    let mut table = shared.lock();
    let Some(gate) = table.gate.clone() else {
        return;
    };
    let snapshot = gate.snapshot();
    let pending = snapshot.pending;
    let ids: Vec<u64> = table.clients.keys().copied().collect();
    for id in ids {
        let Some(client) = table.clients.get(&id) else {
            continue;
        };
        // A client sees its panel only once it has the frame the engine is paused on; otherwise the
        // choice would describe a position it cannot draw yet.
        let caught_up =
            client.generation == table.generation && client.frames_sent >= table.frames.len();
        let mine = pending
            .as_ref()
            .filter(|pending| pending.actor == client.seat && caught_up);
        let fingerprint = mine.map(|pending| pending.fingerprint.clone());
        if fingerprint != client.pending_sent {
            let message = ServerMessage::Pending(mine.cloned().map(Box::new));
            if let Some(client) = table.clients.get_mut(&id) {
                client.pending_sent = fingerprint;
            }
            send(&mut table, id, message);
        }
    }
    let seats = table.frames.last().map_or_else(Vec::new, |frame| {
        frame
            .state
            .players
            .iter()
            .map(|player| SeatStatus {
                seat: player.id.clone(),
                holder: match table.claims.get(&player.id) {
                    Some(claim) => Holder::Remote {
                        name: claim.name.clone(),
                        connected: claim.client.is_some(),
                    },
                    None if snapshot.seats.is_manual(&player.id) => Holder::Host,
                    None => Holder::Bot,
                },
            })
            .collect()
    });
    let status = ServerMessage::Table {
        seats,
        state: format!("{:?}", snapshot.state),
        waiting_on: pending.map(|pending| pending.actor),
    };
    if table.last_status.as_ref() != Some(&status) {
        table.last_status = Some(status.clone());
        let ids: Vec<u64> = table.clients.keys().copied().collect();
        for id in ids {
            send(&mut table, id, status.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_compare_by_value_and_length() {
        assert!(same_secret("abc", "abc"));
        assert!(!same_secret("abc", "abd"));
        assert!(!same_secret("abc", "abcd"));
    }

    #[test]
    fn a_given_code_is_used_and_a_weak_one_refused() {
        let local = SocketAddr::from(([127, 0, 0, 1], 0));
        let host = NetHost::start_with_code(local, Some("abcdef0123456789".to_owned())).unwrap();
        assert_eq!(host.code(), "abcdef0123456789");
        for weak in ["short", "sixteen-but-dash", ""] {
            let error = NetHost::start_with_code(local, Some(weak.to_owned()))
                .err()
                .unwrap();
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{weak:?}");
        }
    }

    #[test]
    fn tokens_are_128_bits_of_hex_and_differ() {
        let (a, b) = (random_token(), random_token());
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|ch| ch.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }
}
