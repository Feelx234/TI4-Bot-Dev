//! Questions of the abilities lane G4 ported (`docs/G4_RESUME.md`), the last loud gap rows. They are variants of `silent` (`silent::Ask::U`, `Opt::U`, `Subtype::U`), so the
//! lanes that add to `silent` and this one never touch the same lines; the renderer is `render/silent4.rs`.

use crate::generated::{Planet, System, Unit, UnitBaseType};
use crate::ids::Seat;

/// What a G4 ability asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// Stymie (Arborec promissory note): the holder says in which system `mover`'s command token goes.
    Stymie { mover: Seat },
    /// Magen Defense Grid: the owner assigns the hit to one of the opponent's ground forces.
    MagenHit,
    /// The owner of `unit` on `planet` of `system` is offered SUSTAIN DAMAGE for a hit an effect assigned to it (`staging`: the game has a faction module seated, which
    /// words the question with the planet).
    GroundSustain { unit: Unit, system: System, planet: Planet, staging: bool },
    /// Foresight (Naalu): the Naalu says which adjacent system its ships leave the active system `from` for.
    Foresight { from: System },
    /// Place up to `fit` units of the base type from the reinforcements (`action_cards::place_units_choosing`): the largest number any spot takes.
    Place { base: UnitBaseType, fit: u8 },
}

/// One answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// A system (the option id is the system's text).
    System(System),
    /// A ground force the hit can go to (`index` in the list the question was built from): its owner, kind, whether it is damaged and what it costs in halves.
    Target { index: u8, owner: Seat, unit: Unit, damaged: bool, sturdy: bool, cost_halves: u8 },
    /// Use SUSTAIN DAMAGE (the answer `sustain`).
    Sustain,
    /// A system the Naalu ships go to (the option id is `system|{system}`).
    Destination(System),
    /// A spot for the placement: a planet (`planet`) or the space area of the system, and how many units go there (the option id is `{system}|{planet}` or `{system}|space`).
    Spot { system: System, planet: Option<Planet>, fit: u8 },
}

/// The machine identifier of the question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    /// `stymie_system`.
    Stymie,
    /// `magen_defense_hit`.
    MagenHit,
    /// `ground_effect_sustain`.
    GroundSustain,
    /// `destination`.
    Foresight,
    /// `place_units_from_reinforcements`.
    Place,
}

impl Ask {
    /// Whether the question's context carries the trigger of its window (see `silent::Ask::carries_trigger`): none of these does.
    #[must_use]
    pub const fn carries_trigger(&self) -> bool {
        false
    }
}
