//! Fleet and capacity enforcement (`fleet::enforce_seeing`, LRR 37 and 16): after movement the active seat removes
//! the units that are over its fleet supply, one question each, then the ones over capacity. The head of the aftermath
//! (`AftermathWindow::new`) and four other call sites run it.

use crate::choice::generic::Rule;
use crate::choice::tactical::{Ask, Opt, Over, Subtype};
use crate::choice::{Choice, Opt as COpt, Opts, Pick};
use crate::generated::{System, Unit};
use crate::ids::Seat;
use crate::kernel::fx_move;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::fleet::{self, Standing};
use crate::query::{board, reach::scope_of};
use crate::state::{CoreState, Piece};

/// What the core does not derive: the fleet limit as the string works it out (pool, laws, abilities), and the fighter
/// types that count as half a ship for the seat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The limit the enforcement judges by (Darktalon Treilla's lift is gated by the rules version).
    pub enforce: i64,
    /// The limit the question shows (`fleet::standing`: the lift is not gated).
    pub shown: i64,
    /// The fighter types counted as half (Naalu Hybrid Crystal Fighter II), at most one per unit form.
    pub halves: [Option<Unit>; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EStage {
    Supply,
    Capacity,
    Done,
}

/// The enforcement window of one seat in one system.
#[derive(Debug, Clone)]
pub struct Enforce {
    seat: Seat,
    system: System,
    limits: Limits,
    stage: EStage,
    /// Units removed so far.
    pub removed: u32,
}

impl Enforce {
    #[must_use]
    pub const fn new(seat: Seat, system: System, limits: Limits) -> Self {
        Self { seat, system, limits, stage: EStage::Supply, removed: 0 }
    }

    /// The enforcement as the position calls for it now: the stage with something to remove, or the end. A held window is rebuilt this
    /// way from `(seat, system, limits)` alone (`settle` is a function of the position, the units removed so far need no memory).
    #[must_use]
    pub fn resumed(mut self, core: &CoreState, k: &Kernel) -> Self {
        self.settle(core, k);
        self
    }

    fn half(&self, unit: Unit) -> bool {
        self.limits.halves.contains(&Some(unit))
    }

    fn standing(&self, core: &CoreState, k: &Kernel, limit: i64, halves: bool) -> Standing {
        let never = |_: Unit| false;
        let half = |u: Unit| self.half(u);
        fleet::standing(core, scope_of(k), self.seat, self.system, limit, None, if halves { &half } else { &never })
    }

    /// The units that could be removed for `over`, in the order of the seat's units in the space area.
    fn candidates(&self, core: &CoreState, over: Over) -> Vec<Piece> {
        board::space_units(core, self.system)
            .iter()
            .filter(|p| p.owner() == self.seat)
            .filter(|p| match over {
                Over::Supply => fleet::counts_against_supply(p.kind()),
                Over::Capacity => fleet::consumes_capacity(p.kind()),
            })
            .copied()
            .collect()
    }

    fn excess(&self, core: &CoreState, k: &Kernel, over: Over) -> i64 {
        match over {
            Over::Supply => self.standing(core, k, self.limits.enforce, true).fleet_excess(),
            Over::Capacity => self.standing(core, k, 0, false).capacity_excess,
        }
    }

    /// The stage the position calls for: a stage with nothing to remove (no excess, or no candidate) is skipped.
    fn settle(&mut self, core: &CoreState, k: &Kernel) {
        loop {
            let over = match self.stage {
                EStage::Supply => Over::Supply,
                EStage::Capacity => Over::Capacity,
                EStage::Done => return,
            };
            if self.excess(core, k, over) > 0 && !self.candidates(core, over).is_empty() {
                return;
            }
            self.stage = if over == Over::Supply { EStage::Capacity } else { EStage::Done };
        }
    }

    fn over(&self) -> Option<Over> {
        match self.stage {
            EStage::Supply => Some(Over::Supply),
            EStage::Capacity => Some(Over::Capacity),
            EStage::Done => None,
        }
    }
}

fn clip(n: i64) -> i32 {
    i32::try_from(n).unwrap_or(i32::MAX)
}

impl Window for Enforce {
    type Stage = EStage;

    fn stage(&self) -> EStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        self.settle(sx.state, sx.kernel);
        match self.ask_of(sx.state, sx.kernel) {
            Some(c) => Next::Continue(c.ask),
            None => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        let over = self.over()?;
        let cands = self.candidates(s, over);
        if cands.is_empty() || self.excess(s, k, over) <= 0 {
            return None;
        }
        let shown = self.standing(s, k, self.limits.shown, true);
        let mut seen: Vec<(Unit, bool)> = Vec::new();
        let mut opts = Opts::default();
        for (index, piece) in cands.iter().enumerate() {
            let key = (piece.kind(), piece.sustained());
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            opts.0.push(COpt::Tactical(Opt::Remove { index: u16::try_from(index).unwrap_or(u16::MAX), kind: piece.kind() }));
        }
        Some(Choice {
            actor: self.seat,
            ask: crate::choice::Ask::Tactical(Ask::Remove {
                seat: self.seat,
                system: self.system,
                over,
                charged: clip(shown.fleet_charged),
                limit: clip(shown.fleet_limit),
                consumed: clip(shown.consumed),
                transport: clip(shown.transport),
            }),
            ctx: super::ctx(Rule::R89_2, Subtype::RemoveUnit, None),
            opts,
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let Pick::Opt(COpt::Tactical(Opt::Remove { index, kind })) = pick else { return Err(Illegal) };
        let over = self.over().ok_or(Illegal)?;
        let cands = self.candidates(sx.state, over);
        // The string reads the index back from the id and removes that candidate (the first when it names none).
        let doomed = *cands.get(usize::from(index)).ok_or(Illegal)?;
        if doomed.kind() != kind {
            return Err(Illegal);
        }
        fx_move::destroy_ship(sx.state, self.system, doomed);
        self.removed += 1;
        self.settle(sx.state, sx.kernel);
        Ok(())
    }
}

