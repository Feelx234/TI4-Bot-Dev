//! The strategy card abilities that pay or place through the production rules (package B8; string
//! `strategy_cards`): Leadership (the free tokens and the purchase loop over [`pay::PayStep`]) and
//! Construction (PDS and space dock placement). They live in the production area's subtree because the layer
//! rule keeps one L4 area from importing another; the rest of the strategy card windows are in
//! [`super::strategy_primaries`].

use crate::choice::generic::{Content, Rule};
use crate::choice::strategy::{gain_pool_choice, rule_ctx, waiver_ctx, Ask as SAsk, Opt as SOpt, Subtype as SSub};
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::{Planet, System, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::children::{HomeFacts, Made};
use crate::kernel::fx;
use crate::kernel::stage::{Child, Illegal, Ran, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::economy::Spend;
use crate::query::{board, reach};
use crate::state::{Code, CoreState};
use crate::tables::Scope;

use crate::rules::production as prod;
use super::home::HomeProduction;
use super::pay::{PayBill, PayStep, Stage as PayStage};
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

/// What a [`Construction`] window needs of the position that the core does not hold, and the placement of one structure
/// kind. They are kernel types (an action card asks for a Construction child through the kernel and carries these facts).
pub use crate::kernel::children::{Placement, StructureFacts};

/// The suspension points of [`Construction`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CStage {
    Start,
    /// The Thunder's Edge printing: a structure or a production?
    Ability,
    /// The use of PRODUCTION the Thunder's Edge primary chose (a child window of the production area).
    Producing,
    /// The first (or only) structure.
    First,
    /// The primary's second structure (a PDS).
    Second,
    /// A PDS was picked where the seat may place something else instead (Hecatoncheires): waiting on which.
    Alternative,
    Done,
}

/// Construction: the primary (a PDS or a space dock on a planet the seat controls, then a PDS) and the
/// secondary (one PDS or space dock). The Titans' alternative to a PDS, the production Minister of Industry
/// grants after a space dock and a unit form the unit tables do not name stay with the string caller (the
/// adapter bails).
#[derive(Debug, Clone)]
pub struct Construction {
    pub who: Seat,
    primary: bool,
    facts: StructureFacts,
    stage: CStage,
    /// The Thunder's Edge primary: the interned `te4construction` and the systems that can produce.
    te: Option<(Code, Vec<System>)>,
    /// The facts of the use of PRODUCTION in each of those systems ([`Self::with_uses`]); empty: the production is not carried.
    uses: Vec<HomeFacts>,
    using: Child<HomeProduction>,
    /// The holder chose the production of the Thunder's Edge primary and the window does not carry it (no facts for the use): the window ends.
    pub produces: bool,
    /// The use of PRODUCTION the Thunder's Edge primary ran: the system and what it made.
    pub produced: Option<(System, Made)>,
    /// The use of PRODUCTION held something the core does not carry (a Self-Assembly Routines window the table has no link to).
    pub unsupported: bool,
    /// A structure was placed (the Titan Prototype reads it: "if they do not").
    pub placed: bool,
    /// The planet a PDS was picked for and the step it was picked in, until the alternative is answered.
    alt_at: Option<(System, Planet, CStage)>,
}

impl Construction {
    #[must_use]
    pub const fn new(who: Seat, primary: bool, facts: StructureFacts) -> Self {
        Self { who, primary, facts, stage: CStage::Start, te: None, uses: Vec::new(), using: Child::idle(), produces: false, produced: None, unsupported: false, placed: false, alt_at: None }
    }

    /// The primary of the Thunder's Edge Construction: one of a structure or a use of PRODUCTION (`systems` can produce,
    /// `source` is the interned `te4construction`), then another structure; neither structure is limited to a PDS. The
    /// production is not carried: choosing it sets [`Construction::produces`] and ends the window.
    #[must_use]
    pub fn new_te(who: Seat, facts: StructureFacts, source: Code, systems: &[System]) -> Self {
        Self { who, primary: true, facts, stage: CStage::Start, te: Some((source, systems.to_vec())), uses: Vec::new(), using: Child::idle(), produces: false, produced: None, unsupported: false, placed: false, alt_at: None }
    }

    /// The Thunder's Edge primary with its production carried: `uses` are the facts of the use of PRODUCTION in each system that can produce, in
    /// the order the choice offers them (`source` is the interned `te4construction`). A use is the window of the production area
    /// ([`HomeProduction`]: the Hacan hero's offer, the production, the effects at the end of the use), then the second structure.
    #[must_use]
    pub fn new_te_producing(who: Seat, facts: StructureFacts, source: Code, uses: &[HomeFacts]) -> Self {
        let systems: Vec<System> = uses.iter().map(|u| u.produce.system).collect();
        let mut window = Self::new_te(who, facts, source, &systems);
        window.uses = uses.to_vec();
        window
    }

    fn only_pds(&self) -> bool {
        self.stage == CStage::Second && self.te.is_none()
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
                    && crate::query::fleet::remaining_all(s, cx.scope, self.who, unit) >= 1
                {
                    out.push(SOpt::Structure { pds, system: *system, planet: *planet });
                }
            }
        }
        out
    }

    /// Hecatoncheires is on offer on `planet`: the seat has the alternative and both units fit in the reinforcements
    /// (`hecatoncheires_offer`: `supply::allowed` of each, effect bans having been read into the facts).
    fn alternative_offered(&self, s: &CoreState, k: &Kernel) -> bool {
        let Some((mech, infantry)) = self.facts.alt else { return false };
        let scope = Scope(u32::from(k.sources.0));
        crate::query::fleet::remaining_all(s, scope, self.who, mech) >= 1 && crate::query::fleet::remaining_all(s, scope, self.who, infantry) >= 1
    }

    fn ask_now(&self) -> Option<SAsk> {
        match self.stage {
            CStage::Alternative => self.alt_at.map(|(system, planet, _)| SAsk::PdsAlternative { system, planet }),
            CStage::Ability => Some(SAsk::ConstructionAbility),
            CStage::Producing => None,
            CStage::First | CStage::Second => Some(SAsk::PlaceStructure { step: self.step_no(), only_pds: self.only_pds() }),
            CStage::Start | CStage::Done => None,
        }
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
            self.stage = if self.te.is_some() { CStage::Ability } else { CStage::First };
        }
        if self.stage == CStage::Producing {
            match self.using.step(sx) {
                Some(Ran::Asks(ask)) => return Next::Continue(ask),
                Some(Ran::Pass(next)) => return next,
                Some(Ran::Done(used)) => {
                    self.unsupported |= used.unsupported;
                    if let Some((_, made)) = &mut self.produced {
                        *made = used.made();
                    }
                    self.stage = if self.unsupported { CStage::Done } else { CStage::Second };
                }
                None => self.stage = CStage::Second,
            }
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
        if self.stage == CStage::Producing {
            return self.using.ask_of(s, k);
        }
        let a = self.ask_now()?;
        if let (CStage::Ability, Some((source, systems))) = (self.stage, &self.te) {
            let options = opts(std::iter::once(SOpt::Build).chain(systems.iter().map(|system| SOpt::Produce(*system))));
            return Some(Choice {
                actor: self.who,
                ask: ask(a),
                ctx: Ctx {
                    origin: Origin::Content(Content::Other(*source)),
                    subtype: Subtype::Strategy(SSub::ConstructionChooseAbility),
                    optional: false,
                    target: None,
                    outstanding: SmallList::new(),
                    space_battle: false,
                    invasion_seq: None,
                },
                opts: options,
            });
        }
        if self.stage == CStage::Alternative {
            return Some(Choice {
                actor: self.who,
                ask: ask(a),
                ctx: Ctx {
                    origin: Origin::Content(Content::Other(self.facts.source)),
                    subtype: Subtype::Strategy(SSub::PlaceStructurePdsAlternative),
                    optional: false,
                    target: None,
                    outstanding: SmallList::new(),
                    space_battle: false,
                    invasion_seq: None,
                },
                opts: opts([SOpt::PlacePds, SOpt::Alternative].into_iter()),
            });
        }
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
        if self.stage == CStage::Producing {
            return self.using.answer(sx, pick);
        }
        let offered = self.ask_of(sx.state, sx.kernel).ok_or(Illegal)?;
        let Pick::Opt(chosen) = pick else { return Err(Illegal) };
        if !offered.opts.as_slice().contains(&chosen) {
            return Err(Illegal);
        }
        match chosen {
            Opt::Strategy(SOpt::Build) if self.stage == CStage::Ability => {
                self.stage = CStage::First;
                return Ok(());
            }
            Opt::Strategy(SOpt::Produce(system)) if self.stage == CStage::Ability => {
                match self.uses.iter().find(|u| u.produce.system == system) {
                    Some(facts) => {
                        self.using.start(HomeProduction::new(*facts));
                        self.produced = Some((system, Made::new()));
                        self.stage = CStage::Producing;
                    }
                    None => {
                        self.produces = true;
                        self.stage = CStage::Done;
                    }
                }
                return Ok(());
            }
            Opt::Decline if !matches!(self.stage, CStage::Ability | CStage::Alternative) => {}
            // "When you would place a PDS on a planet, you may ... instead": the alternatives are asked before anything is placed.
            Opt::Strategy(SOpt::Structure { pds: true, system, planet }) if !matches!(self.stage, CStage::Ability | CStage::Alternative) && self.alternative_offered(sx.state, sx.kernel) => {
                self.alt_at = Some((system, planet, self.stage));
                self.stage = CStage::Alternative;
                return Ok(());
            }
            Opt::Strategy(SOpt::Structure { pds, system, planet }) if !matches!(self.stage, CStage::Ability | CStage::Alternative) => {
                let (unit, in_space) = self.placement(pds).ok_or(Illegal)?;
                fx::place_unit(sx.state, self.who, system, (!in_space).then_some(planet), unit);
                self.placed = true;
            }
            Opt::Strategy(SOpt::PlacePds) if self.stage == CStage::Alternative => {
                let (system, planet, from) = self.alt_at.take().ok_or(Illegal)?;
                let (unit, in_space) = self.placement(true).ok_or(Illegal)?;
                fx::place_unit(sx.state, self.who, system, (!in_space).then_some(planet), unit);
                self.placed = true;
                self.stage = from;
            }
            Opt::Strategy(SOpt::Alternative) if self.stage == CStage::Alternative => {
                let (system, planet, from) = self.alt_at.take().ok_or(Illegal)?;
                let (mech, infantry) = self.facts.alt.ok_or(Illegal)?;
                fx::place_unit(sx.state, self.who, system, Some(planet), mech);
                fx::place_unit(sx.state, self.who, system, Some(planet), infantry);
                self.placed = true;
                self.stage = from;
            }
            _ => return Err(Illegal),
        }
        self.advance();
        Ok(())
    }
}


