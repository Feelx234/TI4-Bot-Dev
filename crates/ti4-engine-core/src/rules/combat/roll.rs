//! The "Roll Dice" step of a space combat round (string `CombatWindow::roll_round`, LRR 78.5): the
//! presentation, both sides' dice (the attacker first, group by group in ascending combat value, drawn with
//! `Sx::die` and recorded in `ledger.dice`), each side's reroll window (string `reroll_window`, fleet kind:
//! Munitions Reserves after the roll, the Crown of Thalnos relic and law with the casualties of their
//! rerolls, the Heart of Ixth, then Wrath of Kenara), War Funding once both sides have rolled, the round's
//! hits (free and 0.0.1-forced, J.N.S. Hylarim's extra hits, every die's adjustment counted),
//! `combat_round_hits`, `combat_round_dice` and the assignment queue (forced hits first, the attacker's hits
//! against the defender before the defender's).
//!
//! A side's dice sit in a staged [`RerollSet`] (mirrored to `transient.combat.reroll_staging` while one of its
//! questions is open, so the adapter can describe the dice); each question is a stage: Munitions' offer, the
//! per-die "reroll die n of u" of the Crowns and of Munitions' "choose which dice", the Heart of Ixth's
//! adjust per holder, Wrath's purchase, War Funding's offer.
//!
//! Not covered (the adapter returns `None`, the string result stands; see `dual_window::roll_step`): a readied
//! The Thundarian / Yssaril agent (the roll-step reaction), the Keleres agent granting commodities as trade
//! goods, neutral sides, test faces, a pending stream restore, any hit a module
//! produces after the roll, and a reaction window inside a reroll casualty's announcement.

use super::assign::{announce_loss, remove_ship};
use super::end::finished;
use super::{fleet_groups, forced_hull, forces_non_fighters, planets_of_sorted, record_rolls, ships_of, FleetRoll, Rd, DIE_SIDES};
use crate::choice::combat::{Ask as CAsk, Opt as COpt, RerollSource, Subtype as CSub};
use crate::choice::generic::Content;
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype, Target};
use crate::containers::{SmallList, SortedMap};
use crate::generated::{Ability, Agenda, PromissoryNote, Relic, System, Unit};
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::keys::Outcome;
use crate::query::{laws as qlaws, reach, relic as qrelic};
use crate::state::{Code, CombatDie, CoreState, NoteKey, Piece, RerollEntry, RerollSet, RngDomain, RollPurpose};

/// Munitions Reserves' price in trade goods (`faction_abilities::MUNITIONS_COST`).
pub const MUNITIONS_COST: i32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    /// Draw the dice of side `side` (0 attacker, 1 defender) and stage them.
    Roll,
    /// Munitions Reserves after the roll: gate and offer.
    Munitions,
    MunitionsAsking,
    /// Walk the staged dice asking one question per die (`src` asks).
    Dice(RerollSource),
    DiceAsking(RerollSource),
    /// The Crowns' casualties and the move to the Heart of Ixth.
    Casualties,
    Heart,
    HeartAsking,
    Wrath,
    WrathAsking,
    /// The side's window is closed.
    SideDone,
    Funding,
    FundingAsking,
    /// The end of the step (78.5): `SPACE_COMBAT_ROLL_STEP_ENDED` when someone listens, and the replay a Thundarian asked for.
    StepEnd,
    Finish,
    Done,
}

/// Most replays of one step (`combat::MAX_ROLL_REPLAYS`).
pub const MAX_ROLL_REPLAYS: u32 = 8;

/// What names a roll group in the presentation and the questions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name {
    pub seat: Seat,
    pub value: i64,
    pub tag: u8,
    /// The interned text in the state's table (`CombatDie.unit`, `RerollEntry.unit`).
    pub state: Code,
    /// The interned text in the question renderer's table (labels and prompts).
    pub shown: Code,
    /// The unit types behind the roll with how many of each, in the alphabetical order of their ids.
    pub types: Vec<(Unit, u32)>,
}

/// The facts of one roll step.
#[derive(Debug, Clone)]
pub struct Facts {
    pub attacker: Seat,
    pub defender: Seat,
    pub system: System,
    /// The round (the step-ended event names it).
    pub round: u32,
    /// The presentation phase `resolving_hits`.
    pub phase: Code,
    /// The text of every roll group the sides will roll: the group's unit types joined with `+` in id order.
    pub names: Vec<Name>,
    /// Munitions Reserves after the roll is asked of this side (rules version and the ability), attacker first.
    pub munitions_after: [bool; 2],
    /// The resolver carries the timing machinery (a reroll casualty's loss is announced).
    pub announce: bool,
    /// The interned cause `space_combat` of a casualty's `SHIP_DESTROYED`.
    pub cause: Code,
    /// The question renderer's text of the Hacan flagship's origin (`hacan_flagship`).
    pub hacan: Code,
}

