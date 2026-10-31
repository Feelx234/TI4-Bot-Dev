//! The stage transitions of a space combat that move between windows without doing a window's own work
//! (string `CombatWindow::{announcing_step, retreating_step, hit_queue_head}`, the arms of `settle_stages`):
//! the step-start announcement, who is asked about retreating next, the end of a round, the hit queue's
//! bookkeeping (empty and unanswerable entries, the once-per-round HITS_TO_ASSIGN announcement, the sustain
//! shortcut) and the decision that is left pending.
//!
//! The window is `Window::step` for one transition: [`Flow::step`] is the stage, `out` the next stage. Facts
//! the string window answers from its own rules (a player with nowhere to go, whether a ship is left, the
//! sustain candidates) are inputs; the control flow, the events and the fields of the window it changes are
//! the core's. Not covered (the adapter returns `None`): the casualty shortcut of a
//! single candidate, a neutral side's automatic answer, Duranium Armor repairs, a fleet that retreats
//! without a question, hits modules produce after a round, destructions still to announce, a reaction
//! window opened by an announcement (`Recording::nested`).

use crate::choice::{Choice, Pick};
use crate::generated::System;
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::state::{Code, CoreState, EventKind, Key, Payload, Val};

/// Most rounds of one combat (`combat::MAX_ROUNDS`).
pub const MAX_ROUNDS: u32 = 50;

pub use crate::state::{CombatPend as Pend, CombatStep as Step, HitPhase as Phase};

/// What the string window answers from its own rules, plus the fields of the window the transition reads.
#[derive(Debug, Clone)]
pub struct Facts {
    pub attacker: Seat,
    pub defender: Seat,
    pub system: System,
    /// The resolver carries the timing machinery (`ctx.emit` announces).
    pub announce: bool,
    /// The fight is finished (fewer than two fleets).
    pub over: bool,
    /// The seat being asked in `Announcing` has somewhere to retreat to.
    pub asking_can_retreat: bool,
    /// Where the first leaver of `Retreating` may still go.
    pub leaver_destinations: usize,
    /// Ships the player at the front of the queue has left.
    pub front_alive: usize,
    /// The once-per-round announcement was made for the front player already.
    pub offered_before: bool,
    /// Something of the front player could sustain.
    pub sustainers: bool,
    pub pending_retreats: Vec<Seat>,
    pub phase: Phase,
    pub resuming: bool,
}

/// What a transition leaves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Out {
    pub next: Step,
    /// A decision is pending (or the fight is over): `settle_stages` stops here.
    pub returned: bool,
    pub phase: Phase,
    pub resuming: bool,
    pub pending_retreats: Vec<Seat>,
    /// The front player's once-per-round announcement was made now.
    pub offered_now: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    Done,
}

#[derive(Debug, Clone)]
pub struct Flow {
    /// The causes a retreat's lost ships are staged and announced under: set, the effects staged on the end of a round are announced after it.
    drain: Option<(Code, Code)>,
    pub f: Facts,
    pub step: Step,
    pub out: Option<Out>,
    stage: Stage,
}

impl Flow {
    #[must_use]
    pub fn new(f: Facts, step: Step) -> Self {
        Self { drain: None, f, step, out: None, stage: Stage::Start }
    }

    /// The same transition draining the destructions an effect staged on the end of a round (`announce_round_ended`).
    #[must_use]
    pub const fn with_drain(mut self, retreat: (Code, Code)) -> Self {
        self.drain = Some(retreat);
        self
    }

    /// Announce the end of the round; whether the fight is over AFTER it (an ability of the window, Exotrireme II, may have destroyed the last ships of
    /// a side: the string reads `over` again in `next_round_or_end`, after the announcement).
    fn round_ended(&self, sx: &mut Sx<'_>, round: u32) -> bool {
        let finished = |sx: &Sx<'_>| {
            let rd = super::Rd::new(sx.state, sx.kernel).with_blanked(crate::query::reach::blanked(sx.state));
            super::end::finished(rd, self.f.system, self.f.attacker, self.f.defender)
        };
        let before = finished(sx);
        if self.f.announce {
            let payload = Payload::new()
                .with(Key::System, Val::System(self.f.system))
                .with(Key::Round, Val::Int(i64::from(round)))
                .with(Key::Attacker, Val::Seat(self.f.attacker))
                .with(Key::Defender, Val::Seat(self.f.defender));
            let _ = sx.emit(fx::event(EventKind::SpaceCombatRoundEnded, payload));
        }
        if let Some(retreat) = self.drain {
            super::assign::drain_staged(sx, self.f.announce, retreat);
        }
        // (Over before the announcement: the fact the caller read. Over only after it: the window's doing.)
        self.f.over || (!before && finished(sx))
    }

    /// `spend_cancellations`: up to `wanted` of the seat's round-scoped cancellations, how many were spent.
    fn spend_cancellations(&self, sx: &mut Sx<'_>, who: Seat, wanted: u32) -> u32 {
        let round = sx.state.meta().combat_round_seq;
        let Some(p) = sx.state.players_mut().by_seat_mut(who) else { return 0 };
        let available = match p.card_timers.cancel_hits_round {
            Some((held, had)) if held == round => had,
            _ => 0,
        };
        let spent = available.min(wanted);
        if spent > 0 {
            p.card_timers.cancel_hits_round = Some((round, available - spent));
        }
        spent
    }

    fn after_round(over: bool, round: u32) -> Step {
        if over || round >= MAX_ROUNDS {
            Step::Done
        } else {
            Step::Opening { round: round + 1 }
        }
    }
}

