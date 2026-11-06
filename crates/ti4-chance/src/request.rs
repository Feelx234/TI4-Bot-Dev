//! What the engine asks of chance ([`Request`]) and what it gets back ([`Faces`]).
//!
//! A request is one roll group: a number of dice of one kind, drawn in order from one domain. It carries no state and
//! no rule: the answer is the same whether it comes from the game's own streams (play), from a search that picks the
//! faces, or from a sampler. That is what lets a chance node be a resumable position (docs/FORK_RNG_THEORY.md 4.3).

use crate::Domain;

/// The most faces one request carries. [`Faces`] is 13 bytes, so the core's `Pick` stays 16 bytes (the faces fit beside the niche
/// of its option variant; with 15 faces it grew to 20, and the browser build by 8 KB: lane C3); a larger group is
/// split in order ([`Request::dice_groups`]), which draws exactly the same faces as one long group.
pub const MAX_FACES: usize = 12;

/// One roll group.
#[cfg_attr(not(target_family = "wasm"), derive(Debug))]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Request {
    pub domain: Domain,
    pub kind: Kind,
    /// The seat the roll belongs to (who would see the chance node), if any.
    pub owner: Option<u8>,
    /// An opaque tag of the asking site (for counters and search filters); never read by [`crate::answer`].
    pub purpose: u32,
}

/// What is drawn. Every kind draws one die per face, `1..=sides`, as today's `die(domain, sides)`.
#[cfg_attr(not(target_family = "wasm"), derive(Debug))]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// `n` dice of `sides` sides (`n <= MAX_FACES`). `hits_on` is the threshold (a face `>= hits_on` hits); it does not
    /// change the draw and is there for the search helpers and for display.
    Dice { n: u8, sides: u8, hits_on: Option<u8> },
    /// A uniform choice of one element among `n`: one face in `1..=n` (Kyver hero, Neuraloop, Spy, Sanctions: `die(n)`).
    Index { n: u8 },
}

impl Request {
    #[must_use]
    pub const fn new(domain: Domain, kind: Kind) -> Self {
        Self { domain, kind, owner: None, purpose: 0 }
    }

    #[must_use]
    pub const fn owned(self, owner: Option<u8>, purpose: u32) -> Self {
        Self { owner, purpose, ..self }
    }

    /// `n` dice of one kind as requests of at most [`MAX_FACES`] dice each, in draw order. Answering them one after
    /// the other draws the same faces as `n` single dice.
    pub fn dice_groups(domain: Domain, n: usize, sides: u8, hits_on: Option<u8>) -> impl Iterator<Item = Self> {
        let full = n / MAX_FACES;
        let rest = n % MAX_FACES;
        #[allow(clippy::cast_possible_truncation)] // both are at most MAX_FACES
        let sizes = core::iter::repeat_n(MAX_FACES as u8, full).chain((rest > 0).then_some(rest as u8));
        sizes.map(move |n| Self::new(domain, Kind::Dice { n, sides, hits_on }))
    }

    /// How many faces the answer has.
    #[must_use]
    pub const fn len(&self) -> usize {
        match self.kind {
            Kind::Dice { n, .. } => n as usize,
            Kind::Index { .. } => 1,
        }
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The sides of each die drawn.
    #[must_use]
    pub const fn sides(&self) -> u8 {
        match self.kind {
            Kind::Dice { sides, .. } => sides,
            Kind::Index { n } => n,
        }
    }

    /// Whether `faces` is a legal answer: the right count, every face in `1..=sides`. What `advance` checks for an
    /// answer a search hands in (docs/FORK_RNG_THEORY.md 4.3).
    #[must_use]
    pub fn accepts(&self, faces: &Faces) -> bool {
        let sides = self.sides();
        faces.len() == self.len() && faces.as_slice().iter().all(|&f| (1..=sides).contains(&f))
    }
}

/// The answer to a request: the faces in draw order. 13 bytes (a length and 12 slots; unused slots stay 0 so the
/// derived equality is the equality of the faces).
#[cfg_attr(not(target_family = "wasm"), derive(Debug))]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Faces {
    len: u8,
    faces: [u8; MAX_FACES],
}

impl Faces {
    /// The faces of `slice`; `None` for more than [`MAX_FACES`].
    #[must_use]
    pub fn from_slice(slice: &[u8]) -> Option<Self> {
        let mut out = Self::default();
        for &f in slice {
            out.try_push(f)?;
        }
        Some(out)
    }

    /// Append one face; `None` when full.
    pub fn try_push(&mut self, face: u8) -> Option<()> {
        let slot = self.faces.get_mut(usize::from(self.len))?;
        *slot = face;
        self.len += 1;
        Some(())
    }

    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.faces[..usize::from(self.len)]
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The faces that hit (`>= hits_on`).
    #[must_use]
    pub fn hits(&self, hits_on: u8) -> usize {
        self.as_slice().iter().filter(|&&f| f >= hits_on).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faces_is_thirteen_bytes() {
        assert_eq!(core::mem::size_of::<Faces>(), 13);
        assert!(Faces::from_slice(&[1; MAX_FACES]).is_some());
        assert!(Faces::from_slice(&[1; MAX_FACES + 1]).is_none());
    }

    #[test]
    fn groups_split_in_order() {
        let sizes = |n| Request::dice_groups(Domain::Dice, n, 10, None).map(|r| r.len()).collect::<Vec<_>>();
        assert_eq!(sizes(0), Vec::<usize>::new());
        assert_eq!(sizes(12), [12]);
        assert_eq!(sizes(13), [12, 1]);
        assert_eq!(sizes(47), [12, 12, 12, 11]);
    }

    #[test]
    fn accepts_checks_count_and_range() {
        let r = Request::new(Domain::Dice, Kind::Dice { n: 2, sides: 10, hits_on: Some(7) });
        assert!(r.accepts(&Faces::from_slice(&[1, 10]).unwrap()));
        assert!(!r.accepts(&Faces::from_slice(&[0, 10]).unwrap()));
        assert!(!r.accepts(&Faces::from_slice(&[11, 1]).unwrap()));
        assert!(!r.accepts(&Faces::from_slice(&[1]).unwrap()));
        let i = Request::new(Domain::Neuraloop, Kind::Index { n: 3 });
        assert!(i.accepts(&Faces::from_slice(&[3]).unwrap()));
        assert!(!i.accepts(&Faces::from_slice(&[4]).unwrap()));
    }

}

// The browser build prints a chance request or its faces nowhere a player sees: a fixed text keeps the derived `Debug` (and the names of
// the variants and fields) out of the wasm file (lane C3, the size budget). Every native build derives it.
#[cfg(target_family = "wasm")]
impl core::fmt::Debug for Request {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Request")
    }
}

#[cfg(target_family = "wasm")]
impl core::fmt::Debug for Kind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Kind")
    }
}

#[cfg(target_family = "wasm")]
impl core::fmt::Debug for Faces {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Faces")
    }
}