/// One entry of the assignment queue (the string `Pending` without `producer_assigns`, which a roll never
/// queues).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Queued {
    pub player: Seat,
    pub hits: u32,
    pub producer: Seat,
    pub non_fighters_only: bool,
}

/// What a staged entry stands for.
#[derive(Debug, Clone)]
struct Meta {
    tag: u8,
    shown: Code,
    /// The unit types behind the roll with how many of each, in the string's (alphabetical) order.
    types: Vec<(Unit, u32)>,
}

#[derive(Debug, Clone)]
struct Staged {
    set: RerollSet,
    meta: Vec<Meta>,
}

type Pos = (u8, u16);

#[derive(Debug, Clone)]
pub struct RollStep {
    pub f: Facts,
    /// The rolls of the step in order: (hit number, faces), for the dual to compare with the string dice.
    pub drawn: Vec<(u32, Vec<u32>)>,
    pub queue: Vec<Queued>,
    /// Each side's damaged ships at the time the hits were queued (`damaged_before_round`).
    pub damaged: Vec<(Seat, Vec<Unit>)>,
    /// A side's window left a side without ships (the Crowns destroy the units that reroll and miss): the fight is over inside the
    /// step, so the other side does not roll, nothing is queued and nothing is announced (string `roll_both_sides` returning `None`).
    pub ended: bool,
    stage: Stage,
    side: usize,
    staged: [Option<Staged>; 2],
    /// The dice a per-die walk visits and where it stands; the picks it collected.
    walk: Vec<Pos>,
    at: usize,
    picks: Vec<Pos>,
    thalnos: Vec<Pos>,
    crown: Vec<Pos>,
    heart_holders: Vec<Seat>,
    heart_at: usize,
    funding_at: usize,
    /// How many times the step was played again (a Thundarian).
    replays: u32,
}

impl RollStep {
    #[must_use]
    pub fn new(f: Facts) -> Self {
        Self {
            f,
            drawn: Vec::new(),
            queue: Vec::new(),
            damaged: Vec::new(),
            ended: false,
            stage: Stage::Start,
            side: 0,
            staged: [None, None],
            walk: Vec::new(),
            at: 0,
            picks: Vec::new(),
            thalnos: Vec::new(),
            crown: Vec::new(),
            heart_holders: Vec::new(),
            heart_at: 0,
            funding_at: 0,
            replays: 0,
        }
    }

    fn seat(&self, side: usize) -> Seat {
        if side == 0 { self.f.attacker } else { self.f.defender }
    }

    fn name(&self, side: Seat, value: i64, tag: u8) -> Name {
        self.f
            .names
            .iter()
            .find(|n| n.seat == side && n.value == value && n.tag == tag)
            .cloned()
            .unwrap_or(Name { seat: side, value, tag, state: self.f.phase, shown: self.f.hacan, types: Vec::new() })
    }

    fn rd<'a>(s: &'a CoreState, k: &'a Kernel) -> Rd<'a> {
        Rd::new(s, k).with_blanked(reach::blanked(s))
    }

    fn cur(&self) -> Option<&Staged> {
        self.staged[self.side].as_ref()
    }

    fn cur_mut(&mut self) -> Option<&mut Staged> {
        self.staged[self.side].as_mut()
    }

    /// Mirror the open side's set to the state (for the adapter's `Choice.details`).
    fn mirror(&self, sx: &mut Sx<'_>) {
        let seat = self.seat(self.side);
        let Some(staged) = self.cur() else { return };
        let list = sx.state.transient_mut().combat.reroll_staging.edit();
        list.retain(|(s, _)| *s != seat);
        let at = list.partition_point(|(s, _)| *s < seat);
        list.insert(at, (seat, staged.set.clone()));
    }

    fn unmirror(&self, sx: &mut Sx<'_>) {
        let seat = self.seat(self.side);
        sx.state.transient_mut().combat.reroll_staging.edit().retain(|(s, _)| *s != seat);
    }

