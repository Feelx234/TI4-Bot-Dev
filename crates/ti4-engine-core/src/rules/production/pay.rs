//! One iteration of a free payment loop (the string `pay_with_observation_credit`) as a [`Window`]: the
//! options of the step, a lone option taken without a question, a question otherwise, and the application of
//! the answer. The caller (the string loop) repeats it until the bill is paid.

use crate::rules::production as rules;
use crate::rules::production::{Cx, Held};
use crate::choice::generic::Rule;
use crate::choice::production::{Ask as PAsk, Currency, Opt as POpt, Subtype as PSub};
use crate::choice::{Ask, Choice, Ctx, Opt, Origin, Owed, OwedKind, Pick, Subtype};
use crate::containers::SmallList;
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Child, Illegal, Ran, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::economy::Spend;
use crate::query::reach;
use crate::state::CoreState;
use crate::tables::Scope;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Paying,
    Done,
}

/// One step of paying `owed` of a bill of `cost`, with `paid` already collected in this loop.
#[derive(Debug, Clone)]
pub struct PayStep {
    pub who: Seat,
    pub kind: Spend,
    pub cost: i64,
    pub used_credit: i64,
    pub owed: i64,
    pub paid: i64,
    /// What this step collected (the worth of the face taken), once it has.
    pub took: Option<i64>,
    pub stage: Stage,
}

impl PayStep {
    pub(crate) fn cx<'a>(s: &'a CoreState, k: &'a Kernel) -> Cx<'a> {
        Cx::new(s, k, Scope(u32::from(k.sources.0)), reach::blanked(s))
    }

    pub(crate) fn question(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        if self.stage == Stage::Done {
            return None;
        }
        let cx = Self::cx(s, k);
        let (ask, opts) = rules::pay_opts(&cx, self.who, self.kind, self.paid, self.owed, &Held::of(&cx, self.who).extras());
        if opts.as_slice().is_empty() {
            return None;
        }
        let owed_kind = match self.kind {
            Spend::Resources => OwedKind::Resources,
            Spend::Influence => OwedKind::Influence,
        };
        let currency = match self.kind {
            Spend::Resources => Currency::Resources,
            Spend::Influence => Currency::Influence,
        };
        let mut outstanding = SmallList::new();
        outstanding.push(Owed { kind: owed_kind, amount: self.cost, paid: self.used_credit + self.paid });
        let mut ctx = Ctx {
                origin: Origin::Rule(Rule::R34_3_75_2_75_3),
                subtype: Subtype::Production(PSub::Pay(currency)),
                optional: false,
                target: None,
                outstanding,
                space_battle: false,
                invasion_seq: None,
            };
        rules::enrich_ctx(s, &mut ctx);
        Some(Choice {
            actor: self.who,
            ask: Ask::Production(PAsk::Pay(ask)),
            ctx,
            opts: opts.as_slice().iter().map(|o| Opt::Production(*o)).collect(),
        })
    }

    /// Take `picked`; the worth of the face, `None` for one that cannot be paid.
    pub(crate) fn apply(&self, s: &mut CoreState, k: &Kernel, picked: POpt) -> Option<i64> {
        let cx = Self::cx(s, k);
        let (_, offered) = rules::pay_opts(&cx, self.who, self.kind, self.paid, self.owed, &Held::of(&cx, self.who).extras());
        let chosen = *offered.as_slice().iter().find(|o| **o == picked)?;
        match chosen {
            POpt::Exhaust { planet, shown, .. } => {
                fx::exhaust_planet(s, planet);
                Some(i64::from(shown.worth))
            }
            POpt::TradeGood { shown } => {
                let p = s.players_mut().by_seat_mut(self.who)?;
                if p.econ.trade_goods <= 0 {
                    return None;
                }
                p.econ.trade_goods -= 1;
                Some(i64::from(shown.worth))
            }
            POpt::Commodity { shown } => {
                // Keleres' agent: a commodity spent as if it were a trade good.
                s.players_mut().by_seat_mut(self.who)?.econ.commodities -= 1;
                Some(i64::from(shown.worth))
            }
            _ => None,
        }
    }
}

impl Window for PayStep {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Done {
            return Next::Done;
        }
        let cx = Self::cx(sx.state, sx.kernel);
        let (_, opts) = rules::pay_opts(&cx, self.who, self.kind, self.paid, self.owed, &Held::of(&cx, self.who).extras());
        match opts.as_slice() {
            [] => {
                self.stage = Stage::Done;
                Next::Done
            }
            [only] => {
                let only = *only;
                self.took = self.apply(sx.state, sx.kernel, only);
                self.stage = Stage::Done;
                Next::Done
            }
            _ => match self.question(sx.state, sx.kernel) {
                Some(c) => Next::Continue(c.ask),
                None => Next::Done,
            },
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        self.question(s, k)
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let Pick::Opt(Opt::Production(opt)) = pick else { return Err(Illegal) };
        self.took = self.apply(sx.state, sx.kernel, opt);
        self.stage = Stage::Done;
        Ok(())
    }
}

