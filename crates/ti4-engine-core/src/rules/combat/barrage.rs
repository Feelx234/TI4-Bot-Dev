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
//! The Jol-Nar commander (`Facts::commander`, package W10): after a side rolls, its holder is asked about each die in turn (the
//! string `reroll_window` / `choose_reroll_dice`: one question per die, the answers collected, then the picked dice are drawn again
//! in order); the event of the roll and the presentation read the final faces.
//!
//! A Waylay played before a side's roll (`card_timers.waylay_barrage_round`, package W10): that side's hits are produced against every
//! ship, so they land through the [`Absorb`] child (cancellations, SUSTAIN DAMAGE, the owner's casualties) instead of the fighters.
//!
//! Not covered (the adapter returns `None`, the string result stands): a Heart of Ixth holder (reroll
//! questions), neutral sides, test faces, a pending stream restore, and any
//! ability effect that ran inside an announcement.

use super::{barrage_plan, record_rolls, FleetRoll, Rd};
use crate::choice::combat::{Ask as CAsk, Opt as COpt, RerollSource, Subtype as CSub};
use crate::choice::generic::Content;
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype, Target};
use crate::containers::SmallList;
use crate::generated::{System, Unit};
use crate::ids::Seat;
use crate::kernel::fx;
use super::effect_hits::Absorb;
use super::raid_formation::{Facts as RaidFacts, RaidFormation};
use crate::kernel::children::HitTexts;
use crate::kernel::stage::{Child, Illegal, Ran, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::reach;
use crate::state::{Code, CombatDie, CoreState, EventKind, Feat, Key, Payload, RerollEntry, RerollSet, RngDomain, RollPurpose, Val};

const DIE_SIDES: u32 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    /// Begin the roll of side `side`: the announcement, the dice.
    Side,
    /// The commander's walk over the side's dice (one question each).
    Walk,
    Asking,
    /// The side's roll is final: the event, the presentation.
    Finish,
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
    /// Per side (attacker, defender): the Jol-Nar commander is in play for it and may reroll any of its dice.
    pub commander: [bool; 2],
    /// The unit types' texts in the question renderer's table (a question's labels); `extra_shown` is the relic's `extra barrage`.
    pub shown: Vec<(Unit, Code)>,
    pub extra_shown: Code,
    /// The texts of a Waylay's hits (the question renderer's `letnevcommander`, `assign_casualty`, `effect_hit`).
    pub hit_texts: HitTexts,
}

/// One entry of a side's staged roll: the unit type behind it (`None`: the relic's dice), the hit number and the faces.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    unit: Option<Unit>,
    hits_on: u32,
    faces: Vec<u32>,
    /// The dice drawn again (a bit mask).
    rerolled: u64,
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
    /// The rolls of the step in order: (hit number, faces), for the dual to compare with the string dice. A commander's redraw is one of
    /// them (the composed machine compares the whole history); [`Self::first_draws`] leaves them out (the standalone step compares the
    /// string's rolls of the barrage's own reason).
    pub drawn: Vec<(u32, Vec<u32>)>,
    redraws: Vec<usize>,
    /// The seats that scored the barrage feat.
    pub feat: Vec<Seat>,
    stage: Stage,
    /// The sides that rolled hits, attacker first, and the one being applied.
    results: Vec<(Seat, usize)>,
    at: usize,
    /// The target's fighters when the side being applied began (the feat is scored when they are all gone).
    before: usize,
    raid: Child<RaidFormation>,
    /// A Waylay's hits land on every ship (the string `absorb_hits_seeing_with_context`).
    absorb: Child<Absorb>,
    /// The side rolling (0 attacker, 1 defender) and its staged entries.
    side: usize,
    entries: Vec<Entry>,
    /// The dice the commander's walk visits (entry, die), where it stands, and the dice it picked.
    walk: Vec<(u8, u16)>,
    walk_at: usize,
    picks: Vec<(u8, u16)>,
}

impl BarrageStep {
    #[must_use]
    pub fn new(f: Facts) -> Self {
        Self { f, drawn: Vec::new(), feat: Vec::new(), redraws: Vec::new(), stage: Stage::Start, results: Vec::new(), at: 0, before: 0, raid: Child::idle(), absorb: Child::idle(), side: 0, entries: Vec::new(), walk: Vec::new(), walk_at: 0, picks: Vec::new() }
    }

    fn unit_code(&self, unit: Option<Unit>) -> Code {
        match unit {
            Some(u) => self.f.unit_codes.iter().find(|(k, _)| *k == u).map_or(self.f.extra, |(_, c)| *c),
            None => self.f.extra,
        }
    }

    /// The rolls of the step without the commander's redraws.
    #[must_use]
    pub fn first_draws(&self) -> Vec<(u32, Vec<u32>)> {
        self.drawn.iter().enumerate().filter(|(i, _)| !self.redraws.contains(i)).map(|(_, d)| d.clone()).collect()
    }

