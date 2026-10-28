//! One iteration of a free payment loop (the string `pay_with_observation_credit`) as a [`Window`]: the
//! options of the step, a lone option taken without a question, a question otherwise, and the application of
//! the answer. The caller (the string loop) repeats it until the bill is paid.

use crate::rules::production as rules;
use crate::rules::production::{Cx, Extras};
use crate::choice::generic::Rule;
use crate::choice::production::{Ask as PAsk, Currency, Opt as POpt, Subtype as PSub};
use crate::choice::{Ask, Choice, Ctx, Opt, Origin, Owed, OwedKind, Pick, Subtype};
use crate::containers::SmallList;
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
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
    fn cx<'a>(s: &'a CoreState, k: &'a Kernel) -> Cx<'a> {
        Cx::new(s, k, Scope(u32::from(k.sources.0)), reach::blanked(s))
    }

    fn question(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        if self.stage == Stage::Done {
            return None;
        }
        let (ask, opts) = rules::pay_opts(&Self::cx(s, k), self.who, self.kind, self.paid, self.owed, &Extras::default());
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
        Some(Choice {
            actor: self.who,
            ask: Ask::Production(PAsk::Pay(ask)),
            ctx: Ctx {
                origin: Origin::Rule(Rule::R34_3_75_2_75_3),
                subtype: Subtype::Production(PSub::Pay(currency)),
                optional: false,
                target: None,
                outstanding,
                space_battle: false,
                invasion_seq: None,
            },
            opts: opts.as_slice().iter().map(|o| Opt::Production(*o)).collect(),
        })
    }

    /// Take `picked`; the worth of the face, `None` for one that cannot be paid.
    fn apply(&self, s: &mut CoreState, k: &Kernel, picked: POpt) -> Option<i64> {
        let (_, offered) = rules::pay_opts(&Self::cx(s, k), self.who, self.kind, self.paid, self.owed, &Extras::default());
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
        let (_, opts) = rules::pay_opts(&Self::cx(sx.state, sx.kernel), self.who, self.kind, self.paid, self.owed, &Extras::default());
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
