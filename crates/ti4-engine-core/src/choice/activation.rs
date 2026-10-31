//! Questions of the tactical-island abilities that lane M6d ported (Awaken, Ouranos, the Arborec commander and flagship, ...). They are variants
//! of the reaction area (`reaction::Ask::Act`, `Opt::Act`, `Subtype::Act`): an ability asks them from inside a reaction window; the renderer is
//! `render/activation.rs`.

use crate::generated::Planet;

/// What a sleeper token becomes (Awaken).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Awakening {
    /// 1 PDS (`pds`).
    Pds,
    /// Hecatoncheires: 1 mech and 1 infantry instead of the PDS (`mech`).
    MechAndInfantry,
}

/// What an activation-window ability asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// Awaken (Titans): what the sleeper token on `planet` becomes.
    Awaken { planet: Planet },
    /// Ouranos (the Titans flagship, or the Nekro one with the Z token): which PDS the flagship replaces.
    Ouranos,
}

/// One answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// A way to awaken a sleeper token.
    Awaken(Awakening),
    /// A planet (the option id is the planet).
    Planet(Planet),
}

/// The machine identifier of the question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    /// `awaken_replace`.
    Awaken,
    /// `ouranos_pds`.
    Ouranos,
}

impl Ask {
    /// Whether the question's context carries the trigger of its window (see `silent::Ask::carries_trigger`): the ones the string builds with
    /// `DecisionContext::new` by hand do not.
    #[must_use]
    pub const fn carries_trigger(&self) -> bool {
        match self {
            Self::Awaken { .. } | Self::Ouranos => false,
        }
    }
}