    /// Draw one side's dice (`roll_fleet_and_open_staged` up to the window).
    fn roll_side(&mut self, sx: &mut Sx<'_>) {
        let side = self.seat(self.side);
        let system = self.f.system;
        let round_seq = sx.state.meta().combat_round_seq;
        let (groups, munitions) = {
            let rd = Self::rd(sx.state, sx.kernel);
            let munitions = sx.state.players().by_seat(side).is_some_and(|p| p.combat_timers.munitions_round.is(round_seq));
            (fleet_groups(rd, side, system), munitions)
        };
        let mut rolls: Vec<FleetRoll> = Vec::new();
        let mut entries: Vec<RerollEntry> = Vec::new();
        let mut meta: Vec<Meta> = Vec::new();
        for ((value, tag), group) in groups {
            let count = usize::try_from(group.dice).unwrap_or(0);
            if count == 0 {
                continue;
            }
            let hits_on = u32::try_from(value).unwrap_or(u32::MAX);
            let faces: Vec<u32> = (0..count).map(|_| sx.die(RngDomain::Dice, DIE_SIDES)).collect();
            let misses: Vec<usize> = faces.iter().enumerate().filter(|(_, f)| **f < hits_on).map(|(i, _)| i).collect();
            rolls.push(FleetRoll { hits_on, faces: faces.clone(), rerolled: Vec::new() });
            let mut last = faces;
            if munitions && !misses.is_empty() {
                let mut again = last.clone();
                let rerolled = misses.iter().map(|i| *i as u32).collect();
                for i in misses {
                    again[i] = sx.die(RngDomain::Dice, DIE_SIDES);
                }
                rolls.push(FleetRoll { hits_on, faces: again.clone(), rerolled });
                last = again;
            }
            let name = self.name(side, value, tag);
            let types = name.types.clone();
            entries.push(RerollEntry {
                unit: name.state,
                planet: None,
                hits_on: u8::try_from(value).ok(),
                faces: last.iter().map(|f| u8::try_from(*f).unwrap_or(u8::MAX)).collect(),
                rerolled: 0,
                deltas: SortedMap::new(),
                unit_types: SortedMap::new(),
            });
            meta.push(Meta { tag, shown: name.shown, types });
        }
        record_rolls(sx.state.ledger_mut(), side, Some(system), &rolls, RollPurpose::SpaceCombat);
        for r in &rolls {
            self.drawn.push((r.hits_on, r.faces.clone()));
        }
        if entries.iter().any(|e| !e.faces.is_empty()) {
            self.staged[self.side] = Some(Staged { set: RerollSet { kind: self.f.phase, system, rolls: entries }, meta });
            sx.state.transient_mut().combat.last_reroll_player = Some(side);
        }
    }

    fn all_dice(set: &RerollSet) -> Vec<Pos> {
        set.rolls
            .iter()
            .enumerate()
            .flat_map(|(e, r)| (0..r.faces.len()).map(move |d| (e as u8, d as u16)))
            .collect()
    }

    fn adjusted(entry: &RerollEntry, die: u16) -> i64 {
        i64::from(entry.faces[usize::from(die)]) + i64::from(entry.deltas.get(&(die as u8)).copied().unwrap_or(0))
    }

    /// `staged_misses`.
    fn misses(set: &RerollSet) -> Vec<Pos> {
        Self::all_dice(set)
            .into_iter()
            .filter(|(e, d)| {
                let entry = &set.rolls[usize::from(*e)];
                entry.hits_on.is_some_and(|on| Self::adjusted(entry, *d) < i64::from(on))
            })
            .collect()
    }

    /// `RerollEntry::hits`.
    fn entry_hits(entry: &RerollEntry) -> u32 {
        entry
            .hits_on
            .map_or(0, |on| (0..entry.faces.len()).filter(|d| Self::adjusted(entry, *d as u16) >= i64::from(on)).count() as u32)
    }

    fn staged_hits(set: &RerollSet) -> u32 {
        set.rolls.iter().map(Self::entry_hits).sum()
    }

    /// `apply_reroll_dice`: draw again each picked die in order, recording each draw.
    fn apply(&mut self, sx: &mut Sx<'_>, owner: usize, picks: &[Pos]) {
        let seat = self.seat(owner);
        let system = self.f.system;
        let Some(staged) = self.staged[owner].as_mut() else { return };
        for (e, d) in picks {
            let Some(entry) = staged.set.rolls.get_mut(usize::from(*e)) else { continue };
            if usize::from(*d) >= entry.faces.len() {
                continue;
            }
            let face = sx.die(RngDomain::Dice, DIE_SIDES);
            entry.faces[usize::from(*d)] = u8::try_from(face).unwrap_or(u8::MAX);
            if *d < 64 {
                entry.rerolled |= 1 << *d;
            }
            entry.deltas.remove(&(*d as u8));
            let hits_on = u32::from(entry.hits_on.unwrap_or(0));
            let faces: Vec<u32> = entry.faces.iter().map(|f| u32::from(*f)).collect();
            self.drawn.push((hits_on, faces.clone()));
            record_rolls(sx.state.ledger_mut(), seat, Some(system), &[FleetRoll { hits_on, faces, rerolled: vec![u32::from(*d)] }], RollPurpose::SpaceCombat);
        }
    }

