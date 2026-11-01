//! Questions of the abilities that were string-only and silently missing from the core table (lane G, `docs/G_SILENT_RESUME.md`). They are
//! variants of the reaction area (`reaction::Ask::G`, `Opt::G`, `Subtype::G`): an ability asks them from inside a reaction window, the renderer
//! is `render/silent.rs`.

use crate::generated::{Planet, System, Unit};
use crate::ids::Seat;

/// Which card's units land on a planet of the home system at the start of the owner's next turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Landing {
    /// Letani Warrior II (Arborec): `lw2_return`.
    Warrior,
    /// Crimson Legionnaire II (Mahact): `legionnaire_return`.
    Legionnaire,
}

/// Which card lets a unit roll an additional die (Strike Wing, Argent): the note or the commander.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DieSource {
    /// Strike Wing Ambuscade (`ambuscade`), a promissory note.
    Ambuscade,
    /// Trrakan Aun Zulok (`argentcommander`).
    Commander,
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
    /// A question of lane G4 (`silent4`).
    U(super::silent4::Ask),
    /// Reclaimer (the Winnu mech): place a structure on a planet the action took.
    Reclaimer,
    /// Hegemonic Trade Policy (Winnu): swap the values of which planet.
    TradePolicy,
    /// Sai Seravus (the Ghosts' commander): how many fighters go with the ships that moved through wormholes.
    CommanderFighters,
    /// Voidwatch (Empyrean): which promissory note the mover gives.
    Voidwatch,
    /// The Stillness of Stars (Cabal): which unit of the opponent's reinforcements is captured.
    Stillness,
    /// Garv and Gunn (Naaz): which of the ending player's planets is explored.
    NaazAgent,
    /// Ssruu's copy of Garv and Gunn (the planets are named by their names).
    NaazCopy,
    /// Strike Wing Ambuscade or the Argent commander: which of the rolling units rolls 1 additional die.
    ExtraDie(DieSource),
    /// Strike Wing Alpha II: which type of the opponent's infantry in the space area is destroyed.
    StrikeWing,
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
    /// An answer of lane G4 (`silent4`).
    U(super::silent4::Opt),
    /// A PDS (`pds`) or a space dock placed by the Reclaimer on `planet` of `system`.
    Reclaimer { pds: bool, system: System, planet: Planet },
    /// The planet of `system` whose resource and influence values Hegemonic Trade Policy swaps.
    Swap { system: System, planet: Planet },
    /// `count` fighters placed in the space area of `system`.
    Fighters { count: u8, system: System },
    /// A promissory note given.
    Note(crate::state::NoteKey),
    /// The seat's own Support for the Throne given (the faction it belongs to).
    Support(crate::generated::Faction),
    /// A model of `victim`'s reinforcements captured.
    Capture { unit: crate::generated::Unit, victim: Seat },
    /// A planet explored.
    Explore(Planet),
    /// The unit that rolls an additional die: the staged roll `entry` (the option id is its index), of the type `unit`.
    ExtraDie { entry: u8, unit: Unit },
    /// One infantry of `victim` of this type destroyed.
    Infantry { unit: Unit, victim: Seat },
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
    /// A subtype of lane G4 (`silent4`).
    U(super::silent4::Subtype),
    /// `reclaimer`.
    Reclaimer,
    /// `htp_planet`.
    TradePolicy,
    /// `commander_fighters`.
    CommanderFighters,
    /// `voidwatch_note`.
    Voidwatch,
    /// `stillness_target`.
    Stillness,
    /// `agent_planet`.
    NaazAgent,
    /// `borrowed_agent_planet`.
    NaazCopy,
    /// `extra_die_unit`.
    ExtraDie,
    /// `swa2_infantry`.
    StrikeWing,
}

impl Ask {
    /// Whether the question's context carries the trigger of its window (`DecisionTrigger::from_event`): the ones the string builds with the
    /// timing context's `ask_seeing` do, the ones it builds by hand do not.
    #[must_use]
    pub const fn carries_trigger(&self) -> bool {
        match self {
            Self::Landing(_) | Self::Mitosis | Self::Pillage => true,
            Self::ExtraDie(_) | Self::StrikeWing => false,
            Self::Reclaimer | Self::TradePolicy | Self::CommanderFighters | Self::Voidwatch | Self::Stillness | Self::NaazAgent | Self::NaazCopy => false,
            Self::T(t) => t.carries_trigger(),
            Self::U(u) => u.carries_trigger(),
        }
    }
}
