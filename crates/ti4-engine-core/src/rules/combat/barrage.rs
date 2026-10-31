//! The anti-fighter barrage step of round 1 (string `CombatWindow::barrage_step`, LRR 78.3), as a [`Window`]
//! that never asks. Per side, attacker first: `ANTI_FIGHTER_BARRAGE_STARTED`, the dice of the side's
//! [`barrage_plan`] drawn from the Dice stream (`Sx::die`) and recorded in `ledger.dice`, the presentation
//! (`barrage_hits`, `barrage_dice`) and `UNIT_ABILITY_ROLLED`. Then both sides' hits are applied: the
//! `ANTI_FIGHTER_BARRAGE_HITS` window, the target's first fighters removed in unit order, and the
//! `BarrageTookTheLastFighters` feat.
//!
//! With `Facts::raid` set the window asks: the hits beyond the target's fighters go to Raid Formation (Argent), a child
//! [`RaidFormation`] window that asks the producer which ship is damaged (the composed machine leaves it unset and its adapter keeps
//! an Argent side string).
//!
//! Not covered (the adapter returns `None`, the string result stands): a Jol-Nar commander or a Heart of
//! Ixth holder (reroll questions), a Waylay played this round, neutral sides, test faces, a pending stream
//! restore, and any ability effect that ran inside an announcement.

use super::{barrage_plan, record_rolls, FleetRoll, Rd};
use crate::choice::{Choice, Pick};
use crate::generated::{System, Unit};
use crate::ids::Seat;
use crate::kernel::fx;
use super::raid_formation::{Facts as RaidFacts, RaidFormation};
use crate::kernel::stage::{Child, Illegal, Ran, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::reach;
use crate::state::{Code, CombatDie, CoreState, EventKind, Feat, Key, Payload, RngDomain, RollPurpose, Val};

const DIE_SIDES: u32 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    /// Applying the hits of `results[at]`.
    Applying,
    Done,
}

/// The facts of one barrage step.
#[derive(Debug, Clone)]
pub struct Facts {
    pub attacker: Seat,
    pub defender: Seat,
    pub system: System,
    pub round: u32,
    /// The feat occurrence of the combat (`CombatWindow::combat_occurrence`).
    pub occurrence: u64,
    /// `supply::staging_enabled`: a fighter lost to the barrage is announced.
    pub staging: bool,
    /// The resolver carries the timing machinery (`ctx.emit` announces).
    pub announce: bool,
    /// Interned texts: the presentation phase `barrage`, the cause `unit_ability:anti_fighter_barrage`, the
    /// kind `anti_fighter_barrage` of the roll event, and the relic's `extra barrage`.
    pub phase: Code,
    pub cause: Code,
    pub kind: Code,
    pub extra: Code,
    /// The interned unit-type text of every unit that may roll (`CombatDie.unit`).
    pub unit_codes: Vec<(Unit, Code)>,
    /// Raid Formation's subtype text (`"raid_formation_damage"`): set, an Argent producer's excess hits are offered to the ability.
    pub raid: Option<Code>,
}

/// Every ship that will roll a barrage die for one of these sides has an interned unit-type text in `codes`. The adapter interns the
/// types on the board when the settle begins; a type that arrives in the window (a technology gained at the start of the combat
/// swaps the owner's units to their upgrade) would be named `extra barrage`, the relic's label, in the presentation of the dice.
#[must_use]
pub fn every_roller_is_named(rd: Rd<'_>, sides: [Seat; 2], system: System, codes: &[(Unit, Code)]) -> bool {
    sides.into_iter().all(|side| barrage_plan(rd, side, system).iter().all(|r| r.unit.is_none_or(|u| codes.iter().any(|(k, _)| *k == u))))
}

#[derive(Debug, Clone)]
pub struct BarrageStep {
    pub f: Facts,
    /// The rolls of the step in order: (hit number, faces), for the dual to compare with the string dice.
    pub drawn: Vec<(u32, Vec<u32>)>,
    /// The seats that scored the barrage feat.
    pub feat: Vec<Seat>,
    stage: Stage,
    /// The sides that rolled hits, attacker first, and the one being applied.
    results: Vec<(Seat, usize)>,
    at: usize,
    /// The target's fighters when the side being applied began (the feat is scored when they are all gone).
    before: usize,
    raid: Child<RaidFormation>,
}

