//! The head of the aftermath chain as one window that RUNS in the core (package D1b2; `AftermathWindow::new` and
//! `AftermathWindow::at_commit_step` of `game.rs`, up to the opening of the space combat): the fleet enforcement of the active
//! seat in the system (`rules::tactical::fleet::Enforce`, which asks which units go), the space cannon offense
//! (`rules::combat::space_cannon_offense`, which rolls, announces and asks the victims' casualties), then the snapshot of the
//! system before the fight (`combat::before_combat_with_notes`) and the fight's opening writes (`active_space_combat`, the
//! presentation `pre_roll`, the round scratch cleared).
//!
//! L5: composes two areas, so it lives in the driver. What the fight itself does (the combat machine), and the invasion and
//! production steps that follow, are the chain's later stages: the combat machine is a window of its own
//! (`Window::Combat`), the other two have no machine state and stay with the string (documented bail D1b2-bail-02). The
//! transitions between them are `driver::aftermath::next`.

use crate::choice::{Choice, Pick};
use crate::generated::{System, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::stage::{Child, Illegal, Ran, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::reach;
use crate::rules::combat::space_cannon_offense::{Facts as CannonFacts, SpaceCannonOffense};
use crate::rules::combat::{combatants, Rd};
use crate::rules::tactical::fleet::{Enforce, Limits};
use crate::state::{Before, Code, CoreState, NoteHold};

/// The facts of one head.
#[derive(Debug, Clone)]
pub struct Facts {
    /// The active seat.
    pub player: Seat,
    pub system: System,
    pub limits: Limits,
    pub cannon: CannonFacts,
    /// The note holdings of the tactical action's start (carried into the snapshot).
    pub notes: NoteHold,
    /// The action skipped to the Commit Ground Forces step (the Sardakk hero): no space cannon, no fight.
    pub skip_to_commit: bool,
    /// The interned presentation phase `pre_roll`.
    pub pre_roll: Code,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Enforcing,
    Firing,
    Opening,
    Done,
}

#[derive(Debug, Clone)]
pub struct Head {
    pub f: Facts,
    stage: Stage,
    enforce: Child<Enforce>,
    cannon: Child<SpaceCannonOffense>,
    /// The system before the fight.
    pub before: Option<Before>,
    /// The feat occurrence of the cannon's event-scoring pause.
    pub scoring: Option<u64>,
    /// The dice of the offense, in order: (hit number, faces).
    pub drawn: Vec<(u32, Vec<u32>)>,
}

impl Head {
    /// The gunner whose hits are being absorbed and whether their loss is bound to non-fighters (the hit detail of the
    /// casualty question).
    #[must_use]
    pub fn absorbing(&self) -> Option<(Seat, bool)> {
        self.cannon.window().and_then(SpaceCannonOffense::absorbing)
    }

    #[must_use]
    pub fn new(f: Facts) -> Self {
        Self { f, stage: Stage::Enforcing, enforce: Child::idle(), cannon: Child::idle(), before: None, scoring: None, drawn: Vec::new() }
    }

    /// A head whose enforcement is already running (the live path rebuilds a held head at its question).
    #[must_use]
    pub fn enforcing(f: Facts, enforce: Enforce) -> Self {
        let mut head = Self::new(f);
        head.enforce.start(enforce);
        head
    }

    /// `combat::before_combat_with_notes`: the sides in seating order and what each had in war suns and flagships.
    fn snapshot(&self, s: &CoreState, k: &Kernel) -> Before {
        let rd = Rd::new(s, k).with_blanked(reach::blanked(s));
        let sides = combatants(rd, self.f.system);
        let mut capitals = crate::containers::SmallList::new();
        for side in sides.iter() {
            let n = s
                .board()
                .sys(self.f.system)
                .units
                .iter()
                .filter(|u| u.owner() == *side)
                .filter(|u| rd.data(u.kind()).is_some_and(|d| matches!(d.base_type, UnitBaseType::Warsun | UnitBaseType::Flagship)))
                .count();
            capitals.push((*side, u16::try_from(n).unwrap_or(u16::MAX)));
        }
        Before { sides, capitals, notes: self.f.notes.clone() }
    }

    /// The writes that open the fight: `state.active_space_combat` and the presentation of a battle about to be rolled.
    fn open_fight(&mut self, sx: &mut Sx<'_>) {
        let Some(before) = &self.before else { return };
        if before.sides.len() != 2 {
            sx.state.transient_mut().combat.active_space_combat = None;
            return;
        }
        let sides = before.sides.as_slice();
        let attacker = if sides.contains(&self.f.player) { self.f.player } else { sides[0] };
        let defender = if sides[0] == attacker { sides[1] } else { sides[0] };
        let seq = sx.state.meta().combat_round_seq.saturating_add(1);
        let combat = &mut sx.state.transient_mut().combat;
        combat.active_space_combat = Some((self.f.system, attacker, defender));
        let p = combat.presentation.edit();
        *p = crate::state::CombatPresentation::default();
        p.battle_seq = seq;
        p.phase = self.f.pre_roll;
        combat.combat_round_hits = Default::default();
        combat.combat_round_dice = Default::default();
    }
}

impl Window for Head {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                Stage::Enforcing => {
                    if !self.enforce.running() {
                        self.enforce.start(Enforce::new(self.f.player, self.f.system, self.f.limits));
                    }
                    match self.enforce.step(sx) {
                        Some(Ran::Asks(ask)) => return Next::Continue(ask),
                        Some(Ran::Pass(next)) => return next,
                        Some(Ran::Done(_)) => {
                            self.stage = if self.f.skip_to_commit { Stage::Opening } else { Stage::Firing };
                            if self.stage == Stage::Firing {
                                self.cannon.start(SpaceCannonOffense::new(self.f.cannon.clone()));
                            }
                        }
                        None => self.stage = Stage::Opening,
                    }
                }
                Stage::Firing => match self.cannon.step(sx) {
                    Some(Ran::Asks(ask)) => return Next::Continue(ask),
                    Some(Ran::Pass(next)) => return next,
                    Some(Ran::Done(c)) => {
                        self.scoring = c.scoring;
                        self.drawn = c.drawn.clone();
                        self.stage = Stage::Opening;
                    }
                    None => self.stage = Stage::Opening,
                },
                Stage::Opening => {
                    self.before = Some(self.snapshot(sx.state, sx.kernel));
                    // A pause for the cannon's event scoring comes before the fight opens: the string opens the fight only
                    // when no pause is pending (`settle_open`), but writes `active_space_combat` first.
                    if !self.f.skip_to_commit {
                        self.open_fight(sx);
                    }
                    self.stage = Stage::Done;
                }
                Stage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        match self.stage {
            Stage::Enforcing => self.enforce.ask_of(s, k),
            Stage::Firing => self.cannon.ask_of(s, k),
            Stage::Opening | Stage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match self.stage {
            Stage::Enforcing => self.enforce.answer(sx, pick),
            Stage::Firing => self.cannon.answer(sx, pick),
            Stage::Opening | Stage::Done => Err(Illegal),
        }
    }
}

// --- the end of the space combat -------------------------------------------------------------------------

use crate::containers::SmallList;
use crate::kernel::fx;
use crate::rules::combat::feats::Outcome;
use crate::state::{EventKind, Key, List, Payload, Val};

use super::enforce_all::EnforceAll;

/// The facts of the end of a fight.
#[derive(Debug, Clone)]
pub struct OverFacts {
    /// The active seat.
    pub player: Seat,
    pub system: System,
    pub outcome: Outcome,
    /// The sides when the fight opened, in seating order (`BeforeCombat::sides`).
    pub sides: SmallList<Seat, 8>,
    /// The resolver carries the timing machinery (`ctx.emit` announces).
    pub announce: bool,
    /// The interned payload key `opponents`.
    pub opponents_key: Code,
    /// Every seat's enforcement in the system, in the table's order (those with ships there).
    pub pairs: Vec<(Seat, System, Limits)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverStage {
    Announcing,
    Enforcing,
    Done,
}

/// `AftermathWindow::settle`, the part of the Fighting arm after the feats: whether the active seat holds the space, "after you
/// win a space combat", the fight's scratch cleared, and the enforcement of every seat's fleet in the system.
#[derive(Debug, Clone)]
pub struct FightOver {
    pub f: OverFacts,
    stage: OverStage,
    enforce: Child<EnforceAll>,
    /// An invasion step follows: a ship of the active seat holds the space, or Coalescence forces a ground combat.
    pub holds: bool,
}

impl FightOver {
    /// A fight over whose announcement was made and whose enforcement is under way (the live path rebuilds a held window at its question):
    /// the enforcement resumes from the position.
    #[must_use]
    pub fn enforcing(f: OverFacts, holds: bool, core: &CoreState, k: &Kernel) -> Self {
        let mut over = Self::new(f);
        over.holds = holds;
        over.stage = OverStage::Enforcing;
        let all = EnforceAll::new(over.f.pairs.clone()).resumed(core, k);
        over.enforce.start(all);
        over
    }

    #[must_use]
    pub fn new(f: OverFacts) -> Self {
        Self { f, stage: OverStage::Announcing, enforce: Child::idle(), holds: false } // mutants:skip: `holds` is set by the first step
    }
}

impl Window for FightOver {
    type Stage = OverStage;

    fn stage(&self) -> OverStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                OverStage::Announcing => {
                    let (player, system) = (self.f.player, self.f.system);
                    // 49: an invasion only happens if the active seat still holds the space (membership, not seating order),
                    // or Coalescence forces a ground combat.
                    let rd = Rd::new(sx.state, sx.kernel).with_blanked(reach::blanked(sx.state));
                    self.holds = combatants(rd, system).as_slice().contains(&player)
                        || !sx.kernel.hooks.ground.forced_combat_planets(sx.state, sx.kernel, player, system).is_empty();
                    if self.f.outcome.rounds > 0 {
                        if let Some(winner) = self.f.outcome.winner {
                            let mut opponents = SmallList::<Seat, 4>::new();
                            for side in self.f.sides.as_slice().iter().filter(|s| **s != winner) {
                                opponents.push(*side);
                            }
                            sx.state.transient_mut().combat.last_combat_sides = Some((system, opponents.clone()));
                            if self.f.announce {
                                let mut payload = Payload::new().with(Key::Player, Val::Seat(winner)).with(Key::System, Val::System(system));
                                payload.set_list(Key::Other(self.f.opponents_key), List::Seats(opponents));
                                let _ = sx.emit(fx::event(EventKind::SpaceCombatWon, payload));
                            }
                        }
                    }
                    sx.state.transient_mut().combat.active_space_combat = None;
                    self.enforce.start(EnforceAll::new(self.f.pairs.clone()));
                    self.stage = OverStage::Enforcing;
                }
                OverStage::Enforcing => match self.enforce.step(sx) {
                    Some(Ran::Asks(ask)) => return Next::Continue(ask),
                    Some(Ran::Pass(next)) => return next,
                    Some(Ran::Done(_)) | None => self.stage = OverStage::Done,
                },
                OverStage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        if self.stage == OverStage::Enforcing { self.enforce.ask_of(s, k) } else { None }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage == OverStage::Enforcing { self.enforce.answer(sx, pick) } else { Err(Illegal) }
    }
}
