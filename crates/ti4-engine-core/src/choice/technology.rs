//! Decision types of the `technology` area: the free technology windows of a turn (package W12): Psychoarchaeology at the
//! start of a turn, Predictive Intelligence and Bio-Stims at its end, and the component action of Production Biomes. (The research questions are the strategy area's.)
//! The adapter side is `ti4-engine/src/render/technology.rs`.

use super::agenda::Shown;
use super::strategy::Pool;
use crate::generated::{Planet, System, Technology, Unit};
use crate::ids::Seat;

/// What the area asks. Parameters only; the options are the window's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// "Psychoarchaeology: exhaust a specialty for 1 trade good".
    Psychoarchaeology,
    /// "Predictive Intelligence: redistribute command tokens".
    Restack,
    /// "Bio-Stims".
    BioStims,
    /// "AI Development Algorithm: exhaust to reduce this use's combined cost by {upgrades}".
    Aida { upgrades: u8 },
    /// "Self-Assembly Routines: exhaust to place 1 mech from your reinforcements on a planet you control in this system".
    SelfAssembly,
    /// "Transit Diodes: redeploy ground forces ({left} left)".
    TransitDiodes { left: u8 },
    /// "Production Biomes: who gains 2 trade goods".
    ProductionBiomes,
    /// "Vortex: which unit type do you take" (Cabal): the unit type of another seat captured from its reinforcements.
    Vortex,
    /// "Wormhole Generator: place or move a Creuss wormhole token" (Ghost): into which system, which token.
    WormholeGenerator,
    /// "Chaos Mapping": in which system to produce one unit (at the start of a turn).
    ChaosMapping,
    /// "choose a starting technology" (`number` of `of` when the seat picks several), drawn by `rule` (setup, before the first card is drafted).
    StartingTech { number: u8, of: u8, rule: StartRule },
    /// "Temporal Command Suite: perform a transaction with {partner}" (Nomad): `partner` owns the agent that was readied.
    TcsTransaction { partner: Seat },
    /// "Quantum Datahub Node: spend a strategy token and 3 trade goods to swap a strategy card" (Yin; at the end of the strategy phase).
    QuantumDatahub,
}

/// Where a starting technology pick draws from (`ti4_model::state::StartingTechSource`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StartRule {
    /// A list on the faction card (Argent Flight).
    Listed,
    /// The technologies other seats own, none of a faction (Council Keleres).
    OwnedByOthers,
    /// Any technology the seat could research now by its prerequisites (Winnu).
    Researchable,
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// A specialty planet to exhaust (`exhaust {planet}`) or, for Bio-Stims, to ready; `Shown::system` is its place on the map.
    Planet(Planet, Shown),
    /// Move one token from a pool to another (`{from}|{to}`).
    Move { from: Pool, to: Pool },
    /// Bio-Stims readies an exhausted technology.
    ReadyTechnology(Technology),
    /// Exhaust AI Development Algorithm for the discount.
    Exhaust,
    /// Move one ground force of the seat (`unit`, from its space area when `from` is `None`, else from that planet of
    /// `from_system`) to the planet `to` of `to_system` (Transit Diodes).
    Transit { from_system: System, from: Option<Planet>, unit: Unit, to_system: System, to: Planet },
    /// A player (Production Biomes: the one who gains 2 trade goods).
    Player(Seat),
    /// Vortex: capture this unit type of this seat.
    Capture(Seat, Unit),
    /// Wormhole Generator: this Creuss token goes into this system.
    Token(crate::generated::Wormhole, System),
    /// Chaos Mapping: produce one unit in this system.
    System(System),
    /// A starting technology.
    Tech(Technology),
    /// Temporal Command Suite: perform the transaction (`transact`).
    Transact,
    /// Quantum Datahub Node: give `partner` 3 trade goods and the card `give`, take the card `take` (the answer `qdn|{partner}|{give}|{take}`).
    Swap { partner: Seat, give: crate::generated::StrategyCard, take: crate::generated::StrategyCard },
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    PsychoarchaeologyExhaustSpecialty,
    PredictiveIntelligenceRedistribute,
    BioStimsReady,
    AidaExhaustForProductionDiscount,
    SelfAssemblyMech,
    TransitDiodesRedeploy,
    ProductionBiomesChoosePlayer,
    VortexTarget,
    /// `creuss_token`.
    CreussToken,
    /// `chaos_mapping_choose_system`.
    ChaosMappingChooseSystem,
    /// `starting_technology`.
    StartingTechnology,
    /// `tcs_transaction`.
    TcsTransaction,
    /// `quantum_datahub_swap`.
    QuantumDatahubSwap,
}
