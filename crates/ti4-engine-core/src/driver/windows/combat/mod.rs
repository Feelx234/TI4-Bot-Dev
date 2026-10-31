//! The space combat of an aftermath as a window the core drives and holds (lane M5 / M4 "combat live"; the string `CombatWindow::{new, settle,
//! pending_choice, resolve}`). L5: it runs the composed machine of `rules::combat::machine` in its LIVE mode and keeps where the fight stands as
//! the typed payload [`crate::state::CombatFields`] on the `Pending` stack (`Window::Combat`, above the aftermath's entry). The machine and its
//! environment are rebuilt from `(fields, position)` at every call; a checkpoint is a plain `CoreState::clone`.
//!
//! The machine stops at the decisions of the players (the announcement of a retreat, the retreat's destination, a sustain, a casualty): those
//! are the arm's stage questions. A question of a child window (a reroll, Raid Formation, an Ambush hit, Assault Cannon's loss, the Letnev
//! commander's payment) is asked through the tape ([`Machine::live`]): the driver reports it as a nested question and replays the step with
//! the answer, so nothing of a child has to be kept between two calls.
//!
//! Pauses: the anti-fighter barrage's feat stops the machine (`Stop::Scoring`); the arm leaves the combat on the stack and opens the scoring
//! window above it, as the invasion does. The fight's end is the window leaving the stack: [`CombatLive::closed`] writes the outcome into the
//! aftermath's entry, which owns the feats and everything after.
//!
//! What the machine does not carry is a counted bail: `Unknown::UnportedAftermath(AftermathGap::CombatUnported | RollEndsFight)`.

mod env;

use crate::choice::{Choice, Pick};
use crate::containers::SmallList;
use crate::driver::live::{ask, Asking, Env, Live, LiveError, Out};
use crate::driver::windows::live_common::{advance_parkable, After};
use crate::generated::System;
use crate::ids::Seat;
use crate::kernel::ability::{AftermathGap, Unknown};
use crate::kernel::stage::{Held, Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::reach;
use crate::rules::combat::machine::{Bail, Machine, Stage, Stop};
use crate::rules::combat::{combatants, ships_of, Rd};
use crate::state::{AFight, CombatFields, CombatStep, CoreState, HitPhase, Window as OpenWindow, WindowKind};

pub use env::{env_of, sides, staging_enabled};

/// The occurrence the aftermath's entry holds for this fight (0: none yet).
fn occurrence_of(core: &CoreState) -> u64 {
    match core.pending().window(WindowKind::Aftermath) {
        Some(OpenWindow::Aftermath(a)) => a.occurrence,
        _ => 0,
    }
}

/// The open fight.
#[derive(Debug, Clone)]
pub struct CombatLive {
    /// The machine, in its live mode.
    pub m: Machine,
    /// The environment was rebuilt with the adapter's texts and the table of this call (it is once per object: the occurrence the machine
    /// begins in a step must survive the steps that follow the answer).
    refreshed: bool,
}

impl CombatLive {
    fn refresh(&mut self, sx: &Sx<'_>) {
        if std::mem::replace(&mut self.refreshed, true) {
            return;
        }
        if let Some(env) = env_of(sx.state, sx.kernel, &sx.table.facts.combat, &self.m.fields, self.m.env.occurrence) {
            self.m.env = env;
        }
    }
}

impl Window for CombatLive {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.m.stage()
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        self.refresh(sx);
        self.m.step(sx)
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        self.m.ask_of(s, k)
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        self.refresh(sx);
        self.m.answer(sx, pick)
    }
}

impl Held for CombatLive {
    const KIND: WindowKind = WindowKind::Combat;

    fn from_pending(core: &CoreState, k: &Kernel) -> Option<Self> {
        let OpenWindow::Combat(fields) = core.pending().window(WindowKind::Combat)? else { return None };
        let env = env_of(core, k, &crate::kernel::ability::CombatTexts::default(), fields, occurrence_of(core))?;
        Some(Self { m: Machine::new(env, (**fields).clone()).live(), refreshed: false })
    }