// -- the Titan Prototype's build ---------------------------------------------------------------------------

/// The suspension points of [`TitanBuild`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TStage {
    Start,
    /// Paying the three resources ([`PayBill`]).
    Paying,
    /// Placing the structure (the Construction secondary).
    Placing,
    Done,
}

/// `relics::titan_prototype` after the owner named the player: "that player may spend 3 resources to place a structure
/// on a planet they control. If they do not, they gain 1 trade good." The three resources are charged only once there
/// is somewhere to build (paying first and finding nowhere to put it would spend them for the consolation) and only if
/// they are held; a declined placement keeps the payment and still gives the trade good ("if they do not" covers every
/// path that does not place). The Titans' PDS alternative and Minister of Industry's production are not carried
/// (the adapter bails: `StructureFacts` is `None`). [`Self::built`] is whether a structure was placed.
#[derive(Debug, Clone)]
pub struct TitanBuild {
    pub who: Seat,
    facts: StructureFacts,
    /// The interned source `relic` of the trade good.
    goods: Code,
    stage: TStage,
    pay: Child<PayBill>,
    place: Child<Construction>,
    pub built: bool,
}

impl TitanBuild {
    /// Resources the structure costs.
    pub const COST: i64 = 3;

    #[must_use]
    pub const fn new(who: Seat, facts: StructureFacts, goods: Code) -> Self {
        Self { who, facts, goods, stage: TStage::Start, pay: Child::idle(), place: Child::idle(), built: false }
    }