/// The suspension points of [`PayBill`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BillStage {
    Start,
    Paying,
    Done,
}

/// A whole bill paid with the free payment loop (the string `production::pay` without a credit): the cost is checked against
/// what the seat holds (`false` with no question when it cannot be met), then [`PayStep`]s run as a [`Child`] until the
/// worth taken covers the bill. [`Self::outcome`] is the string's `Ok(bool)`.
///
/// Not carried: the Keleres agent's offer before the payment (the caller bails while it could be offered, `Facts`).
#[derive(Debug, Clone)]
pub struct PayBill {
    pub who: Seat,
    pub kind: Spend,
    pub cost: i64,
    paid: i64,
    step: Child<PayStep>,
    stage: BillStage,
    /// Whether the bill was paid; `None` while it runs.
    pub outcome: Option<bool>,
}

impl PayBill {
    #[must_use]
    pub const fn new(who: Seat, kind: Spend, cost: i64) -> Self {
        Self { who, kind, cost, paid: 0, step: Child::idle(), stage: BillStage::Start, outcome: None }
    }

    fn next_step(&self) -> PayStep {
        PayStep { who: self.who, kind: self.kind, cost: self.cost, used_credit: 0, owed: self.cost, paid: self.paid, took: None, stage: Stage::Paying }
    }

    fn finish(&mut self, paid: bool) {
        self.outcome = Some(paid);
        self.stage = BillStage::Done;
    }
}

impl Window for PayBill {
    type Stage = BillStage;

    fn stage(&self) -> BillStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                BillStage::Start => {
                    if self.cost <= 0 {
                        self.finish(true);
                        continue;
                    }
                    let cx = PayStep::cx(sx.state, sx.kernel);
                    if rules::available(&cx, self.who, self.kind, &Held::of(&cx, self.who).extras()) < self.cost {
                        self.finish(false);
                        continue;
                    }
                    self.step.start(self.next_step());
                    self.stage = BillStage::Paying;
                }
                BillStage::Paying => match self.step.step(sx) {
                    Some(Ran::Asks(ask)) => return Next::Continue(ask),
                    Some(Ran::Pass(next)) => return next,
                    Some(Ran::Done(step)) => match step.took {
                        Some(worth) => {
                            self.paid += worth;
                            if self.paid >= self.cost {
                                self.finish(true);
                            } else {
                                self.step.start(self.next_step());
                            }
                        }
                        None => self.finish(false),
                    },
                    None => self.finish(false),
                },
                BillStage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        self.step.ask_of(s, k)
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        self.step.answer(sx, pick)
    }
}

#[cfg(test)]
mod bill_tests {
    use super::*;
    use crate::driver::{Driver, Status};
    use crate::generated::Faction;
    use crate::kernel::stage::AbilityTable;
    use crate::kernel::Resolver;
    use crate::state::{Player, Players};

    fn game(goods: i16) -> CoreState {
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::TheArborec)]);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.trade_goods = goods;
        s
    }

    fn pay(s: &mut CoreState, cost: i64) -> Driver<PayBill> {
        let k = Kernel::plain();
        let mut d = Driver::new(PayBill::new(Seat(0), Spend::Influence, cost), Resolver::new(&[Seat(0)], Some(Seat(0))), AbilityTable::default(), 1);
        let mut status = d.run(s, &k).unwrap();
        while let Status::Asking(c) = status {
            let pick = c.pick(0).unwrap();
            status = d.resume(s, &k, pick).unwrap();
        }
        d
    }

    /// Trade goods stand in for influence one at a time; a lone option is taken without a question.
    #[test]
    fn a_bill_is_paid_in_trade_goods_without_a_question_when_they_are_the_only_way() {
        let mut s = game(5);
        let d = pay(&mut s, 3);
        assert_eq!(d.window.outcome, Some(true));
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 2);
    }

    /// A bill the seat cannot meet is `false` with nothing spent and no question (the string `Ok(false)`).
    #[test]
    fn a_bill_that_cannot_be_met_changes_nothing() {
        let mut s = game(2);
        let d = pay(&mut s, 3);
        assert_eq!(d.window.outcome, Some(false));
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 2);
    }

    /// Nothing owed is paid.
    #[test]
    fn a_bill_of_nothing_is_paid() {
        let mut s = game(0);
        assert_eq!(pay(&mut s, 0).window.outcome, Some(true));
    }
}