impl BarrageStep {
    #[must_use]
    pub fn new(f: Facts) -> Self {
        Self { f, drawn: Vec::new(), feat: Vec::new(), stage: Stage::Start, results: Vec::new(), at: 0, before: 0, raid: Child::idle() }
    }

    fn unit_code(&self, unit: Option<Unit>) -> Code {
        match unit {
            Some(u) => self.f.unit_codes.iter().find(|(k, _)| *k == u).map_or(self.f.extra, |(_, c)| *c),
            None => self.f.extra,
        }
    }

    /// One side's roll (the string `roll_barrage_side` and the reroll window with nothing to ask).
    fn roll_side(&mut self, sx: &mut Sx<'_>, side: Seat) -> usize {
        let f = &self.f;
        let rd = Rd::new(sx.state, sx.kernel).with_blanked(reach::blanked(sx.state));
        let plan = barrage_plan(rd, side, f.system);
        let mut entries: Vec<(Option<Unit>, u32, Vec<u32>)> = Vec::new();
        for r in plan {
            let hits_on = u32::try_from(r.hits_on).unwrap_or(u32::MAX);
            let faces: Vec<u32> = (0..r.dice).map(|_| sx.die(RngDomain::Dice, DIE_SIDES)).collect();
            entries.push((r.unit, hits_on, faces));
        }
        let rolls: Vec<FleetRoll> = entries.iter().map(|(_, on, faces)| FleetRoll { hits_on: *on, faces: faces.clone(), rerolled: Vec::new() }).collect();
        record_rolls(sx.state.ledger_mut(), side, Some(self.f.system), &rolls, RollPurpose::AntiFighterBarrage);
        for (_, on, faces) in &entries {
            self.drawn.push((*on, faces.clone()));
        }
        let hits: usize = entries.iter().map(|(_, on, faces)| faces.iter().filter(|face| **face >= *on).count()).sum();
        if entries.iter().any(|(_, _, faces)| !faces.is_empty()) {
            if self.f.announce {
                let payload = Payload::new()
                    .with(Key::Kind, Val::Code(self.f.kind))
                    .with(Key::Player, Val::Seat(side))
                    .with(Key::System, Val::System(self.f.system))
                    .with(Key::Hits, Val::Int(i64::try_from(hits).unwrap_or(0)));
                let _ = sx.emit(fx::event(EventKind::UnitAbilityRolled, payload));
            }
            let dice: Vec<CombatDie> = entries
                .iter()
                .flat_map(|(unit, on, faces)| {
                    let code = self.unit_code(*unit);
                    faces.iter().map(move |face| CombatDie { player: side, unit: code, roll: *face, target: *on, hit: *on > 0 && *face >= *on })
                })
                .collect();
            let p = sx.state.transient_mut().combat.presentation.edit();
            p.barrage_hits.insert(side, u32::try_from(hits).unwrap_or(u32::MAX));
            p.barrage_dice.extend(dice);
        }
        hits
    }
}

