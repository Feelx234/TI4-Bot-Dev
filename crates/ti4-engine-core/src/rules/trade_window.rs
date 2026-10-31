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
//!
//! # Held on the stack (package M1, the live loop)
//!
//! [`TradeWindow`] is also a `Held` window (`Window::Trade(state::Trade)`): the live loop rebuilds it from the payload and the position at
//! every call ([`live_facts`] reads the reach, the Arbiters, the commanders and the free-form deal from the CORE) and runs it to the next
//! question. A window built that way ([`TradeWindow::is_live`]) does what the string's `Game::step_trade` does around the answer: it
//! announces `TRANSACTION_OPENED` in its first call, stages the `TRADE_GOODS_GAINED` events of an accepted deal (and Dark Pact's), emits the typed
//! `TRANSACTION_RESOLVED` (the reaction windows of the table open there) and the plain event of the outcome, and clears the Black Market mark
//! when the negotiation ended after an answer. The windows of the dual (`At`, `single`) do none of it: the string still does it for them.

use crate::choice::trade::{Ask, Opt, Shape, Subtype};
use crate::choice::{Choice, Ctx, Opt as COpt, Opts, Origin, Pick, Subtype as CSub, Target};
use crate::choice::generic::Rule;
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Held, Illegal, Runner, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::promissory as qn;
use crate::query::trade::{self as q, Illegal as Why};
use crate::query::trade_deals::{self as deals_q, DealFacts};
use crate::state::{CoreState, EventKind, Key, PlainKind, PlainRec, Trade, TransientFlags, Val, Window as OpenWindow, WindowKind};

use super::trade::{dark_pact_gains, exchange, record_resolved};

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
    /// The window is held on the stack by the live loop: it also does what the string does around an answer (see the module docs).
    live: bool,
    /// `TRANSACTION_OPENED` was announced.
    announced: bool,
    /// The position has no map: the string drops the window without a word.
    unmapped: bool,
}

