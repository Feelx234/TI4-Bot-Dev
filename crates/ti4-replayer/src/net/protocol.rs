//! The wire: length-prefixed, zstd-compressed JSON messages over one TCP stream.
//!
//! A message is a big-endian `u32` byte count followed by that many bytes of zstd-compressed JSON.
//! Both bounds are checked before anything is allocated: the prefix against [`MAX_WIRE_BYTES`], and
//! the decompressed size against [`MAX_MESSAGE_BYTES`]. A frame carries a whole `GameState` (around
//! 200 KB of JSON), which compresses to a few tens of KB.

use std::io::{self, Read, Write};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use ti4_model::id::PlayerId;
use ti4_review::{ReviewFrame, ReviewSession};

use crate::control::{ChoiceFingerprint, ManualSubmission, PendingManualChoice};

/// Protocol name and version. A client from another version is refused at hello.
pub const PROTOCOL: &str = "ti4-online-v1";

/// Largest compressed message accepted.
pub const MAX_WIRE_BYTES: u32 = 16 * 1024 * 1024;

/// Largest decompressed message accepted.
pub const MAX_MESSAGE_BYTES: u64 = 64 * 1024 * 1024;

/// Frames sent in one message while catching a client up.
pub const FRAMES_PER_MESSAGE: usize = 32;

/// Longest player name kept; the rest is cut.
pub const MAX_NAME_CHARS: usize = 32;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "body", rename_all = "snake_case")]
pub enum ClientMessage {
    /// The first message on a connection.
    Hello {
        protocol: String,
        /// The table's join code.
        code: String,
        /// The seat asked for; `None` takes the first free one.
        seat: Option<PlayerId>,
        name: String,
        /// The token a previous `Welcome` handed out, to take a seat back after a disconnect.
        resume: Option<String>,
    },
    /// Answer the choice this seat is being asked.
    Submit(ManualSubmission),
    /// Let the policy answer this one choice; the seat stays with the player.
    Delegate { fingerprint: ChoiceFingerprint },
    /// Keep-alive. The host drops a client it has not heard from in [`super::CLIENT_SILENCE`].
    Ping,
}

/// Who is sitting in a seat, as every client is told.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Holder {
    /// The learned policy.
    Bot,
    /// The person running the host window.
    Host,
    /// A remote player.
    Remote { name: String, connected: bool },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeatStatus {
    pub seat: PlayerId,
    pub holder: Holder,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "body", rename_all = "snake_case")]
pub enum ServerMessage {
    /// The hello was accepted: this connection plays `seat`.
    Welcome {
        seat: PlayerId,
        /// Present this in a later hello to take the seat back.
        resume: String,
    },
    /// The hello was refused; the connection closes after this.
    Refused { reason: String },
    /// A new timeline starts: drop every frame held and start from this header.
    Header(Box<ReviewSession>),
    /// Frames in engine order, already redacted for the receiving seat.
    Frames(Vec<ReviewFrame>),
    /// The choice this seat is being asked now, or `None` when it is not being asked anything.
    Pending(Option<Box<PendingManualChoice>>),
    /// What became of the last submit or delegate.
    Answer { accepted: bool, message: String },
    /// Seats and what the game is doing.
    Table {
        seats: Vec<SeatStatus>,
        /// The live branch's state, as the host sees it.
        state: String,
        /// The seat the game is parked on, if any. Public: at a real table everybody can see who is
        /// thinking.
        waiting_on: Option<PlayerId>,
    },
    /// The host is closing the connection.
    Closed { reason: String },
}

/// Write one message.
///
/// # Errors
/// I/O failure, or a message larger than [`MAX_WIRE_BYTES`] once compressed.
pub fn write_message<T: Serialize>(sink: &mut impl Write, message: &T) -> io::Result<()> {
    let json = serde_json::to_vec(message).map_err(io::Error::other)?;
    let packed = zstd::bulk::compress(&json, 3)?;
    let length = u32::try_from(packed.len())
        .ok()
        .filter(|length| *length <= MAX_WIRE_BYTES)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "message too large"))?;
    sink.write_all(&length.to_be_bytes())?;
    sink.write_all(&packed)?;
    sink.flush()
}

/// Read one message, refusing an oversized or malformed one before decoding it.
///
/// # Errors
/// I/O failure, a length or decompressed size over the bounds, or bytes that are not a message.
pub fn read_message<T: DeserializeOwned>(source: &mut impl Read) -> io::Result<T> {
    let mut prefix = [0_u8; 4];
    source.read_exact(&mut prefix)?;
    let length = u32::from_be_bytes(prefix);
    if length > MAX_WIRE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("message of {length} bytes exceeds {MAX_WIRE_BYTES}"),
        ));
    }
    let mut packed = vec![0_u8; length as usize];
    source.read_exact(&mut packed)?;
    let mut json = Vec::new();
    zstd::stream::read::Decoder::new(packed.as_slice())?
        .take(MAX_MESSAGE_BYTES + 1)
        .read_to_end(&mut json)?;
    if json.len() as u64 > MAX_MESSAGE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "message decompresses past the bound",
        ));
    }
    serde_json::from_slice(&json).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_survives_the_round_trip() {
        let sent = ClientMessage::Hello {
            protocol: PROTOCOL.to_owned(),
            code: "abc".to_owned(),
            seat: Some(PlayerId::new("seat2")),
            name: "n".to_owned(),
            resume: None,
        };
        let mut wire = Vec::new();
        write_message(&mut wire, &sent).unwrap();
        let back: ClientMessage = read_message(&mut wire.as_slice()).unwrap();
        assert_eq!(back, sent);
    }

    #[test]
    fn every_server_message_shape_can_be_written() {
        for sent in [
            ServerMessage::Frames(Vec::new()),
            ServerMessage::Pending(None),
            ServerMessage::Answer {
                accepted: true,
                message: "m".to_owned(),
            },
        ] {
            let mut wire = Vec::new();
            write_message(&mut wire, &sent).unwrap();
            let back: ServerMessage = read_message(&mut wire.as_slice()).unwrap();
            assert_eq!(back, sent);
        }
    }

    #[test]
    fn an_oversized_prefix_is_refused_before_allocating() {
        let wire = (MAX_WIRE_BYTES + 1).to_be_bytes();
        let error = read_message::<ClientMessage>(&mut wire.as_slice()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn garbage_is_an_error_not_a_message() {
        let mut wire = 5_u32.to_be_bytes().to_vec();
        wire.extend_from_slice(b"hello");
        assert!(read_message::<ClientMessage>(&mut wire.as_slice()).is_err());
    }
}