    fn goods(s: &CoreState, seat: Seat) -> i32 {
        s.players().by_seat(seat).map_or(0, |p| i32::from(p.econ.trade_goods).max(0))
    }

    fn holds_crown_relic(s: &CoreState, seat: Seat) -> bool {
        qrelic::holds(s, seat, Relic::TheCrownOfThalnos)
    }

    fn owns_crown_law(s: &CoreState, seat: Seat) -> bool {
        qlaws::elected(s, reach::blanked(s), Agenda::TheCrownOfThalnos) == Some(Outcome::Seat(seat))
    }

    /// The War Funding note `holder` holds from another faction.
    fn war_funding_note(s: &CoreState, holder: Seat) -> Option<NoteKey> {
        let own = crate::query::promissory::faction_of(s, holder);
        s.cards().promissory.iter().find(|(k, h)| **h == holder && k.note == PromissoryNote::WarFunding && Some(k.faction) != own).map(|(k, _)| *k)
    }

    /// The Crowns' casualties: entries whose rerolled dice produced no hit are lost with their ships.
    fn casualties(&mut self, sx: &mut Sx<'_>) {
        if self.thalnos.is_empty() && self.crown.is_empty() {
            return;
        }
        let side = self.seat(self.side);
        let system = self.f.system;
        let Some(staged) = self.cur() else { return };
        let doomed: Vec<usize> = staged
            .set
            .rolls
            .iter()
            .enumerate()
            .filter_map(|(i, entry)| {
                let thalnos = self.thalnos.iter().any(|(e, _)| usize::from(*e) == i) && Self::entry_hits(entry) == 0;
                let crown = {
                    let dice: Vec<u16> = self.crown.iter().filter(|(e, _)| usize::from(*e) == i).map(|(_, d)| *d).collect();
                    !dice.is_empty() && entry.hits_on.map_or(0, |on| dice.iter().filter(|d| entry.faces[usize::from(**d)] >= on).count()) == 0
                };
                (thalnos || crown).then_some(i)
            })
            .collect();
        if doomed.is_empty() {
            return;
        }
        let mut lost: Vec<Meta> = Vec::new();
        if let Some(staged) = self.cur_mut() {
            for i in doomed.iter().rev() {
                staged.set.rolls.remove(*i);
                lost.push(staged.meta.remove(*i));
            }
        }
        for entry in lost {
            for (unit, count) in entry.types {
                if count == 0 {
                    continue;
                }
                let mut matching: Vec<Piece> = sx.state.board().sys(system).units.iter().filter(|p| p.owner() == side && p.kind() == unit).copied().collect();
                for planet in planets_of_sorted(sx.state, system).iter() {
                    matching.extend(sx.state.board().planet(*planet).units.iter().filter(|p| p.owner() == side && p.kind() == unit).copied());
                }
                matching.truncate(count as usize);
                for piece in matching {
                    remove_ship(sx, system, side, piece);
                    announce_loss(sx, system, side, piece.kind(), self.f.cause, true, self.f.announce);
                }
            }
        }
    }

    /// The Heart of Ixth holders that still have it ready, in seat order.
    fn heart_candidates(s: &CoreState) -> Vec<Seat> {
        s.seats().players().iter().copied().filter(|seat| qrelic::ready(s, *seat, Relic::HeartOfIxth)).collect()
    }

    fn wrath_near(&self) -> Vec<Pos> {
        let Some(staged) = self.cur() else { return Vec::new() };
        Self::all_dice(&staged.set)
            .into_iter()
            .filter(|(e, d)| {
                let entry = &staged.set.rolls[usize::from(*e)];
                entry.hits_on.is_some_and(|on| Self::adjusted(entry, *d) + 1 == i64::from(on))
            })
            .collect()
    }

    fn has_hacan_flagship(s: &CoreState, seat: Seat, system: System) -> bool {
        s.board().sys(system).units.iter().any(|p| p.owner() == seat && super::flagship_has_text(s, seat, p.kind(), super::FlagshipText::Hacan))
    }