    fn seat(&self, side: usize) -> Seat {
        if side == 0 { self.f.attacker } else { self.f.defender }
    }

    fn rd<'a>(s: &'a CoreState, k: &'a Kernel) -> Rd<'a> {
        Rd::new(s, k).with_blanked(reach::blanked(s))
    }

    /// The unit text of the renderer's table.
    fn shown_code(&self, unit: Option<Unit>) -> Code {
        match unit {
            Some(u) => self.f.shown.iter().find(|(k, _)| *k == u).map_or(self.f.extra_shown, |(_, c)| *c),
            None => self.f.extra_shown,
        }
    }

    /// The side's dice (the string `roll_barrage_side`): drawn from the Dice stream, recorded, kept as `entries`.
    fn draw_side(&mut self, sx: &mut Sx<'_>, side: Seat) {
        let rd = Self::rd(sx.state, sx.kernel);
        let plan = barrage_plan(rd, side, self.f.system);
        self.entries.clear();
        for r in plan {
            let hits_on = u32::try_from(r.hits_on).unwrap_or(u32::MAX);
            let faces: Vec<u32> = (0..r.dice).map(|_| sx.die(RngDomain::Dice, DIE_SIDES)).collect();
            self.entries.push(Entry { unit: r.unit, hits_on, faces, rerolled: 0 });
        }
        let rolls: Vec<FleetRoll> = self.entries.iter().map(|e| FleetRoll { hits_on: e.hits_on, faces: e.faces.clone(), rerolled: Vec::new() }).collect();
        record_rolls(sx.state.ledger_mut(), side, Some(self.f.system), &rolls, RollPurpose::AntiFighterBarrage);
        for e in &self.entries {
            self.drawn.push((e.hits_on, e.faces.clone()));
        }
    }

    fn entry_hits(e: &Entry) -> usize {
        e.faces.iter().filter(|face| **face >= e.hits_on).count()
    }

    /// The side's dice as the staged set the questions describe (`transient.combat.reroll_staging`, mirrored while a question is open).
    fn mirror(&self, sx: &mut Sx<'_>, seat: Seat) {
        let rolls: Vec<RerollEntry> = self
            .entries
            .iter()
            .map(|e| RerollEntry {
                unit: self.unit_code(e.unit),
                planet: None,
                hits_on: Some(u8::try_from(e.hits_on).unwrap_or(u8::MAX)),
                faces: e.faces.iter().map(|f| u8::try_from(*f).unwrap_or(u8::MAX)).collect(),
                rerolled: e.rerolled,
                deltas: crate::containers::SortedMap::default(),
                unit_types: crate::containers::SortedMap::default(),
            })
            .collect();
        let set = RerollSet { kind: self.f.phase, system: self.f.system, rolls };
        let list = sx.state.transient_mut().combat.reroll_staging.edit();
        list.retain(|(s, _)| *s != seat);
        let at = list.partition_point(|(s, _)| *s < seat);
        list.insert(at, (seat, set));
    }

    fn unmirror(sx: &mut Sx<'_>, seat: Seat) {
        sx.state.transient_mut().combat.reroll_staging.edit().retain(|(s, _)| *s != seat);
    }

    /// The question of the open walk: the die the walk stands at.
    fn question(&self) -> Option<(Seat, CAsk, COpt)> {
        let (e, d) = *self.walk.get(self.walk_at)?;
        let entry = self.entries.get(usize::from(e))?;
        let face = u8::try_from(*entry.faces.get(usize::from(d))?).unwrap_or(u8::MAX);
        let unit = self.shown_code(entry.unit);
        let hits_on = Some(u8::try_from(entry.hits_on).unwrap_or(u8::MAX));
        Some((self.seat(self.side), CAsk::RerollDie { source: RerollSource::Commander, unit, die: u8::try_from(d).unwrap_or(u8::MAX) }, COpt::RerollDie { unit, entry: e, die: u8::try_from(d).unwrap_or(u8::MAX), face, hits_on }))
    }

    /// `apply_reroll_dice`: draw again each picked die in order, recording each draw.
    fn redraw(&mut self, sx: &mut Sx<'_>, seat: Seat) {
        let picks = std::mem::take(&mut self.picks);
        let system = self.f.system;
        for (e, d) in picks {
            let Some(entry) = self.entries.get_mut(usize::from(e)) else { continue };
            if usize::from(d) >= entry.faces.len() {
                continue;
            }
            entry.faces[usize::from(d)] = sx.die(RngDomain::Dice, DIE_SIDES);
            if d < 64 {
                entry.rerolled |= 1 << d;
            }
            let (hits_on, faces) = (entry.hits_on, entry.faces.clone());
            self.drawn.push((hits_on, faces.clone()));
            self.redraws.push(self.drawn.len() - 1);
            record_rolls(sx.state.ledger_mut(), seat, Some(system), &[FleetRoll { hits_on, faces, rerolled: vec![u32::from(d)] }], RollPurpose::AntiFighterBarrage);
        }
    }

