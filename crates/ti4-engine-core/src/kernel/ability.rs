//! The ability table (package `core-reactions`; docs/ENGINE_REACTIONS.md): the rules behind the abilities a
//! reaction window offers.
//!
//! A timing window is the resolver's bucket `(EventKind, Relation)`. What the resolver stores for an ability is
//! its metadata ([`AbilityDef`]); what the ability *does* is a [`Spec`] in the [`AbilityTable`], found by the
//! [`Slot`] carried in `AbilityDef::arg`. A spec is both halves: [`Spec::def`] is the registry row of one owner
//! and `condition`/`effect` are the rule, so there is one place per ability (S8's `AbilityDef` + `EffectId`).
//!
//! [`AbilityTable::registry`] builds the registry of a game in the string order of `reactions::arm` (per seat,
//! the specs in table order), so the offer order of every window equals the string's. The table need not be
//! complete for a window to be followed: an ability whose condition does not hold changes nothing, so it may be
//! missing; one that is eligible must be in the table ([`AbilityTable::knows`]).

use super::stage::Sx;
use super::resolver::{AbilityDef, CannotDef, Host, Resolver, TimingError};
use super::{Asker, Kernel};
use crate::choice::reaction::{AbilityId, AbilityKind, Frequency, Relation};
use crate::choice::{Choice, Opt, Pick};
use crate::generated::Faction;
use crate::ids::Seat;
use crate::state::{CoreState, EventKind, EventRec, RngDomain};

/// The identity of an ability *rule*, independent of its owner (`AbilityDef::arg`). Each crate that ports
/// abilities owns a range: the kernel `0..1000`, the faction packages `1000..` in steps of 1000.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Slot(pub u16);

impl Slot {
    /// `AbilityDef::arg` of an ability the table does not know (the adapter's mapping found no slot).
    pub const UNPORTED: u32 = u32::MAX;

    #[must_use]
    pub const fn arg(self) -> u32 {
        self.0 as u32
    }
}

/// Who gets the registry row: the string registers some abilities for every seat (the condition limits them to
/// the holder, because a technology can be gained mid-game and the resolver has no late registration) and the
/// faction ones for the seats of that faction.
#[derive(Clone, Copy)]
pub enum Applies {
    EverySeat,
    Faction(Faction),
    Held(fn(&CoreState, Seat) -> bool),
}

impl Applies {
    #[must_use]
    pub fn holds(self, s: &CoreState, who: Seat) -> bool {
        match self {
            Self::EverySeat => true,
            Self::Faction(f) => s.players().by_seat(who).is_some_and(|p| p.faction == f),
            Self::Held(f) => f(s, who),
        }
    }
}

/// Why a condition cannot be answered by the core: a documented bail of the rule it reads (the adapter words it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unknown {
    /// A printed card cost the core does not judge.
    CardCost,
    /// An action card played into a window at once (the Sabotage window, or a rules version before 3): the string path.
    ImmediateCardPlay,
    /// A played card whose effect is not one of the stamps the core plays, or one that comes from the salvage yard.
    UnportedCard,
    /// A fact the adapter should have computed for the window is missing (`Facts`).
    MissingFact,
    /// A card was played while destructions were staged and the table has no announcement for them (`AbilityTable::after_play` unset:
    /// the removals are not announced).
    PendingDestructions,
    /// A card was discarded while the Data Skimmer could take it (a breakthrough's pile).
    DataSkimmer,
    /// The play of a card needs what the adapter did not compute (`AdapterFacts::plays`).
    PlayFacts,
    /// A position the native tactical action does not run yet (a documented bail of `driver::windows::tactical`, named by its gap).
    UnportedTactical(TacticalGap),
    /// A position the native aftermath chain or its space combat does not run (a documented bail of `driver::windows::{chain, combat}`).
    UnportedAftermath(AftermathGap),
    /// A position the native invasion does not run yet (a documented bail of `driver::windows::invasion`, named by its reason).
    Unported(Unported),
    /// A position the native production step does not run (a documented bail of `driver::windows::production`, named by its reason).
    UnportedProduction(ProductionGap),
    /// The live turn flow cannot follow the string here yet (lane B, `driver::turn`).
    TurnGap(super::turn_codes::TurnGap),
    /// The live agenda phase cannot follow the string here yet (lane D, `driver::agenda`).
    AgendaGap(super::agenda_codes::AgendaGap),
    /// An ability the string registers and the core table has no rule for, held by a seat at the moment its event is announced (lane G,
    /// `ti4_engine_core_factions::silent::gaps`): the slot of the listed gap.
    AbilityGap(Slot),
}

/// The positions the native production step (`driver::windows::production`) does not run, each a counted bail of the live path. The words are
/// the adapter's (`production_live::gap_text`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionGap {
    /// The table has no faction unit roster (`AdapterFacts::roster`): the buildable units cannot be named.
    NoRoster,
    /// The facts of the use cannot be read from the position (a seat that does not exist, a number beyond 32 bits).
    NoFacts,
    /// The table has no link to Self-Assembly Routines' window (`Spawn::SelfAssembly`).
    NoLink,
    /// A window a start or end effect runs asked the driver for something that is not a question (`Next::Open` / `Emit` / `Chain`).
    ChildPassed,
    /// A window a start or end effect runs asks something its own answer does not settle.
    ChildStuck,
    /// A stage that neither asks nor moves on.
    NoProgress,
}

/// The positions the native aftermath chain and its space combat do not run, each a counted bail of the live path. The words are the adapter's
/// (`combat_live::gap_text`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AftermathGap {
    /// A stage that neither asks nor moves on.
    NoProgress,
    /// A contact pause (`Aftermath::Negotiating`): structured diplomacy is dropped.
    Negotiating,
    /// A space combat whose environment cannot be built from the position (no galaxy, an unseated side).
    NoCombatEnv,
    /// The machine reached a position it does not carry (see `Machine::bailed`).
    CombatUnported,
    /// A roll that ended the fight inside the roll step and left the machine in a position it does not carry.
    RollEndsFight,
    /// A neutral side (Thunder's Edge neutral units).
    NeutralSide,
    /// A scoring window that asks for something the core does not offer (a plot, a priced objective).
    EventScoring,
}

/// The positions the native tactical action (`driver::windows::tactical`) does not run, each a counted bail of the live path. The words are
/// the adapter's (`tactical_live::gap_text`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TacticalGap {
    /// A stage that neither asks nor moves on.
    NoProgress,
    /// A hook of the activation that asks, pays or ends the turn is in the position (Spatial Conduit Cylinders, Scanlink Drone Network,
    /// E-Res Siphons, Nullification Field, Mahact, Minister of Peace, the Dominus Orb, Ceasefire).
    ActivationHook,
    /// Nova Seed replaced a tile during `MOVEMENT_FINISHED`: the derived map is replayed by the per-step hooks, not here.
    NovaSeed,
    /// A hit of the space cannon offense needs a question (sustain damage, a casualty to choose).
    CannonHits,
    /// The Crown of Emphidia or Dark Energy Tap explores at the end of the action: no native planet exploration window.
    ExplorationAtTheEnd,
    /// The map slice holds no galaxy.
    NoGalaxy,
}

/// The positions the native invasion (`driver::windows::invasion`) does not run, each a counted bail of the live path. The words are the adapter's
/// (`invasion_live::unported_text`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unported {
    /// The machine made no progress: a stage that neither asks nor moves on.
    NoProgress,
    Slumberstate,
    RollCannotName,
    IntegratedEconomy,
    AssimilateRoster,
    /// The exploration of a first claim holds something the native window does not carry (`ExploreRun::why`).
    Exploration(crate::state::EWhy),
    BombardmentDiceMismatch,
    X89,
}

/// What a condition reads: the state, the kernel, the ability and the event it is asked about.
pub struct CondCx<'a> {
    pub state: &'a CoreState,
    pub kernel: &'a Kernel,
    pub def: &'a AbilityDef,
    pub event: &'a EventRec,
    pub resolver: &'a Resolver,
    /// Political Secret bars this seat's faction abilities and action cards (`AbilityTable::bars`).
    pub barred: bool,
    /// The texts the payloads carry as codes (`AbilityTable::codes`).
    pub codes: Codes,
    /// What the adapter computed for the window (`AbilityTable::facts`).
    pub facts: &'a AdapterFacts,
    unknown: &'a std::cell::Cell<Option<Unknown>>,
}

impl CondCx<'_> {
    /// The condition cannot be answered by the core (a documented bail of the rule it reads): the answer given
    /// stands for nothing, and the driver reports `why` (the dual falls back to the string run).
    pub fn unknown(&self, why: Unknown) -> bool {
        if self.unknown.get().is_none() {
            self.unknown.set(Some(why));
        }
        false
    }
}

