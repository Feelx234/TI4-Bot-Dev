//! The primary and secondary abilities of the strategy cards that ask (package B8; string
//! `strategy_cards`), as [`Window`]s: Warfare's primary, Diplomacy's two abilities, Politics' two abilities.
//! The Trade primary is in [`super::strategy_windows`]; Leadership and Construction, which read the production
//! rules, are in [`super::production::strategy_cards`].
//!
//! Each is the string function split at its questions: `step` runs to the next question, `answer` is the only
//! place an answer changes the position. What a window cannot do stays with the string callers (the adapter
//! bails): a draw the faction hooks change, the `COMMAND_TOKEN_PLACED` event of a Diplomacy token (staged by
//! the caller after the window), a question asked inside a battle or an invasion.

use crate::choice::agenda::Shown;
use crate::choice::generic::Rule;
use crate::choice::strategy::{ctx, gain_pool_choice, rule_ctx, Ask as SAsk, Opt as SOpt, Subtype as SSub};
use crate::choice::generic::Content;
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::{Agenda, Faction, Planet, PublicObjective, StrategyCard, System};
use crate::ids::Seat;
use crate::kernel::{fx, fx_pay};
use crate::kernel::children::{ErasedChild, Spawn};
use crate::kernel::stage::{Illegal, Ran, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::{board, seats};
use crate::state::{Code, CoreState};

fn ask(a: SAsk) -> Ask {
    Ask::Strategy(a)
}

fn opts(it: impl IntoIterator<Item = SOpt>) -> Opts {
    it.into_iter().map(Opt::Strategy).collect()
}

// -- the command token questions (52.4) ----------------------------------------------------------------

/// Command tokens on the sheet (the three pools).
#[must_use]
pub fn sheet_total(core: &CoreState, who: Seat) -> i32 {
    core.players().by_seat(who).map_or(0, |p| {
        i32::from(p.econ.tactic_tokens) + i32::from(p.econ.fleet_tokens) + i32::from(p.econ.strategic_tokens)
    })
}

/// `tokens::distribution_options`: every way to split `total` tokens, at least `floor` in the fleet pool
/// (none when the total is below the floor).
pub fn splits(total: u8, floor: u8) -> impl Iterator<Item = SOpt> {
    let min_fleet = if total >= floor { floor } else { 0 };
    (0..=total).flat_map(move |tactic| {
        (min_fleet..=(total - tactic)).map(move |fleet| SOpt::Split { tactic, fleet, strategic: total - tactic - fleet })
    })
}

/// Set the pools of a seat to a split.
fn apply_split(core: &mut CoreState, who: Seat, tactic: u8, fleet: u8, strategic: u8) {
    if let Some(p) = core.players_mut().by_seat_mut(who) {
        p.econ.tactic_tokens = i16::from(tactic);
        p.econ.fleet_tokens = i16::from(fleet);
        p.econ.strategic_tokens = i16::from(strategic);
    }
}

// -- Warfare ---------------------------------------------------------------------------------------------

/// The suspension points of [`WarfarePrimary`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WStage {
    Start,
    /// Waiting on "recall a command token".
    Recall,
    /// Waiting on the pool of the recovered token.
    Gain,
    /// Waiting on the final split of the sheet.
    Redistribute,
    Done,
}

/// "Remove 1 of your command tokens from the game board; then gain 1 command token. Then you may
/// redistribute your command tokens."
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarfarePrimary {
    pub who: Seat,
    stage: WStage,
    /// `RULES_LETNEV_FLEET_POOL`: the Letnev fleet pool may be emptied.
    letnev_zero_floor: bool,
}

impl WarfarePrimary {
    #[must_use]
    pub const fn new(who: Seat, letnev_zero_floor: bool) -> Self {
        Self { who, stage: WStage::Start, letnev_zero_floor }
    }

    /// Systems that hold one of the seat's command tokens, in system text order.
    fn tokens_on_board(core: &CoreState, who: Seat) -> SmallList<System, 16> {
        core.board().systems().filter(|(_, cell)| cell.cmd.contains(who)).map(|(s, _)| s).collect()
    }

    fn floor(&self, core: &CoreState) -> u8 {
        let letnev = core.players().by_seat(self.who).is_some_and(|p| p.faction == Faction::TheBaronyOfLetnev);
        if letnev && self.letnev_zero_floor { 0 } else { 2 }
    }

    fn ask_now(&self, core: &CoreState) -> Option<SAsk> {
        match self.stage {
            WStage::Recall => Some(SAsk::WarfareRecall),
            WStage::Gain => Some(SAsk::GainPool { left: 1 }),
            WStage::Redistribute => Some(SAsk::WarfareRedistribute {
                total: u8::try_from(sheet_total(core, self.who)).unwrap_or(u8::MAX),
                floor: self.floor(core),
            }),
            WStage::Start | WStage::Done => None,
        }
    }
}

impl Window for WarfarePrimary {
    type Stage = WStage;