    fn to_pending(&self, core: &mut CoreState) {
        core.pending_mut().open(OpenWindow::Combat(Box::new(self.m.fields.clone())));
        if let Some(OpenWindow::Aftermath(a)) = core.pending_mut().window_mut(WindowKind::Aftermath) {
            a.occurrence = self.m.env.occurrence;
        }
    }

    fn closed(&self, core: &mut CoreState, _: &Kernel) {
        if let Some(OpenWindow::Aftermath(a)) = core.pending_mut().window_mut(WindowKind::Aftermath) {
            a.occurrence = self.m.env.occurrence;
            a.outcome = self.m.fields.outcome;
            a.fight = AFight::Closed;
        }
    }
}

/// What opening the fight found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opened {
    /// `Window::Combat` is on the stack: the first run settles the opening of round 1.
    Fight,
    /// Fewer (or more) than two sides hold the system: there is no fight. The winner (the one side with ships, none when a draw was declared) and
    /// zero rounds (`CombatWindow::new`'s `Stage::Done`).
    NoFight(Option<Seat>),
}

/// Open the fight of `system` (`CombatWindow::new`): with two sides (the head wrote them into `active_space_combat`) the payload goes on the
/// stack; else the aftermath goes on without one.
pub fn open_combat(core: &mut CoreState, k: &Kernel, system: System) -> Opened {
    let rd = Rd::new(core, k).with_blanked(reach::blanked(core));
    match sides(core) {
        Some((at, attacker, defender)) if at == system => {
            let mut at_start = Vec::new();
            for side in [attacker, defender] {
                let mut fleet: Vec<_> = ships_of(rd, side, system).iter().map(|p| p.kind()).collect();
                fleet.sort_unstable();
                at_start.push((side, fleet));
            }
            let fields = CombatFields {
                step: CombatStep::Opening { round: 1 },
                phase: HitPhase::Round,
                resuming: false,
                pending_retreats: Vec::new(),
                offered: Vec::new(),
                damaged: Vec::new(),
                fled: Vec::new(),
                at_start,
                scoring: false,
                outcome: None,
                end_announced: false,
            };
            core.pending_mut().open(OpenWindow::Combat(Box::new(fields)));
            Opened::Fight
        }
        _ => {
            let meta = core.meta();
            let drawn = meta.combat_draw_round.is(meta.combat_round_seq);
            let first: SmallList<Seat, 8> = combatants(rd, system);
            Opened::NoFight(if drawn { None } else { first.as_slice().first().copied() })
        }
    }
}

/// The question the open fight waits on (the arm's `AskFn` for the sub-window).
///
/// # Errors
/// As [`ask`].
pub fn ask_combat(core: &CoreState, env: &Env<'_>) -> Result<Option<Asking>, LiveError> {
    ask::<CombatLive>(core, env)
}

/// Run the open fight to its next decision, the pause for the barrage's scoring window, or its end. `Live::Done` is the end of the STEP: the
/// fight has left the stack (the outcome is in the aftermath's entry), or (a pause) it stays on it with the scoring window opened above it.
///
/// # Errors
/// See [`LiveError`]; the position is then what it was before the call.
pub fn advance_combat(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    advance_parkable::<CombatLive>(core, env, answer, &|w, core, out| match w.m.stop {
        Some(Stop::Bailed(why)) => After::Fail(Unknown::UnportedAftermath(match why {
            Bail::RollEndsFight => AftermathGap::RollEndsFight,
            Bail::Unported => AftermathGap::CombatUnported,
        })),
        Some(Stop::Scoring) => {
            // The window of the barrage's feat opens before the round resumes; the fight goes on at the announcement.
            w.m.fields.scoring = false;
            let opened = crate::driver::windows::scoring::open_event_scoring(core, w.m.env.occurrence, true);
            out.opened.push(opened);
            After::Park
        }
        _ => After::Close,
    })
}
