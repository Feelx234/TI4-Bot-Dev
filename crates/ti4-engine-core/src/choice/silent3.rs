//! Questions of the abilities lane G3 ported (`docs/G3_RESUME.md`), part 2 of the loud gap rows. They are variants of `silent` (`silent::Ask::T`, `Opt::T`,
//! `Subtype::T`), so the lanes that add to `silent` and this one never touch the same lines; the renderer is `render/silent3.rs`.

use crate::generated::{Faction, Planet, System, Unit};
use crate::ids::Seat;
use crate::state::NoteKey;

/// What a G3 ability asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// Ragh's Call (Saar promissory note): the Saar player says where the Saar ground forces on `planet` go.
    Ragh { planet: Planet },
    /// The Alastor (Nekro flagship): how many of the ground forces of type `unit` on `planet` fight as ships.
    Alastor { planet: Planet, unit: Unit },
    /// Exotrireme II (Sardakk): which of the owner's Exotrireme II is destroyed.
    ExoSelf,
    /// Exotrireme II: the ship to destroy, number `round + 1` of up to 2.
    ExoVictim { round: u8 },
    /// Suffi An (Mentak agent): exhaust to draw 1 action card each with `target`, the player pillaged.
    SuffiAn { target: Seat },
    /// S'ula Mentarion (Mentak commander): which opponent gives a promissory note.
    SulaOpponent,
    /// S'ula Mentarion: the opponent says which promissory note of the hand goes to `to`.
    SulaGive { to: Seat },
}

/// One answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// A planet in a system (the option id is `{system}|{planet}`).
    Site { system: System, planet: Planet },
    /// A count (the option id is `alastor|{n}`).
    Count(u32),
    /// A ship of the list the question was built from (`index`), its owner, kind and whether it is damaged; `victim`: one of the up-to-two ships (the other is the
    /// Exotrireme II itself).
    Ship { index: u8, owner: Seat, unit: Unit, damaged: bool, victim: bool },
    /// Exhaust the agent (the answer `exhaust`).
    Exhaust,
    /// A seat (the option id is the seat's player id).
    Seat(Seat),
    /// A promissory note of the hand.
    Note(NoteKey),
    /// The seat's own Support for the Throne (`support:{faction}`).
    Support(Faction),
}

/// The machine identifier of the question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    /// `ragh_destination`.
    Ragh,
    /// `alastor_participants`.
    Alastor,
    /// `exotrireme_self`.
    ExoSelf,
    /// `exotrireme_victim`.
    ExoVictim,
    /// `suffi_an`.
    SuffiAn,
    /// `commander_opponent`.
    SulaOpponent,
    /// `commander_give_note`.
    SulaGive,
}

impl Ask {
    /// Whether the question's context carries the trigger of its window (see `silent::Ask::carries_trigger`): decided per question by the lockstep, the
    /// answers of a seat other than the window's holder (Ragh's Call asks the Saar) carry none.
    #[must_use]
    pub const fn carries_trigger(&self) -> bool {
        match self {
            Self::Ragh { .. } => false,
            Self::Alastor { .. } | Self::ExoSelf | Self::ExoVictim { .. } | Self::SuffiAn { .. } | Self::SulaOpponent | Self::SulaGive { .. } => false,
        }
    }
}