/// The few texts an ability compares an event payload against (a payload holds `Code`s, interned by the
/// adapter; the core has no text). Set once by the adapter when it builds the resolver.
#[derive(Debug, Clone, Copy, Default)]
pub struct Codes {
    /// The cause `ground_combat` of a destroyed ground force.
    pub ground_combat: crate::state::Code,
    /// The cause `harrow` of a destroyed ground force.
    pub harrow: crate::state::Code,
    /// The `source` of the trade good Self-Assembly Routines gains.
    pub self_assembly: crate::state::Code,
    /// `unit_ability:yin_flagship`: the cause of the ships Van Hauge destroys.
    pub van_hauge: crate::state::Code,
    /// The `source` `action_card` and `forward_supply_base` of the trade goods a card gains (`Env`).
    pub action_card: crate::state::Code,
    pub supply_base: crate::state::Code,
    /// The `source` `mentak` of the trade good Salvage Operations gains.
    pub mentak: crate::state::Code,
    /// The `source` `future_sight` of the trade good the Nomad's Future Sight gains.
    pub future_sight: crate::state::Code,
    /// The `reason` of the strategy token Instinct Training spends.
    pub instinct_training: crate::state::Code,
    /// The causes `action_card:direct_hit` and `action_card:courageous` of a destroyed ship.
    pub direct_hit: crate::state::Code,
    /// The cause `retreat_capacity` of a ship a retreat strands, and the pool `reinforcements` of its command token.
    /// The unit aliases `mahact_infantry` and `mahact_infantry2`: the source of a Crimson Legionnaire's trade good and the
    /// name of its question's source.
    pub mahact_infantry: crate::state::Code,
    pub mahact_infantry2: crate::state::Code,
    /// The alias `cavalry`, the source of the question of The Cavalry.
    pub cavalry: crate::state::Code,
    /// The name `technological_singularity`, the source of the question of the Nekro's ability.
    pub technological_singularity: crate::state::Code,
    /// The name `galactic_threat`, the source of the question of the Nekro's ability.
    pub galactic_threat: crate::state::Code,
    /// The unit alias `empyrean_mech`, the source of the Watcher's question.
    pub empyrean_mech: crate::state::Code,
    /// The technology alias `so`, the source of Salvage Operations' question.
    pub salvage: crate::state::Code,
    /// The faction ability `indoctrination`, the source of the Yin's question.
    pub indoctrination: crate::state::Code,
    /// The faction ability `terragenesis`, the source of the Titans' question.
    pub terragenesis: crate::state::Code,
    pub retreat_capacity: crate::state::Code,
    pub reinforcements: crate::state::Code,
    /// The subtype `assign_casualty` of a casualty question and the cause `action_card:reflective_shielding` of a loss to the hits a
    /// Reflective Shielding stages.
    pub assign_casualty: crate::state::Code,
    pub reflective_cause: crate::state::Code,
    /// The `source` texts of a gain of trade goods that is not from the supply: `transaction`, `pillage`, `extreme_duress`,
    /// `trade_agreement`, `artuno` (Artuno the Betrayer does not copy them).
    pub not_from_supply: [crate::state::Code; 5],
    /// The texts of the tactical composer (`driver::windows::tactical`).
    pub tactical: TacticalCodes,
    /// The texts of the invasion composer (`driver::windows::invasion`).
    pub invasion: InvasionCodes,
    /// The texts of the questions of the activation-window abilities (lane M6c).
    pub activation: ActivationCodes,
    /// The texts of the events of an exploration (`rules::explore_windows::Exploring`).
    pub explore: ExploreCodes,
    /// The texts of the events of a use of PRODUCTION (`driver::windows::production`).
    pub production: ProductionCodes,
    /// The texts of the turn flow (lane B).
    pub turn: super::turn_codes::TurnCodes,
    /// The texts of the agenda phase (lane D).
    pub agenda: super::agenda_codes::AgendaCodes,
    /// The texts of the formerly silent abilities (lane G).
    pub silent: SilentCodes,
    /// The texts of the leader and unit abilities lane G2 ported (`silent::{winnu, ...}`).
    pub g2: G2Codes,
}

/// The texts the questions of lane G2's abilities name as their source (`Content::Other`) or compare an event against, interned by the adapter like the rest of [`Codes`].
#[derive(Debug, Clone, Copy, Default)]
pub struct G2Codes {
    /// The unit `winnu_mech` (Reclaimer).
    pub winnu_mech: crate::state::Code,
    /// The `source` `muaat` of the trade good Magmus gains.
    pub muaat: crate::state::Code,
    /// The leader `cabalagent`, the `source` of the trade goods The Stillness of Stars gains.
    pub cabal_agent: crate::state::Code,
}

/// The texts of the questions and gains of the formerly silent abilities (lane G), interned by the adapter like the rest of [`Codes`].
#[derive(Debug, Clone, Copy, Default)]
pub struct SilentCodes {
    /// The unit alias `lw2`: the source of the Letani Warrior II's landing question.
    pub lw2: crate::state::Code,
    /// The `source` `scavenge` of the trade good Scavenge gains.
    pub scavenge: crate::state::Code,
    /// The faction ability `mitosis`, the source of Arborec's question.
    pub mitosis: crate::state::Code,
    /// The leader `naazagent`, the source of Garv and Gunn's question.
    pub naaz_agent: crate::state::Code,
    /// The texts lane G3's rules name (`reactions_core::G3_CODE_TEXTS`, the places of `ti4_engine_core_factions::silent::g3::code`): the aliases of the questions' sources.
    pub g3: [crate::state::Code; 16],
    /// The texts lane G4's rules name (`reactions_core::G4_CODE_TEXTS`, the places of `ti4_engine_core_factions::silent::g4::code`).
    pub g4: [crate::state::Code; 16],
}

/// The `source` texts of `UNITS_PRODUCED`: `ability` for the units an ability produces, `production` for the PRODUCTION of a unit (the adapter's
/// event texts).
#[derive(Debug, Clone, Copy, Default)]
pub struct ProductionCodes {
    pub ability: crate::state::Code,
    pub production: crate::state::Code,
    /// The payload key `units` (the adapter's event text: `Key::Other`).
    pub units: crate::state::Code,
}

/// The texts the events of an exploration carry: the source `exploration` of the trade goods it gains, the deck and the card of
/// `PLANET_EXPLORED` and the `trade_agreement` source of a replenishment (the adapter's event texts, not the state's). Not filled
/// (`filled` false): the exploration declines its run.
#[derive(Debug, Clone, Copy)]
pub struct ExploreCodes {
    pub filled: bool,
    pub goods: crate::state::Code,
    pub trade_agreement: crate::state::Code,
    /// The text of each deck, by `ExploreType` index.
    pub decks: [crate::state::Code; crate::generated::ExploreType::COUNT],
    /// The text of each card, by `Explore` index.
    pub cards: [crate::state::Code; crate::generated::Explore::COUNT],
    /// The printed name `Freelancers`, the source of the unit its cards produce.
    pub freelancers: crate::state::Code,
}

impl Default for ExploreCodes {
    fn default() -> Self {
        let z = crate::state::Code(0);
        Self { filled: false, goods: z, trade_agreement: z, decks: [z; crate::generated::ExploreType::COUNT], cards: [z; crate::generated::Explore::COUNT], freelancers: z }
    }
}

/// The texts of the questions of the activation-window abilities (lane M6c), interned by the adapter like the rest of [`Codes`].
#[derive(Debug, Clone, Copy, Default)]
pub struct ActivationCodes {
    /// The unit alias `ghost_mech`, the source of Icarus Drive's question.
    pub ghost_mech: crate::state::Code,
    /// The leader alias `saaragent`, the source of Captain Mendosa's question.
    pub saar_agent: crate::state::Code,
    /// The leader alias `sardakkagent`, the source of T'ro's placement.
    pub sardakk_agent: crate::state::Code,
    /// The faction ability `edict`, the source of the Mahact's question.
    pub edict: crate::state::Code,
    /// The faction ability `awaken`, the source of the Titans' question (lane M6d).
    pub awaken: crate::state::Code,
    /// The unit alias `titans_flagship`, the source of Ouranos' question.
    pub titans_flagship: crate::state::Code,
    /// The leader alias `yssarilcommander`, the source of So Ata's question.
    pub yssaril_commander: crate::state::Code,
    /// The leader alias `nomadagentmercer`, the source of Mercer's questions.
    pub mercer: crate::state::Code,
    /// The cause `breakthrough:nova_seed` of the ships Nova Seed destroys.
    pub nova_seed_cause: crate::state::Code,
}

/// The texts the tactical composer puts into the position and its questions' facts: the pool of the activation token, the roll kind
/// and loss causes of the space cannon offense, the casualty subtype and the Letnev commander's source, the presentation phase of a
/// battle about to be rolled. Interned by the adapter like the rest of [`Codes`].
#[derive(Debug, Clone, Copy, Default)]
pub struct TacticalCodes {
    /// `tactic`.
    pub tactic: crate::state::Code,
    /// `space_cannon`, `unit_ability:space_cannon`, `assign_casualty`, `letnevcommander`, `pre_roll`.
    pub space_cannon: crate::state::Code,
    pub space_cannon_cause: crate::state::Code,
    pub casualty: crate::state::Code,
    pub commander: crate::state::Code,
    pub pre_roll: crate::state::Code,
    /// `faction_technology`: the source of the trade goods E-Res Siphons gains.
    pub faction_technology: crate::state::Code,
    /// `nullification_field`: the reason of the strategy token the Nullification Field spends.
    pub nullification_field: crate::state::Code,
    /// `mahact_mech`: the source of the Starlancer's question.
    pub starlancer: crate::state::Code,
    /// The prompt of the Minister of Peace's offer to discard its law, and the law's alias (`minister_peace`).
    pub peace_prompt: crate::state::Code,
    pub peace_source: crate::state::Code,
}

/// The texts the invasion composer puts into the position: the step kinds and causes of its records, the dice groups and the text of every
/// unit type (a roll names the unit types behind it). Interned by the adapter like the rest of [`Codes`].
#[derive(Debug, Clone, Copy)]
pub struct InvasionCodes {
    /// The reroll kind of the ground dice (`ground`) and of the bombardment's (`bombardment`), and the step kinds `ground_round`,
    /// `bombardment`, `space_cannon_defense`.
    pub ground: crate::state::Code,
    pub bombardment: crate::state::Code,
    pub ground_round: crate::state::Code,
    pub space_cannon_defense: crate::state::Code,
    /// The causes `bombardment` and `space_cannon_defense` of the events of the hits (the adapter's event texts, not the state's).
    pub bombardment_cause: crate::state::Code,
    pub space_cannon_cause: crate::state::Code,
    /// The payload key `planets` of `GROUND_COMMITMENT_FINISHED`.
    pub planets: crate::state::Code,
    /// The source `dunlain_reaper` of the Reaper's question.
    pub dunlain_reaper: crate::state::Code,
    /// The source `minister_exploration` of the trade good the Minister of Exploration pays for a planet taken (an event text).
    pub minister: crate::state::Code,
    /// The phases of the public boundary `ActiveInvasion.phase`: bombardment, custodians, landing, ground_battle, planet_result,
    /// next_defender, control, complete.
    pub phases: [crate::state::Code; 8],
    /// `combat value N` for N in 1..=12 (index N - 1).
    pub names: [crate::state::Code; 12],
    /// The text of every unit type, by `Unit` index.
    pub units: [crate::state::Code; crate::generated::Unit::COUNT],
    /// The same texts in the question renderer's table (the label of the Jol-Nar commander's question about a die of the bombardment); all
    /// zero where the host has no renderer: a question is then labelled by the staged entry's own code.
    pub shown_units: [crate::state::Code; crate::generated::Unit::COUNT],
}

impl Default for InvasionCodes {
    fn default() -> Self {
        let z = crate::state::Code(0);
        Self { ground: z, bombardment: z, ground_round: z, space_cannon_defense: z, bombardment_cause: z, space_cannon_cause: z, planets: z, dunlain_reaper: z, minister: z, phases: [z; 8], names: [z; 12], units: [z; crate::generated::Unit::COUNT], shown_units: [z; crate::generated::Unit::COUNT] }
    }
}

impl InvasionCodes {
    /// The text of `unit`.
    #[must_use]
    pub fn unit(&self, unit: crate::generated::Unit) -> crate::state::Code {
        self.units[unit as usize]
    }
}

