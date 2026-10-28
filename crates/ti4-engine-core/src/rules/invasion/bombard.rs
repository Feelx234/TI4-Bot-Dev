//! The bombardment after its dice are final (LRR 49.1, coexistence 7, 7.1, 7.2): the string
//! `InvasionWindow::step_bombardment`, the `ChoosingBombardment` re-check of `settle`, `answer_bombardment_target` and
//! `complete_bombard_plan`, as one [`Window`].
//!
//! The plan comes from the string window (its dice were rolled and rerolled before; those steps are dual-checked
//! elsewhere): per planet the hit groups of the bombarding units, how many ground forces were held and whose they
//! are, and the `InvasionStep` template (what `complete_bombard_plan` records in `active_invasion.last_step`).
//!
//! * `Bombarding`: planet after planet. A planet with no hits is completed. A planet with one owner takes its
//!   groups at once (Proxima Targeting VI cancels first), each group through the hits step ([`super::hits`]: window,
//!   apply, announce); several owners pause the window on the coexistence question of the first group.
//! * `Choosing`: the planet's ground forces may already be gone (7.2); then it is completed without asking.
//! * `Asking`: the invader names whose ground forces take the next group; the answer lands it and either completes
//!   the planet or comes back to `Choosing` with the next group.
//! * `Paused`: the window stops where the string `settle` returns to wait for the answer; the question itself is
//!   [`super::stages::BombardTarget`]'s and compared on its own.
//!
//! `complete`: records the step in `active_invasion` (when there is one) and, when the planet's last ground forces
//! were destroyed by this bombardment alone (every unit left is the invader's), the
//! `BombardedOutTheLastGroundForces` feat and a scoring occurrence for the caller to open.

use super::hits::{self as step, Batch};
use super::stages::BombardTarget;
use crate::choice::invasion::Opt as IOpt;
use crate::choice::{Choice, Opt, Pick};
use crate::containers::{SeatSet, SmallList};
use crate::generated::{Planet, System};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{hits, Kernel, Next};
use crate::state::{Code, CoreState, Feat, InvasionStep, Piece};

/// One planet of the plan.
#[derive(Debug, Clone)]
pub struct Entry {
    pub planet: Planet,
    /// The step as recorded when the planet is done, `after` still empty.
    pub template: InvasionStep,
    pub groups: SmallList<usize, 8>,
    pub held: usize,
    pub victims: SeatSet,
}

/// The position inside a coexistence pause (`BombardCursor`).
#[derive(Debug, Clone)]
pub struct Cursor {
    pub planet_index: usize,
    pub planet: Planet,
    pub groups: SmallList<usize, 8>,
    pub next_group: usize,
    pub taken: usize,
    pub held: usize,
    pub victims: SeatSet,
}

/// The suspension points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Bombarding,
    Choosing,
    Asking,
    Paused,
    Done,
}

/// The string `Stage` the window corresponds to after a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    Bombarding,
    Choosing,
    Done,
}

/// The bombardment of `invader` in `system`.
#[derive(Debug, Clone)]
pub struct Bombardment {
    pub invader: Seat,
    pub system: System,
    /// The feat occurrence of the invasion.
    pub occurrence: u64,
    /// The resolver carries the timing machinery (the hits window and the events are announced).
    pub announce: bool,
    /// The interned cause `"bombardment"` of the hits events.
    pub cause: Code,
    pub plan: Vec<Entry>,
    pub index: usize,
    pub cursor: Option<Cursor>,
    /// `InvasionReport.bombardment_kills`, as the window grew it.
    pub kills: usize,
    /// `pending_scoring_occurrences` as the window pushed them: (occurrence, false).
    pub scoring: Vec<(u64, bool)>,
    /// Stop once the answer is applied (the string `answer_bombardment_target` does not settle).
    pub answer_only: bool,
    halt: bool,
    stage: Stage,
}

impl Bombardment {
    /// A window in the string's `Stage::Bombarding`.
    #[must_use]
    pub fn new(invader: Seat, system: System, occurrence: u64, announce: bool, cause: Code, plan: Vec<Entry>, index: usize) -> Self {
        Self { invader, system, occurrence, announce, cause, plan, index, cursor: None, kills: 0, scoring: Vec::new(), answer_only: false, halt: false, stage: Stage::Bombarding }
    }

    /// A window in the string's `Stage::ChoosingBombardment(cursor)`, about to be answered (`answer_only`) or settled.
    #[must_use]
    pub fn at_cursor(mut self, cursor: Cursor, asking: bool) -> Self {
        self.cursor = Some(cursor);
        self.stage = if asking { Stage::Asking } else { Stage::Choosing };
        self.answer_only = asking;
        self
    }

    /// Whether the window ended at the coexistence pause (the string stays in `ChoosingBombardment`).
    #[must_use]
    pub fn paused(&self) -> bool {
        matches!(self.stage, Stage::Paused | Stage::Choosing | Stage::Asking)
    }

    /// Where the string window stands: bombarding, choosing (at a pause) or done (the plan is exhausted).
    #[must_use]
    pub fn string_stage(&self) -> Where {
        match self.stage {
            Stage::Bombarding => Where::Bombarding,
            Stage::Done => Where::Done,
            Stage::Choosing | Stage::Asking | Stage::Paused => Where::Choosing,
        }
    }

    /// Announce the staged hit events when the resolver carries the timing machinery (`ctx.emit` without it
    /// announces nothing); drop them otherwise.
    fn flush(&self, sx: &mut Sx<'_>) {
        if self.announce {
            hits::flush_staged(sx);
        } else {
            let _ = crate::kernel::fx::drain_staged(sx.state);
        }
    }

