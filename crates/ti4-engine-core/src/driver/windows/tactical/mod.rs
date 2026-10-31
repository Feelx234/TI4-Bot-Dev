//! The tactical action as ONE window the core drives and holds (lane M6; the string `TacticalWindow` and `Game::{step_tactical,
//! apply_tactical, begin_one_move, finish_tactical, finish_head, finish_tactical_inner, close_tactical}`). L5: it composes the leaf windows of
//! `rules::tactical` (the activation, the movement step, the hold of the ship that moves, the move with its rift rolls, the close) with the
//! head of the aftermath (`driver::windows::aftermath::Head`: the fleet enforcement and the space cannon offense) and the tactical tail
//! (`driver::tactical_tail`), and keeps where the action stands as the typed payload [`crate::state::Tactical`] on the `Pending` stack. The
//! machine is rebuilt from that payload and the position at every call ([`Held::from_pending`]); a checkpoint is a plain `CoreState::clone`.
//!
//! # Stages and the string
//!
//! | payload stage   | string                                  | question asked (the leaf) |
//! |-----------------|-----------------------------------------|---------------------------|
//! | `Activating`    | `TacticalStage::Activating`             | the system to activate |
//! | `Free(system)`  | `begin_free_tactical`                   | none: the activation of the free action |
//! | `Hooks`         | the rest of `apply_tactical`'s arm      | none (a hook that asks is a counted gap) |
//! | `Moving`        | `TacticalStage::Moving`                 | the next ship, or "finish movement" |
//! | `Loading{..}`   | `TacticalStage::Loading`                | the hold of the ship that moves |
//! | `Closing`       | `finish_head` (taken window)            | none: `MOVEMENT_FINISHED`, the hero's skip, the blockade |
//! | `Heading(..)`   | `AftermathWindow::head`                 | a unit to remove (enforcement) |
//! | `Aftermath`     | `Game.aftermath` is open                | (the aftermath arm's) |
//! | `Tail`          | `close_tactical`                        | none |
//!
//! The head ends by opening the `Window::Aftermath` entry above this one ([`live::open_aftermath`]); the aftermath arm runs it (the chain of
//! the space combat, the invasion and the production step) and closes it; this window then runs the tail and leaves the stack. The end of
//! the ACTION (`finish_action`: `ACTION_COMPLETED`, the closing menu or the next turn) belongs to the turn flow, not to this window.
//!
//! What the window does not carry is a counted bail of the native path: `Unknown::UnportedTactical(gap)`.

mod facts;
mod hooks;
mod live;
mod tail;

use crate::choice::{Choice, Pick};
use crate::ids::Seat;
use crate::kernel::ability::{TacticalGap, Unknown};
use crate::kernel::stage::{Held, Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::rules::tactical::fleet::Enforce;
use crate::rules::tactical::{Facts as LeafFacts, Machine, Step};
use crate::state::{AFight, AStage, Aftermath, CoreState, Code, NoteHold, PauseLimit, TStage, THead, THeadStep, Tactical, Window as OpenWindow, WindowKind};

use super::aftermath::Head;

pub use facts::{fleet_limit, limits, note_holdings, REACHING};
pub use live::{advance_tactical, ask_tactical, begin_free, open_free, open_tactical};

/// The suspension points, as the payload names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tag {
    Activating,
    Free,
    Hooks,
    Moving,
    Loading,
    Closing,
    Heading,
    Aftermath,
    Tail,
}

/// The open tactical action.
#[derive(Debug, Clone)]
pub struct TacticalAction {
    /// The payload: the source of truth.
    pub p: Tactical,
    /// The leaf machine of the activation, movement and hold stages (rebuilt from the payload and the position).
    leaf: Option<Machine>,
    /// The head of the aftermath while it runs.
    head: Option<Head>,
    /// The step ended by handing the action to the aftermath: the entry stays on the stack ([`live::advance_tactical`]).
    pub parked: bool,
}

