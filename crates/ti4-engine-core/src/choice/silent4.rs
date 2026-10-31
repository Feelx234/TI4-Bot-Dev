//! Questions of the abilities lane G4 ported (`docs/G4_RESUME.md`), the last loud gap rows. They are variants of `silent` (`silent::Ask::U`, `Opt::U`, `Subtype::U`), so the
//! lanes that add to `silent` and this one never touch the same lines; the renderer is `render/silent4.rs`.

use crate::generated::{Planet, System, Unit};
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
}

impl Ask {
    /// Whether the question's context carries the trigger of its window (see `silent::Ask::carries_trigger`): none of these does.
    #[must_use]
    pub const fn carries_trigger(&self) -> bool {
        false
    }
}
