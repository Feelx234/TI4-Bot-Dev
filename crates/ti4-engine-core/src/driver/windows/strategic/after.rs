//! The end of the free tactical action of a strategy card's primary (`Game::close_tactical`'s hero and `secondary_after_tactical` branches; Thunder's
//! Edge Warfare): "you may redistribute your command tokens before and after this action". The seat redistributes once more, then the hero's
//! strategy goes on to its followers (`select_leader_strategy_followers`) or the follower window held for the end of the action opens.
//!
//! The tactical tail (lane M6, `driver::tactical_tail::then`) opens this job with [`open_after`] where it says `Then::LeaderStrategy` or
//! `Then::Secondary`; a plain tactical action ends with `finish_action` (`driver::turn::open_finish`).

use crate::choice::strategy::{ctx, Ask as SAsk, Opt as SOpt, Subtype as SSub};
use crate::choice::{Ask, Choice, Opt, Opts, Pick};
use crate::driver::turn::inline::{gap, run, Flow};
use crate::driver::turn::open_job;
use crate::generated::{Faction, StrategyCard};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::turn_codes::TurnGap;
use crate::kernel::{Kernel, Next};
use crate::rules::strategy_primaries::{apply_split, sheet_total, splits};
use crate::state::{CoreState, TurnJob, Window as OpenWindow, WindowKind};

/// The redistribution after the free tactical action (`redistribute_around_te_warfare(.., before = false)`): one question, the final split of the
/// sheet; none for a seat with no command tokens.
#[derive(Debug, Clone)]
pub struct AfterRedistribute {
    who: Seat,
    zero_floor: bool,
    asked: bool,
}

impl AfterRedistribute {
    #[must_use]
    pub const fn new(who: Seat, zero_floor: bool) -> Self {
        Self { who, zero_floor, asked: false }
    }

    fn floor(&self, core: &CoreState) -> u8 {
        let letnev = core.players().by_seat(self.who).is_some_and(|p| p.faction == Faction::TheBaronyOfLetnev);
        if letnev && self.zero_floor { 0 } else { 2 }
    }

    fn ask_now(&self, core: &CoreState) -> Option<SAsk> {
        let total = u8::try_from(sheet_total(core, self.who)).unwrap_or(u8::MAX);
        (!self.asked && total > 0).then(|| SAsk::TeWarfareRedistributeAfter { total, floor: self.floor(core) })
    }
}

impl Window for AfterRedistribute {
    type Stage = bool;

    fn stage(&self) -> bool {
        self.asked
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        match self.ask_now(sx.state) {
            Some(a) => Next::Continue(Ask::Strategy(a)),
            None => Next::Done,
        }
    }

    fn ask_of(&self, core: &CoreState, _: &Kernel) -> Option<Choice> {
        let a = self.ask_now(core)?;
        let SAsk::TeWarfareRedistributeAfter { total, floor } = a else { return None };
        Some(Choice {
            actor: self.who,
            ask: Ask::Strategy(a),
            ctx: ctx(StrategyCard::Warfare, false, SSub::WarfareRedistributeTokens),
            opts: splits(total, floor).map(Opt::Strategy).collect::<Opts>(),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let Pick::Opt(Opt::Strategy(chosen)) = pick else { return Err(Illegal) };
        let offered = self.ask_of(sx.state, sx.kernel).ok_or(Illegal)?;
        if !offered.opts.as_slice().contains(&Opt::Strategy(chosen)) {
            return Err(Illegal);
        }
        let SOpt::Split { tactic, fleet, strategic } = chosen else { return Err(Illegal) };
        apply_split(sx.state, self.who, tactic, fleet, strategic);
        self.asked = true;
        Ok(())
    }
}

/// `Game::redistribute_after_te_warfare`: the seat redistributes (nothing once the game is over).
fn redistribute(sx: &mut Sx<'_>, who: Seat) -> Flow {
    if sx.state.meta().finished {
        return Flow::Done;
    }
    // (Latest rules: the Letnev fleet pool may be emptied.)
    let zero_floor = true;
    run(sx, &mut AfterRedistribute::new(who, zero_floor))
}

/// The job: the hero's strategy, else the held follower window; nothing else owes the end of a free tactical action.
pub fn run_after(sx: &mut Sx<'_>) -> Flow {
    if let Some(OpenWindow::LeaderStrategy { player, .. }) = sx.state.pending().window(WindowKind::LeaderStrategy).cloned() {
        if redistribute(sx, player) == Flow::Halt {
            return Flow::Halt;
        }
        super::select_leader_strategy_followers(sx.state);
        return Flow::Done;
    }
    if let Some(OpenWindow::StrategySecondaryAfterTactical(window)) = sx.state.pending().window(WindowKind::StrategySecondaryAfterTactical).cloned() {
        sx.state.pending_mut().close(WindowKind::StrategySecondaryAfterTactical);
        // Only Thunder's Edge Warfare returns a free tactical action from a primary ability: the active seat redistributes.
        let Some(active) = sx.state.meta().active else {
            gap(sx, TurnGap::ReplayDiverged);
            return Flow::Halt;
        };
        if redistribute(sx, active) == Flow::Halt {
            return Flow::Halt;
        }
        sx.state.pending_mut().open(OpenWindow::StrategySecondary(window));
    }
    Flow::Done
}

/// The free tactical action ended: open the job (the next live call runs it).
pub fn open_after(core: &mut CoreState) {
    open_job(core, TurnJob::AfterTactical);
}
