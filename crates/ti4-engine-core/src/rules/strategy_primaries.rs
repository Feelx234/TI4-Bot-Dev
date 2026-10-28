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
use crate::choice::{Ask, Choice, Opt, Opts, Pick};
use crate::containers::SmallList;
use crate::generated::{Agenda, Faction, Planet, StrategyCard, System};
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::{board, seats};
use crate::state::CoreState;

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
    /// Waiting on where an agenda goes.
    Agenda,
    Done,
}

/// Politics: the primary (a new speaker, two action cards, the top two agendas back on the deck in an order
/// of the holder's choosing) and the secondary (two action cards).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Politics {
    pub who: Seat,
    primary: bool,
    stage: PStage,
    /// The unofficial action cards leave the deck before the draw.
    drop_unofficial: bool,
    /// The agendas looked at, in the order they were taken off the deck.
    looked: SmallList<Agenda, 2>,
    next: u8,
}

impl Politics {
    #[must_use]
    pub const fn new(who: Seat, primary: bool, drop_unofficial: bool) -> Self {
        Self { who, primary, stage: PStage::Start, drop_unofficial, looked: SmallList::new(), next: 0 }
    }

    /// Seats that may become speaker: everyone but the speaker, in seating order.
    fn candidates(core: &CoreState) -> SmallList<Seat, 16> {
        let speaker = core.meta().speaker;
        seats::seating_order(core).as_slice().iter().copied().filter(|s| *s != speaker).collect()
    }

    /// Draw the two action cards and look at the top two agendas.
    fn draw_and_look(&mut self, core: &mut CoreState) {
        fx::draw_action_cards(core, self.who, 2, self.drop_unofficial);
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
                self.draw_and_look(sx.state);
            }
        }
        match self.stage {
            PStage::Speaker => Next::Continue(ask(SAsk::PoliticsSpeaker)),
            PStage::Agenda => Next::Continue(ask(SAsk::PoliticsAgenda { agenda: self.looked.as_slice()[usize::from(self.next)] })),
            PStage::Start | PStage::Done => Next::Done,
        }
    }

    fn ask_of(&self, core: &CoreState, _: &Kernel) -> Option<Choice> {
        match self.stage {
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
        let Pick::Opt(Opt::Strategy(chosen)) = pick else { return Err(Illegal) };
        let offered = self.ask_of(sx.state, sx.kernel).ok_or(Illegal)?;
        if !offered.opts.as_slice().contains(&Opt::Strategy(chosen)) {
            return Err(Illegal);
        }
        match (self.stage, chosen) {
            (PStage::Speaker, SOpt::Speaker(seat)) => {
                sx.state.meta_mut().speaker = seat;
                self.draw_and_look(sx.state);
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
}