/// What the effects of the action cards read from outside the core, per seat (the faction and source answers of
/// the string side): the placed unit forms, the Mahact tokens held.
#[derive(Debug, Clone, Copy)]
pub struct SeatPlay {
    pub who: Seat,
    /// `GameState::tokens_held_by_mahact`.
    pub mahact_held: i32,
    /// The infantry the seat places (`None`: nothing) and the infantry on the Crimson card.
    pub infantry: (Option<crate::generated::Unit>, i64),
    /// The cruiser the seat places and the cruisers on the Crimson card.
    pub cruiser: (Option<crate::generated::Unit>, i64),
    /// The fighters on the Crimson card.
    pub fighter_on_card: i64,
    /// Reveal Prototype (package W4): `Some` when the core plays it for this seat, the unit upgrades of the line of a unit of the seat in the
    /// active system; `None` leaves the card to the string (a seat whose research the core window does not carry, or none open).
    pub reveal: Option<crate::state::TechSet>,
    /// Reveal Prototype: the seat can spend the four resources and a unit upgrade is open to it (`action_cards::cost_is_payable`, which the core
    /// does not judge: the adapter vouches for it).
    pub reveal_payable: bool,
    /// The other facts a card played from inside a window reads of this seat (the forms it places, the Keleres agent, Deepwrought).
    pub more: SeatPlayMore,
}

/// The unit a seat places (`None`: nothing) and the ships of that type on the Crimson card (`rules::action_cards::Placed`).
pub type PlacedFacts = (Option<crate::generated::Unit>, i64);

/// What a card played from inside a window reads of the seat that plays it, besides the infantry and the cruiser (the adapter's
/// answers, handed in by the host window like the rest of [`PlayFacts`]; `rules::action_cards::Facts`).
#[derive(Debug, Clone, Copy, Default)]
pub struct SeatPlayMore {
    pub destroyer: PlacedFacts,
    pub dreadnought: PlacedFacts,
    pub mech: PlacedFacts,
    pub pds: PlacedFacts,
    pub spacedock: PlacedFacts,
    /// A readied Keleres agent could be offered before a payment of the seat.
    pub agent_offered: bool,
    /// The seat is a Deepwrought seat (dropped by decision).
    pub holder_deepwrought: bool,
}

/// What a card played from inside a window reads of the table, besides what [`PlayFacts`] says (handed in by the host window).
#[derive(Debug, Clone, Copy)]
pub struct SharedPlay {
    /// The seat of the neutral owner, when the table has one.
    pub neutral: Option<Seat>,
    /// A replenishment at this table causes events the core does not hold.
    pub replenish_events: bool,
    /// The Fracture is in play.
    pub fracture_in_play: bool,
    /// The breakthrough of each seat's faction.
    pub own_breakthrough: [Option<crate::generated::Breakthrough>; 16],
    /// The order of unit types by their id text.
    pub unit_order: fn(crate::generated::Unit, crate::generated::Unit) -> core::cmp::Ordering,
    /// Courageous To The End: the interned subtype of its casualty question and the cause of the destructions it stages.
    pub courageous_subtype: crate::state::Code,
    pub courageous_cause: crate::state::Code,
    /// The texts and the flag of the hit flows a card runs as children of the combat area.
    pub hits: crate::kernel::children::HitTexts,
}

impl Default for SharedPlay {
    fn default() -> Self {
        Self {
            neutral: None,
            replenish_events: false,
            fracture_in_play: false,
            own_breakthrough: [None; 16],
            unit_order: |a, b| a.cmp(&b),
            courageous_subtype: crate::state::Code(0),
            courageous_cause: crate::state::Code(0),
            hits: crate::kernel::children::HitTexts::default(),
        }
    }
}

/// The play of an action card from inside a window (the reaction slot, the cards declared in a window): the
/// facts the string side answers.
#[derive(Debug, Clone)]
pub struct PlayFacts {
    /// The sources in play (`tables::Scope` bits).
    pub scope: u32,
    /// A law blanks the law-reading answers.
    pub blanked: bool,
    /// The interned `action_card` and `forward_supply_base`.
    pub action_card: crate::state::Code,
    pub supply_base: crate::state::Code,
    /// The order of the promissory notes of a hand.
    pub note_order: fn(crate::state::NoteKey, crate::state::NoteKey) -> core::cmp::Ordering,
    pub seats: Vec<SeatPlay>,
    /// The printed name of each card as the index of the first card with the same name (the slot offers one option per name).
    pub name_group: Vec<u8>,
    /// The cards the string side has an effect for (`action_cards::effect_for`); a card without one is announced unresolved.
    pub has_effect: Vec<bool>,
    /// A Data Skimmer could take a discard.
    pub skimmer: bool,
    /// The faction units of the scope (a unit upgrade changes the units on the board), for the cards that research.
    pub roster: Option<&'static crate::query::tech::UnitRoster>,
    /// `GameState::rules_version`.
    pub rules_version: u32,
    /// The order of technologies by their id text.
    pub tech_order: fn(crate::generated::Technology, crate::generated::Technology) -> core::cmp::Ordering,
    /// What the cards that read the table read (see [`SharedPlay`]).
    pub shared: SharedPlay,
}

/// Computes the [`PlayFacts`] of a position: a function of the core position and the kernel alone (the live loop has no string state).
/// The text tables it needs (interned codes, note order) are captured by the host that builds it.
#[derive(Clone)]
pub struct PlaysProvider(pub std::sync::Arc<PlaysFn>);

/// What a [`PlaysProvider`] calls.
pub type PlaysFn = dyn Fn(&crate::state::CoreState, &crate::kernel::Kernel) -> Option<PlayFacts> + Send + Sync;

impl core::fmt::Debug for PlaysProvider {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("PlaysProvider")
    }
}

/// What a [`PlaysProvider`] reads besides the position it is given: the kernel (a `'static` one, the game's) and the unit roster of its scope.
/// With it the facts of a call are computed when first read, from a snapshot of the position the call started from.
#[derive(Clone, Copy)]
pub struct PlaysSource {
    pub kernel: &'static Kernel,
    /// What the provider answers for `PlayFacts::roster` (`None`: it answers `None` altogether).
    pub roster: Option<&'static crate::query::tech::UnitRoster>,
}

impl core::fmt::Debug for PlaysSource {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("PlaysSource")
    }
}

/// The facts of one call, computed on the first read: the position the call started from (a clone: persistent, three allocations), the provider and
/// its kernel. The value is a function of those alone, so reading it later in the call gives what computing it at the start gave.
struct LazyPlays {
    at: CoreState,
    provider: PlaysProvider,
    kernel: &'static Kernel,
    value: std::sync::OnceLock<Option<PlayFacts>>,
}

/// [`AdapterFacts::plays`]: an `Option<PlayFacts>` that may be computed on first use. `as_ref` / `as_mut` / `is_some` / `is_none` read like the
/// `Option`'s; a clone of the table shares the one computation.
#[derive(Clone, Default)]
pub struct Plays {
    eager: Option<PlayFacts>,
    lazy: Option<std::sync::Arc<LazyPlays>>,
}

impl Plays {
    /// The facts of a given value (a test's, or a provider that was called at once).
    #[must_use]
    pub fn of(value: Option<PlayFacts>) -> Self {
        Self { eager: value, lazy: None }
    }

    #[must_use]
    pub fn as_ref(&self) -> Option<&PlayFacts> {
        match &self.lazy {
            Some(lazy) => lazy.value.get_or_init(|| (lazy.provider.0)(&lazy.at, lazy.kernel)).as_ref(),
            None => self.eager.as_ref(),
        }
    }

    /// Mutable access (tests): a deferred value is computed first and kept as the table's own.
    pub fn as_mut(&mut self) -> Option<&mut PlayFacts> {
        if self.lazy.is_some() {
            self.eager = self.as_ref().cloned();
            self.lazy = None;
        }
        self.eager.as_mut()
    }

    #[must_use]
    pub fn is_some(&self) -> bool {
        self.as_ref().is_some()
    }

    #[must_use]
    pub fn is_none(&self) -> bool {
        self.as_ref().is_none()
    }

    /// Whether the value is still to be computed (nobody read it).
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.lazy.as_ref().is_some_and(|lazy| lazy.value.get().is_none())
    }
}

impl core::fmt::Debug for Plays {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match &self.lazy {
            Some(lazy) => match lazy.value.get() {
                Some(value) => value.fmt(f),
                None => f.write_str("<deferred>"),
            },
            None => self.eager.fmt(f),
        }
    }
}

impl From<Option<PlayFacts>> for Plays {
    fn from(value: Option<PlayFacts>) -> Self {
        Self::of(value)
    }
}

/// Facts the adapter computes for the window and the core cannot (no galaxy is lent to the windows that follow
/// abilities): the systems adjacent to a system as one seat sees them (`PlayerAdjacency::neighbours`).
#[derive(Debug, Clone, Default)]
pub struct AdapterFacts {
    reach: Vec<(Seat, crate::generated::System, Vec<crate::generated::System>)>,
    /// The play of action cards (B6). Readers use `plays.as_ref()` as with an `Option`; the live loop's value is computed from its position on the
    /// first read (see [`Plays`]).
    pub plays: Plays,
    /// The live loop's source of [`Self::plays`]: facts that change with the position (what a seat places, the research a card opens,
    /// the Mahact pools) are computed from the core position every time a live driver is built (`driver::live`), not once per game.
    pub plays_provider: Option<PlaysProvider>,
    /// What the provider reads besides the position, when it reads nothing that outlives its game: lets [`AdapterFacts::arm_plays`] defer the
    /// computation to the first read of [`Self::plays`]. `None`: the provider is called at once, as before.
    pub plays_source: Option<PlaysSource>,
    /// `supply::staging_enabled` of the position (a registered faction module is seated): losses are announced and a Direct
    /// Hit removes the exact sustained ship.
    pub staging: bool,
    withdraw: Vec<(Seat, crate::generated::System, u32, Vec<crate::generated::System>)>,
    /// The kernel whose economy hooks a payment inside an ability reads (the production kernel; the combat kernel holds none of
    /// the faction hooks of a payment). Set by the adapter.
    pub pay_kernel: Option<fn() -> &'static Kernel>,
    /// The production kernel of a source scope (the faction hooks of a production with the units of the sources in play), for a use of PRODUCTION
    /// an ability runs (lane M6d). Set by the adapter; [`Self::pay_kernel`]'s scope is the full one.
    pub production_kernel: Option<fn(super::Sources) -> &'static Kernel>,
    /// The faction units of the scope (the form Assimilate places; set by the live `Env`).
    pub roster: Option<&'static crate::query::tech::UnitRoster>,
    adjacent: Vec<(crate::generated::System, crate::generated::System, bool)>,
    neighbours: Vec<(Seat, Seat, bool)>,
    buildable: Vec<(Seat, Vec<crate::generated::Unit>)>,
    /// The texts of the staged rerolls (Fire Team, Scramble Frequency), set by the adapter of a window that stages dice.
    pub reroll: Option<RerollFacts>,
    /// The printed name of each action card as the index of the first card with the same name, for the windows that offer one
    /// option per name outside a card play (the hand limit, package W6). Empty: the adapter did not say, the window declines.
    pub card_groups: Vec<u8>,
    /// The words of the log row an exploration closes (`exploration::row_words`), set by the adapter. `None`: the exploration declines.
    pub explore_words: Option<crate::kernel::fx_explore::WordsFn>,
    /// The texts of the space combat the live loop runs (`driver::windows::combat`): unit and roll-group names, causes and phases. The
    /// core holds no text; the adapter interns them for the unit types in the fight at every call (`ti4-engine/src/combat_live.rs`).
    pub combat: CombatTexts,
}

