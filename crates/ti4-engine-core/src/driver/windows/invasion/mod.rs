//! The invasion of an aftermath as ONE window the core drives and holds (lane M3; the string `InvasionWindow::{new_with_notes, at_commit_step, settle,
//! pending_choice, resolve}`). L5: it composes the leaf windows of `rules::invasion` (bombardment, custodians, the commit step, the space cannon
//! defense, the ground round, the control gain) in the order of the string and keeps the 16 fields of the string window as the typed payload
//! [`crate::state::Invasion`] on the `Pending` stack. The machine is rebuilt from that payload and the position at every call
//! ([`Held::from_pending`]); a checkpoint is a plain `CoreState::clone`.
//!
//! # Stages and the string
//!
//! | payload stage            | string `Stage`                       | question asked (the leaf) |
//! |--------------------------|--------------------------------------|---------------------------|
//! | `Begin`                  | (`AftermathWindow::next_step`)       | none: `INVASION_BEGAN`, the rolled bombardment |
//! | `Bombarding`             | `Bombarding` / `ChoosingBombardment` | `BombardTarget` while the cursor is set |
//! | `Custodians(..)`         | `Custodians`                         | `Custodians`; its payment is a `PayBill` |
//! | `Committing`             | `Committing`                         | `LandOne` over the commit pool |
//! | `Fighting{round}`        | `Fighting`                           | `FightRound`; the Dunlain Reaper of the round start |
//! | `Advancing`              | `Advancing`                          | none |
//! | `ChoosingNextCombat`     | `ChoosingNextCombat`                 | `NextCombat` |
//! | `FinalizingControl`      | `FinalizingControl`                  | none |
//! | `Done`                   | `Done`                               | none |
//!
//! Like the string, the machine runs in CHUNKS (`settle` between two questions): an occurrence queued for an event scoring window pauses it
//! after the chunk that queued it (`pause`); the arm ([`live::advance_invasion`]) opens that window on the stack and the machine goes on when the
//! window has closed.
//!
//! What the leaf windows do not carry (and the string did through lends) is a counted bail of the native path: `Unknown::Unported(reason)`.

mod fight;
mod live;
mod ops;

use crate::choice::{Choice, Pick};
use crate::kernel::stage::{Held, Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::state::{CoreState, IStage, Invasion, Window as OpenWindow, WindowKind};

pub use live::{advance_invasion, ask_invasion, open_at_commit, open_invasion, scoring_window};

/// The suspension points, as the string names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tag {
    Begin,
    Bombarding,
    Custodians,
    Committing,
    Fighting,
    Advancing,
    NextCombat,
    Control,
    Done,
}

/// The open invasion.
#[derive(Debug, Clone)]
pub struct InvasionMachine {
    /// The payload: the source of truth.
    pub p: Invasion,
    /// An event scoring occurrence the machine stopped for (`(occurrence, combat)`): the arm opens the window. Not persisted.
    pub pause: Option<(u64, bool)>,
}

impl InvasionMachine {
    #[must_use]
    pub const fn new(p: Invasion) -> Self {
        Self { p, pause: None }
    }

    /// The machine pauses to let the scoring window of an occurrence close, or has nothing left to do.
    #[must_use]
    pub fn is_done(&self) -> bool {
        matches!(self.p.stage, IStage::Done) && self.p.scoring.is_empty()
    }

    /// What the invasion did (the hero callers read `captured`).
    #[must_use]
    pub fn report(&self) -> &crate::state::IReport {
        &self.p.report
    }

    fn bail(sx: &mut Sx<'_>, why: &'static str) -> Next {
        if sx.tape.unknown.get().is_none() {
            sx.tape.unknown.set(Some(crate::kernel::ability::Unknown::Unported(why)));
        }
        Next::Done
    }
}

impl Window for InvasionMachine {
    type Stage = Tag;

    fn stage(&self) -> Tag {
        match &self.p.stage {
            IStage::Begin => Tag::Begin,
            IStage::Bombarding => Tag::Bombarding,
            IStage::Custodians(_) => Tag::Custodians,
            IStage::Committing => Tag::Committing,
            IStage::Fighting { .. } => Tag::Fighting,
            IStage::Advancing { .. } => Tag::Advancing,
            IStage::ChoosingNextCombat { .. } => Tag::NextCombat,
            IStage::FinalizingControl { .. } => Tag::Control,
            IStage::Done => Tag::Done,
        }
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        self.pause = None;
        let mut idle = 0_u8;
        loop {
            // The aftermath's `settle` takes a queued occurrence before it settles the window again.
            if !self.p.scoring.is_empty() {
                self.pause = Some(self.p.scoring.remove(0));
                return Next::Done;
            }
            if matches!(self.p.stage, IStage::Done) {
                self.publish(sx);
                return Next::Done;
            }
            if let Some(choice) = self.ask_of(sx.state, sx.kernel) {
                return Next::Continue(choice.ask);
            }
            let before = (self.p.stage.clone(), self.p.bombard.index, self.p.bombard.cursor.is_some(), self.p.report.committed.len());
            if let Err(why) = self.settle(sx) {
                return Self::bail(sx, why);
            }
            self.publish(sx);
            if sx.tape.unknown.get().is_some() {
                return Next::Done;
            }
            let after = (self.p.stage.clone(), self.p.bombard.index, self.p.bombard.cursor.is_some(), self.p.report.committed.len());
            idle = if after == before { idle + 1 } else { 0 };
            if idle >= 3 {
                return Self::bail(sx, "the invasion made no progress (a stage that neither asks nor moves on)");
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        self.question(s, k)
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        self.resolve(sx, pick)
    }
}

impl Held for InvasionMachine {
    const KIND: WindowKind = WindowKind::Invasion;

    fn from_pending(core: &CoreState, _: &Kernel) -> Option<Self> {
        match core.pending().window(WindowKind::Invasion)? {
            OpenWindow::Invasion(p) => Some(Self::new((**p).clone())),
            _ => None,
        }
    }

    fn to_pending(&self, core: &mut CoreState) {
        core.pending_mut().open(OpenWindow::Invasion(Box::new(self.p.clone())));
    }

    /// The invasion is over: the public boundary goes (the aftermath's `state.active_invasion = None`).
    fn closed(&self, core: &mut CoreState, _: &Kernel) {
        core.transient_mut().invasion.active = None;
    }
}
