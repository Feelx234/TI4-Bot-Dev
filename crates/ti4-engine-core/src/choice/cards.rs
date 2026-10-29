//! Decision types of the `cards` area: action cards.
//!
//! Owner: package B6 (docs/ENGINE_CORE_STATE.md 4.4). Ported so far: the own-hand question
//! (`action_cards::choose_from_own_hand`): one option per distinct printed card, then optionally decline.
//! The adapter side is `ti4-engine/src/render/cards.rs`. Never text; every type is `Copy`.

use crate::choice::reaction::Relation;
use crate::generated::{ActionCard, Planet, System};
use crate::state::EventKind;
use crate::state::NoteKey;
use crate::ids::Seat;
use crate::keys::Outcome;

use super::agenda::Frame;

/// What a card's pick question is about (`action_cards::pick`'s `kind`): it decides the option kind and the
/// subtype tail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum What {
    /// kind `player`: a seat.
    Player,
    /// kind `elect`: a seat, Confusing Legal Text's redirect.
    Elect,
    /// kind `recall`: a system holding the asked seat's command token.
    Recall,
    /// kind `system`: a system to bombard (Tactical Bombardment).
    Bombard,
    /// kind `system`: a system a cruiser is placed in (War Effort).
    Deploy,
    /// kind `silence`: a system holding the asked seat's ships (In The Silence Of Space).
    Silence,
    /// kind `promissory note`: a note of the asked seat's hand (Diplomatic Pressure).
    Note,
    /// kind `planet`: a `system|planet` spot.
    Planet,
    /// kind `rescue_from`: a system holding a ship of the asked seat (Rescue).
    Rescue,
    /// kind `strategy_card`: a card for the trade good of Manipulate Investments; `step` of five, `owed` more distinct cards.
    Invest { step: u8, owed: u8 },
    /// kind `repeal`: a law in play (Repeal Law).
    Law,
    /// kind `fragment`: a relic fragment of the asked seat's neighbour (Seize Artifact).
    Fragment,
    /// kind `reparations_exhaust`: the planet the new controller exhausts (asked of the new controller).
    Exhaust,
    /// kind `reparations_ready`: the planet the holder readies.
    Ready,
    /// kind `planet`, the id the planet alone: the deck of a planet the holder controls (Archaeological Expedition).
    Dig,
    /// kind `planet`, label `planet in system`: a planet of the other seat the holder offers coexistence on (Exchange Program).
    Offer,
    /// kind `answer`: the other seat accepts the exchange or refuses.
    Accept,
    /// kind `ship`: the ship Lucky Shot destroys.
    Hull,
    /// kind `ship`: a ship Scuttle returns; the second question (`second`) also offers to stop after one.
    Scuttle { second: bool },
    /// kind `crashlanding_ground` (id `ground|<unit>`) and then `crashlanding_planet` (id `planet|<planet>`): the ground force and the
    /// planet Crash Landing lands (`second` for the planet).
    Crash { system: System, second: bool },
    /// kind `count`, label `spend <n> trade goods`: how many trade goods Bribery spends.
    Spend,
    /// kind `action_card`, label `take <card> from <seat>`, detail `from`: the card Lie in Wait takes from `from`.
    Lie { from: Seat },
    /// kind `technology`, label `research <id>`: the technology Focused Research researches.
    Focus,
    /// kind `technology`, label the name: the technology Divert Funding returns.
    Return,
    /// kind `technology`, label the name: the technology Divert Funding researches.
    Funding,
    /// kind `technology`, label the name: the prototype Reveal Prototype researches.
    Prototype,
    /// kind `technology`, id `<neighbour>|<tech>`, label `<neighbour>'s <name>`: the technology Plagiarize takes.
    Steal,
    /// kind `player`, label `<seat> gains their own breakthrough`: whose breakthrough Brilliance gives.
    Gain,
    /// kind `move`, subtype `ghost_squad_move`, with a decline: a move of ground forces between the planets of `system`.
    Squad { system: System },
    /// kind `unit`, id `system|planet|index`, label `<unit> in <system>`: a ground force Decoy Operation pulls; the second
    /// question (`second`) also offers to stop after one.
    Pull { second: bool },
    /// kind `planet`, the id the planet alone: where Decoy Operation's ground forces land.
    Land,
    /// kind `infantry`, id `system|planet|index`, label `infantry in <system>`: an infantry Refit Troops replaces; the second
    /// question (`second`) also offers to stop after one.
    Troops { second: bool },
    /// kind `cruiser`, id the place in the active system, label the unit: the cruiser Upgrade replaces.
    Cruiser,
    /// kind `system`, label `jam <system>`: the system Signal Jamming closes.
    Jam,
    /// kind `player`, id the faction, label `<faction>'s command token`: whose token Signal Jamming places in `system`.
    JamWho { system: System },
    /// kind `system`, label `destroyer into <system>`: the wormhole system Ghost Ship's destroyer enters.
    Ghost,
    /// kind `planet`, label `planet in system`: where Mercenary Contract's neutral infantry land.
    Landing,
    /// kind `system`: the system a neutral destroyer enters (Pirate Contract).
    Pirate,
    /// kind `system`, label the id: the system whose frontier token Exploration Probe explores.
    Probe,
    /// kind `space dock`, label `dock in <system>`, detail `system`: the system whose space dock fires (Experimental Battlestation).
    Dock,
    /// kind `strategy card` (with the space), label `<holder>'s <name>` (a held card) or `<name>` (an unclaimed one): the strategy card
    /// whose secondary (Strategize) or primary (Overrule) ability the holder performs.
    Perform,
    /// kind `strategy_card`: the picker chooses again; `first` was taken away, `by` played the card (Public Disgrace).
    Redraft { first: crate::generated::StrategyCard, by: Seat },
}