    /// The consolation: one trade good (staged, with the announcement).
    fn consolation(&mut self, sx: &mut Sx<'_>) -> Next {
        fx::gain_trade_goods_staged(sx.state, self.who, 1, self.goods);
        self.stage = TStage::Done;
        Next::Done
    }
}

impl Window for TitanBuild {
    type Stage = TStage;

    fn stage(&self) -> TStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                TStage::Start => {
                    if Construction::new(self.who, false, self.facts).options(sx.state, sx.kernel).is_empty() {
                        return self.consolation(sx);
                    }
                    self.pay.start(PayBill::new(self.who, Spend::Resources, Self::COST));
                    self.stage = TStage::Paying;
                }
                TStage::Paying => match self.pay.step(sx) {
                    Some(Ran::Asks(ask)) => return Next::Continue(ask),
                    Some(Ran::Pass(next)) => return next,
                    Some(Ran::Done(bill)) if bill.outcome == Some(true) => {
                        self.place.start(Construction::new(self.who, false, self.facts));
                        self.stage = TStage::Placing;
                    }
                    Some(Ran::Done(_)) | None => return self.consolation(sx),
                },
                TStage::Placing => match self.place.step(sx) {
                    Some(Ran::Asks(ask)) => return Next::Continue(ask),
                    Some(Ran::Pass(next)) => return next,
                    Some(Ran::Done(placed)) => {
                        self.built = placed.placed;
                        if self.built {
                            self.stage = TStage::Done;
                            return Next::Done;
                        }
                        return self.consolation(sx);
                    }
                    None => return self.consolation(sx),
                },
                TStage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        match self.stage {
            TStage::Paying => self.pay.ask_of(s, k),
            TStage::Placing => self.place.ask_of(s, k),
            TStage::Start | TStage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match self.stage {
            TStage::Paying => self.pay.answer(sx, pick),
            TStage::Placing => self.place.answer(sx, pick),
            TStage::Start | TStage::Done => Err(Illegal),
        }
    }
}

// -- Technology ------------------------------------------------------------------------------------------

use crate::generated::{Faction, PromissoryNote, StrategyCard, Technology};
use crate::kernel::{fx_pay, fx_strategy};
use crate::query::tech::{self, Colours, ResearchCtx, ResearchHooks, UnitRoster};
use crate::state::NoteKey;

/// The answers of the faction hooks to the research questions, read from the kernel's hook table (package B8c-2):
/// the printed Analytical and the modules' waived prerequisites, the colours a module counts extra (Yin's commander),
/// the research waivers modules offer, and whether two resources are at hand for Inheritance Systems. Ta Zern's purge
/// has no core home: the adapter declines a position where the string says a technology is purged.
pub struct KernelHooks<'a> {
    s: &'a CoreState,
    k: &'a Kernel,
    who: Seat,
    afford_inheritance: bool,
}

impl<'a> KernelHooks<'a> {
    #[must_use]
    pub fn new(s: &'a CoreState, k: &'a Kernel, who: Seat) -> Self {
        let cx = ProdCx::new(s, k, Scope(u32::from(k.sources.0)), reach::blanked(s));
        let afford_inheritance = prod::available(&cx, who, Spend::Resources, &Extras::default()) >= tech::INHERITANCE_SYSTEMS_COST;
        Self { s, k, who, afford_inheritance }
    }
}