    fn stage(&self) -> WStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == WStage::Start {
            self.stage = if Self::tokens_on_board(sx.state, self.who).is_empty() { WStage::Done } else { WStage::Recall };
        }
        if self.stage == WStage::Redistribute && sheet_total(sx.state, self.who) == 0 {
            self.stage = WStage::Done;
        }
        match self.ask_now(sx.state) {
            Some(a) => Next::Continue(ask(a)),
            None => Next::Done,
        }
    }

    fn ask_of(&self, core: &CoreState, _: &Kernel) -> Option<Choice> {
        let a = self.ask_now(core)?;
        let (ctx, options) = match a {
            SAsk::WarfareRecall => (
                ctx(StrategyCard::Warfare, false, SSub::WarfareRecallToken),
                opts(Self::tokens_on_board(core, self.who).as_slice().iter().map(|s| SOpt::System(*s))),
            ),
            SAsk::GainPool { left } => return Some(gain_pool_choice(self.who, left)),
            SAsk::WarfareRedistribute { total, floor } => {
                (ctx(StrategyCard::Warfare, false, SSub::WarfareRedistributeTokens), opts(splits(total, floor)))
            }
            _ => return None,
        };
        Some(Choice { actor: self.who, ask: ask(a), ctx, opts: options })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let Pick::Opt(Opt::Strategy(chosen)) = pick else { return Err(Illegal) };
        let offered = self.ask_of(sx.state, sx.kernel).ok_or(Illegal)?;
        if !offered.opts.as_slice().contains(&Opt::Strategy(chosen)) {
            return Err(Illegal);
        }
        match (self.stage, chosen) {
            (WStage::Recall, SOpt::System(system)) => {
                fx::remove_command_token(sx.state, self.who, system);
                self.stage = WStage::Gain;
            }
            (WStage::Gain, SOpt::Pool(pool)) => {
                fx::gain_token(sx.state, self.who, pool, 1);
                self.stage = WStage::Redistribute;
            }
            (WStage::Redistribute, SOpt::Split { tactic, fleet, strategic }) => {
                apply_split(sx.state, self.who, tactic, fleet, strategic);
                self.stage = WStage::Done;
            }
            _ => return Err(Illegal),
        }
        Ok(())
    }
}

// -- Diplomacy -------------------------------------------------------------------------------------------

/// Which ability of Diplomacy a [`Diplomacy`] window resolves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DMode {
    /// The primary: choose a system, the other players place a token there, then ready up to two planets (the
    /// base printing: every exhausted planet of the chosen system, asked nothing).
    Primary { base_printing: bool },
    /// The secondary: ready up to two planets.
    Secondary,
}

/// The suspension points of [`Diplomacy`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DStage {
    Start,
    Choose,
    Ready,
    Done,
}

/// Diplomacy, primary or secondary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diplomacy {
    pub who: Seat,
    /// The printed card (both printings are "Diplomacy" in the context).
    card: StrategyCard,
    mode: DMode,
    stage: DStage,
    chosen: Option<System>,
    /// Planets still to ready (the primary and the secondary ready up to two).
    left: u8,
    /// The seats that got a token in the chosen system, in seating order (for the caller's staged events).
    pub placed: SmallList<Seat, 8>,
}

impl Diplomacy {
    #[must_use]
    pub const fn new(who: Seat, card: StrategyCard, mode: DMode) -> Self {
        Self { who, card, mode, stage: DStage::Start, chosen: None, left: 2, placed: SmallList::new() }
    }

    /// The system the primary placed tokens in.
    #[must_use]
    pub const fn chosen(&self) -> Option<System> {
        self.chosen
    }

    /// The systems the primary may choose: those with a planet the seat controls, Mecatol excluded.
    fn systems(core: &CoreState, who: Seat) -> SmallList<System, 16> {
        let mut out: SmallList<System, 16> = SmallList::new();
        for (system, _) in board::controlled_planets(core, who).as_slice() {
            if matches!(system, System::MecatolRex18 | System::MecatolRex112) || out.as_slice().last() == Some(system) {
                continue;
            }
            out.push(*system);
        }
        out
    }

    /// Exhausted planets the seat controls, `(system, planet)` sorted.
    fn exhausted(core: &CoreState, who: Seat) -> SmallList<(System, Planet), 16> {
        board::controlled_planets(core, who).as_slice().iter().copied().filter(|(_, p)| board::exhausted(core, *p)).collect()
    }

    /// The part after the system is chosen (or none was).
    fn after_choice(&mut self, core: &mut CoreState) {
        match self.mode {
            DMode::Primary { base_printing: true } => {
                if let Some(system) = self.chosen {
                    for (at, planet) in Self::exhausted(core, self.who).as_slice() {
                        if *at == system {
                            fx::ready_planet(core, *planet);
                        }
                    }
                }
                self.stage = DStage::Done;
            }
            _ => self.stage = DStage::Ready,
        }
    }

    fn ask_now(&self) -> Option<SAsk> {
        match self.stage {
            DStage::Choose => Some(SAsk::DiplomacySystem),
            DStage::Ready => Some(SAsk::ReadyPlanet),
            DStage::Start | DStage::Done => None,
        }
    }
}