/// Why a seat is asked to pick a card from its own hand; it decides the prompt and the subtype.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Reason {
    /// Yssaril Stall Tactics.
    StallTactics,
    /// Stall Tactics allowed by Deepgloom Executable.
    DeepgloomStallTactics,
    /// Nekro Malleon's discard.
    MalleonDiscard,
    /// The C4R discard (`ERROR_ERROR`).
    C4rDiscard,
    /// Shown to `viewer` (`show_action_card`).
    Show { viewer: Seat },
}

/// What the area asks. Parameters only; the option list is derived by `opts(state, ask)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// Pick one action card from the asked seat's own hand.
    OwnHand { reason: Reason },
    /// `action_cards::pick`: the played `card` asks the actor to pick one of several (the string side takes the only
    /// option without asking).
    Pick { card: ActionCard, what: What },
    /// `action_cards::predicted_outcome`: a Rider (or Sanction) names the outcome it predicts on the agenda in `frame`.
    Predict { card: ActionCard, frame: Frame },
    /// The reaction slot's second question, when the seat could play cards of several names into the window of
    /// `event` (`reactions::slot`: "play an action card (when X)"); the options are one card per printed name, then decline.
    PlayCard { relation: Relation, event: EventKind },
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// The first copy in hand of one printed card.
    Card(ActionCard),
    /// A seat (`What::Player`, `What::Elect`).
    Seat(Seat),
    /// A system (`What::Recall`).
    System(System),
    /// A planet and the system the string board files it under (`What::Planet`).
    Spot(System, Planet),
    /// A predicted outcome.
    Outcome(Outcome),
    /// A promissory note (`What::Note`).
    Note(NoteKey),
    /// A strategy card (`What::Invest`).
    Strat(crate::generated::StrategyCard),
    /// A law in play (`What::Law`).
    Law(crate::generated::Agenda),
    /// A ship in a system's space area (`What::Hull`, `What::Scuttle`): the system, its place in the unit list, the owner
    /// and the unit form (the label names them).
    Ship { system: System, index: u8, owner: Seat, kind: crate::generated::Unit },
    /// yes or no (`What::Accept`).
    Answer(bool),
    /// "stop after one" (`What::Scuttle { second: true }`).
    Stop,
    /// A fragment trait (`What::Fragment`).
    Fragment(crate::generated::ExploreType),
    /// A unit on a planet: the system, the planet and its place in the planet's unit list (`What::Troops`).
    Troop { system: System, planet: Planet, index: u8, kind: crate::generated::Unit },
    /// Every ground force of one type moves from a planet to another (`What::Squad`).
    Tech(crate::generated::Technology),
    /// A unit type (`What::Crash`).
    Kind(crate::generated::Unit),
    /// A number (`What::Spend`).
    Count(u8),
    Steal { from: Seat, tech: crate::generated::Technology },
    /// A strategy card another seat holds (`holder`) or nobody does (`None`, on the mat) (`What::Perform`).
    Perform { card: crate::generated::StrategyCard, holder: Option<Seat> },
    Move { from: Planet, to: Planet, kind: crate::generated::Unit, count: u8 },
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    StallTacticsDiscard,
    DeepgloomStallTacticsDiscard,
    MalleonDiscard,
    C4rDiscard,
    ShowActionCard,
    /// `<card alias>_pick_<kind>`, or `<card alias>_elect` for `What::Elect`.
    Pick { card: ActionCard, what: What },
    /// `predict_agenda_outcome`.
    PredictOutcome,
    /// `play_reaction_<rel>_<EVENT>`.
    PlayReaction { relation: Relation, event: EventKind },
}

impl Reason {
    /// The subtype the reason names.
    #[must_use]
    pub const fn subtype(self) -> Subtype {
        match self {
            Self::StallTactics => Subtype::StallTacticsDiscard,
            Self::DeepgloomStallTactics => Subtype::DeepgloomStallTacticsDiscard,
            Self::MalleonDiscard => Subtype::MalleonDiscard,
            Self::C4rDiscard => Subtype::C4rDiscard,
            Self::Show { .. } => Subtype::ShowActionCard,
        }
    }
}