    /// The side's roll is final: its hits, the event and the presentation. The hits are returned.
    ///
    /// The dice are staged for the event (`mirror`, `last_reroll_player`: the string `roll_barrage_side` stages them and
    /// `open_reroll_windows` emits), a card played in its windows may draw them again (Scramble Frequency), and the hits are read from
    /// the faces that remain (`read_back`).
    fn finish_side(&mut self, sx: &mut Sx<'_>, side: Seat) -> usize {
        if self.entries.iter().any(|e| !e.faces.is_empty()) {
            self.mirror(sx, side);
            sx.state.transient_mut().combat.last_reroll_player = Some(side);
            if self.f.announce {
                let hits: usize = self.entries.iter().map(Self::entry_hits).sum();
                let payload = Payload::new()
                    .with(Key::Kind, Val::Code(self.f.kind))
                    .with(Key::Player, Val::Seat(side))
                    .with(Key::System, Val::System(self.f.system))
                    .with(Key::Hits, Val::Int(i64::try_from(hits).unwrap_or(0)));
                let mark = Self::ledger_mark(sx);
                let _ = sx.emit(fx::event(EventKind::UnitAbilityRolled, payload));
                self.read_back(sx, side, mark);
            }
            Self::unmirror(sx, side);
        }
        let hits: usize = self.entries.iter().map(Self::entry_hits).sum();
        if self.entries.iter().any(|e| !e.faces.is_empty()) {
            let dice: Vec<CombatDie> = self
                .entries
                .iter()
                .flat_map(|e| {
                    let code = self.unit_code(e.unit);
                    e.faces.iter().map(move |face| CombatDie { player: side, unit: code, roll: *face, target: e.hits_on, hit: e.hits_on > 0 && *face >= e.hits_on })
                })
                .collect();
            let p = sx.state.transient_mut().combat.presentation.edit();
            p.barrage_hits.insert(side, u32::try_from(hits).unwrap_or(u32::MAX));
            p.barrage_dice.extend(dice);
        }
        hits
    }

    /// How many rolls the ledger holds (a window lent none has no mark to take).
    fn ledger_mark(sx: &Sx<'_>) -> usize {
        if sx.state.loaded.has(crate::state::Slice::Ledger) { sx.state.ledger().dice.len() } else { 0 }
    }