impl ResearchHooks for KernelHooks<'_> {
    fn purged(&self, _: Technology) -> bool {
        false
    }

    fn waived_prerequisites(&self, t: Technology) -> usize {
        // `faction_abilities::waived_prerequisites`: Analytical waives one prerequisite of a non-upgrade, plus the modules'.
        let analytical = !tech::is_unit_upgrade(t)
            && self.s.players().by_seat(self.who).is_some_and(|p| p.faction.data().abilities.contains(&crate::generated::Ability::Analytical));
        usize::from(analytical) + self.k.hooks.general.waived_prerequisites(self.s, self.k, self.who, t)
    }

    fn extra_colours(&self) -> Colours {
        let mut out = [0u16; 4];
        for extra in self.k.hooks.strategy.extra_prerequisite_colours(self.s, self.k, self.who).as_slice() {
            if let Some(i) = tech::colour_index(extra.colour) {
                out[i] += u16::try_from(extra.count).unwrap_or(u16::MAX);
            }
        }
        out
    }

    fn inheritance_affordable(&self) -> bool {
        self.afford_inheritance
    }

    fn waiver_offer(&self, t: Technology) -> bool {
        self.k.hooks.strategy.research_waiver_offer(self.s, self.k, self.who, t).is_some()
    }
}

/// `ti4_model::state::RULES_TECH_PAYMENT_CHOICE`: from it a paid research asks the researcher which planets and trade goods pay.
pub const RULES_TECH_PAYMENT_CHOICE: u32 = 6;

/// The suspension points of [`Research`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RStage {
    Start,
    /// The primary's free research.
    Free,
    /// The paid research: waiting on the technology (or the decline).
    Choose,
    /// Paying the bill, one face at a time ([`PayStep`]).
    Pay,
    /// A technology only a faction waiver opens: waiting on the waiver (or the decline).
    Waive,
    /// The waiver's payment: waiting on what pays (or the decline).
    WaivePay,
    Done,
}

/// A technology picked whose prerequisites only a faction waiver covers (`strategy_cards::resolve_research`): the waiver question under way.
#[derive(Debug, Clone, Copy)]
struct Waiving {
    tech: Technology,
    /// The waiver chosen (its place in `research_waiver_offers`), once the payment is asked.
    picked: Option<u8>,
}

/// What a chosen technology came to ([`Research::try_settle`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Settled {
    Landed,
    /// The path is not the window's (the run is the string's).
    Failed,
    /// Only a faction waiver opens it: the waiver is asked.
    Waiver,
}

/// Technology: the primary (one free research, then one for six resources) and the secondary (one for four).
///
/// The technology is chosen among what the seat can research under the faction hooks of the kernel
/// ([`KernelHooks`]); a chosen technology lands as `technology::research` lands it: the prerequisites the colours do
/// not cover are paid by Inheritance Systems (two resources by the first plan, the card exhausted) or covered by
/// exhausting specialty planets (`exhaust_specialties_for_research`: the standing waivers first, AI Development
/// Algorithm when nothing else covers it), then the card is gained, the unit forms of an upgrade change on the board,
/// the Anti-Intellectual Revolution takes a ship, and Research Agreement hands the technology to its holder. A Nekro
/// seat gains three command tokens instead (Propagation).
///
/// A technology only a faction waiver opens (Cabal's Riftmeld, Yin's Brother Omar) asks the waiver and then its payment
/// (`strategy_cards::resolve_research`); declining either puts the technology on the list the offer skips, and in the paid
/// research restores the position from before the bill (the string pays first, then asks, then `*state = before`).
///
/// Not carried (the window marks [`Research::failed`]; the adapter declines the run, one test each): a research
/// Inheritance Systems can no longer pay after the bill.
#[derive(Debug, Clone)]
pub struct Research {
    pub who: Seat,
    primary: bool,
    cost: i64,
    scope: Scope,
    roster: Option<UnitRoster>,
    stage: RStage,
    pay: Option<PayStep>,
    chosen: Option<Technology>,
    /// An answer left the plain path: the run is the string's.
    pub failed: bool,
    /// The technologies researched, in order.
    pub researched: SmallList<Technology, 2>,
    /// Technology Rider (package W4; `strategy_cards::offer_free_research`): one free research asked as the card's own question,
    /// and nothing after it (no paid second research).
    rider: bool,
    /// The technologies whose waiver was declined in this research (`declined_waivers`): the offer skips them.
    declined: SmallList<Technology, 8>,
    /// The waiver question under way.
    waiving: Option<Waiving>,
    /// The position before the bill of a paid research (`before = state.clone()`), restored when the waiver is declined.
    before: Option<Box<CoreState>>,
}

impl Research {
    /// `cost`: the resources the paid research costs (6 for the primary's second, 4 for the secondary).
    #[must_use]
    pub const fn new(who: Seat, primary: bool, cost: i64, scope: Scope) -> Self {
        Self { who, primary, cost, scope, roster: None, stage: RStage::Start, pay: None, chosen: None, failed: false, researched: SmallList::new(), rider: false, declined: SmallList::new(), waiving: None, before: None }
    }