    fn origin_of(&self, src: RerollSource) -> Origin {
        match src {
            RerollSource::Relic => Origin::Content(Content::Relic(Relic::TheCrownOfThalnos)),
            RerollSource::Law => Origin::Agenda(Agenda::TheCrownOfThalnos),
            RerollSource::Munitions => Origin::FactionAbility(Content::Ability(Ability::MunitionsReserves)),
            RerollSource::FireTeam => Origin::ActionCard(crate::generated::ActionCard::FireTeam),
            RerollSource::Commander => Origin::Content(Content::Leader(crate::generated::Leader::AgnlanOln)),
        }
    }

    fn ctx(&self, origin: Origin, subtype: CSub, target: bool) -> Ctx {
        Ctx {
            origin,
            subtype: Subtype::Combat(subtype),
            optional: false,
            target: target.then_some(Target::System(self.f.system)),
            outstanding: SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        }
    }

    /// The question of the open stage, if it is one.
    fn question(&self) -> Option<(Seat, CAsk)> {
        let side = self.seat(self.side);
        let staged = self.cur()?;
        match self.stage {
            Stage::MunitionsAsking => Some((side, CAsk::MunitionsRoll)),
            Stage::DiceAsking(src) => {
                let (e, d) = *self.walk.get(self.at)?;
                Some((side, CAsk::RerollDie { source: src, unit: staged.meta.get(usize::from(e))?.shown, die: d as u8 }))
            }
            Stage::HeartAsking => Some((*self.heart_holders.get(self.heart_at)?, CAsk::HeartIxth)),
            Stage::WrathAsking => Some((side, CAsk::Wrath)),
            Stage::FundingAsking => Some((self.seat(self.funding_at), CAsk::WarFunding)),
            _ => None,
        }
    }
}