impl Window for Diplomacy {
    type Stage = DStage;

    fn stage(&self) -> DStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == DStage::Start {
            if matches!(self.mode, DMode::Primary { .. }) && !Self::systems(sx.state, self.who).is_empty() {
                self.stage = DStage::Choose;
            } else {
                self.after_choice(sx.state);
            }
        }
        if self.stage == DStage::Ready && (self.left == 0 || Self::exhausted(sx.state, self.who).is_empty()) {
            self.stage = DStage::Done;
        }
        match self.ask_now() {
            Some(a) => Next::Continue(ask(a)),
            None => Next::Done,
        }
    }

    fn ask_of(&self, core: &CoreState, _: &Kernel) -> Option<Choice> {
        match self.ask_now()? {
            SAsk::DiplomacySystem => Some(Choice {
                actor: self.who,
                ask: ask(SAsk::DiplomacySystem),
                ctx: ctx(self.card, false, SSub::DiplomacyChooseSystem),
                opts: opts(Self::systems(core, self.who).as_slice().iter().map(|s| SOpt::System(*s))),
            }),
            SAsk::ReadyPlanet => Some(Choice {
                actor: self.who,
                ask: ask(SAsk::ReadyPlanet),
                ctx: rule_ctx(Rule::R34_2, SSub::ReadyPlanet),
                opts: opts(
                    Self::exhausted(core, self.who)
                        .as_slice()
                        .iter()
                        .map(|(system, planet)| SOpt::Ready(*planet, Shown { votes: 0, system: Some(*system) })),
                ),
            }),
            _ => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let Pick::Opt(Opt::Strategy(chosen)) = pick else { return Err(Illegal) };
        let offered = self.ask_of(sx.state, sx.kernel).ok_or(Illegal)?;
        if !offered.opts.as_slice().contains(&Opt::Strategy(chosen)) {
            return Err(Illegal);
        }
        match (self.stage, chosen) {
            (DStage::Choose, SOpt::System(system)) => {
                self.chosen = Some(system);
                for other in seats::seating_order(sx.state).as_slice().iter().copied().filter(|s| *s != self.who) {
                    if fx::place_command_token(sx.state, other, system) {
                        self.placed.push(other);
                    }
                }
                self.after_choice(sx.state);
            }
            (DStage::Ready, SOpt::Ready(planet, _)) => {
                fx::ready_planet(sx.state, planet);
                self.left -= 1;
            }
            _ => return Err(Illegal),
        }
        Ok(())
    }
}

// -- Politics --------------------------------------------------------------------------------------------

/// The suspension points of [`Politics`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PStage {
    Start,
    /// Waiting on "who becomes speaker".
    Speaker,
    /// The two action cards are drawn as a child window, which asks the hand limit's discard (rule 2.4).
    Drawing,
    /// Waiting on where an agenda goes.
    Agenda,
    Done,
}

/// Politics: the primary (a new speaker, two action cards, the top two agendas back on the deck in an order
/// of the holder's choosing) and the secondary (two action cards).
#[derive(Debug, Clone)]
pub struct Politics {
    pub who: Seat,
    primary: bool,
    stage: PStage,
    /// The unofficial action cards leave the deck before the draw.
    drop_unofficial: bool,
    /// The agendas looked at, in the order they were taken off the deck.
    looked: SmallList<Agenda, 2>,
    next: u8,
    /// The draw and the hand limit (`action_cards::draw`), a child of the action-card area.
    draw: ErasedChild,
    /// The table has no link to the draw window, or gave it no printed-name groups: the caller declines the run.
    pub unsupported: bool,
}

impl Politics {
    #[must_use]
    pub const fn new(who: Seat, primary: bool, drop_unofficial: bool) -> Self {
        Self { who, primary, stage: PStage::Start, drop_unofficial, looked: SmallList::new(), next: 0, draw: ErasedChild::idle(), unsupported: false }
    }

    /// Seats that may become speaker: everyone but the speaker, in seating order.
    fn candidates(core: &CoreState) -> SmallList<Seat, 16> {
        let speaker = core.meta().speaker;
        seats::seating_order(core).as_slice().iter().copied().filter(|s| *s != speaker).collect()
    }

    /// Start the draw of the two action cards (a child window: the hand limit may ask).
    fn begin_draw(&mut self, sx: &mut Sx<'_>) {
        match sx.table.links.start(&Spawn::DrawCards { who: self.who, count: 2, drop_unofficial: self.drop_unofficial }) {
            Some(window) => {
                self.draw.start(window);
                self.stage = PStage::Drawing;
            }
            None => {
                self.unsupported = true;
                self.stage = PStage::Done;
            }
        }
    }

    /// Look at the top two agendas (the primary).
    fn look(&mut self, core: &mut CoreState) {
        if self.primary {
            for _ in 0..2 {
                let Some(agenda) = core.decks_mut().agenda.draw() else { break };
                self.looked.push(agenda);
            }
        }
        self.stage = if self.looked.is_empty() { PStage::Done } else { PStage::Agenda };
    }
}