impl TacticalAction {
    #[must_use]
    pub const fn new(p: Tactical) -> Self {
        Self { p, leaf: None, head: None, parked: false }
    }

    /// The seat whose action it is.
    #[must_use]
    pub const fn seat(&self) -> Seat {
        self.p.player
    }

    fn bail(sx: &mut Sx<'_>, gap: TacticalGap) -> Next {
        if sx.tape.unknown.get().is_none() {
            sx.tape.unknown.set(Some(Unknown::UnportedTactical(gap)));
        }
        Next::Done
    }

    fn leaf_facts(codes: &crate::kernel::ability::TacticalCodes) -> LeafFacts {
        LeafFacts { pool: codes.tactic }
    }

    /// The leaf machine of the stage the payload is in.
    fn build_leaf(&self, core: &CoreState, k: &Kernel, pool: Code) -> Option<Machine> {
        let facts = LeafFacts { pool };
        match &self.p.stage {
            TStage::Activating | TStage::Moving | TStage::Loading { .. } => Some(Machine::from_payload(core, k, facts, &self.p).live()),
            TStage::Free(system) => Some(Machine::free(self.p.player, facts, *system).live()),
            _ => None,
        }
    }

    /// The head of the aftermath at the enforcement (a fresh one, or the held one at its question).
    fn build_head(&self, core: &CoreState, k: &Kernel, codes: &crate::kernel::ability::TacticalCodes, skip_to_commit: bool) -> Option<Head> {
        let system = core.meta().active_system?;
        let f = facts::head_facts(core, k, codes, self.p.player, system, &self.p.notes_at_start, skip_to_commit);
        let enforce = Enforce::new(self.p.player, system, f.limits).resumed(core, k);
        Some(Head::enforcing(f, enforce))
    }

    /// Whether a hook of the activation that this window does not carry is in the position.
    fn hook_gap(core: &CoreState, k: &Kernel, seat: Seat) -> Option<TacticalGap> {
        facts::activation_hook_held(core, k, seat).then_some(TacticalGap::ActivationHook)
    }

    /// The stage of the leaf machine after an answer, as the payload.
    fn sync_from_leaf(&mut self, leaf: &Machine) {
        self.p.stage = leaf.payload(self.p.notes_at_start.clone()).stage;
    }

    /// The head is over: the aftermath opens above this entry (`finish_tactical_inner`'s second half, up to the chain).
    fn open_aftermath(&mut self, sx: &mut Sx<'_>, head: &Head, skip_to_commit: bool) {
        let system = sx.state.meta().active_system.unwrap_or(head.f.system);
        let mut aftermath = Aftermath::new(
            self.p.player,
            system,
            // `at_commit_step` hands the invasion its commit step with the feats already noted.
            if skip_to_commit { AStage::Invading } else { AStage::Fighting },
            head.before.clone().unwrap_or_default(),
            self.p.notes_at_start.clone(),
        );
        aftermath.feats_noted = skip_to_commit;
        aftermath.event_scoring = head.scoring.map(|o| (o, PauseLimit::AnyPerPlayer));
        if skip_to_commit {
            aftermath.fight = AFight::Over;
        }
        sx.state.pending_mut().open(OpenWindow::Aftermath(Box::new(aftermath)));
        self.p.stage = TStage::Aftermath;
        self.parked = true;
        self.head = None;
    }
}

impl Window for TacticalAction {
    type Stage = Tag;

