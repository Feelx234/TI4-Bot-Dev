//! The data of one exploration in progress (package X, `rules::explore_windows::Exploring`): everything the window keeps between two
//! questions, without text and without a window object, so a machine that holds an exploration (the invasion's first claim) can store
//! it in its payload and a checkpoint stays a plain clone. The logic is `rules::explore_windows`; this is its memory.

use crate::choice::explore::{Opt as EOpt, PayReward, Step};
use crate::containers::SmallList;
use crate::generated::{Explore, ExploreType, Planet, System, Wormhole};
use crate::ids::Seat;

/// What a seat holds that an exploration card can change (`exploration::Holdings`); compared before and after a card to say
/// what it did. The counts of the seat's own infantry and mechs are those on `planet`, by base type (the string engine reads
/// `infantry` / `mech` in the unit-type id; `scratch` showed them equal on the corpus).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Holdings {
    pub trade_goods: i32,
    pub commodities: i32,
    pub command_tokens: i32,
    pub action_cards: usize,
    pub secrets: usize,
    pub relics: usize,
    pub technologies: usize,
    pub play_area: usize,
    pub own_infantry: usize,
    pub own_mechs: usize,
    pub exhausted: bool,
}

/// What is explored: a planet (its deck is the seat's choice or given) or a frontier token's system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ETarget {
    Planet(Planet),
    Frontier(System),
}

/// How the deck is known: asked of the seat among the traits of the planet now (`exploration::choose_deck`), or given by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EDeck {
    Choose,
    Fixed(ExploreType),
}

/// The card arm that is waiting on a child (what to do with the child's answer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EThen {
    /// Command tokens (and then `goods` trade goods) of an entropic field or a Keleres ship.
    Tokens,
    Warehouses,
    Merchant,
    FunctioningBase,
    Fabricators,
    Mercenaries,
    /// A mech or an infantry pays: the reward after it (`PayReward`).
    Paying(PayReward),
    /// The command token of a Volatile Fuel Source after the payment.
    PaidToken,
    /// Lost Crew: two action cards.
    LostCrew,
    /// Derelict Vessel: a secret objective and the hand limit.
    Derelict,
    /// Functioning Base's action card.
    BaseCard,
}

/// The suspension points of an exploration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EStage {
    Start,
    /// The deck among the planet's traits is asked.
    Deck,
    /// Distant Suns: which of the two cards.
    Suns,
    /// A command token goes into a pool of the seat's choice.
    Pooling,
    /// A reward question of an instant card.
    Rewarding(EThen),
    /// A mech or an infantry pays for the card.
    Paying(EThen),
    /// The ion storm's faceup side.
    Ion,
    /// A window of another area runs as a child (the draw of action cards, the secret objective's hand limit).
    Linked(EThen),
    /// Freelancers: one unit is produced (`ExploreRun::one` is the window's state).
    Producing,
    Done,
}

/// How the drawn card resolved (the words of the log row are the adapter's).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EHow {
    /// A relic fragment of this deck was gained.
    Fragment(ExploreType),
    /// The card was attached to the planet.
    Attached,
    /// The card did what it says.
    Resolved,
    /// The card had nothing to act on (the ion storm without a planet, a gamma token without an active system): nothing happened.
    Unresolved,
    /// An attachment drawn with no planet to attach to.
    Discarded,
}

/// Why the exploration is not carried (a counted bail of the host).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EWhy {
    /// The table has no text for the log row or the event (the adapter did not fill `Codes::explore`).
    NoTexts,
    /// The table has no link to the window of another area the card runs (hand limit, draw), or no roster for a placed unit.
    NoLink,
    /// A window of another area asked a question and the host cannot keep it between two questions (the draw's discard, the hand limit).
    LinkedAsks,
    /// Freelancers: a unit produced with influence.
    Freelancers,
    /// Titans' Terragenesis ("after you explore a planet") is not a rule of the core table.
    Terragenesis,
    /// A replenishment the core does not follow (two seats of one faction).
    Replenish,
    /// An unsupported draw (a faction hook changing it).
    Draw,
}

/// The data of an exploration in progress.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreRun {
    pub who: Seat,
    pub target: ETarget,
    pub deck_mode: EDeck,
    /// The events of the exploration (the goods gained, `PLANET_EXPLORED`) are announced through the resolver (the string has a timing handle).
    pub announce: bool,
    pub stage: EStage,
    pub deck: Option<ExploreType>,
    pub card: Option<Explore>,
    /// The planet's traits when the deck is asked.
    pub traits: SmallList<ExploreType, 4>,
    /// The index of the log row the draw opened.
    pub row: Option<u32>,
    pub before: Holdings,
    pub how: EHow,
    /// A gamma token newly placed in this system.
    pub gamma: Option<System>,
    /// An ion storm token placed: the system and the face.
    pub ion: Option<(System, Wormhole)>,
    /// The trade goods an entropic field gives after its command tokens.
    pub goods: i32,
    pub pool_left: u8,
    /// The second card of Distant Suns.
    pub second: Option<Explore>,
    /// The options of the reward question being asked.
    pub options: SmallList<EOpt, 3>,
    /// The token was taken from the frontier before the draw (the exploration of a token spends it).
    pub take_token: bool,
    /// A frontier exploration names the system of the token: the log row records it and a Mirage drawn there is placed in it. Exploration
    /// Probe's string draws the card with no system (`explore_with(.., FRONTIER, None)`): its row has none and a Mirage has nowhere to act.
    pub named: bool,
    /// The deck ran out: nothing was explored.
    pub empty: bool,
    pub why: Option<EWhy>,
    /// The card was resolved to its end and the row closed.
    pub finished: bool,
    /// Freelancers' unit in production (`EStage::Producing`): the plain state of `ProduceOne`, kept here between two questions.
    pub one: Option<crate::state::OneRun>,
}

impl ExploreRun {
    #[must_use]
    pub fn new(who: Seat, target: ETarget, deck_mode: EDeck, announce: bool) -> Self {
        Self {
            who,
            target,
            deck_mode,
            announce,
            stage: EStage::Start,
            deck: None,
            card: None,
            traits: SmallList::new(),
            row: None,
            before: Holdings::default(),
            how: EHow::Resolved,
            gamma: None,
            ion: None,
            goods: 0,
            pool_left: 0,
            second: None,
            options: SmallList::new(),
            take_token: false,
            named: true,
            empty: false,
            why: None,
            finished: false,
            one: None,
        }
    }

    /// The reward step a card arm asks with.
    #[must_use]
    pub const fn step_of(then: EThen) -> Option<Step> {
        match then {
            EThen::Warehouses => Some(Step::AbandonedWarehouses),
            EThen::Merchant => Some(Step::MerchantStation),
            EThen::FunctioningBase => Some(Step::FunctioningBase),
            EThen::Fabricators => Some(Step::LocalFabricators),
            EThen::Mercenaries => Some(Step::MercenaryOutfit),
            _ => None,
        }
    }
}