    /// After the event: the staged faces are the side's roll now (a card drew dice again), and the redraws the cards recorded in the
    /// ledger since `mark` are the window's own, recorded in `drawn` as the string's dice history records them.
    // (The rows after `mark` are the redraws of this event's cards: the roller's, nothing else is recorded between the mark and the read.)
    fn read_back(&mut self, sx: &mut Sx<'_>, side: Seat, mark: usize) {
        let staged = sx.state.transient().combat.reroll_staging.iter().find(|(s, _)| *s == side).map(|(_, set)| set.clone());
        if let Some(set) = staged {
            for (entry, now) in self.entries.iter_mut().zip(set.rolls.iter()) {
                entry.faces = now.faces.iter().map(|f| u32::from(*f)).collect();
            }
        }
        if sx.state.loaded.has(crate::state::Slice::Ledger) {
            let redraws: Vec<(u32, Vec<u32>)> = sx
                .state
                .ledger()
                .dice
                .iter()
                .skip(mark)
                .filter(|r| r.purpose == RollPurpose::Reroll)
                .map(|r| (u32::from(r.hits_on.unwrap_or(0)), r.faces.iter().map(|f| u32::from(*f)).collect()))
                .collect();
            for redraw in redraws {
                self.drawn.push(redraw);
                self.redraws.push(self.drawn.len() - 1);
            }
        }
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
                    self.results = Vec::new();
                    self.side = 0;
                    self.stage = Stage::Side;
                }
                Stage::Side => {
                    let (side, system) = (self.seat(self.side), self.f.system);
                    if self.f.announce {
                        let payload = Payload::new()
                            .with(Key::System, Val::System(system))
                            .with(Key::Player, Val::Seat(side))
                            .with(Key::Round, Val::Int(i64::from(self.f.round)));
                        let _ = sx.emit(fx::event(EventKind::AntiFighterBarrageStarted, payload));
                    }
                    sx.state.transient_mut().combat.presentation.edit().phase = self.f.phase;
                    self.draw_side(sx, side);
                    // "After you roll dice for a unit ability: you may reroll any of those dice" (the Jol-Nar commander).
                    let dice = self.entries.iter().any(|e| !e.faces.is_empty());
                    if self.f.commander[self.side] && dice {
                        self.walk = self
                            .entries
                            .iter()
                            .enumerate()
                            .flat_map(|(e, entry)| (0..entry.faces.len()).map(move |d| (e as u8, d as u16)))
                            .collect();
                        self.walk_at = 0;
                        self.picks.clear();
                        self.stage = Stage::Walk;
                    } else {
                        self.stage = Stage::Finish;
                    }
                }
                Stage::Walk => {
                    let side = self.seat(self.side);
                    if self.walk_at >= self.walk.len() {
                        self.redraw(sx, side);
                        Self::unmirror(sx, side);
                        self.stage = Stage::Finish;
                        continue;
                    }
                    self.mirror(sx, side);
                    self.stage = Stage::Asking;
                }
                Stage::Asking => {
                    return match self.question() {
                        Some((_, ask, _)) => Next::Continue(Ask::Combat(ask)),
                        None => {
                            self.walk_at += 1;
                            self.stage = Stage::Walk;
                            continue;
                        }
                    };
                }
                Stage::Finish => {
                    let side = self.seat(self.side);
                    let hits = self.finish_side(sx, side);
                    if hits > 0 {
                        self.results.push((side, hits));
                    }
                    if self.side == 0 {
                        self.side = 1;
                        self.stage = Stage::Side;
                    } else {
                        sx.state.transient_mut().combat.last_reroll_player = None;
                        sx.state.transient_mut().combat.presentation.edit().phase = self.f.phase;
                        self.at = 0;
                        self.stage = Stage::Applying;
                    }
                }
                Stage::Applying => {
                    match self.absorb.step(sx) {
                        Some(Ran::Asks(ask)) => return Next::Continue(ask),
                        Some(Ran::Pass(next)) => return next,
                        Some(Ran::Done(_)) => {
                            self.score_feat(sx);
                            self.at += 1;
                            continue;
                        }
                        None => {}
                    }
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
                    // The window opens only when something can be hit: the target has fighters, or a Waylay widens the hits to every ship.
                    let waylay = sx.state.players().by_seat(player).is_some_and(|p| p.card_timers.waylay_barrage_round.is(sx.state.meta().combat_round_seq));
                    let hits = if hits > 0 && (before > 0 || waylay) && self.f.announce {
                        let payload = Payload::new()
                            .with(Key::System, Val::System(system))
                            .with(Key::Player, Val::Seat(target))
                            .with(Key::Gunner, Val::Seat(player))
                            .with(Key::Hits, Val::Int(i64::try_from(hits).unwrap_or(0)));
                        usize::try_from(crate::kernel::hits::open_window(sx, EventKind::AntiFighterBarrageHits, payload, target, u32::try_from(hits).unwrap_or(u32::MAX))).unwrap_or(0)
                    } else {
                        hits
                    };
                    if hits > 0 && waylay {
                        // Waylay ("hits from this roll are produced against all ships (not just fighters)"): assigned like any other
                        // combat hits, SUSTAIN DAMAGE first, then the target's owner chooses the losses.
                        let absorb = Absorb::unit_ability(target, player, system, u16::try_from(hits).unwrap_or(u16::MAX), self.f.hit_texts, self.f.cause, false, false, self.f.announce).in_space_combat();
                        self.absorb.start(absorb);
                        continue;
                    }
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
        if self.stage == Stage::Asking {
            let (actor, ask, opt) = self.question()?;
            return Some(Choice {
                actor,
                ask: Ask::Combat(ask),
                ctx: Ctx {
                    origin: Origin::Content(Content::Leader(crate::generated::Leader::AgnlanOln)),
                    subtype: Subtype::Combat(CSub::Reroll(RerollSource::Commander)),
                    optional: false,
                    target: Some(Target::System(self.f.system)),
                    outstanding: SmallList::new(),
                    space_battle: false,
                    invasion_seq: None,
                },
                opts: Opts([Opt::Combat(opt), Opt::Decline].into_iter().collect()),
            });
        }
        if self.absorb.running() {
            return self.absorb.ask_of(s, k);
        }
        self.raid.ask_of(s, k)
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage == Stage::Asking {
            let Some((_, _, asked)) = self.question() else { return Err(Illegal) };
            let Pick::Opt(opt) = pick else { return Err(Illegal) };
            match opt {
                Opt::Decline => {}
                Opt::Combat(chosen) if chosen == asked => {
                    if let Some(at) = self.walk.get(self.walk_at) {
                        self.picks.push(*at);
                    }
                }
                _ => return Err(Illegal),
            }
            self.walk_at += 1;
            self.stage = Stage::Walk;
            return Ok(());
        }
        if self.absorb.running() {
            return self.absorb.answer(sx, pick);
        }
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