impl Window for RollStep {
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
                    let rd = Self::rd(sx.state, sx.kernel);
                    let mut round_start = ships_of(rd, a, system);
                    round_start.extend(ships_of(rd, d, system));
                    let p = sx.state.transient_mut().combat.presentation.edit();
                    p.phase = self.f.phase;
                    p.round_start = round_start;
                    self.side = 0;
                    self.stage = Stage::Roll;
                }
                Stage::Roll => {
                    self.roll_side(sx);
                    self.stage = if self.staged[self.side].is_some() { Stage::Munitions } else { Stage::SideDone };
                }
                Stage::Munitions => {
                    let side = self.seat(self.side);
                    let misses = self.cur().map(|s| Self::misses(&s.set)).unwrap_or_default();
                    if self.f.munitions_after[self.side] && !misses.is_empty() && Self::goods(sx.state, side) >= MUNITIONS_COST {
                        self.mirror(sx);
                        self.stage = Stage::MunitionsAsking;
                    } else {
                        self.begin_walk(sx);
                    }
                }
                Stage::Dice(src) => {
                    if self.at >= self.walk.len() {
                        self.finish_walk(sx, src);
                        continue;
                    }
                    self.mirror(sx);
                    self.stage = Stage::DiceAsking(src);
                }
                Stage::Casualties => {
                    self.casualties(sx);
                    self.thalnos.clear();
                    self.crown.clear();
                    self.heart_holders = match self.cur() {
                        Some(s) if !Self::all_dice(&s.set).is_empty() => Self::heart_candidates(sx.state),
                        _ => Vec::new(),
                    };
                    self.heart_at = 0;
                    self.stage = Stage::Heart;
                }
                Stage::Heart => {
                    if self.heart_at >= self.heart_holders.len() {
                        self.stage = Stage::Wrath;
                        continue;
                    }
                    self.mirror(sx);
                    self.stage = Stage::HeartAsking;
                }
                Stage::Wrath => {
                    let side = self.seat(self.side);
                    let near = self.wrath_near();
                    let goods = usize::try_from(Self::goods(sx.state, side)).unwrap_or(0);
                    self.stage = if self.cur().is_some()
                        && Self::has_hacan_flagship(sx.state, side, self.f.system)
                        && goods > 0
                        && near.len().min(goods) > 0
                    {
                        self.mirror(sx);
                        Stage::WrathAsking
                    } else {
                        Stage::SideDone
                    };
                }
                Stage::SideDone => {
                    self.unmirror(sx);
                    sx.state.transient_mut().combat.last_reroll_player = None;
                    // The string asks `over` after each side's window: a fleet the rerolls emptied ends the fight where it stands.
                    if finished(Self::rd(sx.state, sx.kernel), self.f.system, self.f.attacker, self.f.defender) {
                        self.ended = true;
                        self.stage = Stage::Done;
                        return Next::Done;
                    }
                    if self.side == 0 {
                        self.side = 1;
                        self.stage = Stage::Roll;
                    } else {
                        self.funding_at = 0;
                        self.stage = Stage::Funding;
                    }
                }
                Stage::Funding => {
                    if self.funding_at >= 2 {
                        self.stage = Stage::StepEnd;
                        continue;
                    }
                    let (holder, opponent) = (self.funding_at, 1 - self.funding_at);
                    let seat = self.seat(holder);
                    let offered = Self::war_funding_note(sx.state, seat).is_some() && {
                        let opp_hits = self.staged[opponent].as_ref().map_or(0, |s| Self::staged_hits(&s.set));
                        let own_misses = self.staged[holder].as_ref().map(|s| Self::misses(&s.set)).unwrap_or_default();
                        opp_hits > 0 || !own_misses.is_empty()
                    };
                    if offered {
                        self.stage = Stage::FundingAsking;
                    } else {
                        self.funding_at += 1;
                    }
                }
                Stage::StepEnd => {
                    // Both sides' dice are final and nothing has landed. The event is emitted only when someone is listening (a readied
                    // The Thundarian), so a game without one allocates no event id for it; its window may ask for the step to be rolled
                    // afresh, which throws the dice away without producing or assigning a hit.
                    sx.state.marks_mut().tail.edit().remove(&crate::keys::MarkKey::PrivateCombatReplayRoll());
                    if self.f.announce && sx.kernel.hooks.direct.nomad_agents_watches_roll_step(sx.state) {
                        let payload = crate::state::Payload::new()
                            .with(crate::state::Key::System, crate::state::Val::System(self.f.system))
                            .with(crate::state::Key::Round, crate::state::Val::Int(i64::from(self.f.round)))
                            .with(crate::state::Key::Attacker, crate::state::Val::Seat(self.f.attacker))
                            .with(crate::state::Key::Defender, crate::state::Val::Seat(self.f.defender));
                        let _ = sx.emit(fx::event(crate::state::EventKind::SpaceCombatRollStepEnded, payload));
                    }
                    if sx.state.marks_mut().tail.edit().remove(&crate::keys::MarkKey::PrivateCombatReplayRoll()).is_some() {
                        self.replays += 1;
                        if self.replays <= MAX_ROLL_REPLAYS {
                            self.staged = [None, None];
                            self.stage = Stage::Start;
                            continue;
                        }
                    }
                    self.replays = 0;
                    self.stage = Stage::Finish;
                }
                Stage::Finish => {
                    self.finish(sx);
                    self.stage = Stage::Done;
                    return Next::Done;
                }
                Stage::MunitionsAsking | Stage::DiceAsking(_) | Stage::HeartAsking | Stage::WrathAsking | Stage::FundingAsking => {
                    return match self.question() {
                        Some((_, ask)) => Next::Continue(Ask::Combat(ask)),
                        None => {
                            // Nothing to ask (a removed entry): skip the question.
                            self.skip_question();
                            continue;
                        }
                    };
                }
            }
        }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        let (actor, ask) = self.question()?;
        let staged = self.cur();
        let (origin, subtype, target, opts): (Origin, CSub, bool, Vec<COpt>) = match self.stage {
            Stage::MunitionsAsking => {
                let misses = Self::misses(&staged?.set).len();
                let mut opts = vec![COpt::Munitions { choose: false, misses: misses as u8 }];
                if misses > 1 {
                    opts.push(COpt::Munitions { choose: true, misses: misses as u8 });
                }
                (self.origin_of(RerollSource::Munitions), CSub::MunitionsRoll, false, opts)
            }
            Stage::DiceAsking(src) => {
                let staged = staged?;
                let (e, d) = *self.walk.get(self.at)?;
                let entry = staged.set.rolls.get(usize::from(e))?;
                let opt = COpt::RerollDie { unit: staged.meta[usize::from(e)].shown, entry: e, die: d as u8, face: entry.faces[usize::from(d)], hits_on: entry.hits_on };
                (self.origin_of(src), CSub::Reroll(src), true, vec![opt])
            }
            Stage::HeartAsking => {
                let staged = staged?;
                let dice = Self::all_dice(&staged.set);
                let single = dice.len() == 1;
                let opts = dice
                    .iter()
                    .flat_map(|(e, d)| {
                        let unit = staged.meta[usize::from(if single { 0 } else { *e })].shown;
                        [true, false].map(|add| COpt::HeartIxth { unit, entry: *e, die: *d as u8, add, single })
                    })
                    .collect();
                (Origin::Content(Content::Relic(Relic::HeartOfIxth)), CSub::HeartIxth, false, opts)
            }
            Stage::WrathAsking => {
                let goods = usize::try_from(Self::goods(s, actor)).unwrap_or(0);
                let most = self.wrath_near().len().min(goods);
                (Origin::Content(Content::Other(self.f.hacan)), CSub::WrathBump, false, (1..=most).map(|c| COpt::Kenara { count: c as u8 }).collect())
            }
            Stage::FundingAsking => (Origin::Content(Content::Promissory(PromissoryNote::WarFunding)), CSub::WarFunding, true, vec![COpt::UseWarFunding]),
            _ => return None,
        };
        let mut all: Vec<Opt> = opts.into_iter().map(Opt::Combat).collect();
        all.push(Opt::Decline);
        Some(Choice { actor, ask: Ask::Combat(ask), ctx: self.ctx(origin, subtype, target), opts: Opts(all.into_iter().collect()) })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let Some(choice) = self.ask_of(sx.state, sx.kernel) else { return Err(Illegal) };
        let Pick::Opt(opt) = pick else { return Err(Illegal) };
        if !choice.opts.as_slice().contains(&opt) {
            return Err(Illegal);
        }
        let side = self.seat(self.side);
        match self.stage {
            Stage::MunitionsAsking => {
                let Opt::Combat(COpt::Munitions { choose, .. }) = opt else {
                    self.begin_walk(sx);
                    return Ok(());
                };
                if !fx::spend_trade_goods(sx.state, side, MUNITIONS_COST) {
                    self.begin_walk(sx);
                    return Ok(());
                }
                if choose {
                    self.walk = self.cur().map(|s| Self::all_dice(&s.set)).unwrap_or_default();
                    self.at = 0;
                    self.picks.clear();
                    self.stage = Stage::Dice(RerollSource::Munitions);
                } else {
                    let picks = self.cur().map(|s| Self::misses(&s.set)).unwrap_or_default();
                    self.apply(sx, self.side, &picks);
                    self.begin_walk(sx);
                }
            }
            Stage::DiceAsking(src) => {
                if let Opt::Combat(COpt::RerollDie { entry, die, .. }) = opt {
                    self.picks.push((entry, u16::from(die)));
                }
                self.at += 1;
                self.stage = Stage::Dice(src);
            }
            Stage::HeartAsking => {
                // The string `heart_ixth` splits the option id (`hfix|e:d:sign`) on `|` and expects four parts, so
                // every pick fails to parse there: no die is shifted and the card is not exhausted. The window
                // copies that exactly (a rules fix is a RULES_VERSION change, not part of this port).
                self.heart_at += 1;
                self.stage = Stage::Heart;
            }
            Stage::WrathAsking => {
                if let Opt::Combat(COpt::Kenara { count }) = opt {
                    let near = self.wrath_near();
                    if fx::spend_trade_goods(sx.state, side, i32::from(count)) {
                        if let Some(staged) = self.cur_mut() {
                            for (e, d) in near.into_iter().take(usize::from(count)) {
                                let delta = staged.set.rolls[usize::from(e)].deltas.get(&(d as u8)).copied().unwrap_or(0);
                                staged.set.rolls[usize::from(e)].deltas.insert(d as u8, delta + 1);
                            }
                        }
                    }
                }
                self.stage = Stage::SideDone;
            }
            Stage::FundingAsking => {
                if matches!(opt, Opt::Combat(COpt::UseWarFunding)) {
                    let (holder, opponent) = (self.funding_at, 1 - self.funding_at);
                    let seat = self.seat(holder);
                    if let Some(note) = Self::war_funding_note(sx.state, seat) {
                        let opponent_dice = self.staged[opponent].as_ref().map(|s| Self::all_dice(&s.set)).unwrap_or_default();
                        let own_misses = self.staged[holder].as_ref().map(|s| Self::misses(&s.set)).unwrap_or_default();
                        self.apply(sx, opponent, &opponent_dice);
                        self.apply(sx, holder, &own_misses);
                        fx::give_back_note(sx.state, note);
                    }
                }
                self.funding_at += 1;
                self.stage = Stage::Funding;
            }
            _ => return Err(Illegal),
        }
        Ok(())
    }
}

