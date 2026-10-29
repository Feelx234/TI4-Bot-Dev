//! The hits step of the invasion (LRR 15.2, 42.3, 49.1, 87): a batch of hits on one player's ground forces on one
//! planet, shared by the bombardment, the ground combat round, Harrow and the space cannon defense.
//!
//! Two parts, as in the string module (`ground_hits_window`, then `remove_ground` / `take_bombard_hits`):
//!
//! * `open`: "before a hit would be assigned": the `GROUND_HITS_TO_ASSIGN` event (system, planet, player, hits,
//!   cause) through the shared [`hits::open_window`](crate::kernel::hits::open_window), spending what a card played in
//!   it cancelled;
//! * `apply`: the hits one after the other: SUSTAIN DAMAGE absorbs a hit before a unit is destroyed (cheapest first),
//!   Non-Euclidean Shielding lets one sustain cancel a second hit, a hit on nothing is wasted (15.2a). Each hit
//!   stages `GROUND_FORCE_SUSTAINED` / `GROUND_FORCE_DESTROYED` in `pending.staged`; the caller announces them
//!   ([`flush_staged`](crate::kernel::hits::flush_staged)) at the point the string module flushes its log.
//!
//! The window asks nothing: the owner never chooses here (the choosing assignment is [`super::absorb`]). Both
//! parts are plain functions of an [`Sx`] so a bigger window (the bombardment, the ground round) calls them at its
//! own program points; [`GroundHits`] wraps them as a window for the dual.

use super::{apply_ground_hit, ground_hit, GroundHit, Rd};
use crate::choice::{Choice, Pick};
use crate::generated::{Planet, System, Unit};
use crate::ids::Seat;
use crate::kernel::seam::data as seam_data;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{fx, hits, Kernel, Next};
use crate::query::reach;
use crate::state::{Code, CoreState, EventKind, Key, Payload, Piece, Val};

/// The facts of one batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Batch {
    pub victim: Seat,
    pub system: System,
    pub planet: Planet,
    /// The interned cause (`"ground_combat"`, `"bombardment"`, `"harrow"`, `"space_cannon_defense"`).
    pub cause: Code,
    /// The hits are a unit ability's (bombardment, space cannon defense, Harrow): units immune to those are skipped.
    pub unit_ability: bool,
    /// The resolver carries the timing machinery: the window is announced (`ctx.emit` without a timing handle
    /// opens no window).
    pub announce: bool,
}

fn rd<'a>(s: &'a CoreState, k: &'a Kernel) -> Rd<'a> {
    Rd::new(s, k).with_blanked(reach::blanked(s))
}

/// `invasion::ground_hits_window`: the hits that remain of `hits` after the window.
pub fn open(sx: &mut Sx<'_>, b: &Batch, hits: u32) -> u32 {
    if hits == 0 || !b.announce {
        return hits;
    }
    let payload = Payload::new()
        .with(Key::System, Val::System(b.system))
        .with(Key::Planet, Val::Planet(b.planet))
        .with(Key::Player, Val::Seat(b.victim))
        .with(Key::Hits, Val::Int(i64::from(hits)))
        .with(Key::Cause, Val::Code(b.cause));
    hits::open_window(sx, EventKind::GroundHitsToAssign, payload, b.victim, hits)
}

fn event(b: &Batch, kind: EventKind, piece: Piece, destroyed: bool) -> crate::state::EventRec {
    let mut p = Payload::new()
        .with(Key::System, Val::System(b.system))
        .with(Key::Planet, Val::Planet(b.planet))
        .with(Key::Player, Val::Seat(b.victim))
        .with(Key::Unit, Val::Unit(piece.kind()));
    if destroyed {
        p.set(Key::Damaged, Val::Flag(piece.sustained()));
        if piece.galvanized() {
            p.set(Key::Galvanized, Val::Flag(true));
        }
    }
    p.set(Key::Cause, Val::Code(b.cause));
    fx::event(kind, p)
}

/// `faction_techs::note_destroyed`: a destroyed Spec Ops II is counted for its survival roll.
fn note_destroyed(s: &mut CoreState, piece: Piece) {
    if piece.kind() == Unit::SpecOpsII {
        if let Some(p) = s.players_mut().by_seat_mut(piece.owner()) {
            p.misc.spec_ops_destroyed += 1;
        }
    }
}

/// `invasion::ground_hit_outcome_logged` over `hits` hits (`take_bombard_hits_text`, `remove_ground`): the board
/// changes now, the events are staged. Returns how many ground forces were destroyed.
pub fn apply(s: &mut CoreState, k: &Kernel, b: &Batch, hits: u32) -> u32 {
    let mut destroyed = 0;
    let mut remaining = hits;
    while remaining > 0 {
        remaining -= 1;
        let hit = ground_hit(rd(s, k), b.system, b.planet, b.victim, b.unit_ability);
        match hit {
            GroundHit::Sustained(p) => {
                fx::stage(s, event(b, EventKind::GroundForceSustained, p, false));
                let extra = match seam_data::cancel_on_sustain(rd(s, k).q(), &k.hooks.seam.rows, b.victim) {
                    n if n > 0 => (n as u32) - 1,
                    _ => 0,
                };
                remaining = remaining.saturating_sub(extra);
            }
            GroundHit::Destroyed(p) => {
                destroyed += 1;
                fx::stage(s, event(b, EventKind::GroundForceDestroyed, p, true));
                note_destroyed(s, p);
            }
            GroundHit::Wasted => {}
        }
        apply_ground_hit(s, b.planet, hit);
    }
    destroyed
}

/// The suspension points (the window never asks).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Open,
    Apply,
    Done,
}

/// One batch as a window: `open` the hits window, `apply` the hits, or both in that order (the hits that remain
/// after the window are the ones applied).
#[derive(Debug, Clone, Copy)]
pub struct GroundHits {
    pub batch: Batch,
    pub hits: u32,
    pub do_open: bool,
    pub do_apply: bool,
    /// Announce the staged events at the end of the window (the string caller flushes its log right after).
    pub flush: bool,
    /// After the window: the hits that remain (the window's result).
    pub remaining: u32,
    /// After the window: how many ground forces were destroyed.
    pub destroyed: u32,
    stage: Stage,
}

impl GroundHits {
    #[must_use]
    pub const fn new(batch: Batch, hits: u32, do_open: bool, do_apply: bool) -> Self {
        Self { batch, hits, do_open, do_apply, flush: false, remaining: hits, destroyed: 0, stage: Stage::Open }
    }
}

impl Window for GroundHits {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Open {
            if self.do_open {
                self.remaining = open(sx, &self.batch, self.hits);
            }
            self.stage = Stage::Apply;
        }
        if self.stage == Stage::Apply {
            if self.do_apply {
                self.destroyed = apply(sx.state, sx.kernel, &self.batch, self.remaining);
            }
            if self.flush {
                hits::flush_staged(sx);
            }
            self.stage = Stage::Done;
        }
        Next::Done
    }

    fn ask_of(&self, _s: &CoreState, _k: &Kernel) -> Option<Choice> {
        None
    }

    fn answer(&mut self, _sx: &mut Sx<'_>, _pick: Pick) -> Result<(), Illegal> {
        Err(Illegal)
    }
}