/// The texts the live space combat needs from outside: the string's `settle_run` interned them per settle, the live adapter does per call.
/// `state` codes are texts of the position (`CoreState::texts`: unit names in a roll's record, the presentation phases), `shown` codes the
/// renderer's (labels, the payloads of events).
#[derive(Debug, Clone, Default)]
pub struct CombatTexts {
    /// The presentation phases `pre_roll`, `barrage`, `resolving_hits`, `retreating`, `complete` (state).
    pub phases: [crate::state::Code; 5],
    /// The state text of every unit type that can be named in the fight (the types present and the other form of a dual-form unit).
    pub unit_codes: Vec<(crate::generated::Unit, crate::state::Code)>,
    /// The renderer's text of the same types.
    pub shown_codes: Vec<(crate::generated::Unit, crate::state::Code)>,
    /// Every roll group: the sorted unit types, the state text (the ids joined with `+`) and the shown text.
    pub groups: Vec<(Vec<crate::generated::Unit>, crate::state::Code, crate::state::Code)>,
    /// Every unit form on the board with a SPACE CANNON: its state text, the renderer's text (the unit type id, what the string stages a gunner's
    /// roll under: the label of a reroll question). Set outside a combat too (the offense of the tactical head).
    pub guns: Vec<(crate::generated::Unit, crate::state::Code, crate::state::Code)>,
    /// The state texts `extra barrage` and `retreat_capacity`; the shown `extra barrage`.
    pub barrage_extra: crate::state::Code,
    pub retreat_cause: crate::state::Code,
    pub barrage_extra_shown: crate::state::Code,
    /// Shown texts: the cause and kind of the barrage, of a roll, the Hacan flagship, a retreat's loss and the pool it returns to, Raid
    /// Formation, Assault Cannon (its subtype and cause), Ambush, Impulse Core, Devotion, the casualty question, the Letnev commander.
    pub barrage_cause: crate::state::Code,
    pub barrage_kind: crate::state::Code,
    pub roll_cause: crate::state::Code,
    pub hacan: crate::state::Code,
    pub retreat_event: crate::state::Code,
    pub retreat_pool: crate::state::Code,
    pub raid: crate::state::Code,
    pub ac_subtype: crate::state::Code,
    pub ac_cause: crate::state::Code,
    pub ambush: crate::state::Code,
    pub cause_core: crate::state::Code,
    pub cause_devotion: crate::state::Code,
    pub casualty: crate::state::Code,
    pub commander: crate::state::Code,
    /// The payload keys `losers`, `retreated` and the reasons `retreat`, `draw`, `destruction`, `round_limit` of the end announcement.
    pub end: [crate::state::Code; 6],
    /// The state text of the payload key `opponents` and the shown text of the presentation phase `pre_roll` are the tactical codes'.
    pub opponents: crate::state::Code,
}

/// What the cards that reroll staged dice (Fire Team) need of the adapter: the state text of the ground kind, and for each
/// unit-group text of a staged entry (a state text) the code the question shows it by (a render code).
#[derive(Debug, Clone, Default)]
pub struct RerollFacts {
    pub ground: crate::state::Code,
    pub shown: Vec<(crate::state::Code, crate::state::Code)>,
}

impl RerollFacts {
    /// The code a question shows the unit group `state` by.
    #[must_use]
    pub fn shown_of(&self, state: crate::state::Code) -> Option<crate::state::Code> {
        self.shown.iter().find(|(s, _)| *s == state).map(|(_, shown)| *shown)
    }
}

impl AdapterFacts {
    /// The facts of the call that starts at `core` with `kernel`: asked of the provider at once, or, when the provider reads nothing but the
    /// position and its own kernel ([`Self::plays_source`], the same kernel as the call's), at the first read.
    pub fn arm_plays(&mut self, core: &CoreState, kernel: &Kernel) {
        let Some(provider) = &self.plays_provider else { return };
        self.plays = match self.plays_source {
            Some(source) if std::ptr::eq(source.kernel, kernel) => {
                Plays { eager: None, lazy: Some(std::sync::Arc::new(LazyPlays { at: core.clone(), provider: provider.clone(), kernel: source.kernel, value: std::sync::OnceLock::new() })) }
            }
            _ => Plays::of((provider.0)(core, kernel)),
        };
    }

    /// The unit roster the provider's facts carry for `core` (`None`: no roster, or no provider): without computing the facts when the source is known.
    #[must_use]
    pub fn provider_roster(&self, core: &CoreState, kernel: &Kernel) -> Option<&'static crate::query::tech::UnitRoster> {
        let provider = self.plays_provider.as_ref()?;
        match self.plays_source {
            Some(source) if std::ptr::eq(source.kernel, kernel) => source.roster,
            _ => (provider.0)(core, kernel)?.roster,
        }
    }

    /// The kernel a use of PRODUCTION an ability runs reads, for the source scope `host` plays: the adapter's production kernel of that scope, else
    /// the one it lent for payments.
    #[must_use]
    pub fn production_kernel_for(&self, host: &Kernel) -> Option<&'static Kernel> {
        match self.production_kernel {
            Some(f) => Some(f(host.sources)),
            None => self.pay_kernel.map(|f| f()),
        }
    }

    /// The printed-name group of each action card (the index of the first card with its name): [`Self::card_groups`], else the
    /// play facts' table. `None` when the adapter gave neither.
    #[must_use]
    pub fn card_group_table(&self) -> Option<&[u8]> {
        let table = if self.card_groups.is_empty() { self.plays.as_ref().map(|p| p.name_group.as_slice())? } else { self.card_groups.as_slice() };
        (!table.is_empty()).then_some(table)
    }

    /// Record the neighbours of `system` for `who`.
    pub fn set_reach(&mut self, who: Seat, system: crate::generated::System, neighbours: Vec<crate::generated::System>) {
        self.reach.retain(|(w, s, _)| !(*w == who && *s == system));
        self.reach.push((who, system, neighbours));
    }

    /// Record the systems Skilled Retreat may withdraw `who` to from `system` (the map's part: the adapter computed them): the adjacent systems the movement
    /// rules let the fleet enter, whether another player's ships stand there or not (the card reads the position for those).
    /// One play is named by its player, the system and the combat round sequence (a window can hold several plays, the ships change
    /// between them).
    pub fn set_withdraw(&mut self, who: Seat, system: crate::generated::System, round_seq: u32, destinations: Vec<crate::generated::System>) {
        self.withdraw.retain(|(w, s, n, _)| !(*w == who && *s == system && *n == round_seq));
        self.withdraw.push((who, system, round_seq, destinations));
    }

    /// The withdrawal destinations of `who` leaving `system`, if the string's card looked for them.
    #[must_use]
    pub fn withdraw(&self, who: Seat, system: crate::generated::System, round_seq: u32) -> Option<&[crate::generated::System]> {
        self.withdraw.iter().find(|(w, s, n, _)| *w == who && *s == system && *n == round_seq).map(|(.., d)| d.as_slice())
    }

    /// Record that the map has `a` next to `b` (or not): an input of the abilities that look at the map (Dynamo, the Watcher).
    pub fn set_adjacent(&mut self, a: crate::generated::System, b: crate::generated::System, adjacent: bool) {
        if !self.adjacent.iter().any(|(x, y, _)| *x == a && *y == b) {
            self.adjacent.push((a, b, adjacent));
        }
    }

    /// Record that `a` and `b` are neighbours (or not) as the string's map has it (`transactions::are_neighbours`): an input of Pillage in a
    /// window that is lent no map. The first answer of a window stands.
    pub fn set_neighbours(&mut self, a: Seat, b: Seat, neighbours: bool) {
        if !self.neighbours.iter().any(|(x, y, _)| *x == a && *y == b) {
            self.neighbours.push((a, b, neighbours));
        }
    }

    /// Whether `a` and `b` are neighbours, if the string's ability asked it.
    #[must_use]
    pub fn neighbours(&self, a: Seat, b: Seat) -> Option<bool> {
        self.neighbours.iter().find(|(x, y, _)| *x == a && *y == b).map(|(_, _, v)| *v)
    }

    /// The units `who` may build now (`production::buildable_for`, in the string's order): an input of Salvage Operations.
    pub fn set_buildable(&mut self, who: Seat, units: Vec<crate::generated::Unit>) {
        self.buildable.retain(|(w, _)| *w != who);
        self.buildable.push((who, units));
    }

    /// The units `who` may build, if the adapter computed them.
    #[must_use]
    pub fn buildable(&self, who: Seat) -> Option<&[crate::generated::Unit]> {
        self.buildable.iter().find(|(w, _)| *w == who).map(|(_, u)| u.as_slice())
    }

    /// Whether the map has `a` next to `b`, if the string's ability asked it.
    #[must_use]
    pub fn adjacent(&self, a: crate::generated::System, b: crate::generated::System) -> Option<bool> {
        self.adjacent.iter().find(|(x, y, _)| *x == a && *y == b).map(|(_, _, v)| *v)
    }

    /// The neighbours of `system` for `who`, if the adapter computed them.
    #[must_use]
    pub fn reach(&self, who: Seat, system: crate::generated::System) -> Option<&[crate::generated::System]> {
        self.reach.iter().find(|(w, s, _)| *w == who && *s == system).map(|(_, _, n)| n.as_slice())
    }
}