impl RollStep {
    /// Start the Crown of Thalnos relic's walk (or skip to the law's).
    fn begin_walk(&mut self, sx: &mut Sx<'_>) {
        self.start(sx, RerollSource::Relic);
    }

    /// Enter the walk of `src`'s dice when its card is held, else the next card.
    fn start(&mut self, sx: &mut Sx<'_>, src: RerollSource) {
        let side = self.seat(self.side);
        let has_dice = self.cur().is_some_and(|s| s.set.rolls.iter().any(|e| !e.faces.is_empty()));
        let held = has_dice
            && match src {
                RerollSource::Relic => Self::holds_crown_relic(sx.state, side),
                RerollSource::Law => Self::owns_crown_law(sx.state, side),
                // Munitions and Fire Team are not walks of the roll step (Fire Team is a card played after the ground roll).
                RerollSource::Munitions | RerollSource::FireTeam | RerollSource::Commander => false,
            };
        if held {
            self.walk = self.cur().map(|s| Self::all_dice(&s.set)).unwrap_or_default();
            self.at = 0;
            self.picks.clear();
            self.stage = Stage::Dice(src);
        } else if src == RerollSource::Relic {
            self.start(sx, RerollSource::Law);
        } else {
            self.stage = Stage::Casualties;
        }
    }