impl TradeWindow {
    /// A negotiation just opened: `proposer` is to propose, with two proposals to spend.
    #[must_use]
    pub fn open(proposer: Seat, partner: Seat, facts: DealFacts) -> Self {
        Self { proposer, partner, stage: TStage::Proposing, deal: None, rounds_left: 2, facts, single: false, outcome: None, live: false, announced: true, unmapped: false }
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
            live: false,
            announced: true,
            unmapped: false,
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
        if self.unmapped {
            // `Game::step_trade` without a galaxy: the window is dropped, nothing is asked or said.
            self.stage = TStage::Done;
            return Next::Done;
        }
        if self.live && !self.announced {
            self.announced = true;
            if self.announce_opened(sx).is_err() {
                sx.unknown(crate::kernel::ability::Unknown::TurnGap(crate::kernel::turn_codes::TurnGap::EmissionRefused));
                self.stage = TStage::Done;
                return Next::Done;
            }
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
        if (self.single && self.outcome.is_some()) || self.unmapped || (self.live && !self.announced) {
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

    fn accepts_unlisted(&self, pick: &Pick) -> bool {
        // The proposer's question takes a deal the proposer names: a registered free-form one is not among the options.
        self.stage == TStage::Proposing
            && matches!(pick, Pick::Opt(COpt::Trade(Opt::Deal(Shape::Free(i)))) if self.facts.free.get(usize::from(*i)).is_some_and(Option::is_some))
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        self.answer_inner(sx, pick)?;
        if self.live {
            self.after_answer(sx);
        }
        Ok(())
    }
}

impl TradeWindow {
    fn answer_inner(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let Pick::Opt(opt) = pick else { return Err(Illegal) };
        match self.stage {
            TStage::Proposing => {
                if opt == COpt::Decline {
                    return self.end(Outcome::NothingOffered);
                }
                let COpt::Trade(Opt::Deal(shape)) = opt else { return Err(Illegal) };
                // A listed deal, or a free-form one the adapter registered (the proposer names it, it is never listed).
                let free = matches!(shape, Shape::Free(i) if self.facts.free.get(usize::from(i)).is_some_and(Option::is_some));
                if !free && !deals(sx.state, &self.facts, self.proposer, self.partner).contains(&shape) {
                    return Err(Illegal);
                }
                // Legality is checked before the partner is troubled with it.
                if let Some(why) = self.why_illegal(sx.state, shape) {
                    return self.end(Outcome::Rejected(why));
                }
                // The deal on the table is slot 0 of the free deals, whichever slot the proposer named it in.
                if let Shape::Free(i) = shape {
                    self.facts.free = [self.facts.free.get(usize::from(i)).copied().flatten(), None];
                    self.deal = Some(Shape::Free(0));
                } else {
                    self.deal = Some(shape);
                }
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
                        if self.live {
                            // `transactions::resolve`: the gains are staged (announced by the next reconcile), Dark Pact's after them. The string
                            // stages only while some seated faction has a module (`supply::staging_enabled`).
                            let (source, staging) = (sx.table.codes.turn.transaction, sx.table.facts.staging);
                            if staging {
                                fx::stage_gain(sx.state, offer.partner, offer.given.trade_goods + offer.given.commodities, source);
                                fx::stage_gain(sx.state, offer.proposer, offer.received.trade_goods + offer.received.commodities, source);
                            }
                            dark_pact_gains(sx.state, &offer, sx.table.codes.turn.dark_pact, staging);
                        }
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
                        self.facts.free = [None, None];
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

impl TradeWindow {
    /// The window as the live loop holds it: whether it announces and clears around the answers (see the module docs).
    #[must_use]
    pub const fn is_live(&self) -> bool {
        self.live
    }

    /// `Game::apply_choice`'s `emit_typed("TRANSACTION_OPENED")`: the chairs are named, either of them may hold the card played "when you
    /// are negotiating a transaction".
    fn announce_opened(&self, sx: &mut Sx<'_>) -> Result<(), crate::kernel::resolver::TimingError> {
        let payload = crate::state::Payload::new().with(Key::Player, Val::Seat(self.proposer)).with(Key::Partner, Val::Seat(self.partner));
        sx.emit_mirrored(fx::event(EventKind::TransactionOpened, payload)).map(|_| ())
    }

    /// What `Game::step_trade` does after the window's answer: the typed `TRANSACTION_RESOLVED` of a deal struck (the reaction windows of
    /// the table open there), then the plain event of the outcome.
    fn after_answer(&mut self, sx: &mut Sx<'_>) {
        let Some(outcome) = self.outcome else { return };
        if outcome == Outcome::Resolved {
            let payload = crate::state::Payload::new().with(Key::Proposer, Val::Seat(self.proposer)).with(Key::Partner, Val::Seat(self.partner));
            if sx.emit_mirrored(fx::event(EventKind::TransactionResolved, payload)).is_err() {
                sx.unknown(crate::kernel::ability::Unknown::TurnGap(crate::kernel::turn_codes::TurnGap::EmissionRefused));
                return;
            }
        }
        let kind = match outcome {
            Outcome::Resolved => PlainKind::Transaction,
            Outcome::Refused => PlainKind::TransactionRefused,
            Outcome::Offered => PlainKind::TransactionOffered,
            Outcome::Countered => PlainKind::Counteroffer,
            Outcome::Rejected(_) => PlainKind::TransactionRejected,
            Outcome::NothingOffered => PlainKind::TransactionAbandoned,
        };
        sx.plain(PlainRec::bare(kind));
    }
}

/// `transactions::may_transact` and the rest of the deal facts of a window from the CORE: the reach by the map, the Arbiters, the unlocked
/// commanders and the free-form deals of the payload, each with the facts of the position they are read against (`None`: the position has
/// no map).
#[must_use]
pub fn live_facts(core: &CoreState, k: &Kernel, t: &Trade) -> Option<DealFacts> {
    let (p, r) = (t.proposer, t.partner);
    let mut facts = super::trade::with_cx(core, k, |cx| DealFacts {
        note_order: deals_q::note_order,
        card_order: deals_q::card_order,
        unlocked: deals_q::commander_unlocked_mask(core),
        pair: (p, r),
        may: [q::may_transact(cx, p, r), q::may_transact(cx, r, p)],
        arbiters: [cx.env.trades_action_cards(p), cx.env.trades_action_cards(r)],
        planet_values: deals_q::planet_values(),
        free: [None, None],
    })?;
    for (slot, spec) in t.free.iter().enumerate() {
        facts.free[slot] = spec.map(|s| deals_q::free_deal(core, &facts, p, r, &s));
    }
    Some(facts)
}

pub use crate::query::trade::{MAX_NEGOTIATIONS_PER_ACTION, MAX_NEGOTIATIONS_PER_ROUND};

/// The payload's form of a free-form deal the window holds: the terms without the facts of the position.
fn spec_of(deal: &deals_q::FreeDeal) -> crate::state::FreeSpec {
    let terms = |t: &q::Terms| crate::state::FreeTerms {
        trade_goods: t.trade_goods,
        commodities: t.commodities,
        fragments: t.fragments,
        note: t.note.and_then(|n| n.mv).map(|m| match m {
            q::NoteMove::Held(key) => crate::state::FreeNote::Held(key),
            q::NoteMove::Support(owner) => crate::state::FreeNote::Support(owner),
        }),
        action_card: t.action_card,
        secret: t.secret,
        planet: t.planet.and_then(|p| p.mv),
    };
    crate::state::FreeSpec { given: terms(&deal.given), received: terms(&deal.received) }
}

impl Held for TradeWindow {
    const KIND: WindowKind = WindowKind::Trade;

    fn from_pending(core: &CoreState, k: &Kernel) -> Option<Self> {
        let OpenWindow::Trade(t) = core.pending().window(WindowKind::Trade)? else { return None };
        let facts = live_facts(core, k, t);
        let mut window = Self::at(t.proposer, t.partner, t.deal, t.rounds_left, facts.unwrap_or_else(|| empty_facts(t)));
        window.live = true;
        window.announced = t.announced;
        window.unmapped = facts.is_none();
        Some(window)
    }

    fn to_pending(&self, core: &mut CoreState) {
        let free = [self.facts.free[0].as_ref().map(spec_of), self.facts.free[1].as_ref().map(spec_of)];
        core.pending_mut().open(OpenWindow::Trade(Trade { proposer: self.proposer, partner: self.partner, rounds_left: self.rounds_left, deal: self.deal, free, announced: self.announced }));
    }

    /// The negotiation ended: the Black Market mark it was set for dies with it, unless the window was dropped before any answer
    /// (`Game::step_trade` clears it after an answer only).
    fn closed(&self, core: &mut CoreState, _: &Kernel) {
        if self.outcome.is_some() {
            core.transient_mut().misc.flags.clear(TransientFlags::BLACK_MARKET);
        }
    }
}

/// The facts of a window on a position without a map: nothing is reachable, nothing is asked ([`TradeWindow::unmapped`]).
fn empty_facts(t: &Trade) -> DealFacts {
    DealFacts {
        note_order: deals_q::note_order,
        card_order: deals_q::card_order,
        unlocked: 0,
        pair: (t.proposer, t.partner),
        may: [false, false],
        arbiters: [false, false],
        planet_values: deals_q::planet_values(),
        free: [None, None],
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
