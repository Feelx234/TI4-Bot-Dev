//! The strategic action as native arms (package KX, lane E; `docs/todo/D2-islands-composition.md` section 6.2): the follower window
//! (`Arm::Secondary`, `Game::step_secondary`), the Winnu hero's follower selection (`Arm::LeaderFollowers`,
//! `Game::step_leader_followers`) and the end of a game during an action (`Arm::Finished`), plus the abilities the followers perform
//! ([`perform`]) and the facts they read ([`facts`]).
//!
//! Two HELD windows, each a machine rebuilt from its payload (`state::Secondary`, `state::LeaderFollowers`):
//!
//! * [`SecondaryWin`]: one step of the string is one follower. The followers who may not follow are recorded as ineligible (nothing is asked
//!   of them), the next one is asked whether to follow, decline or use a faction's token-free way; the answer pays the token or the waiver,
//!   says `STRATEGY_SECONDARY_FOLLOWED/DECLINED` and performs the secondary (or, for a faction whose ability substitutes it, the primary) of
//!   the card as the card's own window ([`perform::run`]: every question it asks goes to the TAPE, like the jobs of the turn flow, so a caller
//!   that returns between two questions continues from the position alone). When the last follower is done the primary player's card is
//!   exhausted, a hero's strategy completes, `STRATEGIC_ACTION_COMPLETE` is said and the action ends (`TurnJob::Finish`, run in the same
//!   call: the string's `finish_action` is part of the step).
//! * [`LeaderFollowersWin`]: Mathis Mathinus' owner is asked, seat by seat, which seats may follow the secondary of the card; the last answer
//!   opens the follower window of that card ([`Secondary::exhausts_card`] false: nobody holds it).
//!
//! L5: it composes the windows of the strategy areas through the table's links, never imports one.

pub mod after;
pub mod begin;
pub mod facts;
pub mod perform;

