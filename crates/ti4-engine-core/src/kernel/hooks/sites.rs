//! The typed "sites" hooks are called with (the string engine's `CombatUnit<'_>`, `HitSite`, `MoveSite`,
//! ...), and the small result types. Copy, no text: seats, generated ids and counts.

use crate::containers::SmallList;
use crate::generated::{Faction, Planet, StrategyCard, System, TechType, Unit};
use crate::ids::Seat;
use crate::kernel::AbilityDef;
use crate::state::Piece;

/// "space" or "ground" (the string hooks pass the text).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CombatCtx {
    Space,
    Ground,
}

/// One unit about to roll combat dice (`factions::CombatUnit`). `unit` is its unit type; the pooled
/// group texts of the dice history (`carrier+fighter`) never reach a hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatUnit {
    pub owner: Seat,
    /// The system of the combat (the active system for space combat).
    pub system: Option<System>,
    /// The planet, for ground combat.
    pub planet: Option<Planet>,
    pub unit: Unit,
    pub ctx: CombatCtx,
}

impl CombatUnit {
    /// A unit for tests and the empty-table probes.
    #[must_use]
    pub const fn probe() -> Self {
        Self { owner: Seat(0), system: None, planet: None, unit: Unit::ALL[0], ctx: CombatCtx::Space }
    }
}

/// `hooks_combat::CombatMoment`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CombatMoment {
    CombatStart,
    AfterRoll,
    RoundEnded,
}

/// `hooks_combat::HitSite`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitSite {
    pub player: Seat,
    pub opponent: Seat,
    pub system: System,
    pub round: u32,
    pub moment: CombatMoment,
}

/// `hooks_combat::ProducedHits`, summed across modules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProducedHits {
    pub any_ship: usize,
    pub producer_assigned: usize,
    pub non_fighter: usize,
}

impl super::Plus for ProducedHits {
    fn plus(self, other: Self) -> Self {
        Self {
            any_ship: self.any_ship + other.any_ship,
            producer_assigned: self.producer_assigned + other.producer_assigned,
            non_fighter: self.non_fighter + other.non_fighter,
        }
    }
}

/// `hooks_combat::AfbExcess`: barrage hits that found no fighter left to destroy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AfbExcess {
    pub producer: Seat,
    pub target: Seat,
    pub system: System,
    pub excess: usize,
}

/// `hooks_combat::BorrowedStats`: the combat stats a flagship borrows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorrowedStats {
    /// The ship as it stands (identical ships are interchangeable: the first equal one in board order
    /// borrows): its type, damage and galvanize.
    pub unit: Unit,
    pub sustained: bool,
    pub galvanized: bool,
    pub hits_on: Option<i8>,
    /// Anti-fighter-barrage `(hits_on, dice)`, as the string `BorrowedStats` has it.
    pub barrage: Option<(i8, i8)>,
}

/// `hooks_ground::CommitOrigin`: where a committed unit already stands.
pub type CommitOrigin = (System, Planet);

/// `hooks_ground::CommitCandidate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CommitCandidate {
    pub system: System,
    pub planet: Planet,
    pub unit: Piece,
}

/// `production::Spend`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Spend {
    Resources,
    Influence,
}

/// `hooks_movement::MoveSite`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoveSite {
    pub player: Seat,
    pub origin: System,
    pub index: Option<u16>,
    pub ship: Unit,
}

/// `hooks_movement::PassSite`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PassSite {
    pub player: Seat,
    pub active: System,
    pub ship: Unit,
}

/// `hooks_cards::RevealKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RevealKind {
    ActionCards,
    PromissoryNotes,
    SecretObjectives,
}

/// `hooks_cards::RevealScope`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RevealScope {
    Choice,
    Action,
    Standing,
}

/// A component action a faction module offers and later claims. The string engine builds
/// `ChoiceOption`s whose id starts `faction|<alias>|`; the typed form names the module (the offering
/// faction) and a module-defined code with one argument. The adapter renders the id from a table the
/// module registers (C shards).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentAction {
    pub module: Faction,
    pub code: u16,
    pub arg: u16,
}

/// Component actions offered at one point.
pub type ComponentList = SmallList<ComponentAction, 4>;

/// Abilities a module registers at seating.
pub type AbilityList = SmallList<AbilityDef, 4>;

/// `use_leader_strategy_primary`'s result: the strategy card whose primary the leader performs and the
/// card ability code (B8 defines the codes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeaderPrimary {
    pub card: StrategyCard,
    pub ability: u16,
}

/// The follower choices of a leader's primary (`Vec<Choice>`).
pub type Followers = Vec<crate::choice::Choice>;

/// A waiver offered with a secondary (`hooks_strategy::SecondaryWaiver`, `{id, label}`): the module
/// and its code; the label is the adapter's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Waiver {
    pub module: Faction,
    pub code: u16,
}

/// `hooks_strategy::ResearchWaiver`: the waiver and its payment alternatives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchWaiver {
    pub waiver: Waiver,
    pub payments: SmallList<Waiver, 4>,
}

/// A technology prerequisite colour a faction counts extra (`extra_prerequisite_colours`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtraColour {
    pub colour: TechType,
    pub count: usize,
}

/// `hooks_cards::StagedCardEvent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StagedCardEvent {
    Discarded { player: Seat, card: crate::generated::ActionCard },
    Taken { player: Seat, from: Seat, card: crate::generated::ActionCard },
}