impl Window for Politics {
    type Stage = PStage;

    fn stage(&self) -> PStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == PStage::Start {
            if self.primary && !Self::candidates(sx.state).is_empty() {
                self.stage = PStage::Speaker;
            } else {
                self.begin_draw(sx);
            }
        }
        if self.stage == PStage::Drawing {
            match self.draw.step(sx) {
                Some(Ran::Asks(a)) => return Next::Continue(a),
                Some(Ran::Pass(next)) => return next,
                Some(Ran::Done(report)) => {
                    self.unsupported |= report == crate::kernel::children::Report::Left;
                    self.look(sx.state);
                }
                None => self.look(sx.state),
            }
        }
        match self.stage {
            PStage::Speaker => Next::Continue(ask(SAsk::PoliticsSpeaker)),
            PStage::Agenda => Next::Continue(ask(SAsk::PoliticsAgenda { agenda: self.looked.as_slice()[usize::from(self.next)] })),
            PStage::Start | PStage::Drawing | PStage::Done => Next::Done,
        }
    }

    fn ask_of(&self, core: &CoreState, k: &Kernel) -> Option<Choice> {
        match self.stage {
            PStage::Drawing => self.draw.ask_of(core, k),
            PStage::Speaker => Some(Choice {
                actor: self.who,
                ask: ask(SAsk::PoliticsSpeaker),
                ctx: ctx(StrategyCard::Politics, false, SSub::PoliticsChooseSpeaker),
                opts: opts(Self::candidates(core).as_slice().iter().map(|s| SOpt::Speaker(*s))),
            }),
            PStage::Agenda => Some(Choice {
                actor: self.who,
                ask: ask(SAsk::PoliticsAgenda { agenda: self.looked.as_slice()[usize::from(self.next)] }),
                ctx: ctx(StrategyCard::Politics, false, SSub::PoliticsPlaceAgenda),
                opts: opts([SOpt::Top, SOpt::Bottom]),
            }),
            PStage::Start | PStage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage == PStage::Drawing {
            return self.draw.answer(sx, pick);
        }
        let Pick::Opt(Opt::Strategy(chosen)) = pick else { return Err(Illegal) };
        let offered = self.ask_of(sx.state, sx.kernel).ok_or(Illegal)?;
        if !offered.opts.as_slice().contains(&Opt::Strategy(chosen)) {
            return Err(Illegal);
        }
        match (self.stage, chosen) {
            (PStage::Speaker, SOpt::Speaker(seat)) => {
                sx.state.meta_mut().speaker = seat;
                self.begin_draw(sx);
            }
            (PStage::Agenda, placed @ (SOpt::Top | SOpt::Bottom)) => {
                let agenda = self.looked.as_slice()[usize::from(self.next)];
                if placed == SOpt::Top {
                    sx.state.decks_mut().agenda.put_top(agenda);
                } else {
                    sx.state.decks_mut().agenda.put_bottom(agenda);
                }
                self.next += 1;
                if usize::from(self.next) == self.looked.len() {
                    self.stage = PStage::Done;
                }
            }
            _ => return Err(Illegal),
        }
        Ok(())
    }
}

// -- Imperial --------------------------------------------------------------------------------------------

/// The suspension points of [`ImperialPrimary`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IStage {
    Start,
    /// Waiting on "score a public objective with Imperial".
    Score,
    /// The secret objective is drawn and the hand limit runs as a child window (45.4: a secret goes back to the deck).
    Limit,
    Done,
}

/// Imperial's primary: score a public objective (the options are the caller's `scoreable_on`: the scoring window is
/// the string's judgment), paying its price first (`objectives::award`: the first plan, the tokens, the goods), then
/// gain a victory point for Mecatol Rex or draw a secret objective. What a public score sets off besides the points
/// (leaders unlocking, Yin's breakthrough) is the caller's, announced by [`ImperialPrimary::scored`]. Without a
/// [`ImperialPrimary::with_price`] environment a bought objective cannot be paid and is not scored (the string refuses it
/// the same way when the price cannot be met).
#[derive(Debug, Clone)]
pub struct ImperialPrimary {
    pub who: Seat,
    scoreable: SmallList<PublicObjective, 16>,
    stage: IStage,
    env: Option<fx_pay::PayEnv>,
    /// The objective the holder scored, if any.
    pub scored: Option<PublicObjective>,
    /// The hand limit after the secret draw (`secrets::enforce_hand_limit`), a child of the objectives area.
    hand: ErasedChild,
    /// The table has no link to the hand limit window (it was not filled by `driver::children::register`): the draw cannot be
    /// followed and the caller declines the run.
    pub unsupported: bool,
}

impl ImperialPrimary {
    #[must_use]
    pub const fn new(who: Seat, scoreable: SmallList<PublicObjective, 16>) -> Self {
        Self { who, scoreable, stage: IStage::Start, env: None, scored: None, hand: ErasedChild::idle(), unsupported: false }
    }