/// A rule-specific eligibility (`Host::condition`).
pub type Condition = fn(&CondCx<'_>) -> bool;
/// Whether `lender`'s rule is copied by `borrower` (Ssruu): `(state, borrower, lender)`.
pub type Pair = fn(&CoreState, Seat, Seat) -> bool;
/// The effect of a selected ability.
pub type Effect = fn(&mut Eff<'_, '_>) -> Result<(), TimingError>;
/// The close hook of a window (`AbilityTable::on_close`).
pub type CloseHook = fn(&mut Inline<'_, '_>) -> Result<(), TimingError>;
/// The cards a seat's reaction slot could play (`AbilityTable::slot_cards`).
pub type SlotCards = fn(&CoreState, &Kernel, &AdapterFacts, bool, Seat, &EventRec, Relation) -> Vec<crate::generated::ActionCard>;
/// What follows the discard of a played card the faction crates register (`AbilityTable::after_play`).
pub type Closed = fn(&mut RulesHost<'_>, &mut Resolver) -> Result<(), TimingError>;
/// The predicate of a registered "cannot" (`Host::forbids`).
pub type Forbids = fn(&CoreState, &CannotDef, &EventRec) -> bool;

/// The effect of a reaction card played from a window, run when the play is announced (after `ACTION_CARD_PLAYED`
/// was not cancelled, before the card reaches the discard pile): the host, the resolver of the window, the holder and
/// the card. Registered by the rules package that owns the effect (an effect that reads the combat or invasion area
/// cannot live in the action-card area: the areas do not import each other).
pub type CardEffect = fn(&mut RulesHost<'_>, &mut Resolver, Seat, crate::generated::ActionCard) -> Result<(), TimingError>;

/// A registered card effect: which cards it plays, and how.
#[derive(Clone, Copy)]
pub struct CardPlay {
    pub covers: fn(crate::generated::ActionCard) -> bool,
    pub effect: CardEffect,
}

/// One ability rule: its registry row and its behaviour.
#[derive(Clone, Copy)]
pub struct Spec {
    pub slot: Slot,
    pub kind: AbilityKind,
    pub applies: Applies,
    pub event: EventKind,
    pub relation: Relation,
    pub frequency: Frequency,
    pub optional: bool,
    pub repeatable: bool,
    pub condition: Option<Condition>,
    pub effect: Effect,
    /// A pair rule: registered for `(borrower, lender)` where this holds, owned by the borrower (`AbilityDef::pair_source` is
    /// the lender). `applies` then tests the lender.
    pub pair: Option<Pair>,
    /// A pair rule the string registers before it walks the seats (`reactions::arm`: the borrowed round agents), so its
    /// offers come before every seat's standing slots in a window.
    pub leads: bool,
    /// The effect is the core's. `false`: only the eligibility is (the offer is rendered and a decline is
    /// followed); a run in which the string took the ability is not followed.
    pub effect_ported: bool,
}

/// The effect of a spec whose eligibility only is ported.
#[allow(clippy::unnecessary_wraps)]
pub fn no_effect(_: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    Ok(())
}

impl Spec {
    /// A mandatory, once-per-trigger, unconditional spec for every seat.
    #[must_use]
    pub const fn new(slot: Slot, kind: AbilityKind, event: EventKind, relation: Relation, effect: Effect) -> Self {
        Self {
            slot,
            kind,
            applies: Applies::EverySeat,
            event,
            relation,
            frequency: Frequency::OncePerTrigger,
            optional: false,
            repeatable: false,
            condition: None,
            effect,
            pair: None,
            leads: false,
            effect_ported: true,
        }
    }

    /// An ability whose eligibility is ported and whose effect is not (yet): it can be offered and declined.
    #[must_use]
    pub const fn eligibility_only(slot: Slot, kind: AbilityKind, event: EventKind, relation: Relation, condition: Condition) -> Self {
        let mut s = Self::new(slot, kind, event, relation, no_effect);
        s.condition = Some(condition);
        s.effect_ported = false;
        s
    }

    #[must_use]
    pub const fn applies(mut self, a: Applies) -> Self {
        self.applies = a;
        self
    }

    #[must_use]
    pub const fn when(mut self, c: Condition) -> Self {
        self.condition = Some(c);
        self
    }

    #[must_use]
    pub const fn pair(mut self, lends: Pair) -> Self {
        self.pair = Some(lends);
        self
    }

    /// Registered before the seats' walk (see [`Spec::leads`]).
    #[must_use]
    pub const fn leading(mut self) -> Self {
        self.leads = true;
        self
    }

    #[must_use]
    pub const fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    #[must_use]
    pub const fn frequency(mut self, f: Frequency) -> Self {
        self.frequency = f;
        self
    }

    #[must_use]
    pub const fn repeatable(mut self) -> Self {
        self.repeatable = true;
        self
    }

    /// The registry row of `owner`; `id` is the registered text interned by the adapter.
    #[must_use]
    pub fn def(&self, id: AbilityId, owner: Seat) -> AbilityDef {
        let mut d = AbilityDef::new(id, owner, self.event, self.relation)
            .with_frequency(self.frequency)
            .with_optional(self.optional)
            .with_repeatable(self.repeatable)
            .with_arg(self.slot.arg());
        if self.condition.is_some() {
            d = d.with_condition();
        }
        d
    }
}

/// The rules behind the abilities the resolver's registry lists, by slot.
#[derive(Clone, Default)]
pub struct AbilityTable {
    /// The rules, shared between the copies of a table: `AbilityTable::clone` runs about five times per step and the specs are 10 KB, so a copy
    /// is a reference count and `register*` copy the vector only when it is shared (`Arc::make_mut`).
    specs: std::sync::Arc<Vec<Spec>>,
    forbids: std::sync::Arc<Vec<(u32, Forbids)>>,
    /// Political Secret (`laws::political_secret_bars`): no faction ability for this seat. Set by the rules
    /// package that owns it (the kernel cannot import the agenda area).
    pub bars: Option<fn(&CoreState, Seat) -> bool>,
    /// After a window's loop ended cleanly: announce the cards declared in it (the rules package that owns the
    /// plays sets it; the kernel cannot import it).
    pub on_close: Option<CloseHook>,
    /// The cards a seat's reaction slot could play into a window now (`reaction_slot::playable`, `Err` read as none):
    /// recorded with the question of a reaction window.
    pub slot_cards: Option<SlotCards>,
    pub codes: Codes,
    pub facts: AdapterFacts,
    /// A key that is equal for the copies of one printed card (the core has no card names); set by the adapter.
    pub card_name: Option<fn(crate::generated::ActionCard) -> u32>,
    /// What follows the discard of a played reaction card: announce what its effect staged (a destroyed ship).
    pub after_play: Option<Closed>,
    /// The calls a window makes into other rule areas (child windows, the research queries): areas do not import each
    /// other, so the layer that may (`driver::children::register`) fills them. Empty by default.
    pub links: super::children::AreaLinks,
    card_plays: std::sync::Arc<Vec<CardPlay>>,
    /// A seat that has nothing to resolve in a reaction window was looked at once (`Table::window_skipped` of the string): the log the
    /// wrapper drains after the step. `None` records nothing.
    pub skipped: Option<SkippedLog>,
    /// A payment with one way left to pay was collapsed without a question (`PayStep`, `pay_in`): the log the wrapper drains after the step to
    /// tell the payer (`Table::auto_resolve`: "it was the only way left to pay"). `None` records nothing and clones nothing.
    pub auto_paid: Option<std::sync::Arc<std::sync::Mutex<Vec<AutoPaid>>>>,
}

/// The log of the seats a reaction window passed by (`AbilityTable::skipped`).
pub type SkippedLog = std::sync::Arc<std::sync::Mutex<Vec<(Seat, EventRec, Relation)>>>;

/// A payment question that was not asked because it had one option: what the wrapper needs to announce it (the question as it would have been
/// put, the option taken, the position it was put in). A step the driver repeats (a rollback and a replay) logs the same entry again: the
/// wrapper takes the log of the committed step only.
#[derive(Debug, Clone)]
pub struct AutoPaid {
    pub choice: Choice,
    pub taken: Opt,
    pub at: Box<CoreState>,
}

impl AbilityTable {
    /// Log a collapsed single-way payment (nothing is built when no log is installed).
    pub fn note_auto_paid(&self, state: &CoreState, question: impl FnOnce() -> Option<Choice>) {
        let Some(log) = &self.auto_paid else { return };
        let Some(choice) = question() else { return };
        let Some(&taken) = choice.opts.as_slice().first() else { return };
        log.lock().expect("auto paid log").push(AutoPaid { choice, taken, at: Box::new(state.clone()) });
    }

    /// Register the effect of some reaction cards.
    pub fn add_card_play(&mut self, play: CardPlay) {
        std::sync::Arc::make_mut(&mut self.card_plays).push(play);
    }

    /// The registered effect that plays `card`, if any.
    #[must_use]
    pub fn card_play(&self, card: crate::generated::ActionCard) -> Option<&CardPlay> {
        self.card_plays.iter().find(|p| (p.covers)(card))
    }

    pub fn add(&mut self, spec: Spec) {
        debug_assert!(self.of_slot(spec.slot).is_none(), "slot {:?} registered twice", spec.slot);
        std::sync::Arc::make_mut(&mut self.specs).push(spec);
    }

    /// The predicate of the cannot whose `arg` is given.
    pub fn add_forbid(&mut self, arg: u32, f: Forbids) {
        std::sync::Arc::make_mut(&mut self.forbids).push((arg, f));
    }

    #[must_use]
    pub fn specs(&self) -> &[Spec] {
        &self.specs
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.specs.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }

    #[must_use]
    pub fn of_slot(&self, slot: Slot) -> Option<&Spec> {
        self.specs.iter().find(|s| s.slot == slot)
    }

    /// The spec behind a registry row (`arg` is the slot).
    #[must_use]
    pub fn of(&self, def: &AbilityDef) -> Option<&Spec> {
        let slot = u16::try_from(def.slot_bits()).ok()?;
        self.of_slot(Slot(slot))
    }

    /// Whether the table has a rule for `arg`.
    #[must_use]
    pub fn knows(&self, arg: u32) -> bool {
        u16::try_from(arg & 0xFFFF).ok().is_some_and(|s| self.of_slot(Slot(s)).is_some())
    }

    /// The registry of a game: per seat of `order`, the specs that apply to it, in table order. `name` interns
    /// the registered text of a spec for an owner (the adapter's; the core holds no text).
    pub fn registry(
        &self,
        s: &CoreState,
        order: &[Seat],
        name: &mut dyn FnMut(&Spec, Seat, Option<Seat>) -> AbilityId,
    ) -> Vec<AbilityDef> {
        // (Sized for a table of a few hundred rows in one allocation: growing from empty by doubling re-allocated it nine times per call.)
        let mut out = Vec::with_capacity(self.specs.len() * order.len());
        // The pair rules the string registers first (`leads`), borrower by borrower, then each lender in seat order.
        for &borrower in order {
            for &lender in order {
                for spec in self.specs.iter().filter(|spec| spec.leads) {
                    if let Some(lends) = spec.pair {
                        if borrower != lender && spec.applies.holds(s, lender) && lends(s, borrower, lender) {
                            out.push(spec.def(name(spec, borrower, Some(lender)), borrower).with_pair_source(lender));
                        }
                    }
                }
            }
        }
        // The string registers while it walks the seats, each module's abilities for the seat it walks: a pair rule (a
        // copy of this seat's agent by another seat) is registered for the borrower at the point of the lender's walk.
        for &lender in order {
            for spec in self.specs.iter().filter(|spec| !spec.leads) {
                if !spec.applies.holds(s, lender) {
                    continue;
                }
                match spec.pair {
                    None => out.push(spec.def(name(spec, lender, None), lender)),
                    Some(lends) => {
                        for &borrower in order {
                            if borrower != lender && lends(s, borrower, lender) {
                                out.push(spec.def(name(spec, borrower, Some(lender)), borrower).with_pair_source(lender));
                            }
                        }
                    }
                }
            }
        }
        out
    }
}

/// What a nested question was asked about: the event of the window, the relation and the emission chain around
/// it. The adapter builds the question's trigger from it (the display facts of the string `DecisionContext`).
#[derive(Debug, Clone, PartialEq)]
pub struct NestedFacts {
    pub event: EventRec,
    pub relation: Relation,
    pub chain: Vec<u64>,
    /// The cards the asked seat's reaction slot could play when the question was put (a reaction window's pick):
    /// read from the state of that moment, which the driver's rollback of the step no longer holds when the
    /// question is rendered.
    pub offered: Vec<crate::generated::ActionCard>,
}

/// The answers of the current operation, in the order they were given; the nested questions of a step
/// read them in order. `needed` is the first question that found the tape dry.
#[derive(Debug, Clone, Default)]
pub struct Tape {
    answers: Vec<Pick>,
    used: usize,
    needed: Option<Choice>,
    facts: Option<NestedFacts>,
    /// The state at the moment the question was put (the driver rolls the state back; the display facts of the question
    /// are read from this one).
    state_at: Option<Box<CoreState>>,
    /// A condition the core could not answer (`CondCx::unknown`); survives the rewind of a re-run.
    pub unknown: std::cell::Cell<Option<Unknown>>,
    /// What [`Self::unknown`] held when the run found the tape dry (`None`: not dry). The rest of that run follows the default pick
    /// of the unanswered question (the decline) and is rolled back, so a condition it could not answer belongs to a path that may
    /// never be taken (heavy fix 1: a Future Sight paid in the dry run of a close before the Deadly Plot that discards the agenda
    /// was answered, whose Pillage read a fact the window was not lent). [`Self::take_unknown`] reports the value of that moment.
    unknown_at_dry: Option<Option<Unknown>>,
    /// The events a hook of the table emitted at the top level of the window that called it (`AbilityTable::after_play`: the
    /// `SHIP_DESTROYED` of what a played card staged): the window records them as its own, like the events of its steps.
    pub emitted: Vec<EventRec>,
    /// A converted path's continuation at the question that found the tape dry (stepmachine-1, `docs/STEPMACHINE1_RESUME.md`): what the
    /// run had done up to that question, as plain data of the driver's own type (layer L5; this layer only carries it). Cleared by every
    /// rewind; the driver takes it when it rolls the dry run back.
    pub parked: crate::state::Memo,
}

impl Tape {
    pub fn clear(&mut self) {
        self.answers.clear();
        self.rewind();
    }

    /// The reason a condition could not be answered since the last call, if any; in a run that found the tape dry, only a reason
    /// noted before the unanswered question (what the speculative rest of the run noted is dropped with the rollback).
    pub fn take_unknown(&self) -> Option<Unknown> {
        let noted = self.unknown.take();
        match self.unknown_at_dry {
            Some(at_dry) => at_dry,
            None => noted,
        }
    }

    /// Start reading from the first answer again (a re-run of the step).
    pub fn rewind(&mut self) {
        self.used = 0;
        self.parked = crate::state::Memo::default();
        self.needed = None;
        self.unknown_at_dry = None;
        self.facts = None;
        self.state_at = None;
    }

    pub fn push(&mut self, pick: Pick) {
        self.answers.push(pick);
    }

    /// The answers given so far, in order (what a suspended step stores, `state::Nested`).
    #[must_use]
    pub fn answers(&self) -> &[Pick] {
        &self.answers
    }

    /// Replace the answers (a driver rebuilt from a suspended step).
    pub fn set_answers(&mut self, answers: &[Pick]) {
        self.answers = answers.to_vec();
        self.rewind();
    }

    /// Replace the answers and mark them all read (a run resumed at the question the last answer answers: the answers before it were read
    /// by the run that stopped there).
    pub fn set_consumed(&mut self, answers: &[Pick]) {
        self.set_answers(answers);
        self.used = self.answers.len();
    }

    /// Whether every answer was read (the next question finds the tape dry).
    #[must_use]
    pub fn exhausted(&self) -> bool {
        self.used >= self.answers.len()
    }

    /// The question that found the tape dry in the run just made.
    #[must_use]
    pub fn needed(&self) -> Option<&Choice> {
        self.needed.as_ref()
    }

    /// The state the nested question was put in.
    #[must_use]
    pub fn needed_state(&self) -> Option<&CoreState> {
        self.state_at.as_deref()
    }

    /// What that question was asked about (`None` for a question that has no event).
    #[must_use]
    pub fn needed_facts(&self) -> Option<&NestedFacts> {
        self.facts.as_ref()
    }

    /// [`Asker::pick`] with the facts of the window the question belongs to.
    pub fn pick_with(&mut self, state: &CoreState, choice: &Choice, facts: Option<NestedFacts>) -> Pick {
        if let Some(p) = self.answers.get(self.used) {
            self.used += 1;
            return *p;
        }
        if self.needed.is_none() {
            self.needed = Some(choice.clone());
            self.unknown_at_dry = Some(self.unknown.get());
            self.facts = facts;
            self.state_at = Some(Box::new(state.clone()));
        }
        // The run is rolled back; an option that ends the question lets it finish: the decline when there is one
        // (a repeatable, unlimited ability that is picked again and again would never end the window).
        choice.decline_index().and_then(|i| choice.pick(i)).or_else(|| choice.pick(0)).unwrap_or(Pick::Opt(crate::choice::Opt::Decline))
    }
}

impl Asker for Tape {
    fn pick(&mut self, state: &CoreState, choice: &Choice) -> Pick {
        self.pick_with(state, choice, None)
    }
}

// -- chance (lane C3, docs/FORK_RNG_THEORY.md 4.3): every random event is a question on the tape -------------------------------------

impl Tape {
    /// A random event: the faces the tape holds for it, else a question. The core never draws (decision D1). When the tape is dry,
    /// the request becomes the question the run stops at (unless an earlier question did: then it is part of the speculative rest of a
    /// run that will be rolled back), and the run goes on with the fixed placeholder (every face the highest, [`placeholder`]) to the end of
    /// its operation, exactly as a dry reaction question goes on with its decline. Nothing the placeholder leads to survives the
    /// rollback; the unknowns noted after the question are dropped with it (`take_unknown`).
    #[inline(never)] // one copy for every site (the browser build's size budget)
    pub fn chance(&mut self, state: &CoreState, req: &crate::chance::Request) -> crate::chance::Faces {
        if let Some(p) = self.answers.get(self.used) {
            self.used += 1;
            #[cfg(feature = "probe-counters")]
            crate::state::census::met(req.len(), true, false);
            // (A live answer was checked against its question, `Request::accepts`; a tape whose answer is no answer to this request belongs
            // to another run of the operation: a defect of the caller.)
            if let Pick::Chance(faces) = p {
                debug_assert!(req.accepts(faces), "the tape answers the chance request {req:?} with {faces:?}");
                return *faces;
            }
            debug_assert!(false, "the tape answers the chance request {req:?} with {p:?}");
            return placeholder(req);
        }
        #[cfg(feature = "probe-counters")]
        crate::state::census::met(req.len(), false, self.needed.is_none());
        if self.needed.is_none() {
            let actor = req.owner.map_or_else(|| state.meta().active.unwrap_or(Seat(0)), Seat);
            // (`facts` and `state_at` are set with `needed` only, so they are `None` here: a chance question has neither.)
            self.needed = Some(chance_choice(actor, *req));
            self.unknown_at_dry = Some(self.unknown.get());
        }
        placeholder(req)
    }

    /// `n` dice of `sides` sides from `domain`, in draw order (one request per group of at most `MAX_FACES`, `Request::dice_groups`).
    /// `hits_on`: the roll's threshold (a face `>= hits_on` hits) when one decides the roll; carried by the requests for a search that groups
    /// a chance node's answers by hit count (lane C6), never read by an answer: the draw is the same.
    #[inline(never)] // one copy for every site (the browser build's size budget)
    pub fn dice(&mut self, state: &CoreState, domain: crate::state::RngDomain, n: usize, sides: u32, hits_on: Option<u8>) -> Vec<u32> {
        let sides = die_sides(sides);
        let mut out = Vec::with_capacity(n);
        let mut left = n;
        while left > 0 {
            // At most `MAX_FACES`, so the cast is exact.
            #[allow(clippy::cast_possible_truncation)]
            let k = left.min(crate::chance::MAX_FACES) as u8;
            left -= usize::from(k);
            let req = crate::chance::Request::new(domain, crate::chance::Kind::Dice { n: k, sides, hits_on });
            for f in self.chance(state, &req).as_slice() {
                out.push(u32::from(*f));
            }
        }
        out
    }

    /// One die of `sides` sides from `domain` (`1..=sides`): a request of one die.
    #[inline(never)] // one copy for every site (the browser build's size budget)
    pub fn die(&mut self, state: &CoreState, domain: crate::state::RngDomain, sides: u32) -> u32 {
        let req = crate::chance::Request::new(domain, crate::chance::Kind::Dice { n: 1, sides: die_sides(sides), hits_on: None });
        self.chance(state, &req).as_slice().first().map_or(1, |f| u32::from(*f))
    }

    /// A uniformly chosen element among `n` (`0..n`): an `Index` request, one face in `1..=n` (the Kyver hero's card, the Neuraloop's objective).
    #[inline(never)] // one copy for every site (the browser build's size budget)
    pub fn index(&mut self, state: &CoreState, domain: crate::state::RngDomain, n: usize) -> usize {
        let n = u8::try_from(n).unwrap_or(u8::MAX);
        let req = crate::chance::Request::new(domain, crate::chance::Kind::Index { n });
        self.chance(state, &req).as_slice().first().map_or(0, |f| usize::from(*f).saturating_sub(1))
    }
}

/// The sides of a die as a request carries them (every die of the game has at most 255 sides).
fn die_sides(sides: u32) -> u8 {
    debug_assert!(sides <= u32::from(u8::MAX), "a die of {sides} sides");
    u8::try_from(sides).unwrap_or(u8::MAX)
}

/// The fixed answer of a chance request in a run that is rolled back (lane C3, docs/FORK_RNG_THEORY.md 4.3): every face the highest
/// (`sides`). Fixed rather than drawn, so a speculative run moves no stream. The highest, not the lowest the theory proposed: a ground
/// combat run inline inside one operation (Dannel of the Tenth, the Yin hero's fight) rolls round after round until a side is gone, and with
/// every die a 1 nobody would ever hit; with every die hitting each such loop ends (found by the core's tests, lane C3).
#[must_use]
pub fn placeholder(req: &crate::chance::Request) -> crate::chance::Faces {
    let mut faces = crate::chance::Faces::default();
    for _ in 0..req.len() {
        let _ = faces.try_push(req.sides().max(1));
    }
    faces
}

/// The question of a chance request: no options, the answer is `Pick::Chance`.
#[must_use]
pub fn chance_choice(actor: Seat, req: crate::chance::Request) -> Choice {
    Choice {
        actor,
        ask: crate::choice::Ask::Generic(crate::choice::generic::Ask::Chance(req)),
        ctx: crate::choice::Ctx {
            origin: crate::choice::Origin::Rule(crate::choice::generic::Rule::R2_4),
            subtype: crate::choice::Subtype::Generic(crate::choice::generic::Subtype::Chance),
            optional: false,
            target: None,
            outstanding: crate::containers::SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        },
        opts: crate::choice::Opts::default(),
    }
}

/// The resolver's `Host` under the real rules: dispatches on the slot, asks through the tape.
pub struct RulesHost<'a> {
    pub state: &'a mut CoreState,
    pub kernel: &'a Kernel,
    pub table: &'a AbilityTable,
    pub tape: &'a mut Tape,
    /// The next trace id of an event an ability emits.
    pub next_id: &'a mut u64,
}

impl RulesHost<'_> {
    /// A die from `domain`: a chance question on the tape (`Sx::die`).
    pub fn die(&mut self, domain: RngDomain, sides: u32) -> u32 {
        self.tape.die(&*self.state, domain, sides)
    }

    /// Put a question that belongs to no window of the resolver (a played card's, asked while the play is announced): the
    /// display facts of the question (battle, invasion) are the adapter's, the trigger is not part of it.
    pub fn ask(&mut self, choice: &Choice) -> Pick {
        let event = EventRec { id: 0, kind: EventKind::ActionCardPlayed, payload: crate::state::Payload::new(), cancelled: false };
        let facts = NestedFacts { event, relation: Relation::When, chain: Vec::new(), offered: Vec::new() };
        let state: &CoreState = self.state;
        self.tape.pick_with(state, choice, Some(facts))
    }
}

impl Host for RulesHost<'_> {
    fn state(&self) -> &CoreState {
        self.state
    }

    fn bars_faction_abilities(&mut self, who: Seat) -> bool {
        self.table.bars.is_some_and(|f| f(self.state, who))
    }

    fn window_skipped(&mut self, who: Seat, event: &EventRec, relation: Relation) {
        if let Some(log) = &self.table.skipped {
            log.lock().expect("skipped log").push((who, event.clone(), relation));
        }
    }

    fn condition(&mut self, ability: &AbilityDef, event: &EventRec, resolver: &Resolver) -> bool {
        match self.table.of(ability).and_then(|a| a.condition) {
            Some(c) => {
                let barred = self.table.bars.is_some_and(|f| f(self.state, ability.owner));
                c(&CondCx { state: self.state, kernel: self.kernel, def: ability, event, resolver, barred, codes: self.table.codes, facts: &self.table.facts, unknown: &self.tape.unknown })
            }
            None => true,
        }
    }

    fn forbids(&mut self, cannot: &CannotDef, event: &EventRec) -> bool {
        self.table.forbids.iter().find(|(arg, _)| *arg == cannot.arg).is_some_and(|(_, f)| f(self.state, cannot, event))
    }

    fn effect(&mut self, ability: &AbilityDef, event: &mut EventRec, resolver: &mut Resolver) -> Result<(), TimingError> {
        let Some(spec) = self.table.of(ability) else { return Ok(()) };
        let effect = spec.effect;
        let mut eff = Eff { host: self, resolver, def: *ability, event };
        effect(&mut eff)
    }

    fn window_closed(&mut self, resolver: &mut Resolver) -> Result<(), TimingError> {
        let Some(close) = self.table.on_close else { return Ok(()) };
        close(&mut Inline { host: self, resolver, facts: None })
    }

    fn decide(&mut self, choice: &Choice, event: &EventRec, chain: &[u64]) -> Pick {
        let relation = match choice.ask {
            crate::choice::Ask::Reaction(crate::choice::reaction::Ask::Window { relation, .. }) => Some(relation),
            _ => None,
        };
        let facts = relation.map(|relation| {
            let offered = match (self.table.slot_cards, relation) {
                (Some(cards), _) if choice.opts.iter().any(|o| matches!(o, crate::choice::Opt::Reaction(crate::choice::reaction::Opt::Ability(id)) if id.kind == AbilityKind::Reaction)) => {
                    let barred = self.table.bars.is_some_and(|f| f(self.state, choice.actor));
                    cards(&*self.state, self.kernel, &self.table.facts, barred, choice.actor, event, relation)
                }
                _ => Vec::new(),
            };
            NestedFacts { event: event.clone(), relation, chain: chain.to_vec(), offered }
        });
        let state: &CoreState = self.state;
        self.tape.pick_with(state, choice, facts)
    }
}

