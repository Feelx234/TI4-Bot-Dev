//! Decision types of the `relic` area: the questions a relic puts to its holder.
//!
//! Owner: package B5 (docs/ENGINE_CORE_STATE.md 4.4). Ported: the Dominus Orb's purge offer, the Crown of
//! Emphidia's choice of a planet to explore, the Neuraloop's relic to purge, the Codex's action cards, the
//! Titan Prototype's builder, the Stellar Converter's target and the technology of the Enigmatic Device and the
//! Maw of Worlds. The adapter side is `ti4-engine/src/render/relic.rs`.

use super::agenda::Shown;
use crate::generated::{ActionCard, Planet, PublicObjective, Relic, Technology};
use crate::ids::Seat;
use crate::state::Code;

/// What the area asks. Parameters only; the options are the window's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// "The Dominus Orb: purge to move out of your own command tokens".
    DominusOrb,
    /// "The Crown of Emphidia: exhaust to explore a planet".
    CrownOfEmphidia,
    /// "The Neuraloop: purge a relic to replace {revealed}".
    Neuraloop { revealed: PublicObjective },
    /// "The Codex: take which action card".
    Codex,
    /// "Titan Prototype: which player may build".
    TitanPrototype,
    /// "Stellar Converter: destroy which planet".
    StellarConverter,
    /// "gain which technology" (the source relic is the question's origin).
    GainTechnology { relic: Relic },
    /// "gain which technology" for a source that is no relic (Research Team, an agenda): the source's interned text is the
    /// question's origin (package W13).
    GainTechnologyFrom { source: Code },
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// Purge the Dominus Orb.
    Purge,
    /// A planet to explore, with the system the map shows it in (the `system` payload of the option).
    Planet(Planet, Shown),
    /// A relic of the holder to purge (Neuraloop).
    PurgeRelic(Relic),
    /// An action card of the discard pile; `Shown::votes` is the cards the seat holds now (the preview's "before").
    Take(ActionCard, Shown),
    /// A player who may build (Titan Prototype).
    Builder(Seat),
    /// A planet to destroy, with the system the map shows it in.
    Destroy(Planet, Shown),
    /// A technology to gain.
    Tech(Technology),
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    DominusOrbPurgeToMove,
    CrownOfEmphidiaChoosePlanet,
    NeuraloopChooseRelicToPurge,
    CodexTakeActionCard,
    TitanPrototypeChooseBuilder,
    StellarConverterChooseTarget,
    /// `{relic}_choose_technology`.
    ChooseTechnology(Relic),
    /// `{source}_choose_technology` of a source that is no relic.
    ChooseTechnologyFrom(Code),
}