    /// The window pays the price of a bought objective with this environment.
    #[must_use]
    pub const fn with_price(mut self, env: fx_pay::PayEnv) -> Self {
        self.env = Some(env);
        self
    }

    /// Whether the seat controls Mecatol Rex (either printing).
    #[must_use]
    pub fn controls_mecatol(core: &CoreState, who: Seat) -> bool {
        board::controlled_planets(core, who).as_slice().iter().any(|(system, _)| crate::query::objectives::is_mecatol(*system))
    }

    /// The tail of the ability: a point for Mecatol, else a secret objective and the hand limit after it.
    fn finish(&mut self, sx: &mut Sx<'_>) {
        self.stage = IStage::Done;
        if Self::controls_mecatol(sx.state, self.who) {
            fx::gain_victory_points(sx.state, self.who, 1, crate::state::LedgerReason::ImperialPrimary);
        } else if fx::draw_secret(sx.state, self.who).is_some() {
            match sx.table.links.start(&Spawn::HandLimit { who: self.who }) {
                Some(window) => {
                    self.hand.start(window);
                    self.stage = IStage::Limit;
                }
                None => self.unsupported = true,
            }
        }
    }
}

impl Window for ImperialPrimary {
    type Stage = IStage;

    fn stage(&self) -> IStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == IStage::Start {
            if self.scoreable.is_empty() {
                self.finish(sx);
            } else {
                self.stage = IStage::Score;
            }
        }
        if self.stage == IStage::Limit {
            match self.hand.step(sx) {
                Some(Ran::Asks(a)) => return Next::Continue(a),
                Some(Ran::Pass(next)) => return next,
                Some(Ran::Done(_)) | None => self.stage = IStage::Done,
            }
        }
        match self.stage {
            IStage::Score => Next::Continue(ask(SAsk::ImperialScore)),
            IStage::Start | IStage::Limit | IStage::Done => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        if self.stage == IStage::Limit {
            return self.hand.ask_of(s, k);
        }
        (self.stage == IStage::Score).then(|| {
            let mut options = opts(self.scoreable.as_slice().iter().map(|o| SOpt::Objective(*o)));
            options.push(Opt::Decline);
            Choice { actor: self.who, ask: ask(SAsk::ImperialScore), ctx: ctx(StrategyCard::Imperial, false, SSub::ImperialScoreObjective), opts: options }
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage == IStage::Limit {
            return self.hand.answer(sx, pick);
        }
        if self.stage != IStage::Score {
            return Err(Illegal);
        }
        match pick {
            Pick::Opt(Opt::Decline) => {}
            Pick::Opt(Opt::Strategy(SOpt::Objective(o))) if self.scoreable.as_slice().contains(&o) => {
                // `objectives::award`: the price first (an unaffordable one scores nothing), then the score.
                let paid = match crate::query::objectives::cost_of(o) {
                    None => true,
                    Some(cost) => self.env.is_some_and(|env| fx_pay::pay_cost(sx.state, sx.kernel, env, self.who, cost)),
                };
                if paid {
                    fx::score_public(sx.state, self.who, crate::state::ObjectiveRef::Public(o));
                    self.scored = Some(o);
                }
            }
            _ => return Err(Illegal),
        }
        self.finish(sx);
        Ok(())
    }
}

// -- Imperial's secondary ------------------------------------------------------------------------------------

/// Imperial's secondary: "draw 1 secret objective" (`strategy_cards::imperial_secret`, and the primary of a seat without Mecatol
/// Rex draws the same way): the top secret goes to the hand and the hand limit (45.4) runs as a child window, which asks
/// which unscored secret goes back to the deck when the hand is over its limit.
#[derive(Debug, Clone)]
pub struct ImperialSecondary {
    pub who: Seat,
    started: bool,
    hand: ErasedChild,
    /// The table has no link to the hand limit window: the caller declines the run.
    pub unsupported: bool,
}

impl ImperialSecondary {
    #[must_use]
    pub const fn new(who: Seat) -> Self {
        Self { who, started: false, hand: ErasedChild::idle(), unsupported: false }
    }
}

impl Window for ImperialSecondary {
    type Stage = bool;

    fn stage(&self) -> bool {
        self.started
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if !self.started {
            self.started = true;
            if fx::draw_secret(sx.state, self.who).is_some() {
                match sx.table.links.start(&Spawn::HandLimit { who: self.who }) {
                    Some(window) => self.hand.start(window),
                    None => self.unsupported = true,
                }
            }
        }
        match self.hand.step(sx) {
            Some(Ran::Asks(a)) => Next::Continue(a),
            Some(Ran::Pass(next)) => next,
            Some(Ran::Done(_)) | None => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        self.hand.ask_of(s, k)
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        self.hand.answer(sx, pick)
    }
}

// -- Thunder's Edge Warfare ----------------------------------------------------------------------------------

/// The suspension points of [`TeWarfare`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TStage {
    Start,
    /// Waiting on the split of the sheet before the tactical action.
    Redistribute,
    /// Waiting on the system of the free tactical action.
    Choose,
    Done,
}

/// The primary of the Thunder's Edge Warfare: "You may redistribute your command tokens before and after this action",
/// then a tactical action in any system without spending a command token. The first redistribution and the choice of
/// the system are this window; the tactical action itself and the redistribution after it are the game's
/// (`Ability::FreeTactical`). The systems offered are the caller's (the map's, the string's judgment).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeWarfare {
    pub who: Seat,
    systems: Vec<System>,
    stage: TStage,
    letnev_zero_floor: bool,
    /// The interned `te6warfare`: the source of the system question.
    source: Code,
    /// The system chosen for the free tactical action.
    pub chosen: Option<System>,
}

impl TeWarfare {
    #[must_use]
    pub fn new(who: Seat, systems: &[System], letnev_zero_floor: bool, source: Code) -> Self {
        Self { who, systems: systems.to_vec(), stage: TStage::Start, letnev_zero_floor, source, chosen: None }
    }