    /// Technology Rider ("research 1 technology", `strategy_cards::offer_free_research`): the Technology primary's free research,
    /// asked with the card as the source of the question, and no paid research after it. Prerequisites apply, nothing is paid.
    #[must_use]
    pub const fn rider(who: Seat, scope: Scope) -> Self {
        let mut window = Self::new(who, true, 0, scope);
        window.rider = true;
        window
    }

    /// The faction units of the scope (`ti4_content::units::faction_unit`), for the unit forms an upgrade changes.
    #[must_use]
    pub const fn with_roster(mut self, roster: UnitRoster) -> Self {
        self.roster = Some(roster);
        self
    }

    fn cx<'a>(s: &'a CoreState, k: &'a Kernel) -> ProdCx<'a> {
        ProdCx::new(s, k, Scope(u32::from(k.sources.0)), reach::blanked(s))
    }

    fn open(&self, s: &CoreState, k: &Kernel) -> SmallList<Technology, 40> {
        tech::researchable(s, self.scope, reach::blanked(s), self.who, &KernelHooks::new(s, k, self.who)).iter().filter(|t| !self.declined.as_slice().contains(t)).collect()
    }

    fn begin_paid(&mut self, s: &CoreState, k: &Kernel) {
        let affordable = prod::available(&Self::cx(s, k), self.who, Spend::Resources, &Extras::default()) >= self.cost;
        self.stage = if affordable { RStage::Choose } else { RStage::Done };
    }

    fn pay_step(&self, paid: i64) -> PayStep {
        PayStep { who: self.who, kind: Spend::Resources, cost: self.cost, used_credit: 0, owed: self.cost, paid, took: None, stage: PayStage::Paying }
    }

    /// `technology::exhaust_specialties_for_research`: the specialty planets (and AI Development Algorithm) that pay the
    /// prerequisites the colours owned do not cover.
    fn exhaust_specialties(&self, s: &mut CoreState, k: &Kernel, t: Technology) {
        let blanked = reach::blanked(s);
        if tech::war_sun_prerequisites_waived(s, blanked, t) {
            return;
        }
        let reqs = tech::prerequisites(t);
        if reqs.iter().all(|n| *n == 0) {
            return;
        }
        let held = tech::owned_colours(s, self.who);
        let (mut waivable, aida) = {
            let hooks = KernelHooks::new(s, k, self.who);
            let mut waivable = hooks.waived_prerequisites(t);
            for colour in tech::COLOURS {
                waivable += crate::query::laws::research_team_waivers(s, blanked, self.who, colour);
            }
            waivable += tech::relic_waivers(s, self.who);
            // AI Development Algorithm: spent only when nothing else covers the prerequisites.
            let aida = tech::aida_waiver(s, self.who, t) > 0
                && ResearchCtx::of(s, self.scope, blanked, self.who, &hooks).is_some_and(|c| !c.prerequisites_met(t, &hooks, false));
            (waivable, aida)
        };
        if aida {
            waivable += 1;
            fx_strategy::exhaust_technology(s, self.who, Technology::AIDevelopmentAlgorithm);
        }
        let mut to_exhaust: SmallList<crate::generated::Planet, 8> = SmallList::new();
        for (i, colour) in tech::COLOURS.into_iter().enumerate() {
            let (need, have) = (usize::from(reqs[i]), usize::from(held[i]));
            if need <= have {
                continue;
            }
            let mut shortage = need - have;
            if waivable > 0 {
                let waived = shortage.min(waivable);
                shortage -= waived;
                waivable -= waived;
            }
            let mut exhausted_for_colour = 0;
            for (_, planet) in board::controlled_planets(s, self.who).as_slice() {
                if exhausted_for_colour >= shortage {
                    break;
                }
                if board::exhausted(s, *planet) || to_exhaust.as_slice().contains(planet) {
                    continue;
                }
                if tech::specialties_now(s, self.scope, *planet).as_slice().contains(&colour) {
                    to_exhaust.push(*planet);
                    exhausted_for_colour += 1;
                }
            }
        }
        if tech::psychoarchaeology_frees_specialties(s, self.who) {
            return;
        }
        for planet in to_exhaust.as_slice() {
            fx::exhaust_planet(s, *planet);
        }
    }

    /// `technology::complete_research`.
    fn complete(&mut self, s: &mut CoreState, k: &Kernel, t: Technology) {
        let roster = self.roster.unwrap_or_default();
        let blanked = reach::blanked(s);
        let Some(faction) = s.players().by_seat(self.who).map(|p| p.faction) else { return };
        // Propagation (Nekro): three command tokens instead, nothing is gained.
        if faction == Faction::TheNekroVirus {
            fx_strategy::note_propagation(s, self.who);
            return;
        }
        fx::grant_technology(s, self.who, t);
        self.researched.push(t);
        fx_strategy::apply_unit_upgrades(s, k, self.scope, &roster, self.who);
        fx_strategy::revolution_tax(s, self.scope, blanked, self.who);
        // Research Agreement: a technology that is not a faction technology goes to the holder, and the note goes home.
        if tech::faction_of(t).is_none() {
            let key = NoteKey { note: PromissoryNote::ResearchAgreement, faction };
            let holder = s.cards().promissory.get(&key).copied();
            if let Some(holder) = holder
                && s.players().by_seat(holder).is_some_and(|h| h.faction != faction && !h.tech.owned.contains(t))
            {
                fx::grant_technology(s, holder, t);
                fx_strategy::apply_unit_upgrades(s, k, self.scope, &roster, holder);
                fx::give_back_note(s, key);
            }
        }
    }

