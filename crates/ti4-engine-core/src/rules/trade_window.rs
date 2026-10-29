//! The transaction window (`transactions::TradeWindow`, LRR 94.1a) as a [`Window`]: the proposer puts a deal on the
//! table, the partner accepts, refuses or counters, and a counter is the same negotiation from the other chair.
//! Package B4c.
//!
//! * Stages: [`TStage::Proposing`] (the proposer picks one of [`deals`], or declines), [`TStage::Answering`] (the
//!   partner answers the deal on the table), [`TStage::Done`].
//! * Haggling is bounded: two proposals (an offer and one counter), the third answer that is a counter ends the
//!   negotiation.
//! * An accepted deal is [`exchange`]d, both sides taken before either is given, and recorded as this round's
//!   (Lie in Wait counts them). What stays with the string engine: the staged `TRADE_GOODS_GAINED` events of
//!   both sides and Dark Pact's gains (staged events and a faction module), and the `TRANSACTION_RESOLVED` event
//!   (it opens the reaction windows of the table).
//!
//! The facts that are not in the typed state come with the window ([`DealFacts`]).

use crate::choice::trade::{Ask, Opt, Shape, Subtype};
use crate::choice::{Choice, Ctx, Opt as COpt, Opts, Origin, Pick, Subtype as CSub, Target};
use crate::choice::generic::Rule;
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::promissory as qn;
use crate::query::trade::{self as q, Illegal as Why};
use crate::query::trade_deals::{self as deals_q, DealFacts};
use crate::state::CoreState;

use super::trade::{exchange, record_resolved};

pub use crate::query::trade_deals::deals;

/// The suspension points of [`TradeWindow`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TStage {
    Proposing,
    Answering,
    Done,
}

/// How the last answer went (`transactions::Traded`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The deal was struck.
    Resolved,
    /// The proposer chose to offer nothing, or the haggling ran out.
    NothingOffered,
    /// The partner refused.
    Refused,
    /// A deal was put on the table and awaits an answer.
    Offered,
    /// The partner countered; negotiations continue from the other chair.
    Countered,
    /// The deal was not legal (or stopped being so).
    Rejected(Why),
}

/// An open negotiation between two seats.
#[derive(Debug, Clone, PartialEq)]
pub struct TradeWindow {
    pub proposer: Seat,
    pub partner: Seat,
    stage: TStage,
    /// The deal on the table while [`TStage::Answering`].
    deal: Option<Shape>,
    /// Proposals still to be answered.
    rounds_left: u8,
    facts: DealFacts,
    /// The window ends after one answer (the adapter asks the string's one step at a time); the whole
    /// negotiation runs when unset.
    single: bool,
    /// The outcome of the last answer.
    pub outcome: Option<Outcome>,
}

impl TradeWindow {
    /// A negotiation just opened: `proposer` is to propose, with two proposals to spend.
    #[must_use]
    pub fn open(proposer: Seat, partner: Seat, facts: DealFacts) -> Self {
        Self { proposer, partner, stage: TStage::Proposing, deal: None, rounds_left: 2, facts, single: false, outcome: None }
    }

    /// A negotiation at a position the string engine is in: waiting for a proposal, or for the answer to `deal`.
    #[must_use]
    pub fn at(proposer: Seat, partner: Seat, deal: Option<Shape>, rounds_left: u8, facts: DealFacts) -> Self {
        Self {
            proposer,
            partner,
            stage: if deal.is_some() { TStage::Answering } else { TStage::Proposing },
            deal,
            rounds_left,
            facts,
            single: false,
            outcome: None,
        }
    }

    /// Stop after the next answer (the adapter's one string step).
    #[must_use]
    pub fn single(mut self) -> Self {
        self.single = true;
        self
    }

    fn faction_of(core: &CoreState, seat: Seat) -> Option<crate::generated::Faction> {
        qn::faction_of(core, seat)
    }

    /// The deal of `shape` between the two chairs, as the legality check reads it.
    fn offer(&self, core: &CoreState, shape: Shape) -> q::Offer {
        deals_q::offer_of(core, &self.facts, shape, self.proposer, self.partner)
    }

    /// `transactions::why_illegal` for this window's deal: the reach, the table's action-card door and the
    /// note legs come from the facts and the typed state.
    fn why_illegal(&self, core: &CoreState, shape: Shape) -> Option<Why> {
        let offer = self.offer(core, shape);
        let reach = self.facts.may_from(self.proposer);
        let arbiters = |s: Seat| self.facts.arbiter(s);
        q::why_illegal_with(core, || reach, || core.transient().misc.flags.has(crate::state::TransientFlags::BLACK_MARKET), arbiters, &offer)
    }
}

impl Window for TradeWindow {
    type Stage = TStage;