impl Window for BarrageStep {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                Stage::Done => return Next::Done,
                Stage::Start => {
                    let (a, d, system) = (self.f.attacker, self.f.defender, self.f.system);
                    let mut results: Vec<(Seat, usize)> = Vec::new();
                    for side in [a, d] {
                        if self.f.announce {
                            let payload = Payload::new()
                                .with(Key::System, Val::System(system))
                                .with(Key::Player, Val::Seat(side))
                                .with(Key::Round, Val::Int(i64::from(self.f.round)));
                            let _ = sx.emit(fx::event(EventKind::AntiFighterBarrageStarted, payload));
                        }
                        sx.state.transient_mut().combat.presentation.edit().phase = self.f.phase;
                        let hits = self.roll_side(sx, side);
                        if hits > 0 {
                            results.push((side, hits));
                        }
                    }
                    sx.state.transient_mut().combat.last_reroll_player = None;
                    sx.state.transient_mut().combat.presentation.edit().phase = self.f.phase;
                    self.results = results;
                    self.at = 0;
                    self.stage = Stage::Applying;
                }
                Stage::Applying => {
                    match self.raid.step(sx) {
                        Some(Ran::Asks(ask)) => return Next::Continue(ask),
                        Some(Ran::Pass(next)) => return next,
                        Some(Ran::Done(_)) => {
                            self.score_feat(sx);
                            self.at += 1;
                            continue;
                        }
                        None => {}
                    }
                    let Some(&(player, hits)) = self.results.get(self.at) else {
                        self.stage = Stage::Done;
                        continue;
                    };
                    let (a, system) = (self.f.attacker, self.f.system);
                    let target = if player == a { self.f.defender } else { a };
                    self.before = Self::fighters(sx, target, system);
                    let before = self.before;
                    // "Before a hit would be assigned": the barrage's hits are a batch; what a card or an agent cancelled in the
                    // window is spent before the fighters are taken (string `apply_barrage`).
                    let hits = if hits > 0 && before > 0 && self.f.announce {
                        let payload = Payload::new()
                            .with(Key::System, Val::System(system))
                            .with(Key::Player, Val::Seat(target))
                            .with(Key::Gunner, Val::Seat(player))
                            .with(Key::Hits, Val::Int(i64::try_from(hits).unwrap_or(0)));
                        usize::try_from(crate::kernel::hits::open_window(sx, EventKind::AntiFighterBarrageHits, payload, target, u32::try_from(hits).unwrap_or(u32::MAX))).unwrap_or(0)
                    } else {
                        hits
                    };
                    if hits > 0 {
                        super::destroy_fighters::destroy_first_fighters(
                            sx,
                            super::destroy_fighters::Fighters {
                                target,
                                system,
                                hits: u16::try_from(hits).unwrap_or(u16::MAX),
                                cause: self.f.cause,
                                staging: self.f.staging,
                                announce: self.f.announce,
                            },
                        );
                    }
                    // Hits beyond the target's fighters are discarded (15.2a) unless Raid Formation redirects them.
                    let excess = hits.saturating_sub(before);
                    let argent = sx.state.players().by_seat(player).is_some_and(|p| p.faction.data().abilities.contains(&crate::generated::Ability::RaidFormation));
                    if let (Some(subtype), true, true) = (self.f.raid, excess > 0, argent) {
                        self.raid.start(RaidFormation::new(RaidFacts { producer: player, target, system, excess: u16::try_from(excess).unwrap_or(u16::MAX), subtype }));
                        continue;
                    }
                    self.score_feat(sx);
                    self.at += 1;
                }
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        self.raid.ask_of(s, k)
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        self.raid.answer(sx, pick)
    }
}

impl BarrageStep {
    fn fighters(sx: &Sx<'_>, target: Seat, system: System) -> usize {
        let rd = Rd::new(sx.state, sx.kernel);
        sx.state.board().sys(system).units.iter().filter(|u| u.owner() == target && rd.is_fighter(u.kind())).count()
    }

    /// `BarrageTookTheLastFighters`: the producer of the side being applied took the last of the target's fighters.
    fn score_feat(&mut self, sx: &mut Sx<'_>) {
        let Some(&(player, _)) = self.results.get(self.at) else { return };
        let target = if player == self.f.attacker { self.f.defender } else { self.f.attacker };
        if self.before > 0 && Self::fighters(sx, target, self.f.system) == 0 {
            if let Some(p) = sx.state.players_mut().by_seat_mut(player) {
                let entry = (Feat::BarrageTookTheLastFighters, self.f.occurrence);
                if !p.misc.event_feats.as_slice().contains(&entry) {
                    p.misc.event_feats.push(entry);
                    p.misc.event_feats.as_mut_slice().sort_unstable();
                }
            }
            self.feat.push(player);
        }
    }
}