    /// `technology::research` for a technology just chosen (and the bill paid): the prerequisites are met or paid, then
    /// the research completes. `false`, with the run to be the string's, where the path is not carried.
    fn settle(&mut self, s: &mut CoreState, k: &Kernel, t: Technology) -> bool {
        self.try_settle(s, k, t) == Settled::Landed
    }

    /// [`Self::settle`], telling a technology only a faction waiver opens from a path that is not the window's.
    fn try_settle(&mut self, s: &mut CoreState, k: &Kernel, t: Technology) -> Settled {
        let blanked = reach::blanked(s);
        let (met, inheritance, waiver) = {
            let hooks = KernelHooks::new(s, k, self.who);
            let Some(ctx) = ResearchCtx::of(s, self.scope, blanked, self.who, &hooks) else { return Settled::Failed };
            (ctx.prerequisites_met(t, &hooks, true), tech::inheritance_ready(s, self.who, &hooks), hooks.waiver_offer(t))
        };
        if met {
            self.exhaust_specialties(s, k, t);
        } else if inheritance {
            // Ignore the prerequisites: exhaust the card and pay two resources with the first plan.
            let env = fx_pay::PayEnv { scope: self.scope, blanked };
            if !fx_pay::pay_first_plan(s, k, env, self.who, Spend::Resources, tech::INHERITANCE_SYSTEMS_COST) {
                return Settled::Failed;
            }
            fx_strategy::exhaust_technology(s, self.who, Technology::InheritanceSystems);
        } else {
            // Only a faction waiver opens it (the waiver and its payment are asked), or the two resources are gone after the bill (the string
            // restores the position): not carried.
            return if waiver { Settled::Waiver } else { Settled::Failed };
        }
        self.complete(s, k, t);
        Settled::Landed
    }

    fn land(&mut self, s: &mut CoreState, k: &Kernel, t: Technology) {
        match self.try_settle(s, k, t) {
            Settled::Landed => {}
            Settled::Failed => self.failed = true,
            Settled::Waiver => return self.start_waiver(s, k, t),
        }
        self.stage = RStage::Done;
    }

    /// `strategy_cards::resolve_research`: the waiver is asked (more than two modules' waivers cannot be put in the question: the run is the
    /// string's).
    fn start_waiver(&mut self, s: &CoreState, k: &Kernel, t: Technology) {
        if k.hooks.strategy.research_waiver_offers(s, k, self.who, t).len() > 2 {
            self.failed = true;
            self.stage = RStage::Done;
            return;
        }
        self.waiving = Some(Waiving { tech: t, picked: None });
        self.stage = RStage::Waive;
    }

    /// The waiver or its payment is declined (or fails): nothing is researched; in the paid research the position from before the bill is back;
    /// the technology is not offered again, the technology question is asked anew.
    fn decline_waiver(&mut self, s: &mut CoreState) {
        let Some(waiving) = self.waiving.take() else { return };
        self.stage = match self.before.take() {
            Some(before) => {
                *s = *before;
                RStage::Choose
            }
            None => RStage::Free,
        };
        self.declined.push(waiving.tech);
    }

    /// The waiver is paid and the technology researched without its prerequisites (`technology::research_with_waiver`).
    fn waive_and_pay(&mut self, sx: &mut Sx<'_>, payment: crate::choice::strategy::Waiver) -> Result<(), Illegal> {
        let waiving = self.waiving.ok_or(Illegal)?;
        let backup = sx.state.clone();
        let kernel = sx.kernel;
        if !kernel.hooks.strategy.research_waiver_paid(&mut sx.cx(), self.who, waiving.tech, payment) {
            // The payment failed: the position is back as before it, and nothing is researched.
            *sx.state = backup;
            self.decline_waiver(sx.state);
            return Ok(());
        }
        self.waiving = None;
        let paid = self.before.take().is_some();
        self.complete(sx.state, sx.kernel, waiving.tech);
        if paid || self.rider {
            self.stage = RStage::Done;
        } else {
            self.declined = SmallList::new();
            self.begin_paid(sx.state, sx.kernel);
        }
        Ok(())
    }

    /// The offers of the waiver in play: `resolve_research`'s `waivers`.
    fn waiver_offers(&self, s: &CoreState, k: &Kernel) -> Vec<crate::kernel::hooks::ResearchWaiver> {
        self.waiving.map_or_else(Vec::new, |w| k.hooks.strategy.research_waiver_offers(s, k, self.who, w.tech))
    }

    /// `resolve_research`'s first question: which waiver.
    fn waiver_choice(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        let tech = self.waiving?.tech;
        let offers = self.waiver_offers(s, k);
        let mut offered = [None; 2];
        for (slot, offer) in offered.iter_mut().zip(&offers) {
            *slot = Some(offer.waiver);
        }
        let mut options = opts((0..offers.len().min(2)).filter_map(|i| u8::try_from(i).ok()).map(SOpt::ResearchWaiver));
        options.push(Opt::Decline);
        Some(Choice { actor: self.who, ask: ask(SAsk::ResearchWaiver { tech, offered }), ctx: waiver_ctx(SSub::ResearchWaiver), opts: options })
    }