    fn stage(&self) -> TStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.single && self.outcome.is_some() {
            return Next::Done;
        }
        match self.stage {
            TStage::Proposing => {
                if deals(sx.state, &self.facts, self.proposer, self.partner).is_empty() {
                    // The string asks nothing and drops the window.
                    self.stage = TStage::Done;
                    return Next::Done;
                }
                let Some(name) = Self::faction_of(sx.state, self.partner) else {
                    self.stage = TStage::Done;
                    return Next::Done;
                };
                Next::Continue(crate::choice::Ask::Trade(Ask::Propose { proposer: self.proposer, partner: self.partner, name }))
            }
            TStage::Answering => {
                let (Some(deal), Some(from)) = (self.deal, Self::faction_of(sx.state, self.proposer)) else {
                    self.stage = TStage::Done;
                    return Next::Done;
                };
                Next::Continue(crate::choice::Ask::Trade(Ask::Answer { responder: self.partner, proposer: self.proposer, from, deal }))
            }
            TStage::Done => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        if self.single && self.outcome.is_some() {
            return None;
        }
        match self.stage {
            TStage::Proposing => {
                let shapes = deals(s, &self.facts, self.proposer, self.partner);
                if shapes.is_empty() {
                    return None;
                }
                let name = Self::faction_of(s, self.partner)?;
                let mut opts: Opts = shapes.into_iter().map(|d| COpt::Trade(Opt::Deal(d))).collect();
                opts.push(COpt::Decline);
                Some(Choice {
                    actor: self.proposer,
                    ask: crate::choice::Ask::Trade(Ask::Propose { proposer: self.proposer, partner: self.partner, name }),
                    ctx: ctx(Subtype::Propose, self.partner),
                    opts,
                })
            }
            TStage::Answering => {
                let deal = self.deal?;
                let from = Self::faction_of(s, self.proposer)?;
                let opts: Opts = [Opt::Accept, Opt::Refuse, Opt::Counter].into_iter().map(COpt::Trade).collect();
                Some(Choice {
                    actor: self.partner,
                    ask: crate::choice::Ask::Trade(Ask::Answer { responder: self.partner, proposer: self.proposer, from, deal }),
                    ctx: ctx(Subtype::Answer, self.proposer),
                    opts,
                })
            }
            TStage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let Pick::Opt(opt) = pick else { return Err(Illegal) };
        match self.stage {
            TStage::Proposing => {
                if opt == COpt::Decline {
                    return self.end(Outcome::NothingOffered);
                }
                let COpt::Trade(Opt::Deal(shape)) = opt else { return Err(Illegal) };
                if !deals(sx.state, &self.facts, self.proposer, self.partner).contains(&shape) {
                    return Err(Illegal);
                }
                // Legality is checked before the partner is troubled with it.
                if let Some(why) = self.why_illegal(sx.state, shape) {
                    return self.end(Outcome::Rejected(why));
                }
                self.deal = Some(shape);
                self.stage = TStage::Answering;
                self.outcome = Some(Outcome::Offered);
                Ok(())
            }
            TStage::Answering => {
                let deal = self.deal.ok_or(Illegal)?;
                match opt {
                    COpt::Trade(Opt::Refuse) => self.end(Outcome::Refused),
                    COpt::Trade(Opt::Accept) => {
                        if let Some(why) = self.why_illegal(sx.state, deal) {
                            return self.end(Outcome::Rejected(why));
                        }
                        let offer = self.offer(sx.state, deal);
                        // A Support leg of a faction two seats play: the string decides (the adapter keeps such
                        // a table on the string).
                        exchange(sx.state, &offer).ok_or(Illegal)?;
                        record_resolved(sx.state, self.proposer, self.partner);
                        self.end(Outcome::Resolved)
                    }
                    COpt::Trade(Opt::Counter) => {
                        self.rounds_left = self.rounds_left.saturating_sub(1);
                        if self.rounds_left == 0 {
                            // Haggling is bounded.
                            return self.end(Outcome::NothingOffered);
                        }
                        // The counter is the same negotiation from the other chair.
                        core::mem::swap(&mut self.proposer, &mut self.partner);
                        self.deal = None;
                        self.stage = TStage::Proposing;
                        self.outcome = Some(Outcome::Countered);
                        Ok(())
                    }
                    _ => Err(Illegal),
                }
            }
            TStage::Done => Err(Illegal),
        }
    }
}

impl TradeWindow {
    fn end(&mut self, outcome: Outcome) -> Result<(), Illegal> {
        self.stage = TStage::Done;
        self.deal = None;
        self.outcome = Some(outcome);
        Ok(())
    }

    /// The two chairs as they sit now: `(proposer, partner)` (a counter swaps them).
    #[must_use]
    pub const fn parties(&self) -> (Seat, Seat) {
        (self.proposer, self.partner)
    }
}

fn ctx(subtype: Subtype, other: Seat) -> Ctx {
    Ctx {
        origin: Origin::Rule(Rule::R60),
        subtype: CSub::Trade(subtype),
        optional: false,
        target: Some(Target::Seat(other)),
        outstanding: crate::containers::SmallList::new(),
        space_battle: false,
        invasion_seq: None,
    }
}