    /// The walk of `src` is complete: apply its picks and move on.
    fn finish_walk(&mut self, sx: &mut Sx<'_>, src: RerollSource) {
        let picks = std::mem::take(&mut self.picks);
        self.apply(sx, self.side, &picks);
        match src {
            RerollSource::Munitions => self.begin_walk(sx),
            RerollSource::Relic => {
                if let Some(staged) = self.cur_mut() {
                    for (e, d) in &picks {
                        if let Some(entry) = staged.set.rolls.get_mut(usize::from(*e)) {
                            entry.deltas.insert(*d as u8, 1);
                        }
                    }
                }
                self.thalnos = picks;
                self.start(sx, RerollSource::Law);
            }
            RerollSource::Law => {
                self.crown = picks;
                self.stage = Stage::Casualties;
            }
            RerollSource::FireTeam | RerollSource::Commander => self.stage = Stage::Casualties,
        }
    }

    /// A question whose subject has gone: pass over it.
    fn skip_question(&mut self) {
        self.stage = match self.stage {
            Stage::MunitionsAsking => {
                self.picks.clear();
                Stage::Casualties
            }
            Stage::DiceAsking(src) => {
                self.at += 1;
                Stage::Dice(src)
            }
            Stage::HeartAsking => {
                self.heart_at += 1;
                Stage::Heart
            }
            Stage::WrathAsking => Stage::SideDone,
            _ => {
                self.funding_at += 1;
                Stage::Funding
            }
        };
    }

    /// The round's hits from the final dice, the dice record, the damage note and the queue.
    fn finish(&mut self, sx: &mut Sx<'_>) {
        let (a, d, system) = (self.f.attacker, self.f.defender, self.f.system);
        let mut totals: Vec<(Seat, u32, u32)> = Vec::new();
        for (i, seat) in [a, d].into_iter().enumerate() {
            let (mut free, mut forced) = (0_u32, 0_u32);
            if let Some(staged) = &self.staged[i] {
                let rd = Self::rd(sx.state, sx.kernel);
                let binding = forces_non_fighters(rd, seat, system);
                for (entry, meta) in staged.set.rolls.iter().zip(&staged.meta) {
                    let mut hits = Self::entry_hits(entry);
                    if meta.tag == 1 {
                        hits += 2 * entry.faces.iter().filter(|f| **f >= 9).count() as u32;
                    }
                    if binding && !meta.types.is_empty() && meta.types.iter().all(|(u, _)| forced_hull(rd, *u)) {
                        forced += hits;
                    } else {
                        free += hits;
                    }
                }
            }
            totals.push((seat, free, forced));
        }
        let dice: Vec<CombatDie> = super::round_dice(&[(a, self.staged[0].as_ref().map(|s| &s.set)), (d, self.staged[1].as_ref().map(|s| &s.set))]);
        let combat = &mut sx.state.transient_mut().combat;
        for (side, free, forced) in &totals {
            combat.combat_round_hits.insert(*side, free + forced);
        }
        let mut log = crate::containers::Log::default();
        for die in dice {
            log.push(die);
        }
        combat.combat_round_dice = log;
        combat.last_reroll_player = None;
        let get = |side: Seat| totals.iter().find(|t| t.0 == side).map_or((0, 0), |t| (t.1, t.2));
        let (a_free, a_forced) = get(a);
        let (d_free, d_forced) = get(d);
        self.queue = [(d, a_forced, a, true), (d, a_free, a, false), (a, d_forced, d, true), (a, d_free, d, false)]
            .into_iter()
            .filter(|(_, hits, _, _)| *hits > 0)
            .map(|(player, hits, producer, non_fighters_only)| Queued { player, hits, producer, non_fighters_only })
            .collect();
        self.damaged = [a, d]
            .into_iter()
            .map(|side| {
                let list = sx.state.board().sys(system).units.iter().filter(|u| u.owner() == side && u.sustained()).map(|u| u.kind()).collect();
                (side, list)
            })
            .collect();
    }
}