    /// `resolve_research`'s second question: what pays the waiver chosen.
    fn payment_choice(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        let waiving = self.waiving?;
        let offers = self.waiver_offers(s, k);
        let offer = offers.get(usize::from(waiving.picked?))?;
        let mut options = opts(offer.payments.as_slice().iter().map(|p| SOpt::ResearchPayment(*p)));
        options.push(Opt::Decline);
        Some(Choice {
            actor: self.who,
            ask: ask(SAsk::ResearchWaiverPayment { tech: waiving.tech, waiver: offer.waiver }),
            ctx: waiver_ctx(SSub::ResearchWaiverPayment),
            opts: options,
        })
    }

    fn after_step(&mut self, s: &mut CoreState, k: &Kernel) {
        let Some(step) = self.pay.take() else {
            self.failed = true;
            self.stage = RStage::Done;
            return;
        };
        let Some(worth) = step.took else {
            self.failed = true;
            self.stage = RStage::Done;
            return;
        };
        let paid = step.paid + worth;
        if paid < step.owed {
            self.pay = Some(self.pay_step(paid));
        } else if let Some(t) = self.chosen.take() {
            self.land(s, k, t);
        } else {
            self.failed = true;
            self.stage = RStage::Done;
        }
    }

    fn research_choice(&self, s: &CoreState, k: &Kernel, decline: bool) -> Choice {
        let owned = u8::try_from(tech::owned(s, self.who).len()).unwrap_or(u8::MAX);
        let mut options = opts(self.open(s, k).as_slice().iter().map(|t| SOpt::Research(*t)));
        if decline {
            options.push(Opt::Decline);
        }
        let cost = if decline { u8::try_from(self.cost).unwrap_or(u8::MAX) } else { 0 };
        let mut ctx = crate::choice::strategy::ctx(StrategyCard::Technology, decline, SSub::ResearchTechnology);
        if self.rider {
            ctx.origin = Origin::ActionCard(crate::generated::ActionCard::TechnologyRider);
        }
        Choice { actor: self.who, ask: ask(SAsk::ResearchTechnology { cost, owned, decline }), ctx, opts: options }
    }
}

impl Research {
    /// `technology::researchable` for a card that researches outside a strategy card: the technologies the seat can research
    /// under the faction hooks of the kernel, in the order of the technology set.
    #[must_use]
    pub fn researchable_for(s: &CoreState, k: &Kernel, scope: Scope, who: Seat) -> SmallList<Technology, 40> {
        tech::researchable(s, scope, reach::blanked(s), who, &KernelHooks::new(s, k, who)).iter().collect()
    }

    /// `technology::research` of a technology a card chose, the bill (if any) already paid: the prerequisites are met or
    /// Inheritance Systems pays them, then the research completes. `false`, changing nothing, where the string `research`
    /// answers `false` (a technology only a faction waiver opens, Inheritance Systems that cannot pay).
    pub fn research_chosen(s: &mut CoreState, k: &Kernel, scope: Scope, roster: UnitRoster, who: Seat, t: Technology) -> bool {
        Self::new(who, false, 0, scope).with_roster(roster).settle(s, k, t)
    }
}

impl Window for Research {
    type Stage = RStage;

