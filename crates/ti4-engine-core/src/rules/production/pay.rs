//! One iteration of a free payment loop (the string `pay_with_observation_credit`) as a [`Window`]: the
//! options of the step, a lone option taken without a question, a question otherwise, and the application of
//! the answer. The caller (the string loop) repeats it until the bill is paid.
//!
//! The multi-ask conversion (`docs/MULTIASK1_RESUME.md`): the question pays the REST of the bill at once. Its options are the atoms
//! (planet faces, trade goods, commodities) and its answer a [`PaySet`] the closed form [`set_worth`] checks; a lone atom is still taken
//! without a question first (as the loop did, so the questions line up with the string's: the string asks wherever the loop had a
//! choice and pays a lone option unasked). [`apply_set`] spends a checked set. Every payment of the core goes through here or through
//! the production window, which shares [`apply_set`].

use crate::rules::production as rules;
use crate::rules::production::{Cx, Held};
use crate::choice::generic::Rule;
use crate::choice::production::{set_worth, Ask as PAsk, Currency, Opt as POpt, PayAsk, PaySet, Subtype as PSub};
use crate::choice::{Ask, Choice, Ctx, Opt, Origin, Owed, OwedKind, Pick, Subtype};
use crate::containers::SmallList;
use crate::ids::Seat;
use crate::kernel::ability::{Eff, Unknown};
use crate::kernel::fx;
use crate::kernel::resolver::TimingError;
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

    /// The answer to this step's question: only a [`PaySet`] answers it (the question pays the rest of the bill); the worth taken,
    /// `None` (nothing spent) for anything else or an illegal set.
    pub(crate) fn answer_set(&self, s: &mut CoreState, k: &Kernel, picked: POpt) -> Option<i64> {
        matches!(picked, POpt::Set(_)).then(|| self.apply(s, k, picked)).flatten()
    }

    /// Take `picked` (a lone atom, or the [`PaySet`] that answered the question); the worth taken, `None` for an answer that cannot
    /// be paid.
    pub(crate) fn apply(&self, s: &mut CoreState, k: &Kernel, picked: POpt) -> Option<i64> {
        let cx = Self::cx(s, k);
        let (ask, offered) = rules::pay_opts(&cx, self.who, self.kind, self.paid, self.owed, &Held::of(&cx, self.who).extras());
        if let POpt::Set(set) = picked {
            return pay_set(s, self.who, &ask, offered.as_slice(), set);
        }
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

/// Check `set` against the payment question (`ask`, `opts`) and spend it: the worth paid, `None` (nothing spent) when the set is no
/// legal answer ([`set_worth`]).
pub(crate) fn pay_set(s: &mut CoreState, who: Seat, ask: &PayAsk, opts: &[POpt], set: PaySet) -> Option<i64> {
    let worth = set_worth(ask, opts, set)?;
    apply_set(s, who, opts, set)?;
    Some(worth)
}

/// Spend the atoms of `set` (a set already checked against `opts`): exhaust its planets, spend its trade goods and commodities.
/// `None` (nothing spent) when the seat does not hold what the set spends.
pub(crate) fn apply_set(s: &mut CoreState, who: Seat, opts: &[POpt], set: PaySet) -> Option<()> {
    let p = s.players().by_seat(who)?;
    if i64::from(p.econ.trade_goods) < i64::from(set.goods) || i64::from(p.econ.commodities) < i64::from(set.commodities) {
        return None;
    }
    let planets: SmallList<crate::generated::Planet, 24> = set.exhausted(opts).filter_map(|o| if let POpt::Exhaust { planet, .. } = o { Some(*planet) } else { None }).collect();
    for planet in planets.as_slice() {
        fx::exhaust_planet(s, *planet);
    }
    let p = s.players_mut().by_seat_mut(who)?;
    p.econ.trade_goods -= i16::from(set.goods);
    p.econ.commodities -= i16::from(set.commodities);
    Some(())
}

/// A free payment loop run inside an ability's effect (the string `pay_seeing` of Dynamo, Salvage Operations): the steps of
/// [`PayStep`] repeated until the bill is met, a lone option taken without a question and every real choice put through the
/// effect's tape. `Ok(false)` (the state as the loop left it; the caller restores) when the bill cannot be met. Needs the
/// kernel the adapter lent for payments (`AdapterFacts::pay_kernel`); without one the window is unknown.
///
/// # Errors
/// Never; the result type is the effect's.
pub fn pay_in(eff: &mut Eff<'_, '_>, who: Seat, cost: i64, kind: Spend) -> Result<bool, TimingError> {
    let Some(k) = eff.pay_kernel() else {
        eff.unknown(Unknown::MissingFact);
        return Ok(false);
    };
    if cost <= 0 {
        return Ok(true);
    }
    {
        let s: &CoreState = eff.state();
        let cx = PayStep::cx(s, k);
        if rules::available(&cx, who, kind, &Held::of(&cx, who).extras()) < cost {
            return Ok(false);
        }
    }
    let mut step = PayStep { who, kind, cost, used_credit: 0, owed: cost, paid: 0, took: None, stage: Stage::Paying };
    while step.paid < step.owed {
        let (lone, question) = {
            let s: &CoreState = eff.state();
            let cx = PayStep::cx(s, k);
            let (_, opts) = rules::pay_opts(&cx, who, kind, step.paid, step.owed, &Held::of(&cx, who).extras());
            match opts.as_slice() {
                [] => return Ok(false),
                [only] => (Some(*only), None),
                _ => (None, step.question(s, k)),
            }
        };
        let picked = match (lone, question) {
            (Some(only), _) => {
                let s: &CoreState = eff.read().0;
                eff.host_table().note_auto_paid(s, || step.question(s, k));
                only
            }
            (None, Some(q)) => match eff.ask(&q) {
                Pick::Opt(Opt::Production(o @ POpt::Set(_))) => o,
                _ => return Ok(false),
            },
            (None, None) => return Ok(false),
        };
        let took = step.apply(eff.state(), k, picked);
        let Some(worth) = took else { return Ok(false) };
        step.paid += worth;
    }
    Ok(true)
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
                sx.table.note_auto_paid(sx.state, || self.question(sx.state, sx.kernel));
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
        // The question pays the rest of the bill: only a set answers it.
        let Pick::Opt(Opt::Production(opt @ POpt::Set(_))) = pick else { return Err(Illegal) };
        let took = self.apply(sx.state, sx.kernel, opt).ok_or(Illegal)?;
        self.took = Some(took);
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

    /// A bill that is being paid with `paid` collected already (a window rebuilt from the position between two questions): the next
    /// step is started at once, without the check that the whole bill can be met (what is left to spend is less than it was).
    #[must_use]
    pub fn resume(who: Seat, kind: Spend, cost: i64, paid: i64) -> Self {
        let mut bill = Self { who, kind, cost, paid, step: Child::idle(), stage: BillStage::Paying, outcome: None };
        bill.step.start(bill.next_step());
        bill
    }

    /// The worth collected so far.
    #[must_use]
    pub const fn paid(&self) -> i64 {
        self.paid
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
            let pick = c.default_pick().unwrap();
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

    fn pay_logged(s: &mut CoreState, cost: i64) -> (Driver<PayBill>, Vec<crate::kernel::ability::AutoPaid>, usize) {
        let k = Kernel::plain();
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut table = AbilityTable::default();
        table.auto_paid = Some(log.clone());
        let mut d = Driver::new(PayBill::new(Seat(0), Spend::Influence, cost), Resolver::new(&[Seat(0)], Some(Seat(0))), table, 1);
        let mut asked = 0;
        let mut status = d.run(s, &k).unwrap();
        while let Status::Asking(c) = status {
            asked += 1;
            let pick = c.default_pick().unwrap();
            status = d.resume(s, &k, pick).unwrap();
        }
        let entries = log.lock().unwrap().clone();
        (d, entries, asked)
    }

    /// A payment with one way left to pay is logged for the wrapper (`AbilityTable::auto_paid`): the question as it would have been put, the lone
    /// option taken, the position it was put in. Each trade good of the bill is one entry.
    #[test]
    fn a_single_way_payment_is_logged_with_its_question_and_the_position() {
        let mut s = game(5);
        let (d, entries, asked) = pay_logged(&mut s, 3);
        assert_eq!((d.window.outcome, asked), (Some(true), 0));
        assert_eq!(entries.len(), 3, "one entry per collapsed step");
        for (n, e) in entries.iter().enumerate() {
            assert_eq!(e.choice.opts.len(), 1);
            assert_eq!(e.choice.opts.as_slice()[0], e.taken);
            assert!(matches!(e.choice.ask, Ask::Production(PAsk::Pay(_))));
            assert_eq!(e.choice.actor, Seat(0));
            assert_eq!(e.at.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 5 - n as i16, "the position before the step took its good");
        }
    }

    /// A payment with a real choice is asked, not logged.
    #[test]
    fn a_payment_with_a_choice_is_asked_and_not_logged() {
        let mut s = game(5);
        let planet = crate::generated::Planet::ALL.iter().copied().find(|p| p.data().influence > 0).expect("a planet with influence");
        s.board_mut().planet_mut(planet).control = Some(Seat(0));
        let (_, entries, asked) = pay_logged(&mut s, 1);
        assert!(asked >= 1, "trade goods or the planet: a question");
        assert!(entries.is_empty());
    }

    /// Nothing owed is paid.
    #[test]
    fn a_bill_of_nothing_is_paid() {
        let mut s = game(0);
        assert_eq!(pay(&mut s, 0).window.outcome, Some(true));
    }
}

/// The multi-ask conversion's proof obligation (`docs/MULTIASK1_RESUME.md`): the ONE payment question with its closed-form check
/// ([`set_worth`]) reaches exactly the final spends the old loop reached (one atom per question, lone atoms taken unasked, the bill
/// stopping as soon as it is covered, the feasibility filter of `pay_opts`), brute-forced over small positions.
#[cfg(test)]
mod set_tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::choice::production::Shown;
    use crate::generated::{Faction, Planet};
    use crate::state::{Player, Players};

    /// What a payment spent: the planets exhausted (sorted) and the trade goods spent.
    type Spent = (Vec<Planet>, i16);

    fn planets_worth(values: &[u8], kind: Spend) -> Vec<Planet> {
        let mut used: Vec<Planet> = Vec::new();
        for v in values {
            let p = Planet::ALL
                .iter()
                .copied()
                .find(|p| {
                    let d = p.data();
                    let worth = if kind == Spend::Resources { d.resources } else { d.influence };
                    d.system.is_some() && i64::from(worth) == i64::from(*v) && !used.contains(p)
                })
                .expect("a plain planet of that worth");
            used.push(p);
        }
        used
    }

    fn position(planets: &[Planet], goods: i16) -> CoreState {
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::TheArborec)]);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.trade_goods = goods;
        for p in planets {
            s.board_mut().planet_mut(*p).control = Some(Seat(0));
        }
        s
    }

    fn spent(s: &CoreState, planets: &[Planet], goods: i16) -> Spent {
        let mut ex: Vec<Planet> = planets.iter().copied().filter(|p| crate::query::board::exhausted(s, *p)).collect();
        ex.sort();
        (ex, goods - s.players().by_seat(Seat(0)).unwrap().econ.trade_goods)
    }

    fn step(kind: Spend, cost: i64, paid: i64) -> PayStep {
        PayStep { who: Seat(0), kind, cost, used_credit: 0, owed: cost, paid, took: None, stage: Stage::Paying }
    }

    struct Case<'a> {
        k: &'a Kernel,
        kind: Spend,
        cost: i64,
        planets: &'a [Planet],
        goods: i16,
    }

    /// The old loop: every path of one-atom answers from `paid`, each final spend once the bill is covered.
    fn old_loop(c: &Case<'_>, s: &CoreState, paid: i64, out: &mut BTreeSet<Spent>) {
        if paid >= c.cost {
            out.insert(spent(s, c.planets, c.goods));
            return;
        }
        let cx = PayStep::cx(s, c.k);
        let (_, opts) = rules::pay_opts(&cx, Seat(0), c.kind, paid, c.cost, &Held::of(&cx, Seat(0)).extras());
        for o in opts.as_slice() {
            let mut next = s.clone();
            let worth = step(c.kind, c.cost, paid).apply(&mut next, c.k, *o).expect("an offered atom is paid");
            old_loop(c, &next, paid + worth, out);
        }
    }

    /// The new question: the lone atoms first (as the loop takes them), then every set of the atoms the question offers that the
    /// closed form accepts, spent.
    fn new_question(c: &Case<'_>, s: &CoreState, mut paid: i64, out: &mut BTreeSet<Spent>) {
        let mut s = s.clone();
        loop {
            if paid >= c.cost {
                out.insert(spent(&s, c.planets, c.goods));
                return;
            }
            let cx = PayStep::cx(&s, c.k);
            let (ask, opts) = rules::pay_opts(&cx, Seat(0), c.kind, paid, c.cost, &Held::of(&cx, Seat(0)).extras());
            match opts.as_slice() {
                [] => return,
                [only] => {
                    let only = *only;
                    paid += step(c.kind, c.cost, paid).apply(&mut s, c.k, only).expect("a lone atom is paid");
                }
                _ => {
                    let faces = opts.as_slice().iter().filter(|o| matches!(o, POpt::Exhaust { .. })).count();
                    // The question's own check agrees with the window's.
                    let question = step(c.kind, c.cost, paid).question(&s, c.k).expect("a question");
                    for mask in 0..(1u32 << faces) {
                        for g in 0..=u8::try_from(ask.goods).unwrap() + 1 {
                            let set = PaySet { faces: mask, goods: g, commodities: 0, buy: 0 };
                            let pick = Pick::Opt(Opt::Production(POpt::Set(set)));
                            let mut next = s.clone();
                            match step(c.kind, c.cost, paid).answer_set(&mut next, c.k, POpt::Set(set)) {
                                Some(worth) => {
                                    assert!(paid + worth >= c.cost, "a legal set covers the bill");
                                    assert!(question.accepts(&pick));
                                    out.insert(spent(&next, c.planets, c.goods));
                                }
                                None => {
                                    assert_eq!(next, s, "an illegal set spends nothing");
                                    assert!(!question.accepts(&pick));
                                }
                            }
                        }
                    }
                    // An atom alone never answers the question.
                    for o in opts.as_slice() {
                        assert!(!question.accepts(&Pick::Opt(Opt::Production(*o))));
                    }
                    return;
                }
            }
        }
    }

    #[test]
    fn the_one_payment_question_reaches_exactly_what_the_loop_reached() {
        let k = Kernel::plain();
        let mut cases = 0;
        for kind in [Spend::Resources, Spend::Influence] {
            for values in [&[1u8][..], &[2], &[1, 2], &[2, 2], &[1, 3], &[3, 1, 2], &[2, 2, 1], &[4, 1, 1], &[1, 1, 1, 2], &[3, 2, 2, 1]] {
                let planets = planets_worth(values, kind);
                for goods in 0..=3i16 {
                    let s = position(&planets, goods);
                    let total: i64 = values.iter().map(|v| i64::from(*v)).sum::<i64>() + i64::from(goods);
                    for cost in 1..=total + 1 {
                        for paid in 0..cost.min(3) {
                            let c = Case { k: &k, kind, cost, planets: &planets, goods };
                            let (mut old, mut new) = (BTreeSet::new(), BTreeSet::new());
                            old_loop(&c, &s, paid, &mut old);
                            new_question(&c, &s, paid, &mut new);
                            assert_eq!(new, old, "{kind:?} planets {values:?} goods {goods} cost {cost} paid {paid}");
                            cases += 1;
                        }
                    }
                }
            }
        }
        assert!(cases > 500, "{cases}");
    }

    /// A lead atom that can be completed always is: a planet whose weaker face is listed before its stronger one (bench seed 88, Freelancers: Moll Primus
    /// 4 resources / 1 influence, Corneeq 1 resources / 2 influence, 3 owed in resources-or-influence) must not make `pick` answer nothing.
    #[test]
    fn a_lead_atom_is_completed_with_the_better_face_of_a_planet_when_the_listed_one_falls_short() {
        let face = |planet: Planet, source: Currency, worth: i32| POpt::Exhaust { planet, source, shown: Shown { worth, pool_after: 0 } };
        let (a, b) = (Planet::ALL[1], Planet::ALL[2]);
        let opts = [face(a, Currency::Resources, 4), face(a, Currency::Influence, 1), face(b, Currency::Resources, 1), face(b, Currency::Influence, 2)];
        let ask = PayAsk { kind: Currency::Resources, owed: 3, pool: 6, goods: 0, commodities: 0 };
        // The weak face of a leads: 1 + the listed face of b (1) is short; b's other face (2) completes it.
        let set = crate::choice::production::set_from(&ask, &opts, 1).expect("completable");
        assert_eq!(set_worth(&ask, &opts, set), Some(3));
        for first in 0..opts.len() {
            let set = crate::choice::production::set_from(&ask, &opts, first).unwrap_or_else(|| panic!("lead {first}"));
            assert!(set_worth(&ask, &opts, set).is_some(), "lead {first}: legal");
        }
    }

    /// The closed form on the numbers alone (owed 3; faces a 3, b 1, c 2; one trade good): the set covers the bill, and without its
    /// largest atom it would not.
    #[test]
    fn the_closed_form_wants_the_bill_covered_and_the_largest_atom_needed() {
        let face = |planet: Planet, worth: i32| POpt::Exhaust { planet, source: Currency::Resources, shown: Shown { worth, pool_after: 0 } };
        let (a, b, c) = (Planet::ALL[1], Planet::ALL[2], Planet::ALL[3]);
        let opts = [face(a, 3), face(b, 1), face(c, 2), POpt::TradeGood { shown: Shown { worth: 1, pool_after: 0 } }];
        let ask = PayAsk { kind: Currency::Resources, owed: 3, pool: 7, goods: 1, commodities: 0 };
        let set = |faces: u32, goods: u8| PaySet { faces, goods, commodities: 0, buy: 0 };
        assert_eq!(set_worth(&ask, &opts, set(0b001, 0)), Some(3), "a alone");
        assert_eq!(set_worth(&ask, &opts, set(0b110, 0)), Some(3), "b and c");
        assert_eq!(set_worth(&ask, &opts, set(0b101, 0)), Some(5), "c, then a: legal");
        assert_eq!(set_worth(&ask, &opts, set(0b011, 0)), Some(4), "b, then a: legal");
        assert_eq!(set_worth(&ask, &opts, set(0b111, 0)), None, "b and c already cover it: a is never asked for");
        assert_eq!(set_worth(&ask, &opts, set(0b010, 1)), None, "b and a trade good: short");
        assert_eq!(set_worth(&ask, &opts, set(0b110, 1)), Some(4), "b, the good, then c");
        assert_eq!(set_worth(&ask, &opts, set(0b000, 2)), None, "more goods than held");
        assert_eq!(set_worth(&ask, &opts, set(0b1000, 0)), None, "a bit past the faces");
        assert_eq!(set_worth(&ask, &opts, PaySet { buy: 1, ..set(0b001, 0) }), None, "a purchase count belongs to Leadership's question");
    }

    /// A whole bill through the window: one question, answered by a set; an atom or an illegal set is refused and changes nothing.
    #[test]
    fn a_bill_with_a_choice_is_one_question_answered_by_a_set() {
        use crate::driver::{Driver, Status};
        use crate::kernel::stage::AbilityTable;
        use crate::kernel::Resolver;
        let k = Kernel::plain();
        let planets = planets_worth(&[2, 2, 1], Spend::Resources);
        let mut s = position(&planets, 1);
        let mut d = Driver::new(PayBill::new(Seat(0), Spend::Resources, 3), Resolver::new(&[Seat(0)], Some(Seat(0))), AbilityTable::default(), 1);
        let Status::Asking(c) = d.run(&mut s, &k).unwrap() else { panic!("a question") };
        assert!(c.is_multi_pick());
        let before = s.clone();
        let atom = Pick::Opt(c.opts.as_slice()[0]);
        assert!(!c.accepts(&atom));
        assert!(d.resume(&mut s, &k, atom).is_err(), "an atom alone is no answer");
        let illegal = Pick::Opt(Opt::Production(POpt::Set(PaySet { faces: 0b111, goods: 0, commodities: 0, buy: 0 })));
        assert!(d.resume(&mut s, &k, illegal).is_err(), "all three planets: more than the bill needs");
        assert_eq!(s, before);
        let legal = Pick::Opt(Opt::Production(POpt::Set(PaySet { faces: 0b100, goods: 0, commodities: 0, buy: 0 })));
        assert!(!c.accepts(&legal), "the one-worth planet alone is short");
        let legal = Pick::Opt(Opt::Production(POpt::Set(PaySet { faces: 0b101, goods: 0, commodities: 0, buy: 0 })));
        assert!(c.accepts(&legal));
        assert_eq!(d.resume(&mut s, &k, legal).unwrap(), Status::Done);
        assert_eq!(d.window.outcome, Some(true));
        assert_eq!(spent(&s, &planets, 1).0.len(), 2);
    }
}
