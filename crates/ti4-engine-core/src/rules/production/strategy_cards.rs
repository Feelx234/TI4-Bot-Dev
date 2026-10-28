//! The strategy card abilities that pay or place through the production rules (package B8; string
//! `strategy_cards`): Leadership (the free tokens and the purchase loop over [`pay::PayStep`]) and
//! Construction (PDS and space dock placement). They live in the production area's subtree because the layer
//! rule keeps one L4 area from importing another; the rest of the strategy card windows are in
//! [`super::strategy_primaries`].

use crate::choice::generic::{Content, Rule};
use crate::choice::strategy::{gain_pool_choice, rule_ctx, Ask as SAsk, Opt as SOpt, Subtype as SSub};
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::{Unit, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::economy::Spend;
use crate::query::{board, reach};
use crate::state::{Code, CoreState};
use crate::tables::Scope;

use crate::rules::production as prod;
use super::pay::{PayStep, Stage as PayStage};
use super::{Cx as ProdCx, Extras};

fn ask(a: SAsk) -> Ask {
    Ask::Strategy(a)
}

fn opts(it: impl IntoIterator<Item = SOpt>) -> Opts {
    it.into_iter().map(Opt::Strategy).collect()
}

// -- Leadership ------------------------------------------------------------------------------------------

/// Influence one command token costs (`strategy_cards::INFLUENCE_PER_TOKEN`).
pub const INFLUENCE_PER_TOKEN: i64 = 3;
/// Tokens the Leadership primary gives.
pub const LEADERSHIP_TOKENS: u8 = 3;

/// The suspension points of [`Leadership`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LStage {
    Start,
    /// The free tokens of the primary.
    Gain,
    /// Waiting on "spend 3 influence for a command token".
    Offer,
    /// Paying the three influence, one face at a time ([`PayStep`]).
    Pay,
    /// The pool of the token just paid for.
    GainOne,
    Done,
}

/// Leadership: the primary (three command tokens, then as many bought for three influence each as the holder
/// wants and can pay) and the secondary (the same purchase loop, the first purchase already accepted by the
/// follower window). The Keleres agent's offer and a payment the production rules do not follow stay with the
/// string caller (the adapter bails).
#[derive(Debug, Clone)]
pub struct Leadership {
    pub who: Seat,
    primary: bool,
    stage: LStage,
    /// Free tokens still to place.
    left: u8,
    /// Influence carried over from an overpayment (`pay_seeing_with_credit`'s credit).
    credit: i64,
    pay: Option<PayStep>,
}

impl Leadership {
    #[must_use]
    pub const fn new(who: Seat, primary: bool) -> Self {
        Self { who, primary, stage: LStage::Start, left: 0, credit: 0, pay: None }
    }

    fn cx<'a>(s: &'a CoreState, k: &'a Kernel) -> ProdCx<'a> {
        ProdCx::new(s, k, Scope(u32::from(k.sources.0)), reach::blanked(s))
    }

    /// `leadership_influence_eligible_with_credit`.
    fn eligible(&self, s: &CoreState, k: &Kernel) -> bool {
        prod::available(&Self::cx(s, k), self.who, Spend::Influence, &Extras::default()) + self.credit >= INFLUENCE_PER_TOKEN
    }

    /// The opening of `pay_influence_and_gain_one`: the first payment step, or the end of the loop when the
    /// bill cannot be met.
    fn begin_pay(&mut self, s: &CoreState, k: &Kernel) {
        let used = self.credit.min(INFLUENCE_PER_TOKEN.max(0));
        let owed = INFLUENCE_PER_TOKEN - used;
        if owed <= 0 {
            self.credit -= used;
            self.free_one();
        } else if prod::available(&Self::cx(s, k), self.who, Spend::Influence, &Extras::default()) < owed {
            self.stage = LStage::Done;
        } else {
            self.pay = Some(self.pay_step(used, owed, 0));
            self.stage = LStage::Pay;
        }
    }

    fn pay_step(&self, used_credit: i64, owed: i64, paid: i64) -> PayStep {
        PayStep {
            who: self.who,
            kind: Spend::Influence,
            cost: INFLUENCE_PER_TOKEN,
            used_credit,
            owed,
            paid,
            took: None,
            stage: PayStage::Paying,
        }
    }