    fn floor(&self, core: &CoreState) -> u8 {
        let letnev = core.players().by_seat(self.who).is_some_and(|p| p.faction == Faction::TheBaronyOfLetnev);
        if letnev && self.letnev_zero_floor { 0 } else { 2 }
    }

    fn ask_now(&self, core: &CoreState) -> Option<SAsk> {
        match self.stage {
            TStage::Redistribute => Some(SAsk::TeWarfareRedistribute {
                total: u8::try_from(sheet_total(core, self.who)).unwrap_or(u8::MAX),
                floor: self.floor(core),
            }),
            TStage::Choose => Some(SAsk::TeWarfareTactical),
            TStage::Start | TStage::Done => None,
        }
    }
}

impl Window for TeWarfare {
    type Stage = TStage;

    fn stage(&self) -> TStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == TStage::Start {
            self.stage = if sheet_total(sx.state, self.who) > 0 { TStage::Redistribute } else { TStage::Choose };
        }
        if self.stage == TStage::Choose && self.systems.is_empty() {
            self.stage = TStage::Done;
        }
        match self.ask_now(sx.state) {
            Some(a) => Next::Continue(ask(a)),
            None => Next::Done,
        }
    }

    fn ask_of(&self, core: &CoreState, _: &Kernel) -> Option<Choice> {
        let a = self.ask_now(core)?;
        Some(match a {
            SAsk::TeWarfareRedistribute { total, floor } => Choice {
                actor: self.who,
                ask: ask(a),
                ctx: ctx(StrategyCard::Warfare, false, SSub::WarfareRedistributeTokens),
                opts: opts(splits(total, floor)),
            },
            _ => Choice {
                actor: self.who,
                ask: ask(a),
                ctx: Ctx {
                    origin: Origin::Content(Content::Other(self.source)),
                    subtype: Subtype::Strategy(SSub::WarfareFreeTactical),
                    optional: false,
                    target: None,
                    outstanding: SmallList::new(),
                    space_battle: false,
                    invasion_seq: None,
                },
                opts: opts(self.systems.iter().map(|s| SOpt::System(*s))),
            },
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let Pick::Opt(Opt::Strategy(chosen)) = pick else { return Err(Illegal) };
        let offered = self.ask_of(sx.state, sx.kernel).ok_or(Illegal)?;
        if !offered.opts.as_slice().contains(&Opt::Strategy(chosen)) {
            return Err(Illegal);
        }
        match (self.stage, chosen) {
            (TStage::Redistribute, SOpt::Split { tactic, fleet, strategic }) => {
                apply_split(sx.state, self.who, tactic, fleet, strategic);
                self.stage = TStage::Choose;
            }
            (TStage::Choose, SOpt::System(system)) => {
                self.chosen = Some(system);
                self.stage = TStage::Done;
            }
            _ => return Err(Illegal),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(total: u8, floor: u8) -> Vec<(u8, u8, u8)> {
        splits(total, floor)
            .map(|o| match o {
                SOpt::Split { tactic, fleet, strategic } => (tactic, fleet, strategic),
                _ => unreachable!(),
            })
            .collect()
    }

    /// `tokens::distribution_options`: every split of the tokens, tactic pool first, at least `floor` in fleet.
    #[test]
    fn the_splits_keep_the_fleet_floor_and_the_string_order() {
        assert_eq!(all(2, 2), vec![(0, 2, 0)], "a total at the floor leaves one split");
        assert_eq!(all(3, 2), vec![(0, 2, 1), (0, 3, 0), (1, 2, 0)]);
        assert_eq!(all(1, 2), vec![(0, 0, 1), (0, 1, 0), (1, 0, 0)], "below the floor every split is offered");
        assert_eq!(all(9, 2).len(), 36);
        assert_eq!(all(9, 0).len(), 55);
        assert!(all(9, 2).iter().all(|(t, f, s)| t + f + s == 9 && *f >= 2));
    }

    // -- Imperial's secret objective draw and the hand limit (package W6) ------------------------------------------

    use crate::containers::Deck;
    use crate::driver::{Driver, Status};
    use crate::generated::SecretObjective as Sec;
    use crate::kernel::stage::AbilityTable;
    use crate::kernel::Resolver;
    use crate::state::{Player, Players};

    /// One seat (an Arborec) holding `hand`, a secret deck of `deck`, and a table with (`linked`) or without the hand-limit link.
    fn drawing(hand: &[Sec], deck: &[Sec]) -> CoreState {
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::TheArborec)]);
        s.seats = crate::state::Seats::new(&[Seat(0)], &[Seat(0)], 1);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.secrets = hand.iter().copied().collect();
        s.decks_mut().secret = Deck::new(deck.to_vec());
        s
    }

    fn table(linked: bool) -> AbilityTable {
        let mut t = AbilityTable::default();
        if linked {
            crate::driver::children::register(&mut t);
        }
        t
    }

    /// Run `window` to its first question; returns the status and the driver.
    fn start<W: Window>(window: W, s: &mut CoreState, linked: bool) -> (Status, Driver<W>) {
        let mut d = Driver::new(window, Resolver::new(&[Seat(0)], Some(Seat(0))), table(linked), 1);
        let status = d.run(s, &Kernel::plain()).unwrap();
        (status, d)
    }

    const FULL: [Sec; 3] = [Sec::AdaptNewStrategies, Sec::BetrayAFriend, Sec::BecomeAMartyr];

    /// 45.4: the drawn fourth secret is asked about, the one handed back goes to the bottom of the deck, the hand is three again.
    #[test]
    fn imperial_secondary_asks_which_secret_to_return_over_the_limit() {
        let mut s = drawing(&FULL, &[Sec::BraveTheVoid, Sec::CutSupplyLines]);
        assert!(!ImperialSecondary::new(Seat(0)).stage(), "not started");
        let (status, mut d) = start(ImperialSecondary::new(Seat(0)), &mut s, true);
        assert!(d.window.stage(), "started once the draw is made");
        let Status::Asking(choice) = status else { panic!("the hand is over its limit: a question") };
        assert_eq!(choice.opts.len(), 4, "the three held and the one drawn");
        let pick = choice.opts.iter().position(|o| matches!(o, Opt::Objectives(crate::choice::objectives::Opt::Return(Sec::BetrayAFriend)))).unwrap();
        let status = d.resume(&mut s, &Kernel::plain(), choice.pick(pick).unwrap()).unwrap();
        assert!(matches!(status, Status::Done));
        let hand = s.players().by_seat(Seat(0)).unwrap().cards.secrets.as_slice().to_vec();
        assert_eq!(hand, vec![Sec::AdaptNewStrategies, Sec::BecomeAMartyr, Sec::BraveTheVoid]);
        assert_eq!(s.decks().secret.cards(), &[Sec::CutSupplyLines, Sec::BetrayAFriend], "the returned secret is at the bottom");
        assert!(!d.window.unsupported);
    }

    /// A hand within the limit, an empty deck and an all-scored hand ask nothing.
    #[test]
    fn imperial_secondary_asks_nothing_within_the_limit_or_without_a_deck() {
        let mut s = drawing(&FULL[..2], &[Sec::BraveTheVoid]);
        let (status, _) = start(ImperialSecondary::new(Seat(0)), &mut s, true);
        assert!(matches!(status, Status::Done));
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.secrets.len(), 3);
        let mut s = drawing(&FULL, &[]);
        let (status, _) = start(ImperialSecondary::new(Seat(0)), &mut s, true);
        assert!(matches!(status, Status::Done), "an empty deck draws nothing, so nothing goes back");
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.secrets.len(), 3);
    }

    /// Bail: a table without the link to the hand-limit window draws and declines (`unsupported`), it never skips the limit silently.
    #[test]
    fn bail_imperial_without_the_hand_limit_link_is_unsupported() {
        let mut s = drawing(&FULL, &[Sec::BraveTheVoid]);
        let (status, d) = start(ImperialSecondary::new(Seat(0)), &mut s, false);
        assert!(matches!(status, Status::Done));
        assert!(d.window.unsupported);
        let mut s = drawing(&FULL, &[Sec::BraveTheVoid]);
        let (_, d) = start(ImperialPrimary::new(Seat(0), SmallList::new()), &mut s, false);
        assert!(d.window.unsupported, "the primary of a seat without Mecatol Rex draws too");
    }

    /// The primary with nothing to score draws and returns a secret the same way; with Mecatol Rex it draws nothing.
    #[test]
    fn imperial_primary_without_mecatol_returns_a_secret_over_the_limit() {
        let mut s = drawing(&FULL, &[Sec::BraveTheVoid]);
        let (status, mut d) = start(ImperialPrimary::new(Seat(0), SmallList::new()), &mut s, true);
        let Status::Asking(choice) = status else { panic!("a question") };
        let status = d.resume(&mut s, &Kernel::plain(), choice.pick(0).unwrap()).unwrap();
        assert!(matches!(status, Status::Done));
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.secrets.len(), 3);
        assert_eq!(s.decks().secret.len(), 1, "one went back");
        let mut s = drawing(&FULL, &[Sec::BraveTheVoid]);
        s.board_mut().planet_mut(crate::generated::Planet::MecatolRex).control = Some(Seat(0));
        let (status, _) = start(ImperialPrimary::new(Seat(0), SmallList::new()), &mut s, true);
        assert!(matches!(status, Status::Done), "a point for Mecatol Rex, no draw");
        assert_eq!(s.decks().secret.len(), 1);
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.victory_points, 1, "exactly one point");
    }

    // -- Politics: the draw is a child window (package W6) -----------------------------------------------------------

    /// Two seats: seat 0 (the speaker, holding Politics) draws two cards from `deck` with `hand` in its hand; `agendas` on the
    /// agenda deck.
    fn politics_table(hand: usize, deck: &[crate::generated::ActionCard], agendas: &[Agenda]) -> CoreState {
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::TheArborec), Player::new(Seat(1), Faction::TheEmiratesOfHacan)]);
        s.seats = crate::state::Seats::new(&[Seat(0), Seat(1)], &[Seat(0), Seat(1)], 2);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.hand = (0..hand).map(|i| crate::generated::ActionCard::ALL[100 + i]).collect();
        s.decks_mut().action = Deck::new(deck.to_vec());
        s.decks_mut().agenda = Deck::new(agendas.to_vec());
        s
    }

    fn groups_table() -> AbilityTable {
        let mut t = table(true);
        t.facts.card_groups = (0..crate::generated::ActionCard::COUNT).map(|i| u8::try_from(i).unwrap()).collect();
        t
    }

    /// The secondary draws two cards; over the limit the discard is asked, and the primary asks the speaker first and the agendas
    /// after the discard.
    #[test]
    fn politics_asks_the_speaker_then_the_discard_then_the_agendas() {
        use crate::generated::ActionCard as C;
        let k = Kernel::plain();
        let mut s = politics_table(6, &[C::ALL[0], C::ALL[1], C::ALL[2]], &[Agenda::ALL[0], Agenda::ALL[1], Agenda::ALL[2]]);
        let mut d = Driver::new(Politics::new(Seat(0), true, false), Resolver::new(&[Seat(0)], Some(Seat(0))), groups_table(), 1);
        let mut asked = Vec::new();
        let mut status = d.run(&mut s, &k).unwrap();
        while let Status::Asking(choice) = status {
            asked.push(choice.ask);
            status = d.resume(&mut s, &k, choice.pick(0).unwrap()).unwrap();
        }
        assert_eq!(asked[0], Ask::Strategy(SAsk::PoliticsSpeaker));
        assert_eq!(asked[1], Ask::Cards(crate::choice::cards::Ask::HandLimit { held: 8 }));
        assert!(matches!(asked[2], Ask::Strategy(SAsk::PoliticsAgenda { .. })) && matches!(asked[3], Ask::Strategy(SAsk::PoliticsAgenda { .. })), "{asked:?}");
        assert_eq!(asked.len(), 4);
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.hand.len(), 7);
        assert_eq!(s.meta().speaker, Seat(1));
        assert_eq!(s.decks().agenda.len(), 3, "the agendas went back");
        assert!(!d.window.unsupported);
        // The secondary draws and looks at no agenda.
        let mut s = politics_table(6, &[C::ALL[0], C::ALL[1], C::ALL[2]], &[Agenda::ALL[0]]);
        let mut d = Driver::new(Politics::new(Seat(0), false, false), Resolver::new(&[Seat(0)], Some(Seat(0))), groups_table(), 1);
        let Status::Asking(choice) = d.run(&mut s, &k).unwrap() else { panic!("a discard") };
        assert_eq!(d.resume(&mut s, &k, choice.pick(0).unwrap()).unwrap(), Status::Done);
        assert_eq!(s.decks().agenda.len(), 1, "no agenda was looked at");
    }

    /// Bail: a table with the link but no printed-name groups cannot ask the discard: the draw window reports `Left` and Politics is
    /// unsupported (the speaker is already named, the string performs the card).
    #[test]
    fn bail_politics_without_name_groups_over_the_limit_is_unsupported() {
        let k = Kernel::plain();
        let mut s = politics_table(6, &[crate::generated::ActionCard::ALL[0], crate::generated::ActionCard::ALL[1]], &[]);
        let mut d = Driver::new(Politics::new(Seat(0), false, false), Resolver::new(&[Seat(0)], Some(Seat(0))), table(true), 1);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(d.window.unsupported);
    }

    /// Bail: a table without the link to the draw window ends the window unsupported, drawing nothing.
    #[test]
    fn bail_politics_without_the_draw_link_is_unsupported() {
        let mut s = politics_table(0, &[crate::generated::ActionCard::ALL[0]], &[]);
        let mut d = Driver::new(Politics::new(Seat(0), false, false), Resolver::new(&[Seat(0)], Some(Seat(0))), table(false), 1);
        assert_eq!(d.run(&mut s, &Kernel::plain()).unwrap(), Status::Done);
        assert!(d.window.unsupported);
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.hand.len(), 0);
    }
}
