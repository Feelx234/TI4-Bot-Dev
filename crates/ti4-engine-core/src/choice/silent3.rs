//! Questions of the abilities lane G3 ported (`docs/G3_RESUME.md`), part 2 of the loud gap rows. They are variants of `silent` (`silent::Ask::T`, `Opt::T`,
//! `Subtype::T`), so the lanes that add to `silent` and this one never touch the same lines; the renderer is `render/silent3.rs`.

use crate::generated::{Planet, System, Unit};

/// What a G3 ability asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// Ragh's Call (Saar promissory note): the Saar player says where the Saar ground forces on `planet` go.
    Ragh { planet: Planet },
    /// The Alastor (Nekro flagship): how many of the ground forces of type `unit` on `planet` fight as ships.
    Alastor { planet: Planet, unit: Unit },
}

/// One answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// A planet in a system (the option id is `{system}|{planet}`).
    Site { system: System, planet: Planet },
    /// A count (the option id is `alastor|{n}`).
    Count(u32),
}

/// The machine identifier of the question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    /// `ragh_destination`.
    Ragh,
    /// `alastor_participants`.
    Alastor,
}

impl Ask {
    /// Whether the question's context carries the trigger of its window (see `silent::Ask::carries_trigger`): decided per question by the lockstep, the
    /// answers of a seat other than the window's holder (Ragh's Call asks the Saar) carry none.
    #[must_use]
    pub const fn carries_trigger(&self) -> bool {
        match self {
            Self::Ragh { .. } => false,
            Self::Alastor { .. } => false,
        }
    }
}
