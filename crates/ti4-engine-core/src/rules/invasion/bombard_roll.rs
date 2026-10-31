//! The rolls of a bombardment, final (LRR 49.1; the string `InvasionWindow::announce_bombard_rerolls`, which calls
//! `combat::open_reroll_windows` for the invader): the dice of every bombarding unit were drawn and staged for the invader
//! (`transient.combat.reroll_staging`, kind `bombardment`, one entry per unit, `last_reroll_player` the invader), and now the event other
//! seats react to goes out: `UNIT_ABILITY_ROLLED` with the kind, the roller, the system and the hits the staged faces make. A card played
//! in its windows may draw the dice again (Scramble Frequency, registered in `ti4-engine-core-factions::cards`); the window then reads the
//! faces back, spends the staging and leaves the faces for the caller to take the planets' hits from (the string recomputes the plan's
//! dice and groups from the staged set).
//!
//! The invader's Jol-Nar commander (`Facts::commander`, W16): before the event the invader is asked about each die in turn
//! ([`Walk`]: one optional question per die of the staged set, the picked dice drawn again afterwards), as the string
//! `reroll_window` does before it emits; the event shows the hits of the faces that remain.
//!
//! Not covered (the adapter returns `None`, the string result stands): the Heart of Ixth (questions of the roller), irreproducible dice.

use crate::choice::{Choice, Pick};
use crate::rules::combat::commander_walk::Walk;
use crate::generated::System;
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::state::{Code, CoreState, EventKind, Key, Payload, Val};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    /// The invader's Jol-Nar commander is asked about each die.
    Walking,
    /// The event and the faces that remain.
    Rolled,
    Done,
}

/// The facts of one bombardment's reroll window.
#[derive(Debug, Clone)]
pub struct Facts {
    pub invader: Seat,
    pub system: System,
    /// The resolver carries the timing machinery (`ctx.emit` announces).
    pub announce: bool,
    /// The interned roll kind `bombardment` of the event.
    pub kind: Code,
    /// The invader's Jol-Nar commander is in play (it may reroll any of the dice).
    pub commander: bool,
    /// The unit text of each staged entry in the question renderer's table (the label of the commander's questions); empty: the staged
    /// entry's own code.
    pub shown: Vec<Code>,
}

#[derive(Debug, Clone)]
pub struct BombardRolled {
    pub f: Facts,
    /// The faces of each staged entry after the event, in order of the entries (what the caller recomputes the planets' hits from).
    pub faces: Vec<Vec<u8>>,
    stage: Stage,
    walk: Option<Walk>,
}

impl BombardRolled {
    #[must_use]
    pub fn new(f: Facts) -> Self {
        Self { f, faces: Vec::new(), stage: Stage::Start, walk: None }
    }

    /// `staged_hits`: the hits the invader's staged set makes (a die hits when its face plus its adjustment reaches the number).
    fn staged_hits(s: &CoreState, invader: Seat) -> usize {
        s.transient()
            .combat
            .reroll_staging
            .iter()
            .find(|(seat, _)| *seat == invader)
            .map_or(0, |(_, set)| {
                set.rolls
                    .iter()
                    .map(|e| {
                        let Some(on) = e.hits_on else { return 0 };
                        e.faces.iter().enumerate().filter(|(i, face)| i64::from(**face) + e.deltas.get(&u8::try_from(*i).unwrap_or(u8::MAX)).map_or(0, |d| i64::from(*d)) >= i64::from(on)).count()
                    })
                    .sum()
            })
    }
}

impl Window for BombardRolled {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                Stage::Done => return Next::Done,
                Stage::Start => {
                    // (The string stages the set, and names the roller, only when some die was rolled; without one there is no window.)
                    if !sx.state.transient().combat.reroll_staging.iter().any(|(seat, _)| *seat == self.f.invader) {
                        self.stage = Stage::Done;
                        return Next::Done;
                    }
                    // "After you roll dice for a unit ability: you may reroll any of those dice" (the Jol-Nar commander).
                    self.walk = if self.f.commander { Walk::start(sx.state, self.f.invader, &self.f.shown) } else { None };
                    self.stage = if self.walk.is_some() { Stage::Walking } else { Stage::Rolled };
                }
                Stage::Walking => {
                    let Some(walk) = self.walk.as_mut() else {
                        self.stage = Stage::Rolled;
                        continue;
                    };
                    if walk.done() {
                        walk.redraw(sx);
                        self.walk = None;
                        self.stage = Stage::Rolled;
                    } else {
                        return match walk.ask(sx.state) {
                            Some(choice) => Next::Continue(choice.ask),
                            None => Next::Done,
                        };
                    }
                }
                Stage::Rolled => {
                    self.stage = Stage::Done;
                    let f = self.f.clone();
                    if f.announce {
                        let hits = Self::staged_hits(sx.state, f.invader);
                        let payload = Payload::new()
                            .with(Key::Kind, Val::Code(f.kind))
                            .with(Key::Player, Val::Seat(f.invader))
                            .with(Key::System, Val::System(f.system))
                            .with(Key::Hits, Val::Int(i64::try_from(hits).unwrap_or(0)));
                        let _ = sx.emit(fx::event(EventKind::UnitAbilityRolled, payload));
                    }
                    // The faces the cards left, then the staging is spent (the string removes it and the roller after the plan is recomputed).
                    self.faces = sx
                        .state
                        .transient()
                        .combat
                        .reroll_staging
                        .iter()
                        .find(|(seat, _)| *seat == f.invader)
                        .map(|(_, set)| set.rolls.iter().map(|e| e.faces.to_vec()).collect())
                        .unwrap_or_default();
                    let combat = &mut sx.state.transient_mut().combat;
                    combat.reroll_staging.edit().retain(|(seat, _)| *seat != f.invader);
                    combat.last_reroll_player = None;
                    return Next::Done;
                }
            }
        }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        if self.stage == Stage::Walking { self.walk.as_ref()?.ask(s) } else { None }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != Stage::Walking {
            return Err(Illegal);
        }
        self.walk.as_mut().ok_or(Illegal)?.answer(sx.state, pick)
    }
}
