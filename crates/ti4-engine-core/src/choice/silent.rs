//! Questions of the abilities that were string-only and silently missing from the core table (lane G, `docs/G_SILENT_RESUME.md`). They are
//! variants of the reaction area (`reaction::Ask::G`, `Opt::G`, `Subtype::G`): an ability asks them from inside a reaction window, the renderer
//! is `render/silent.rs`.

use crate::generated::{Planet, System};
use crate::ids::Seat;

/// Which card's units land on a planet of the home system at the start of the owner's next turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Landing {
    /// Letani Warrior II (Arborec): `lw2_return`.
    Warrior,
    /// Crimson Legionnaire II (Mahact): `legionnaire_return`.
    Legionnaire,
}

/// What a silent-gap ability asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// Where in the home system a unit that waited on its card lands (one question per unit, as the string asks).
    Landing(Landing),
    /// Mitosis (Arborec), at the start of the status phase: place an infantry, or replace one with a mech.
    Mitosis,
    /// Pillage (Mentak): take 1 trade good or commodity from one of the neighbours the event concerns.
    Pillage,
    /// A question of lane G3 (`silent3`).
    T(super::silent3::Ask),
}

/// One answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// A planet (the option id is the planet).
    Planet(Planet),
    /// A way to resolve Mitosis: an infantry placed on `planet` of `system` (`replace == false`), or an infantry there (the space area for `None`)
    /// replaced by a mech.
    Mitosis { replace: bool, system: System, planet: Option<Planet> },
    /// A trade good (`commodity == false`) or a commodity of `target` taken by Pillage.
    Pillage { commodity: bool, target: Seat },
    /// An answer of lane G3 (`silent3`).
    T(super::silent3::Opt),
}

/// The machine identifier of the question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    /// `lw2_return` / `legionnaire_return`.
    Landing(Landing),
    /// `mitosis_place`.
    Mitosis,
    /// `pillage_take`.
    Pillage,
    /// A subtype of lane G3 (`silent3`).
    T(super::silent3::Subtype),
}

impl Ask {
    /// Whether the question's context carries the trigger of its window (`DecisionTrigger::from_event`): the ones the string builds with the
    /// timing context's `ask_seeing` do, the ones it builds by hand do not.
    #[must_use]
    pub const fn carries_trigger(&self) -> bool {
        match self {
            Self::Landing(_) | Self::Mitosis | Self::Pillage => true,
            Self::T(t) => t.carries_trigger(),
        }
    }
}
