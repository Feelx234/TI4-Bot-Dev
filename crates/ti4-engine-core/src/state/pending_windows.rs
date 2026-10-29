//! The payloads of the windows D1a's remainder types (package D1a; docs/todo/D1b.md W5): the Winnu hero's follower
//! selection, the token gain of the status phase with the status report it carries, the vote of one agenda with the
//! agendas still to come, and an open transaction. Plain data, no rules; `rules/` owns the transitions.
//!
//! Not typed (still `Window::Untyped`, counted): the diplomacy contact window and the agenda talks (structured
//! diplomacy is being disabled at the product edge), and the tactical and aftermath windows (D1b).

use super::{ObjectiveRef, Outcome};
use crate::choice::strategy::Pool;
use crate::choice::trade::Shape;
use crate::containers::SmallList;
use crate::generated::{Agenda, Planet, StrategyCard, System};
use crate::ids::Seat;

/// Mathis Mathinus (Winnu hero): the hero's owner is asked, seat by seat clockwise, whether that seat may follow the
/// secondary of the card (`Game.leader_followers`). The string stores the rendered questions; the core stores whom it
/// still has to ask: the question for a seat is a function of the position (`factions::leader_strategy_followers`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaderFollowers {
    /// The seats still to be asked, the next first.
    pub asks: SmallList<Seat, 8>,
    /// The seats allowed to follow so far, in the order they were answered.
    pub followers: SmallList<Seat, 8>,
}

/// What the status phase changed (`status::StatusPhaseReport`): carried by the token window until the phase ends.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StatusReport {
    /// Seats owed an action card at the end of the phase (Minister of Policy).
    pub action_card_draws: SmallList<Seat, 8>,
    /// The objective revealed at step 81.2.
    pub revealed_objective: Option<ObjectiveRef>,
    /// Action cards drawn per seat, in the initiative order of step 81.3.
    pub action_cards_drawn: SmallList<(Seat, u32), 8>,
    /// Command tokens returned from systems, in board then holder order.
    pub returned_command_tokens: SmallList<(System, Seat), 16>,
    /// The seat that purged the Crown of Emphidia for a victory point.
    pub crown_of_emphidia: Option<Seat>,
    /// Planet cards readied at step 81.6.
    pub readied_planets: SmallList<Planet, 16>,
    /// Leaders readied at step 81.6, per seat.
    pub readied_leaders: SmallList<(Seat, u32), 8>,
    /// Strategy cards returned at step 81.8, in holding order.
    pub returned_strategy_cards: SmallList<(Seat, StrategyCard), 16>,
    /// The initiative order as it stood before step 81.8 returned the cards.
    pub initiative_order: SmallList<Seat, 8>,
    /// Damaged space units repaired at step 81.7.
    pub repaired_units: u32,
    /// The objective deck ran out, so the game ended before the later steps.
    pub game_ended: bool,
}

/// The command token gain of step 81.5 (`tokens::TokenGain` and the report it travels with): tokens are placed one at a
/// time, each into a chosen pool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tokens {
    /// The seat of each token still to place, the next first.
    pub pending: SmallList<Seat, 16>,
    /// The tokens placed so far, in resolution order.
    pub placed: SmallList<(Seat, Pool), 16>,
    pub report: StatusReport,
}

/// Where a vote has reached (`vote::Stage`, `rules::agenda::window::Stage`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoteStage {
    /// Asking `order[index]` which outcome to back.
    Outcome(u8),
    /// Genetic Recombination: asking its holder which outcome `order[index]` must back.
    Recombine(u8),
    /// Genetic Recombination was used on `order[index]`: back the outcome or return a fleet token.
    Tribute(u8),
    /// Asking `order[index]` which planet to exhaust for `outcome`, `votes` cast so far.
    Planets { index: u8, outcome: Outcome, votes: i64 },
    /// Asking `order[index]` how many trade goods to spend for votes.
    TradeGoods { index: u8, outcome: Outcome, votes: i64 },
    /// Asking `order[index]`, who holds a ready Predictive Intelligence, whether to cast 3 more votes.
    Predictive { index: u8, outcome: Outcome, votes: i64 },
    /// The speaker decides a tie or a silent table.
    Tiebreak,
    /// Finished, with the winning outcome if there was one.
    Done(Option<Outcome>),
}

/// One agenda's vote in progress (`vote::VoteWindow`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vote {
    /// The agenda voted on (Covert Legislation itself under Covert Legislation).
    pub agenda: Agenda,
    pub choices: SmallList<Outcome, 8>,
    /// The seats in the order the votes are asked.
    pub order: SmallList<Seat, 8>,
    pub stage: VoteStage,
    /// Who backed what, in the string's map order (the owner ids' text order, which is the seat order).
    pub ballot: SmallList<(Seat, Outcome), 8>,
    /// The votes each outcome received, in the string's map order (the outcomes' text order).
    pub counts: SmallList<(Outcome, i64), 8>,
    /// What each seat cast, in voting order, abstentions included (`None`: abstained).
    pub casts: SmallList<(Seat, Option<Outcome>, i64), 8>,
    /// Executive Order: the seat that may spend trade goods and resources as votes.
    pub spender: Option<Seat>,
    /// The voters (as indexes into `order`) already offered to Genetic Recombination's holder.
    pub recombination_offered: SmallList<u8, 8>,
    /// Genetic Recombination in force: the voter's index and the outcome they must back.
    pub obligation: Option<(u8, Outcome)>,
    /// The outcome the holder demanded of the voter answering the tribute question.
    pub demanded: Option<(u8, Outcome)>,
    /// Predictive Intelligence holders who cast the 3 extra votes, with the outcome they backed.
    pub predictive: SmallList<(Seat, Outcome), 8>,
}

/// The agenda phase's vote and the agendas still to come (`Game.voting`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Voting {
    pub vote: Vote,
    /// The agendas still to be revealed after this one, the next first.
    pub queue: SmallList<Agenda, 4>,
}

/// An open transaction (`transactions::TradeWindow`, LRR 94.1a). The terms of a deal on the table are a function of the
/// shape and the position (`query::trade_deals::offer_of`), so only the shape is kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trade {
    pub proposer: Seat,
    pub partner: Seat,
    /// Proposals still to be answered: two, an offer and one counter.
    pub rounds_left: u8,
    /// The deal on the table while the partner answers it; `None` while the proposer is choosing.
    pub deal: Option<Shape>,
}