    fn batch(&self, victim: Seat, planet: Planet) -> Batch {
        Batch { victim, system: self.system, planet, cause: self.cause, unit_ability: true, announce: self.announce }
    }

    /// `complete_bombard_plan`.
    fn complete(&mut self, s: &mut CoreState, planet: Planet, taken: usize, held: usize, victims: SeatSet) {
        let after: Vec<Piece> = s.board().planet(planet).units.iter().copied().collect();
        let entry = self.plan.iter().find(|e| e.planet == planet).map(|e| e.template.clone());
        if let (Some(mut step), Some(active)) = (entry, s.transient_mut().invasion.active.as_mut()) {
            step.after = after;
            active.edit().last_step = Some(step);
        }
        if taken == held && victims.len() == 1 && s.board().planet(planet).units.iter().all(|u| u.owner() == self.invader) {
            if let Some(p) = s.players_mut().by_seat_mut(self.invader) {
                let entry = (Feat::BombardedOutTheLastGroundForces, self.occurrence);
                if !p.misc.event_feats.as_slice().contains(&entry) {
                    p.misc.event_feats.push(entry);
                    p.misc.event_feats.as_mut_slice().sort_unstable();
                }
            }
            self.scoring.push((self.occurrence, false));
        }
    }

    /// `step_bombardment`: advance until the plan is done or a planet pauses on the coexistence question.
    fn bombard(&mut self, sx: &mut Sx<'_>) {
        loop {
            let Some(entry) = self.plan.get(self.index) else {
                self.stage = Stage::Done;
                return;
            };
            let (planet, held, victims, mut groups) = (entry.planet, entry.held, entry.victims, entry.groups.clone());
            if entry.groups.is_empty() {
                self.complete(sx.state, planet, 0, held, victims);
                self.index += 1;
                continue;
            }
            if victims.len() == 1 {
                let target = victims.iter().next().expect("a single owner");
                let (k, state) = (sx.kernel, &*sx.state);
                k.hooks.direct.bastion_units_cancel_in_groups(state, target, self.system, planet, &mut groups);
                let mut taken = 0;
                for produced in groups.iter() {
                    let b = self.batch(target, planet);
                    let produced = step::open(sx, &b, u32::try_from(*produced).unwrap_or(u32::MAX));
                    taken += step::apply(sx.state, sx.kernel, &b, produced) as usize;
                }
                self.kills += taken;
                self.complete(sx.state, planet, taken, held, victims);
                self.index += 1;
                continue;
            }
            self.cursor = Some(Cursor { planet_index: self.index, planet, groups, next_group: 0, taken: 0, held, victims });
            self.stage = Stage::Choosing;
            return;
        }
    }

    /// The `ChoosingBombardment` arm of `settle`: no ground force of another player left, so the planet is done.
    fn recheck(&mut self, s: &mut CoreState) {
        let Some(c) = self.cursor.as_ref() else { return };
        let (planet, taken, held, victims, index) = (c.planet, c.taken, c.held, c.victims, c.planet_index);
        if s.board().planet(planet).units.iter().any(|u| u.owner() != self.invader) {
            self.stage = Stage::Paused;
        } else {
            self.complete(s, planet, taken, held, victims);
            self.index = index + 1;
            self.cursor = None;
            self.stage = Stage::Bombarding;
        }
    }

    fn next_group(c: &Cursor) -> Option<usize> {
        (c.next_group..c.groups.len()).find(|i| c.groups.as_slice()[*i] > 0)
    }
}

impl Window for Bombardment {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.halt {
            return Next::Done;
        }
        loop {
            match self.stage {
                Stage::Bombarding => {
                    self.bombard(sx);
                    self.flush(sx);
                }
                Stage::Choosing => self.recheck(sx.state),
                Stage::Asking => {
                    let Some(c) = self.cursor.as_ref() else { return Next::Done };
                    let Some(next) = Self::next_group(c) else { return Next::Done };
                    let q = BombardTarget::new(self.invader, self.system, c.planet, u16::try_from(c.groups.as_slice()[next]).unwrap_or(u16::MAX));
                    return Next::Continue(q.ask());
                }
                Stage::Paused | Stage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        if !matches!(self.stage, Stage::Asking | Stage::Paused) {
            return None;
        }
        let c = self.cursor.as_ref()?;
        let next = Self::next_group(c)?;
        let _ = k;
        let q = BombardTarget::new(self.invader, self.system, c.planet, u16::try_from(c.groups.as_slice()[next]).unwrap_or(u16::MAX));
        Some(q.question(s))
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != Stage::Asking {
            return Err(Illegal);
        }
        let Pick::Opt(Opt::Invasion(IOpt::BombardTarget { seat: target })) = pick else { return Err(Illegal) };
        let c = self.cursor.take().ok_or(Illegal)?;
        let next = Self::next_group(&c).ok_or(Illegal)?;
        let b = self.batch(target, c.planet);
        let hits_n = u32::try_from(c.groups.as_slice()[next]).unwrap_or(u32::MAX);
        let produced = step::open(sx, &b, hits_n);
        let applied = step::apply(sx.state, sx.kernel, &b, produced) as usize;
        self.flush(sx);
        self.kills += applied;
        let taken = c.taken + applied;
        let exhausted = next + 1 >= c.groups.len() || !sx.state.board().planet(c.planet).units.iter().any(|u| u.owner() != self.invader);
        if exhausted {
            self.complete(sx.state, c.planet, taken, c.held, c.victims);
            self.index = c.planet_index + 1;
            self.stage = Stage::Bombarding;
        } else {
            self.cursor = Some(Cursor { next_group: next + 1, taken, ..c });
            self.stage = Stage::Choosing;
        }
        // The string `answer_bombardment_target` does not settle: the driver's next `step` ends the window.
        self.halt = self.answer_only;
        Ok(())
    }
}