/// What an ability's effect may do: read and change the state, ask, draw dice, emit a nested event, cancel
/// the event of its window. Everything it does is undone by the driver's rollback when a nested question
/// found the tape dry, so an effect that asks twice still runs once per committed run.
pub struct Eff<'h, 'a> {
    host: &'h mut RulesHost<'a>,
    resolver: &'h mut Resolver,
    pub def: AbilityDef,
    pub event: &'h mut EventRec,
}

impl<'a> Eff<'_, 'a> {
    pub fn state(&mut self) -> &mut CoreState {
        self.host.state
    }

    #[must_use]
    pub fn kernel(&self) -> &Kernel {
        self.host.kernel
    }

    /// The state to write and the kernel to read at once (a placement asks the hooks while it writes).
    pub fn state_and_kernel(&mut self) -> (&mut CoreState, &Kernel) {
        (&mut *self.host.state, self.host.kernel)
    }

    /// Political Secret bars this ability's owner (`AbilityTable::bars`).
    #[must_use]
    pub fn barred(&self) -> bool {
        self.host.table.bars.is_some_and(|f| f(self.host.state, self.def.owner))
    }

    /// The window cannot be followed (the same reason a condition gives with `CondCx::unknown`).
    pub fn unknown(&self, why: Unknown) {
        if self.host.tape.unknown.get().is_none() {
            self.host.tape.unknown.set(Some(why));
        }
    }

    /// Political Secret bars this seat's faction abilities and action cards.
    #[must_use]
    pub fn barred_of(&self, who: Seat) -> bool {
        self.host.table.bars.is_some_and(|f| f(self.host.state, who))
    }

    /// Whether two cards are copies of one printed card.
    #[must_use]
    pub fn card_key(&self, card: crate::generated::ActionCard) -> u32 {
        self.host.table.card_name.map_or(card as u32, |f| f(card))
    }

    /// The texts the adapter interned for the abilities.
    #[must_use]
    pub fn codes(&self) -> Codes {
        self.host.table.codes
    }

    /// What the adapter computed for the window.
    #[must_use]
    pub fn adapter_facts(&self) -> &AdapterFacts {
        &self.host.table.facts
    }

    /// The table of abilities the window runs with (the logs the wrapper drains).
    #[must_use]
    pub fn host_table(&self) -> &AbilityTable {
        self.host.table
    }

    /// The kernel a payment inside this ability reads, if the adapter lent one.
    #[must_use]
    pub fn pay_kernel(&self) -> Option<&'static Kernel> {
        self.host.table.facts.pay_kernel.map(|f| f())
    }

    /// The state and the kernel, read together (`state()` borrows the effect mutably).
    #[must_use]
    pub fn read(&self) -> (&CoreState, &Kernel) {
        (&*self.host.state, self.host.kernel)
    }

    #[must_use]
    pub fn owner(&self) -> Seat {
        self.def.owner
    }

    /// The ids of the emissions around the event of this window (outermost first, the event excluded).
    #[must_use]
    pub fn chain(&self) -> Vec<u64> {
        self.resolver.emission_chain(self.event)
    }

    /// The facts a question asked by this ability carries (its trigger).
    #[must_use]
    pub fn facts(&self) -> NestedFacts {
        NestedFacts { event: self.event.clone(), relation: self.def.relation, chain: self.chain(), offered: Vec::new() }
    }

    /// Run `f` with the host and the resolver of this effect: a nested announcement that is not an event of the window
    /// (a card played at once).
    pub fn with_host<R>(&mut self, f: impl FnOnce(&mut RulesHost<'_>, &mut Resolver) -> R) -> R {
        f(&mut *self.host, &mut *self.resolver)
    }

    /// Put a question and take the answer.
    pub fn ask(&mut self, choice: &Choice) -> Pick {
        let facts = self.facts();
        let state: &CoreState = &*self.host.state;
        self.host.tape.pick_with(state, choice, Some(facts))
    }

    /// A die from `domain` (`Sx::die`): a chance question on the tape.
    pub fn die(&mut self, domain: RngDomain, sides: u32) -> u32 {
        self.host.tape.die(&*self.host.state, domain, sides)
    }

    /// Roll `count` dice with `sides` sides from the dice stream and record the roll in the ledger (`Dice::roll_by`
    /// of an ability: the faces and the record are the string's). Returns the faces.
    pub fn roll(&mut self, by: Seat, count: usize, hits_on: Option<u8>, purpose: crate::state::RollPurpose, sides: u32) -> Vec<u8> {
        let faces: Vec<u8> = self.host.tape.dice(&*self.host.state, RngDomain::Dice, count, sides, hits_on).into_iter().map(|f| u8::try_from(f).unwrap_or(u8::MAX)).collect();
        self.host.state.ledger_mut().dice.push(crate::state::RollRec {
            purpose,
            owner: Some(by),
            unit: None,
            planet: None,
            scene: None,
            hits_on,
            faces: faces.iter().copied().collect(),
            rerolled: 0,
        });
        faces
    }

    /// Emit a nested event through the same resolver (the string `emit_with_context` of an effect). A zero id is
    /// replaced by the next trace id.
    ///
    /// # Errors
    /// What the resolver refuses with.
    pub fn emit(&mut self, mut rec: EventRec) -> Result<EventRec, TimingError> {
        if rec.id == 0 {
            rec.id = *self.host.next_id;
            *self.host.next_id += 1;
        }
        self.resolver.emit(&mut *self.host, rec, |_, _, _| {})
    }

    /// "Cancel that event."
    pub fn cancel(&mut self) {
        self.event.cancelled = true;
    }

    /// [`Inline::with_sx`] over the kernel the adapter lent for payments ([`Self::pay_kernel`]): how an ability runs a window that reads the
    /// faction hooks of production (a use of PRODUCTION by an ability). `None`, `f` not run: the adapter lent none. The window's own
    /// questions are the caller's to put ([`Self::ask`]); its events are announced inside the effect.
    pub fn with_pay_sx<T>(&mut self, f: impl FnOnce(&mut Sx<'_>) -> T) -> Option<T> {
        let kernel = self.host.table.facts.production_kernel_for(self.host.kernel)?;
        let mut emitted = Vec::new();
        let mut sx = Sx {
            state: &mut *self.host.state,
            kernel,
            resolver: &mut *self.resolver,
            table: self.host.table,
            tape: &mut *self.host.tape,
            next_id: &mut *self.host.next_id,
            emitted: &mut emitted,
        };
        Some(f(&mut sx))
    }

    /// The services of this effect as a [`Runner`] (an effect that plays an action card): its questions carry the
    /// facts of the effect's window.
    pub fn inline(&mut self) -> Inline<'_, 'a> {
        let facts = self.facts();
        Inline { host: &mut *self.host, resolver: &mut *self.resolver, facts: Some(facts) }
    }
}

/// What a rule that runs inside a window or an effect needs from its surroundings: the state, the kernel, the
/// adapter's facts, an emission and a question. A [`Window`](super::stage::Window) implements it over its `Sx`
/// (its own questions are stages; `ask` is for the nested ones), an effect or the close of a window over
/// [`Inline`] (every question goes to the tape).
pub trait Runner {
    fn state(&mut self) -> &mut CoreState;
    fn read(&self) -> (&CoreState, &Kernel);
    /// Change the state with the kernel at hand (a rule that reads through hooks while it writes).
    fn with_kernel<T>(&mut self, f: impl FnOnce(&mut CoreState, &Kernel) -> T) -> T;
    fn facts(&self) -> &AdapterFacts;
    /// Emit an event through the windows around it (a zero id is replaced by the next trace id).
    ///
    /// # Errors
    /// What the resolver refuses with.
    fn emit(&mut self, rec: EventRec) -> Result<EventRec, TimingError>;
    /// Put a question to the tape and take the answer.
    fn ask(&mut self, choice: &Choice) -> Pick;
    /// The run cannot be trusted (a documented bail of the rule being run): the adapter falls back to the string.
    fn unknown(&mut self, why: Unknown);
    /// Run `f` with the services of a stage (a window's `Sx`) over this runner: how a rule that is a [`Window`] with a child
    /// window (a payment, a research) is stepped inside an effect or at the close of a window.
    fn with_sx<T>(&mut self, f: impl FnOnce(&mut Sx<'_>) -> T) -> T;
    /// Whether a rules package registered the effect of `card` (`AbilityTable::add_card_play`): the effects that read the
    /// combat or invasion area and run on the host and the resolver of the window.
    fn has_registered_play(&self, _card: crate::generated::ActionCard) -> bool {
        false
    }
    /// Run the registered effect of `card` played by `who` (nothing when there is none).
    ///
    /// # Errors
    /// What the resolver refuses with.
    fn run_registered_play(&mut self, _who: Seat, _card: crate::generated::ActionCard) -> Result<(), TimingError> {
        Ok(())
    }
    /// What follows the discard of a played card: announce what its effect staged (`AbilityTable::after_play`). Whether a hook announced it
    /// (no hook registered: `false`, and what the effect staged is still waiting).
    ///
    /// # Errors
    /// What the resolver refuses with.
    fn after_play(&mut self) -> Result<bool, TimingError> {
        Ok(false)
    }
}

/// The services of a rule that runs inside an effect or at the close of a window.
pub struct Inline<'h, 'a> {
    host: &'h mut RulesHost<'a>,
    resolver: &'h mut Resolver,
    facts: Option<NestedFacts>,
}

impl Runner for Inline<'_, '_> {
    fn state(&mut self) -> &mut CoreState {
        self.host.state
    }

    fn read(&self) -> (&CoreState, &Kernel) {
        (&*self.host.state, self.host.kernel)
    }

    fn with_kernel<T>(&mut self, f: impl FnOnce(&mut CoreState, &Kernel) -> T) -> T {
        f(&mut *self.host.state, self.host.kernel)
    }

    fn facts(&self) -> &AdapterFacts {
        &self.host.table.facts
    }

    fn emit(&mut self, mut rec: EventRec) -> Result<EventRec, TimingError> {
        if rec.id == 0 {
            rec.id = *self.host.next_id;
            *self.host.next_id += 1;
        }
        self.resolver.emit(&mut *self.host, rec, |_, _, _| {})
    }

    fn ask(&mut self, choice: &Choice) -> Pick {
        let state: &CoreState = self.host.state;
        self.host.tape.pick_with(state, choice, self.facts.clone())
    }

    fn unknown(&mut self, why: Unknown) {
        if self.host.tape.unknown.get().is_none() {
            self.host.tape.unknown.set(Some(why));
        }
    }

    fn with_sx<T>(&mut self, f: impl FnOnce(&mut Sx<'_>) -> T) -> T {
        let mut emitted = Vec::new();
        let mut sx = Sx {
            state: &mut *self.host.state,
            kernel: self.host.kernel,
            resolver: &mut *self.resolver,
            table: self.host.table,
            tape: &mut *self.host.tape,
            next_id: &mut *self.host.next_id,
            emitted: &mut emitted,
        };
        f(&mut sx)
    }

    fn has_registered_play(&self, card: crate::generated::ActionCard) -> bool {
        self.host.table.card_play(card).is_some()
    }

    fn run_registered_play(&mut self, who: Seat, card: crate::generated::ActionCard) -> Result<(), TimingError> {
        match self.host.table.card_play(card).copied() {
            Some(play) => (play.effect)(&mut *self.host, &mut *self.resolver, who, card),
            None => Ok(()),
        }
    }

    fn after_play(&mut self) -> Result<bool, TimingError> {
        match self.host.table.after_play {
            Some(after) => after(&mut *self.host, &mut *self.resolver).map(|()| true),
            None => Ok(false),
        }
    }
}

#[cfg(test)]
mod table_clone_tests {
    use super::*;

    fn spec(slot: u16) -> Spec {
        Spec::new(Slot(slot), AbilityKind::Ability, EventKind::TradeGoodsGained, Relation::After, no_effect)
    }

    /// A table clone shares the specs (a reference count, not a copy of 10 KB); a registration on the copy leaves the original as it was
    /// and a registration on the original (once it stopped being shared) leaves the copy as it was.
    #[test]
    fn a_clone_shares_the_specs_and_registering_copies_on_write() {
        let mut a = AbilityTable::default();
        a.add(spec(1));
        a.add(spec(2));
        let mut b = a.clone();
        assert!(std::sync::Arc::ptr_eq(&a.specs, &b.specs), "a clone is a reference count");
        b.add(spec(3));
        assert_eq!((a.len(), b.len()), (2, 3), "the registration on the copy did not reach the original");
        assert!(!std::sync::Arc::ptr_eq(&a.specs, &b.specs));
        assert!(a.of_slot(Slot(3)).is_none() && b.of_slot(Slot(3)).is_some());
        let c = a.clone();
        a.add(spec(4));
        assert_eq!((a.len(), c.len()), (3, 2), "the registration on the original did not reach the earlier copy");
        assert!(c.of_slot(Slot(4)).is_none() && a.of_slot(Slot(4)).is_some());
        a.add_forbid(7, |_, _, _| false);
        assert!(c.forbids.is_empty() && a.forbids.len() == 1);
    }
}

#[cfg(test)]
mod tape_tests {
    use super::*;
    use crate::choice::{Choice, Ctx, Opts, Origin};

    fn question() -> Choice {
        let (relation, event) = (Relation::When, EventKind::AgendaResolved);
        let mut opts = Opts::default();
        opts.push(crate::choice::Opt::Cards(crate::choice::cards::Opt::Card(crate::generated::ActionCard::DeadlyPlot)));
        opts.push(crate::choice::Opt::Decline);
        Choice {
            actor: Seat(2),
            ask: crate::choice::Ask::Cards(crate::choice::cards::Ask::PlayCard { relation, event }),
            ctx: Ctx {
                origin: Origin::Rule(crate::choice::generic::Rule::R22_1),
                subtype: crate::choice::Subtype::Cards(crate::choice::cards::Subtype::PlayReaction { relation, event }),
                optional: true,
                target: None,
                outstanding: crate::containers::SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts,
        }
    }

    /// Heavy fix 1: what a condition could not answer in the rest of a run that found the tape dry belongs to a path the rollback
    /// undoes (the default pick of the unanswered question); only what was noted before the question is reported. A run that is not
    /// dry reports what it noted, and the next run starts clean.
    #[test]
    fn the_unknown_of_a_dry_runs_speculative_rest_is_dropped_and_one_noted_before_the_question_is_kept() {
        let state = CoreState::default();
        let mut tape = Tape::default();
        tape.rewind();
        let _ = tape.pick_with(&state, &question(), None);
        assert!(tape.needed().is_some(), "the tape is dry");
        tape.unknown.set(Some(Unknown::MissingFact));
        assert_eq!(tape.take_unknown(), None, "noted after the unanswered question: dropped");
        tape.rewind();
        tape.unknown.set(Some(Unknown::MissingFact));
        let _ = tape.pick_with(&state, &question(), None);
        tape.unknown.set(Some(Unknown::CardCost));
        assert_eq!(tape.take_unknown(), Some(Unknown::MissingFact), "noted before the question: kept");
        tape.rewind();
        tape.push(Pick::Opt(crate::choice::Opt::Decline));
        let _ = tape.pick_with(&state, &question(), None);
        tape.unknown.set(Some(Unknown::CardCost));
        assert_eq!(tape.take_unknown(), Some(Unknown::CardCost), "a run with every answer reports what it noted");
        assert_eq!(tape.take_unknown(), None);
    }
}