    fn stage(&self) -> RStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                RStage::Start => {
                    if self.primary && !self.open(sx.state, sx.kernel).is_empty() {
                        self.stage = RStage::Free;
                    } else if self.rider {
                        // Nothing to research: the rider ends (the paid research is the primary's, not the card's).
                        self.stage = RStage::Done;
                    } else {
                        self.begin_paid(sx.state, sx.kernel);
                    }
                }
                RStage::Free => {
                    if self.open(sx.state, sx.kernel).is_empty() {
                        // Every technology left was declined (`offer_research_inner`'s loop ends): the paid research follows.
                        self.declined = SmallList::new();
                        if self.rider {
                            self.stage = RStage::Done;
                        } else {
                            self.begin_paid(sx.state, sx.kernel);
                        }
                    } else {
                        return Next::Continue(self.research_choice(sx.state, sx.kernel, false).ask);
                    }
                }
                RStage::Waive | RStage::WaivePay => match self.ask_of(sx.state, sx.kernel) {
                    Some(choice) => return Next::Continue(choice.ask),
                    None => {
                        self.failed = true;
                        self.stage = RStage::Done;
                    }
                },
                RStage::Choose => {
                    if self.open(sx.state, sx.kernel).is_empty() {
                        self.stage = RStage::Done;
                    } else {
                        return Next::Continue(self.research_choice(sx.state, sx.kernel, true).ask);
                    }
                }
                RStage::Pay => {
                    let Some(step) = self.pay.as_mut() else {
                        self.failed = true;
                        self.stage = RStage::Done;
                        continue;
                    };
                    match step.step(sx) {
                        Next::Done => self.after_step(sx.state, sx.kernel),
                        other => return other,
                    }
                }
                RStage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        match self.stage {
            RStage::Free => Some(self.research_choice(s, k, false)),
            RStage::Choose => Some(self.research_choice(s, k, true)),
            RStage::Pay => self.pay.as_ref()?.ask_of(s, k),
            RStage::Waive => self.waiver_choice(s, k),
            RStage::WaivePay => self.payment_choice(s, k),
            RStage::Start | RStage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage == RStage::Pay {
            return self.pay.as_mut().ok_or(Illegal)?.answer(sx, pick);
        }
        let offered = self.ask_of(sx.state, sx.kernel).ok_or(Illegal)?;
        let Pick::Opt(chosen) = pick else { return Err(Illegal) };
        if !offered.opts.as_slice().contains(&chosen) {
            return Err(Illegal);
        }
        match (self.stage, chosen) {
            (RStage::Waive | RStage::WaivePay, Opt::Decline) => self.decline_waiver(sx.state),
            (RStage::Waive, Opt::Strategy(SOpt::ResearchWaiver(index))) => {
                self.waiving.as_mut().ok_or(Illegal)?.picked = Some(index);
                self.stage = RStage::WaivePay;
            }
            (RStage::WaivePay, Opt::Strategy(SOpt::ResearchPayment(payment))) => self.waive_and_pay(sx, payment)?,
            (RStage::Choose, Opt::Decline) => self.stage = RStage::Done,
            (RStage::Free, Opt::Strategy(SOpt::Research(t))) => match self.try_settle(sx.state, sx.kernel, t) {
                Settled::Landed => {
                    self.declined = SmallList::new();
                    if self.rider {
                        self.stage = RStage::Done;
                    } else {
                        self.begin_paid(sx.state, sx.kernel);
                    }
                }
                Settled::Failed => {
                    self.failed = true;
                    self.stage = RStage::Done;
                }
                Settled::Waiver => self.start_waiver(sx.state, sx.kernel, t),
            },
            (RStage::Choose, Opt::Strategy(SOpt::Research(t))) => {
                // The position before the bill: a declined waiver puts it back (`before = state.clone()` of `paid_research`).
                self.before = Some(Box::new(sx.state.clone()));
                if self.cost > 0 && !sx.state.meta().rules_at_least(RULES_TECH_PAYMENT_CHOICE) {
                    // Below `RULES_TECH_PAYMENT_CHOICE` the cheapest plan is spent without asking (`payment::plans`'s first); a bill no plan
                    // pays ends the research with nothing changed.
                    let env = fx_pay::PayEnv { scope: self.scope, blanked: reach::blanked(sx.state) };
                    if fx_pay::pay_first_plan(sx.state, sx.kernel, env, self.who, Spend::Resources, self.cost) {
                        self.land(sx.state, sx.kernel, t);
                    } else {
                        self.stage = RStage::Done;
                    }
                } else if self.cost > 0 {
                    self.chosen = Some(t);
                    self.pay = Some(self.pay_step(0));
                    self.stage = RStage::Pay;
                } else {
                    self.land(sx.state, sx.kernel, t);
                }
            }
            _ => return Err(Illegal),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::{Driver, Status};
    use crate::kernel::hooks::{ResearchWaiver, Waiver};
    use crate::kernel::stage::AbilityTable;
    use crate::kernel::Resolver;
    use crate::state::{Player, Players};

    fn game() -> CoreState {
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::ALL[0])]);
        s
    }

    fn offers_a_waiver(_: &CoreState, _: &Kernel, _: Seat, _: Technology) -> Option<ResearchWaiver> {
        Some(ResearchWaiver { waiver: Waiver { module: Faction::ALL[0], code: 1 }, payments: SmallList::new() })
    }

    /// A technology only a faction waiver opens is a question about the waiver and its payment (lane G5): the window asks it, the run stays the
    /// window's, and nothing changes until the payment is made (`ti4-engine-core-tests/tests/research_waiver.rs` plays the whole of it).
    #[test]
    fn a_technology_only_a_waiver_opens_asks_the_waiver_and_changes_nothing_yet() {
        let mut s = game();
        let mut k = Kernel::plain();
        k.hooks.strategy.research_waiver_offer.push(offers_a_waiver);
        let scope = Scope(u32::from(k.sources.0));
        let window = Research::new(Seat(0), true, 6, scope);
        let open = window.open(&s, &k);
        let walled = open
            .as_slice()
            .iter()
            .position(|t| !tech::prerequisites_met(&s, scope, false, Seat(0), *t, &KernelHooks::new(&s, &k, Seat(0))))
            .expect("the waiver opens a technology the colours do not cover");
        let mut d = Driver::new(window, Resolver::new(&[Seat(0)], Some(Seat(0))), AbilityTable::default(), 1);
        let Status::Asking(c) = d.run(&mut s, &k).unwrap() else { panic!("asks") };
        let before = s.players().by_seat(Seat(0)).unwrap().tech.owned;
        let status = d.resume(&mut s, &k, c.pick(walled).unwrap()).unwrap();
        let Status::Asking(c) = status else { panic!("the waiver is asked") };
        assert!(matches!(c.ask, Ask::Strategy(SAsk::ResearchWaiver { .. })), "{:?}", c.ask);
        assert!(!d.window.failed, "the window stays on its path");
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().tech.owned, before, "nothing was gained");
        assert!(d.window.researched.is_empty());
    }
}
