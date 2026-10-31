//! Questions of the abilities lane G3 ported (`docs/G3_RESUME.md`), part 2 of the loud gap rows. They are variants of `silent` (`silent::Ask::T`, `Opt::T`,
//! `Subtype::T`), so the lanes that add to `silent` and this one never touch the same lines; the renderer is `render/silent3.rs`.

use crate::generated::{Planet, System};

/// What a G3 ability asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// Ragh's Call (Saar promissory note): the Saar player says where the Saar ground forces on `planet` go.
    Ragh { planet: Planet },
}

/// One answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// A planet in a system (the option id is `{system}|{planet}`).
    Site { system: System, planet: Planet },
}

/// The machine identifier of the question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    /// `ragh_destination`.
    Ragh,
}

impl Ask {
    /// Whether the question's context carries the trigger of its window (see `silent::Ask::carries_trigger`): decided per question by the lockstep, the
    /// answers of a seat other than the window's holder (Ragh's Call asks the Saar) carry none.
    #[must_use]
    pub const fn carries_trigger(&self) -> bool {
        match self {
            Self::Ragh { .. } => false,
        }
    }
}