use crate::choice::leaders::{Ask as LAsk, Opt as LOpt, Subtype as LSub};
use crate::choice::strategy::Ask as SAsk;
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::driver::live::{advance, ask, is_open, Asking, Env, Live, LiveError, Out};
use crate::driver::turn::inline::Flow;
use crate::driver::turn::{ask_job, open_job, TurnFlowWin};
use crate::generated::{Leader, StrategyCard};
use crate::ids::Seat;
use crate::kernel::stage::{Held, Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::rules::strategy_flow;
use crate::rules::strategy_secondary::{follow_choice, take_answer, Resolution, WaiverEffect};
use crate::state::{CoreState, LeaderFollowers, PlainKind, PlainRec, Secondary, SecondaryHow, TurnJob, Window as OpenWindow, WindowKind};

/// `Game::complete_leader_strategy`: a hero's strategy that was resolving ends, its follower selection with it, and the hero is purged
/// (`COMPONENT_ACTION_RESOLVED`). Nothing when no hero is resolving.
pub fn complete_leader_strategy(core: &mut CoreState) {
    let Some(OpenWindow::LeaderStrategy { player, leader, .. }) = core.pending().window(WindowKind::LeaderStrategy).cloned() else { return };
    core.pending_mut().close(WindowKind::LeaderStrategy);
    core.pending_mut().close(WindowKind::LeaderFollowers);
    crate::rules::leaders::purge(core, player, leader);
    crate::kernel::fx::plain(core, PlainRec::bare(PlainKind::ComponentActionResolved));
}

// -- the follower window ---------------------------------------------------------------------------------------------------------------

/// The held follower window of a strategic action (`Window::StrategySecondary`).
#[derive(Debug, Clone)]
pub struct SecondaryWin {
    w: Secondary,
}

impl SecondaryWin {
    /// The window of `payload`.
    #[must_use]
    pub const fn new(payload: Secondary) -> Self {
        Self { w: payload }
    }

    /// The payload as it stands.
    #[must_use]
    pub const fn payload(&self) -> &Secondary {
        &self.w
    }

    fn current(&self) -> Option<Seat> {
        self.w.followers.as_slice().get(usize::from(self.w.next)).copied()
    }

    /// `StrategySecondaryWindow::next_choice`'s loop: the followers who may not be asked are recorded as ineligible, in order.
    fn skip_ineligible(&mut self, core: &CoreState, kernel: &Kernel) {
        while let Some(who) = self.current() {
            if facts::follower_facts(core, kernel, self.w.primary, self.w.card, who).eligible {
                break;
            }
            self.w.resolved.push((who, SecondaryHow::Ineligible));
            self.w.next += 1;
        }
    }

    /// The question of the next follower who may be asked, without recording anything (`StrategySecondaryWindow::pending_choice`).
    fn pending(&self, core: &CoreState, kernel: &Kernel) -> Option<Choice> {
        self.w.followers.as_slice().iter().skip(usize::from(self.w.next)).find_map(|who| {
            let f = facts::follower_facts(core, kernel, self.w.primary, self.w.card, *who);
            f.eligible.then(|| follow_choice(core, self.w.card, self.w.primary, &f))
        })
    }

    fn is_complete(&self) -> bool {
        usize::from(self.w.next) == self.w.followers.len()
    }

    /// `StrategySecondaryWindow::exhaust_primary`: the last follower exhausts the primary player's copy of the card.
    fn exhaust_if_complete(&self, core: &mut CoreState) {
        if self.is_complete() && self.w.exhausts_card {
            strategy_flow::exhaust(core, self.w.primary, self.w.card);
        }
    }

    /// The end of the window (`Game::step_secondary`): a preview announces itself and goes; otherwise the hero's strategy completes and the
    /// action is complete.
    fn complete(&self, sx: &mut Sx<'_>) {
        if self.w.preview {
            sx.plain(PlainRec::bare(PlainKind::StrategySecondaryPreviewComplete));
            return;
        }
        complete_leader_strategy(sx.state);
        sx.plain(PlainRec::bare(PlainKind::StrategicActionComplete));
    }

    /// The follower's secondary (or the primary a faction substitutes), as the card's own window; then what the faction does when a strategy card
    /// finishes for it (`resolve_faction_strategy`).
    fn follow(&self, sx: &mut Sx<'_>, who: Seat) -> Flow {
        let card = self.w.card;
        let kind = card.data().kind;
        let substitutes = crate::rules::strategy::Env::substitutes_primary(&crate::driver::kernel_env::KernelEnv::new(sx.state, sx.kernel), who, Some(kind));
        let mut free = None;
        if perform::run(sx, who, card, substitutes, &mut free) == Flow::Halt {
            return Flow::Halt;
        }
        perform::resolved(sx, who, kind)
    }
}

impl Window for SecondaryWin {
    type Stage = bool;

    // mutants:skip: the stage is the driver's debug identity of the window; no rule reads it
    fn stage(&self) -> bool {
        self.is_complete()
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        self.skip_ineligible(sx.state, sx.kernel);
        let Some(who) = self.current() else {
            // No follower may follow: the window is over, the card is spent.
            self.exhaust_if_complete(sx.state);
            self.complete(sx);
            return Next::Done;
        };
        let f = facts::follower_facts(sx.state, sx.kernel, self.w.primary, self.w.card, who);
        Next::Continue(Ask::Strategy(SAsk::Follow { card: self.w.card, primary: self.w.primary, costs_token: f.costs_token }))
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        self.pending(s, k)
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        self.skip_ineligible(sx.state, sx.kernel);
        let who = self.current().ok_or(Illegal)?;
        let Pick::Opt(Opt::Strategy(opt)) = pick else { return Err(Illegal) };
        let f = facts::follower_facts(sx.state, sx.kernel, self.w.primary, self.w.card, who);
        // The question is the follower's: an option it did not offer is refused before anything changes.
        // (`take_answer` refuses an option the follower was not offered too: this check only comes first.)
        if !follow_choice(sx.state, self.w.card, self.w.primary, &f).opts.iter().any(|o| *o == Opt::Strategy(opt)) { // mutants:skip: equivalent, see above
            return Err(Illegal);
        }
        // A waiver whose payment the core does not carry is offered like the string offers it, and stops the step when it is taken.
        if f.waivers.iter().any(|w| w.opt == opt && w.effect == WaiverEffect::Unported) {
            crate::driver::turn::inline::gap(sx, crate::kernel::turn_codes::TurnGap::StrategyAbility);
            return Ok(());
        }
        let resolution = take_answer(sx.state, self.w.primary, sx.table.codes.turn.strategy_secondary, &f, opt)?;
        self.w.resolved.push((who, if resolution == Resolution::Followed { SecondaryHow::Followed } else { SecondaryHow::Declined }));
        self.w.next += 1;
        self.exhaust_if_complete(sx.state);
        sx.plain(PlainRec::bare(if resolution == Resolution::Followed { PlainKind::StrategySecondaryFollowed } else { PlainKind::StrategySecondaryDeclined }));
        if resolution == Resolution::Followed {
            // A halted follow (a question waits for its answer, or a gap was noted) writes nothing more: the driver rolls the step back.
            let _: Flow = self.follow(sx, who);
        }
        Ok(())
    }
}

impl Held for SecondaryWin {
    const KIND: WindowKind = WindowKind::StrategySecondary;

    fn from_pending(core: &CoreState, _: &Kernel) -> Option<Self> {
        match core.pending().window(WindowKind::StrategySecondary)? {
            OpenWindow::StrategySecondary(s) => Some(Self::new(s.clone())),
            _ => None,
        }
    }

    fn to_pending(&self, core: &mut CoreState) {
        core.pending_mut().open(OpenWindow::StrategySecondary(self.w.clone()));
    }
}

/// The follower window of `primary`'s `card` over `followers` (`begin_strategic_action`: clockwise from the primary player, the player
/// excluded; `exhausts_card` false for a card nobody holds).
#[must_use]
pub fn secondary_of(primary: Seat, card: StrategyCard, followers: &[Seat], exhausts_card: bool) -> Secondary {
    Secondary { primary, card, followers: followers.iter().copied().collect(), next: 0, resolved: SmallList::new(), exhausts_card, preview: false }
}

/// Run the job `job` right after a window of this arm ended and merge what it did into `out` (the string's `finish_action` is part of the step).
fn close_with(core: &mut CoreState, env: &Env<'_>, job: TurnJob, mut out: Out) -> Result<(Live, Out), LiveError> {
    open_job(core, job);
    let (live, more) = advance::<TurnFlowWin>(core, env, None)?;
    out.emitted.extend(more.emitted);
    out.opened.extend(more.opened);
    Ok((live, out))
}

/// `Arm::Secondary`: one follower of the open window; the call that ends the window also ends the action.
///
/// # Errors
/// As [`advance`]; the gaps of the ability a follower performs.
pub fn advance_secondary(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    // A close that asked a question from inside its step is a job on the stack: it continues first.
    if is_open::<TurnFlowWin>(core) && !is_open::<SecondaryWin>(core) {
        return advance::<TurnFlowWin>(core, env, answer);
    }
    let preview = matches!(core.pending().window(WindowKind::StrategySecondary), Some(OpenWindow::StrategySecondary(s)) if s.preview);
    let before = core.clone();
    let result = advance::<SecondaryWin>(core, env, answer).and_then(|(live, out)| match live {
        // A preview must not finish the action (it is a planning fork's one-follower window).
        Live::Done if !preview => close_with(core, env, TurnJob::Finish, out),
        other => Ok((other, out)),
    });
    if result.is_err() {
        *core = before;
    }
    result
}

/// The question of `Arm::Secondary`.
///
/// # Errors
/// As [`ask`].
pub fn ask_secondary(core: &CoreState, env: &Env<'_>) -> Result<Option<Asking>, LiveError> {
    if is_open::<TurnFlowWin>(core) && !is_open::<SecondaryWin>(core) {
        return ask_job(core, env);
    }
    ask::<SecondaryWin>(core, env)
}

// -- the Winnu hero's follower selection ----------------------------------------------------------------------------------------------------

/// Mathis Mathinus' follower selection (`Window::LeaderFollowers`): the hero's owner is asked about the seats still to ask, the next first.
#[derive(Debug, Clone)]
pub struct LeaderFollowersWin {
    w: LeaderFollowers,
    /// The hero's owner, the card whose secondary is offered.
    owner: Seat,
    card: StrategyCard,
}

fn hero_ctx() -> Ctx {
    Ctx {
        origin: Origin::FactionAbility(crate::choice::generic::Content::Leader(Leader::MathisMathinus)),
        subtype: Subtype::Leaders(LSub::HeroFollower),
        optional: false,
        target: None,
        outstanding: SmallList::new(),
        space_battle: false,
        invasion_seq: None,
    }
}

impl LeaderFollowersWin {
    /// The window of `payload` for the hero resolving `card` for `owner`.
    #[must_use]
    pub const fn new(payload: LeaderFollowers, owner: Seat, card: StrategyCard) -> Self {
        Self { w: payload, owner, card }
    }
}

impl Window for LeaderFollowersWin {
    type Stage = usize;

    // mutants:skip: the stage is the driver's debug identity of the window; no rule reads it
    fn stage(&self) -> usize {
        self.w.asks.len()
    }

    fn step(&mut self, _: &mut Sx<'_>) -> Next {
        match self.w.asks.as_slice().first() {
            Some(who) => Next::Continue(Ask::Leaders(LAsk::WinnuFollower { follower: *who })),
            None => Next::Done,
        }
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        let who = *self.w.asks.as_slice().first()?;
        Some(Choice {
            actor: self.owner,
            ask: Ask::Leaders(LAsk::WinnuFollower { follower: who }),
            ctx: hero_ctx(),
            opts: [LOpt::HeroFollower(true), LOpt::HeroFollower(false)].into_iter().map(Opt::Leaders).collect::<Opts>(),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let who = *self.w.asks.as_slice().first().ok_or(Illegal)?;
        let Pick::Opt(Opt::Leaders(LOpt::HeroFollower(yes))) = pick else { return Err(Illegal) };
        let rest: SmallList<Seat, 8> = self.w.asks.as_slice()[1..].iter().copied().collect();
        self.w.asks = rest;
        if yes {
            self.w.followers.push(who);
        }
        if self.w.asks.is_empty() {
            // The last answer opens the follower window of the card nobody holds (`StrategySecondaryWindow::foreign`).
            let followers: Vec<Seat> = self.w.followers.as_slice().to_vec();
            sx.state.pending_mut().open(OpenWindow::StrategySecondary(secondary_of(self.owner, self.card, &followers, false)));
        }
        Ok(())
    }
}

impl Held for LeaderFollowersWin {
    const KIND: WindowKind = WindowKind::LeaderFollowers;

    fn from_pending(core: &CoreState, _: &Kernel) -> Option<Self> {
        let OpenWindow::LeaderFollowers(w) = core.pending().window(WindowKind::LeaderFollowers)? else { return None };
        // The hero resolving: the window cannot be rebuilt without it.
        let OpenWindow::LeaderStrategy { player, card, .. } = core.pending().window(WindowKind::LeaderStrategy)? else { return None };
        Some(Self::new(w.clone(), *player, *card))
    }

    fn to_pending(&self, core: &mut CoreState) {
        core.pending_mut().open(OpenWindow::LeaderFollowers(self.w.clone()));
    }
}

/// `Game::select_leader_strategy_followers`: the hero's primary resolved; the Winnu hero asks who may follow, any other hero's card has no
/// followers. Opens the window that is next.
pub fn select_leader_strategy_followers(core: &mut CoreState) {
    let Some(OpenWindow::LeaderStrategy { player, leader, card }) = core.pending().window(WindowKind::LeaderStrategy).cloned() else { return };
    let is_winnu = core.players().by_seat(player).is_some_and(|p| p.faction == crate::generated::Faction::TheWinnu);
    let asks: SmallList<Seat, 8> = if leader == Leader::MathisMathinus && is_winnu {
        // The seats after the hero's owner, clockwise (`state.seating_order`).
        let order = core.seats().seating_order();
        match order.iter().position(|s| *s == player) {
            Some(start) => (1..order.len()).map(|step| order[(start + step) % order.len()]).collect(),
            None => SmallList::new(),
        }
    } else {
        SmallList::new()
    };
    if asks.is_empty() {
        core.pending_mut().open(OpenWindow::StrategySecondary(secondary_of(player, card, &[], false)));
    } else {
        core.pending_mut().open(OpenWindow::LeaderFollowers(LeaderFollowers { asks, followers: SmallList::new() }));
    }
}

/// `Arm::LeaderFollowers`.
///
/// # Errors
/// As [`advance`].
pub fn advance_leader_followers(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    advance::<LeaderFollowersWin>(core, env, answer)
}

/// The question of `Arm::LeaderFollowers`.
///
/// # Errors
/// As [`ask`].
pub fn ask_leader_followers(core: &CoreState, env: &Env<'_>) -> Result<Option<Asking>, LiveError> {
    ask::<LeaderFollowersWin>(core, env)
}

// -- the end of a game during an action ---------------------------------------------------------------------------------------------------

/// `Arm::Finished`'s body in `Game::step_flow` (the lent remainder: `complete_leader_strategy`, `secondary = None`; the diplomacy settlement is
/// dropped with structured diplomacy): the windows an action held when the game ended are closed.
pub fn close_finished(core: &mut CoreState) {
    complete_leader_strategy(core);
    core.pending_mut().close(WindowKind::StrategySecondary);
}

/// `Arm::Finished`: the held windows of an action are closed (`close_finished`); nothing is asked.
///
/// # Errors
/// Never.
pub fn advance_finished(core: &mut CoreState, _: &Env<'_>, _: Option<Pick>) -> Result<(Live, Out), LiveError> {
    close_finished(core);
    Ok((Live::Done, Out::default()))
}

/// The question of `Arm::Finished`: none.
///
/// # Errors
/// Never.
pub fn ask_finished(_: &CoreState, _: &Env<'_>) -> Result<Option<Asking>, LiveError> {
    Ok(None)
}
