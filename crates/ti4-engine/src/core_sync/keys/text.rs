//! The text of the pieces: ids, seats, numbers, words, blobs.

use ti4_engine_core::Seat;
use ti4_engine_core::generated::{
    ActionCard, Agenda, Breakthrough, Faction, Leader, Planet, PromissoryNote, Relic,
    SecretObjective, System, Technology, Unit,
};
use ti4_engine_core::keys::{Atom, DecimalTextOrd, Word};

use super::word_text::{word_from, word_text};
use crate::generated::IdStr;

/// The owner table of one game: the player ids in text order, a seat being the index.
///
/// Typed order of seats is text order of the ids only while no id is a proper prefix of another
/// (`p1` and `p10`: the byte after `p1` in a key decides, not the table). [`Owners::new`] refuses
/// such a table, and the caller keeps the string state for that game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owners {
    ids: Vec<String>,
}

impl Owners {
    /// The table for these ids (any order, duplicates ignored); `None` for more than 16 owners or
    /// an id that is a proper prefix of another.
    #[must_use]
    pub fn new<'a>(ids: impl IntoIterator<Item = &'a str>) -> Option<Self> {
        let mut ids: Vec<String> = ids.into_iter().map(str::to_owned).collect();
        ids.sort();
        ids.dedup();
        if ids.len() > 16 {
            return None;
        }
        for (i, a) in ids.iter().enumerate() {
            if ids[i + 1..].iter().any(|b| b.starts_with(a.as_str())) {
                return None;
            }
        }
        Some(Self { ids })
    }

    /// The seat of an id.
    #[must_use]
    pub fn seat(&self, id: &str) -> Option<Seat> {
        self.ids
            .binary_search_by(|probe| probe.as_str().cmp(id))
            .ok()
            .map(|i| Seat(i as u8))
    }

    /// The id of a seat.
    #[must_use]
    pub fn id(&self, seat: Seat) -> Option<&str> {
        self.ids.get(usize::from(seat.0)).map(String::as_str)
    }
}

/// Texts of values whose structure belongs to a later package (staged events, borrowed-agent
/// records, a sustain target), interned: `Atom::Blob(i)` is `texts[i]`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Blobs {
    texts: Vec<String>,
}

impl Blobs {
    pub fn intern(&mut self, text: &str) -> u32 {
        if let Some(i) = self.texts.iter().position(|t| t == text) {
            return i as u32;
        }
        self.texts.push(text.to_owned());
        (self.texts.len() - 1) as u32
    }

    #[must_use]
    pub fn get(&self, index: u32) -> Option<&str> {
        self.texts.get(index as usize).map(String::as_str)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.texts.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.texts.is_empty()
    }
}

/// Everything the text of a piece depends on: the owner table, the blob table.
#[derive(Debug, Clone, Default)]
pub struct Codec {
    pub owners: Option<Owners>,
    pub blobs: Blobs,
}

impl Codec {
    #[must_use]
    pub fn new(owners: Owners) -> Self {
        Self {
            owners: Some(owners),
            blobs: Blobs::default(),
        }
    }
}

/// What kind of piece sits at a place of a pattern or a schema.
#[derive(Debug, Clone, Copy)]
pub enum K {
    Seat,
    System,
    Planet,
    Unit,
    Tech,
    Card,
    Note,
    Secret,
    Agenda,
    Faction,
    Leader,
    Relic,
    Breakthrough,
    /// A plain count: decimal digits, no leading zero.
    Num,
    /// A key number that sorts as its text: the same text as `Num`.
    Dec,
    /// A number zero padded to this many digits.
    Pad(u8),
    /// One of these words.
    Word(&'static [Word]),
    Outcome,
    Destroy,
    /// An uninterpreted text (see [`Blobs`]).
    Blob,
    /// The empty text.
    Empty,
    /// The empty text or this kind.
    Opt(&'static K),
}

/// A promissory note as the state writes it: the generic `<color>_an` is `an`.
#[must_use]
pub fn note_text(note: PromissoryNote) -> &'static str {
    let id = note.as_str();
    id.strip_prefix("<color>_").unwrap_or(id)
}

/// The note with this state text.
#[must_use]
pub fn parse_note(text: &str) -> Option<PromissoryNote> {
    if text.starts_with("<color>_") {
        return None;
    }
    PromissoryNote::from_id(text).or_else(|| PromissoryNote::from_id(&format!("<color>_{text}")))
}

/// Canonical decimal: digits, no leading zero (but `0`), fits a `u32`.
#[must_use]
pub fn parse_num(text: &str) -> Option<u32> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if text.len() > 1 && text.starts_with('0') {
        return None;
    }
    text.parse().ok()
}