    /// The token is paid for: its pool is asked.
    fn free_one(&mut self) {
        self.left = 1;
        self.stage = LStage::GainOne;
    }

    /// One payment step ended: collect its worth, pay on or finish the purchase.
    fn after_step(&mut self) {
        let Some(step) = self.pay.take() else {
            self.stage = LStage::Done;
            return;
        };
        let Some(worth) = step.took else {
            // A payment that collected nothing ends the whole purchase loop.
            self.stage = LStage::Done;
            return;
        };
        let paid = step.paid + worth;
        if paid < step.owed {
            self.pay = Some(self.pay_step(step.used_credit, step.owed, paid));
        } else {
            self.credit = self.credit - step.used_credit + paid - step.owed;
            self.free_one();
        }
    }

    fn buy_choice(&self) -> Choice {
        let owed = (INFLUENCE_PER_TOKEN - self.credit).max(0);
        Choice {
            actor: self.who,
            ask: ask(SAsk::LeadershipBuy { owed: u8::try_from(owed).unwrap_or(0), more: self.credit > 0 }),
            ctx: rule_ctx(Rule::R52_3, SSub::BuyTokenWithInfluence),
            opts: opts([SOpt::Buy(false), SOpt::Buy(true)]),
        }
    }
}

impl Window for Leadership {
    type Stage = LStage;

    fn stage(&self) -> LStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                LStage::Start => {
                    if self.primary {
                        self.left = LEADERSHIP_TOKENS;
                        self.stage = LStage::Gain;
                    } else {
                        self.begin_pay(sx.state, sx.kernel);
                    }
                }
                LStage::Gain | LStage::GainOne => return Next::Continue(ask(SAsk::GainPool { left: self.left })),
                LStage::Offer => {
                    if !self.eligible(sx.state, sx.kernel) {
                        self.stage = LStage::Done;
                    } else {
                        return Next::Continue(self.buy_choice().ask);
                    }
                }
                LStage::Pay => {
                    let Some(step) = self.pay.as_mut() else {
                        self.stage = LStage::Done;
                        continue;
                    };
                    match step.step(sx) {
                        Next::Done => self.after_step(),
                        other => return other,
                    }
                }
                LStage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        match self.stage {
            LStage::Gain | LStage::GainOne => Some(gain_pool_choice(self.who, self.left)),
            LStage::Offer => Some(self.buy_choice()),
            LStage::Pay => self.pay.as_ref()?.ask_of(s, k),
            LStage::Start | LStage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage == LStage::Pay {
            return self.pay.as_mut().ok_or(Illegal)?.answer(sx, pick);
        }
        let Pick::Opt(Opt::Strategy(chosen)) = pick else { return Err(Illegal) };
        match (self.stage, chosen) {
            (LStage::Gain | LStage::GainOne, SOpt::Pool(pool)) => {
                fx::gain_token(sx.state, self.who, pool, 1);
                self.left -= 1;
                if self.left == 0 {
                    self.stage = LStage::Offer;
                }
            }
            (LStage::Offer, SOpt::Buy(true)) => self.begin_pay(sx.state, sx.kernel),
            (LStage::Offer, SOpt::Buy(false)) => self.stage = LStage::Done,
            _ => return Err(Illegal),
        }
        Ok(())
    }
}

// -- Construction ----------------------------------------------------------------------------------------

/// The unit a seat places for one structure kind and whether it goes in the space area
/// (`strategy_cards::construction_placement`), as the caller resolved it from the unit tables; `None`: the
/// kind may not be placed at all (a faction effect forbids it).
pub type Placement = Option<(Unit, bool)>;

/// What a [`Construction`] window needs of the position that the core does not hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructureFacts {
    /// The PDS the seat places.
    pub pds: Placement,
    /// The space dock the seat places.
    pub dock: Placement,
    /// The code of the source `place_structure` (the adapter's text table).
    pub source: Code,
}

/// The suspension points of [`Construction`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CStage {
    Start,
    /// The first (or only) structure.
    First,
    /// The primary's second structure (a PDS).
    Second,
    Done,
}