    fn stage(&self) -> Tag {
        match &self.p.stage {
            TStage::Activating => Tag::Activating,
            TStage::Free(_) => Tag::Free,
            TStage::Hooks => Tag::Hooks,
            TStage::Moving => Tag::Moving,
            TStage::Loading { .. } => Tag::Loading,
            TStage::Closing => Tag::Closing,
            TStage::Heading(_) => Tag::Heading,
            TStage::Aftermath => Tag::Aftermath,
            TStage::Tail => Tag::Tail,
        }
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        self.parked = false;
        let codes = sx.table.codes.tactical;
        let seat = self.p.player;
        let mut idle = 0_u8;
        loop {
            if sx.tape.unknown.get().is_some() {
                return Next::Done;
            }
            let before = self.p.stage.clone();
            match self.p.stage.clone() {
                TStage::Activating | TStage::Moving | TStage::Loading { .. } => {
                    if self.leaf.is_none() {
                        self.leaf = self.build_leaf(sx.state, sx.kernel, codes.tactic);
                    }
                    let Some(leaf) = self.leaf.as_mut() else { return Self::bail(sx, TacticalGap::NoProgress) };
                    leaf.facts = Self::leaf_facts(&codes);
                    if matches!(self.p.stage, TStage::Activating) && facts::mahact_in_play(sx.state) {
                        return Self::bail(sx, TacticalGap::ActivationHook);
                    }
                    match leaf.step(sx) {
                        Next::Continue(ask) => return Next::Continue(ask),
                        Next::Done if matches!(self.p.stage, TStage::Activating) => {
                            // Nothing to activate (no token, no system): the action is over (`legal_options` is `None`).
                            self.leaf = None;
                            self.p.stage = TStage::Tail;
                        }
                        Next::Done => return Self::bail(sx, TacticalGap::NoProgress),
                        other => return other,
                    }
                }
                TStage::Free(system) => {
                    let mut leaf = Machine::free(seat, Self::leaf_facts(&codes), system).live();
                    let _ = leaf.step(sx);
                    self.leaf = None;
                    self.p.stage = TStage::Moving;
                }
                TStage::Hooks => {
                    let Some(system) = sx.state.meta().active_system else { return Self::bail(sx, TacticalGap::NoProgress) };
                    if let Some(gap) = Self::hook_gap(sx.state, sx.kernel, seat) {
                        return Self::bail(sx, gap);
                    }
                    self.leaf = None;
                    self.p.stage = match hooks::run(sx, seat, system) {
                        hooks::After::Move => TStage::Moving,
                        hooks::After::Close => TStage::Closing,
                    };
                }
                TStage::Closing => {
                    let Some(system) = sx.state.meta().active_system else {
                        // `finish_tactical_inner` without an active system goes straight to the tail.
                        self.p.stage = TStage::Tail;
                        continue;
                    };
                    let edits = facts::map_edits(sx.state);
                    let mut m = Machine::closing(seat, Self::leaf_facts(&codes)).live();
                    let _ = m.announce_finished(sx);
                    if facts::map_edits(sx.state) != edits {
                        return Self::bail(sx, TacticalGap::NovaSeed);
                    }
                    let skip = sx.kernel.hooks.general.skip_to_commit(&mut sx.cx(), seat, system).unwrap_or(false);
                    Machine::forget_arrivals(sx);
                    crate::rules::tactical::destroy_blockaded_mobile_docks(sx.state, system);
                    self.head = self.build_head(sx.state, sx.kernel, &codes, skip);
                    self.p.stage = TStage::Heading(THead { skip_to_commit: skip, step: THeadStep::Enforcing });
                }
                TStage::Heading(h) => {
                    if self.head.is_none() {
                        self.head = self.build_head(sx.state, sx.kernel, &codes, h.skip_to_commit);
                    }
                    let Some(head) = self.head.as_mut() else { return Self::bail(sx, TacticalGap::NoGalaxy) };
                    head.f.cannon.kind = codes.space_cannon;
                    head.f.cannon.cause = codes.space_cannon_cause;
                    head.f.cannon.casualty = codes.casualty;
                    head.f.cannon.commander = codes.commander;
                    head.f.pre_roll = codes.pre_roll;
                    match head.step(sx) {
                        Next::Continue(ask) => {
                            if head.stage() != super::aftermath::Stage::Enforcing {
                                return Self::bail(sx, TacticalGap::CannonHits);
                            }
                            return Next::Continue(ask);
                        }
                        Next::Done => {
                            let head = self.head.take().expect("checked above");
                            self.open_aftermath(sx, &head, h.skip_to_commit);
                            return Next::Done;
                        }
                        other => return other,
                    }
                }
                TStage::Aftermath => {
                    if sx.state.pending().window(WindowKind::Aftermath).is_some() {
                        self.parked = true;
                        return Next::Done;
                    }
                    self.p.stage = TStage::Tail;
                }
                TStage::Tail => {
                    return tail::run(self, sx);
                }
            }
            idle = if self.p.stage == before { idle + 1 } else { 0 };
            if idle >= 3 {
                return Self::bail(sx, TacticalGap::NoProgress);
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        match &self.p.stage {
            TStage::Activating | TStage::Moving | TStage::Loading { .. } => match &self.leaf {
                Some(leaf) => leaf.ask_of(s, k),
                None => Machine::from_payload(s, k, LeafFacts { pool: Code(0) }, &self.p).ask_of(s, k),
            },
            TStage::Heading(h) if h.step == THeadStep::Enforcing => match &self.head {
                Some(head) => head.ask_of(s, k),
                None => self.build_head(s, k, &crate::kernel::ability::TacticalCodes::default(), h.skip_to_commit)?.ask_of(s, k),
            },
            _ => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let codes = sx.table.codes.tactical;
        match self.p.stage.clone() {
            TStage::Activating | TStage::Moving | TStage::Loading { .. } => {
                if self.leaf.is_none() {
                    self.leaf = self.build_leaf(sx.state, sx.kernel, codes.tactic);
                }
                let mut leaf = self.leaf.take().ok_or(Illegal)?;
                leaf.facts = Self::leaf_facts(&codes);
                let result = leaf.answer(sx, pick);
                if let Some(bail) = leaf.bailed {
                    let gap = match bail {
                        crate::rules::tactical::Bail::NoGalaxy => TacticalGap::NoGalaxy,
                        crate::rules::tactical::Bail::Route => TacticalGap::NoProgress,
                    };
                    Self::bail(sx, gap);
                }
                result?;
                match leaf.step.take() {
                    Some(Step::Activated) => self.p.stage = TStage::Hooks,
                    Some(Step::Finished) => self.p.stage = TStage::Closing,
                    _ => self.sync_from_leaf(&leaf),
                }
                Ok(())
            }
            TStage::Heading(_) => {
                let head = self.head.as_mut().ok_or(Illegal)?;
                head.f.cannon.kind = codes.space_cannon;
                head.f.cannon.cause = codes.space_cannon_cause;
                head.f.cannon.casualty = codes.casualty;
                head.f.cannon.commander = codes.commander;
                head.answer(sx, pick)
            }
            _ => Err(Illegal),
        }
    }
}

impl Held for TacticalAction {
    const KIND: WindowKind = WindowKind::Tactical;

    fn from_pending(core: &CoreState, k: &Kernel) -> Option<Self> {
        let OpenWindow::Tactical(t) = core.pending().window(WindowKind::Tactical)? else { return None };
        let mut me = Self::new(t.clone());
        match &t.stage {
            TStage::Activating | TStage::Moving | TStage::Loading { .. } => me.leaf = me.build_leaf(core, k, Code(0)),
            TStage::Heading(h) => me.head = me.build_head(core, k, &crate::kernel::ability::TacticalCodes::default(), h.skip_to_commit),
            _ => {}
        }
        Some(me)
    }

    fn to_pending(&self, core: &mut CoreState) {
        core.pending_mut().open(OpenWindow::Tactical(self.p.clone()));
    }
}

/// The notes a tactical action begun now would remember (the opener's).
#[must_use]
pub fn notes_now(core: &CoreState) -> NoteHold {
    facts::note_holdings(core)
}