fn parse_padded(text: &str, width: u8) -> Option<u32> {
    if text.len() != usize::from(width) || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

impl Codec {
    /// The piece of this kind with this text.
    pub fn parse_atom(&mut self, kind: K, text: &str) -> Option<Atom> {
        Some(match kind {
            K::Seat => Atom::Seat(self.owners.as_ref()?.seat(text)?),
            K::System => Atom::System(System::from_id(text)?),
            K::Planet => Atom::Planet(Planet::from_id(text)?),
            K::Unit => Atom::Unit(Unit::from_id(text)?),
            K::Tech => Atom::Tech(Technology::from_id(text)?),
            K::Card => Atom::Card(ActionCard::from_id(text)?),
            K::Note => Atom::Note(parse_note(text)?),
            K::Secret => Atom::Secret(SecretObjective::from_id(text)?),
            K::Agenda => Atom::Agenda(Agenda::from_id(text)?),
            K::Faction => Atom::Faction(Faction::from_id(text)?),
            K::Leader => Atom::Leader(Leader::from_id(text)?),
            K::Relic => Atom::Relic(Relic::from_id(text)?),
            K::Breakthrough => Atom::Breakthrough(Breakthrough::from_id(text)?),
            K::Num => Atom::Num(parse_num(text)?),
            K::Dec => Atom::Dec(DecimalTextOrd(parse_num(text)?)),
            K::Pad(width) => Atom::Num(parse_padded(text, width)?),
            K::Word(allowed) => Atom::Word(word_from(text, allowed)?),
            K::Outcome => Atom::Outcome(self.parse_outcome(text, None)?),
            K::Destroy => Atom::Destroy(super::other::parse_destroy(text)?),
            K::Blob => Atom::Blob(self.blobs.intern(text)),
            K::Empty => {
                if !text.is_empty() {
                    return None;
                }
                Atom::Absent
            }
            K::Opt(inner) => {
                if text.is_empty() {
                    Atom::Absent
                } else {
                    self.parse_atom(*inner, text)?
                }
            }
        })
    }

    /// Append the text of a piece of this kind; `None` when the piece is not of that kind.
    pub fn render_atom(&self, kind: K, atom: Atom, out: &mut String) -> Option<()> {
        match (kind, atom) {
            (K::Seat, Atom::Seat(s)) => out.push_str(self.owners.as_ref()?.id(s)?),
            (K::System, Atom::System(x)) => out.push_str(x.as_str()),
            (K::Planet, Atom::Planet(x)) => out.push_str(x.as_str()),
            (K::Unit, Atom::Unit(x)) => out.push_str(x.as_str()),
            (K::Tech, Atom::Tech(x)) => out.push_str(x.as_str()),
            (K::Card, Atom::Card(x)) => out.push_str(x.as_str()),
            (K::Note, Atom::Note(x)) => out.push_str(note_text(x)),
            (K::Secret, Atom::Secret(x)) => out.push_str(x.as_str()),
            (K::Agenda, Atom::Agenda(x)) => out.push_str(x.as_str()),
            (K::Faction, Atom::Faction(x)) => out.push_str(x.as_str()),
            (K::Leader, Atom::Leader(x)) => out.push_str(x.as_str()),
            (K::Relic, Atom::Relic(x)) => out.push_str(x.as_str()),
            (K::Breakthrough, Atom::Breakthrough(x)) => out.push_str(x.as_str()),
            (K::Num, Atom::Num(n)) => out.push_str(&n.to_string()),
            (K::Dec, Atom::Dec(n)) => out.push_str(&n.0.to_string()),
            (K::Pad(width), Atom::Num(n)) => {
                out.push_str(&format!("{n:0width$}", width = usize::from(width)));
            }
            (K::Word(allowed), Atom::Word(w)) if allowed.contains(&w) => {
                out.push_str(word_text(w));
            }
            (K::Outcome, Atom::Outcome(o)) => out.push_str(&self.render_outcome(o)?),
            (K::Destroy, Atom::Destroy(d)) => out.push_str(&super::other::render_destroy(d)),
            (K::Blob, Atom::Blob(i)) => out.push_str(self.blobs.get(i)?),
            (K::Empty, Atom::Absent) | (K::Opt(_), Atom::Absent) => {}
            (K::Opt(inner), atom) => return self.render_atom(*inner, atom, out),
            _ => return None,
        }
        Some(())
    }
}