/// Construction: the primary (a PDS or a space dock on a planet the seat controls, then a PDS) and the
/// secondary (one PDS or space dock). The Titans' alternative to a PDS, the production Minister of Industry
/// grants after a space dock and a unit form the unit tables do not name stay with the string caller (the
/// adapter bails).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Construction {
    pub who: Seat,
    primary: bool,
    facts: StructureFacts,
    stage: CStage,
}

impl Construction {
    #[must_use]
    pub const fn new(who: Seat, primary: bool, facts: StructureFacts) -> Self {
        Self { who, primary, facts, stage: CStage::Start }
    }

    fn only_pds(&self) -> bool {
        self.stage == CStage::Second
    }

    fn step_no(&self) -> u8 {
        match (self.primary, self.stage) {
            (false, _) => 0,
            (true, CStage::Second) => 2,
            (true, _) => 1,
        }
    }

    fn placement(&self, pds: bool) -> Placement {
        if pds { self.facts.pds } else { self.facts.dock }
    }

    fn cx<'a>(s: &'a CoreState, k: &'a Kernel) -> ProdCx<'a> {
        ProdCx::new(s, k, Scope(u32::from(k.sources.0)), reach::blanked(s))
    }

    /// `strategy_cards::structure_options`.
    fn options(&self, s: &CoreState, k: &Kernel) -> SmallList<SOpt, 16> {
        let cx = Self::cx(s, k);
        let mut out = SmallList::new();
        for (system, planet) in board::controlled_planets(s, self.who).as_slice() {
            if crate::query::objectives::is_space_station(*planet) {
                continue;
            }
            for pds in [true, false] {
                if self.only_pds() && !pds {
                    continue;
                }
                let Some((unit, in_space)) = self.placement(pds) else { continue };
                let base = if pds { UnitBaseType::Pds } else { UnitBaseType::Spacedock };
                if (in_space || prod::structure_allowed(&cx, self.who, *planet, base))
                    && crate::query::fleet::remaining(s, cx.scope, self.who, unit, 0) >= 1
                {
                    out.push(SOpt::Structure { pds, system: *system, planet: *planet });
                }
            }
        }
        out
    }

    fn ask_now(&self) -> Option<SAsk> {
        matches!(self.stage, CStage::First | CStage::Second).then_some(SAsk::PlaceStructure { step: self.step_no(), only_pds: self.only_pds() })
    }

    /// The step after the one just done.
    fn advance(&mut self) {
        self.stage = if self.primary && self.stage == CStage::First { CStage::Second } else { CStage::Done };
    }
}

impl Window for Construction {
    type Stage = CStage;

    fn stage(&self) -> CStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == CStage::Start {
            self.stage = CStage::First;
        }
        // A step with no structure to place asks nothing and passes on.
        while matches!(self.stage, CStage::First | CStage::Second) && self.options(sx.state, sx.kernel).is_empty() {
            self.advance();
        }
        match self.ask_now() {
            Some(a) => Next::Continue(ask(a)),
            None => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        let a = self.ask_now()?;
        let mut options = opts(self.options(s, k).as_slice().iter().copied());
        options.push(Opt::Decline);
        Some(Choice {
            actor: self.who,
            ask: ask(a),
            ctx: Ctx {
                origin: Origin::Content(Content::Other(self.facts.source)),
                subtype: Subtype::Strategy(SSub::PlaceStructure),
                optional: false,
                target: None,
                outstanding: SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts: options,
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let offered = self.ask_of(sx.state, sx.kernel).ok_or(Illegal)?;
        let Pick::Opt(chosen) = pick else { return Err(Illegal) };
        if !offered.opts.as_slice().contains(&chosen) {
            return Err(Illegal);
        }
        match chosen {
            Opt::Decline => {}
            Opt::Strategy(SOpt::Structure { pds, system, planet }) => {
                let (unit, in_space) = self.placement(pds).ok_or(Illegal)?;
                fx::place_unit(sx.state, self.who, system, (!in_space).then_some(planet), unit);
            }
            _ => return Err(Illegal),
        }
        self.advance();
        Ok(())
    }
}