impl Window for Flow {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Done {
            return Next::Done;
        }
        let f = self.f.clone();
        let mut out = Out { next: self.step.clone(), returned: false, phase: f.phase, resuming: f.resuming, pending_retreats: f.pending_retreats.clone(), offered_now: false };
        match self.step.clone() {
            Step::Announcing { round, asking, announced } => {
                if asking == f.defender && announced.is_empty() && f.announce {
                    let payload = Payload::new()
                        .with(Key::System, Val::System(f.system))
                        .with(Key::Player, Val::Seat(f.defender))
                        .with(Key::Round, Val::Int(i64::from(round)));
                    let _ = sx.emit(fx::event(EventKind::RetreatStepStarted, payload));
                }
                if f.over {
                    out.next = Step::Done;
                    out.returned = true;
                } else if f.asking_can_retreat {
                    out.returned = true;
                } else if asking == f.defender && !announced.contains(&f.defender) {
                    out.next = Step::Announcing { round, asking: f.attacker, announced };
                } else {
                    out.pending_retreats = announced;
                    out.next = Step::Rolling { round };
                }
            }
            Step::Retreating { round, leaving } => {
                if leaving.is_empty() {
                    let over = self.round_ended(sx, round);
                    out.phase = Phase::Round;
                    out.next = Self::after_round(over, round);
                } else if f.leaver_destinations == 0 {
                    out.next = Step::Retreating { round, leaving: leaving[1..].to_vec() };
                } else {
                    out.returned = true; // several destinations: the question; one is the string's move
                }
            }
            Step::Queue { assigning, round, queue } => {
                let Some(front) = queue.first().copied() else {
                    match f.phase {
                        Phase::CombatStart => {
                            out.phase = Phase::Round;
                            if f.over {
                                out.next = Step::Done;
                                out.returned = true;
                            } else {
                                out.resuming = true;
                                out.next = Step::Opening { round };
                            }
                        }
                        Phase::RoundEnd => {
                            let over = self.round_ended(sx, round);
                            out.phase = Phase::Round;
                            out.next = Self::after_round(over, round);
                        }
                        Phase::Round => {
                            if f.over || round >= MAX_ROUNDS {
                                let _ = self.round_ended(sx, round);
                                out.phase = Phase::Round;
                                out.next = Step::Done;
                                out.returned = true;
                            } else {
                                out.next = Step::Retreating { round, leaving: f.pending_retreats.clone() };
                                out.pending_retreats = Vec::new();
                            }
                        }
                    }
                    self.out = Some(out);
                    self.stage = Stage::Done;
                    return Next::Done;
                };
                if front.hits == 0 || f.front_alive == 0 {
                    out.next = Step::Queue { assigning: false, round, queue: queue[1..].to_vec() };
                } else {
                    // Shields Holding and friends cancel hits before any are assigned (string `hit_queue_head_string`): the pool the
                    // round holds is spent here, before the once-per-round announcement and the sustain offer, because a cancelled hit
                    // is one nobody has to absorb. A neutral side's automatic answer comes after this (the adapter keeps that string).
                    let spent = self.spend_cancellations(sx, front.player, front.hits);
                    if spent > 0 {
                        let mut rest = queue.clone();
                        rest[0].hits -= spent;
                        out.next = Step::Queue { assigning, round, queue: rest };
                        self.out = Some(out);
                        self.stage = Stage::Done;
                        return Next::Done;
                    }
                    if !assigning && !f.offered_before {
                        out.offered_now = true;
                        if f.announce {
                            let payload = Payload::new()
                                .with(Key::System, Val::System(f.system))
                                .with(Key::Player, Val::Seat(front.player))
                                .with(Key::Hits, Val::Int(i64::from(front.hits)))
                                .with(Key::Round, Val::Int(i64::from(round)));
                            let _ = sx.emit(fx::event(EventKind::HitsToAssign, payload));
                            // The window may have granted cancellations (Tellurian, a card): they are spent on the
                            // hits before a sustain is offered or a ship chosen (string `open_hits_window`'s caller).
                            let spent = self.spend_cancellations(sx, front.player, front.hits);
                            if spent > 0 {
                                let mut rest = queue.clone();
                                rest[0].hits -= spent;
                                out.next = Step::Queue { assigning: false, round, queue: rest };
                                self.out = Some(out);
                                self.stage = Stage::Done;
                                return Next::Done;
                            }
                        }
                    }
                    if !assigning && !f.sustainers {
                        out.next = Step::Queue { assigning: true, round, queue };
                    } else {
                        out.returned = true;
                    }
                }
            }
            _ => out.returned = true,
        }
        self.out = Some(out);
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
