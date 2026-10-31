//! The hits of a ground combat round (LRR 42.3) and the step report (the string `assign_ground_round_hits`
//! with L1Z1X's Harrow as the window of its own in [`super::harrow`], then `end_ground_round`), as a [`Window`] that never asks.
//!
//! Both hits windows open first (the attacker's hits on the defender, then the defender's on the attacker, each
//! through [`super::hits::open`], which spends what a card played in it cancelled), then the two removals
//! ([`super::hits::apply`]: sustain first, cheapest ground force). The removals' events are staged and announced
//! after the round, as the string collects them in its log and emits them in `end_ground_round`; between the
//! removals and that announcement the `InvasionStep` of the round is recorded in `active_invasion.last_step`.
//!
//! The hits a faction adds to its own roll (Valkyrie Particle Weave) are the kernel's hook (package W13). Not covered (the adapter returns
//! `None`, the string result stands): Proxima Targeting VI, Alarum (a Ral Nel mech on the planet), neutral sides, nested activity.

use super::hits::{self as step, Batch};
use crate::choice::{Choice, Pick};
use crate::containers::SortedMap;
use crate::generated::{Planet, System};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{hits, Kernel, Next};
use crate::state::{Code, CoreState, InvasionStep, Piece};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    Done,
}

/// The facts of one round's hits.
#[derive(Debug, Clone)]
pub struct Facts {
    pub invader: Seat,
    pub defender: Seat,
    pub system: System,
    pub planet: Planet,
    /// The rolled hits of the invader and the defender.
    pub hits: (u32, u32),
    /// The resolver carries the timing machinery (the windows and the removals are announced).
    pub announce: bool,
    /// The interned event cause `ground_combat`.
    pub cause: Code,
    /// The step as `end_ground_round` records it, `round`, `after`, `hits` and `harrow_hits` still to fill.
    pub template: InvasionStep,
    /// The invader holds the Harrow ability: its window runs after the removals.
    pub harrow: Option<super::harrow::Facts>,
}

#[derive(Debug, Clone)]
pub struct GroundRoundHits {
    pub f: Facts,
    /// The hits finally assigned (invader's, defender's), after the windows.
    pub assigned: (u32, u32),
    /// The Harrow window that ran after the removals.
    pub harrow: Option<super::harrow::Harrow>,
    stage: Stage,
}

impl GroundRoundHits {
    #[must_use]
    pub fn new(f: Facts) -> Self {
        Self { f, assigned: (0, 0), harrow: None, stage: Stage::Start }
    }

    fn batch(&self, victim: Seat) -> Batch {
        Batch { victim, system: self.f.system, planet: self.f.planet, cause: self.f.cause, unit_ability: false, announce: self.f.announce }
    }
}

impl Window for GroundRoundHits {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Done {
            return Next::Done;
        }
        let (invader, defender) = (self.f.invader, self.f.defender);
        let (db, ib) = (self.batch(defender), self.batch(invader));
        // Hits a faction adds to its own roll once the dice are final (Valkyrie Particle Weave): both sides' final dice are read before either side
        // adds anything, and the added hits are assigned by the opponent like any other.
        let (rolled_invader, rolled_defender) = (self.f.hits.0 as usize, self.f.hits.1 as usize);
        let added = |sx: &Sx<'_>, who: Seat, own: usize, opponent: usize| {
            let extra = sx.kernel.hooks.ground.ground_rolls_extra_hits(sx.state, sx.kernel, who, self.f.system, self.f.planet, own, opponent);
            u32::try_from(own + extra).unwrap_or(u32::MAX)
        };
        let (invader_hits, defender_hits) = (added(sx, invader, rolled_invader, rolled_defender), added(sx, defender, rolled_defender, rolled_invader));
        let to_defender = step::open(sx, &db, invader_hits);
        let to_invader = step::open(sx, &ib, defender_hits);
        step::apply(sx.state, sx.kernel, &db, to_defender);
        step::apply(sx.state, sx.kernel, &ib, to_invader);
        self.assigned = (to_defender, to_invader);
        // Harrow rolls after both removals, its hits are assigned before the report.
        let mut harrow_hits = 0;
        if let Some(facts) = self.f.harrow {
            let mut harrow = super::harrow::Harrow::new(facts);
            let _ = harrow.step(sx);
            harrow_hits = harrow.assigned;
            self.harrow = Some(harrow);
        }
        // `end_ground_round`: the step report, then the removals' events.
        let planet = self.f.planet;
        let after: Vec<Piece> = sx.state.board().planet(planet).units.iter().copied().collect();
        if let Some(active) = sx.state.transient_mut().invasion.active.as_mut() {
            let active = active.edit();
            let mut counts = SortedMap::<Seat, u32, 4>::default();
            counts.insert(invader, to_defender);
            counts.insert(defender, to_invader);
            let mut step = self.f.template.clone();
            step.round = active.ground_round;
            step.after = after;
            step.hits = counts;
            step.harrow_hits = harrow_hits;
            active.last_step = Some(step);
        }
        if self.f.announce {
            hits::flush_staged(sx);
        } else {
            let _ = crate::kernel::fx::drain_staged(sx.state);
        }
        self.stage = Stage::Done;
        Next::Done
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        None
    }

    fn answer(&mut self, _: &mut Sx<'_>, _: Pick) -> Result<(), Illegal> {
        Err(Illegal)
    }
}
