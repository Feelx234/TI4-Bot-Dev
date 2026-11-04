//! Decision types of the `relic` area: the questions a relic puts to its holder.
//!
//! Owner: package B5 (docs/ENGINE_CORE_STATE.md 4.4). Ported: the Dominus Orb's purge offer, the Crown of
//! Emphidia's choice of a planet to explore, the Neuraloop's relic to purge, the Codex's action cards, the
//! Titan Prototype's builder, the Stellar Converter's target and the technology of the Enigmatic Device and the
//! Maw of Worlds, and the Nano-Forge's target planet. The adapter side is `ti4-engine/src/render/relic.rs`.

use super::agenda::Shown;
use crate::generated::{ActionCard, Explore, Planet, PublicObjective, Relic, Technology};
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
    /// "research which technology with the Enigmatic Device": the exploration card in the play area is the question's origin.
    DeviceTechnology { card: Explore },
    /// Fabrication, Black Market Forgery: "purge {count} fragment(s) of which type" (the source ability or note is the question's origin).
    FragmentType { count: u8 },
    /// Fabrication: which command pool gains the token.
    TokenPool,
    /// "Nano-Forge: attach it to which planet".
    NanoForge,
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
    /// A fragment type to purge from.
    Fragment(crate::generated::ExploreType),
    /// A command pool to gain a token in.
    Pool(super::strategy::Pool),
    /// A planet to attach the Nano-Forge to, with the system the map shows it in.
    Attach(Planet, Shown),
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
    /// `enigmatic_device_choose_technology`.
    DeviceChooseTechnology,
    /// `fragment_type`.
    FragmentType,
    /// `token_pool`.
    TokenPool,
    NanoForgeChooseTarget,
}
