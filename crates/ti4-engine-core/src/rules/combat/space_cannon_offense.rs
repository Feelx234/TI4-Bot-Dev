//! The space cannon offense of the aftermath prelude (LRR 77.1; the string `combat::space_cannon_offense` and the loop of
//! `AftermathWindow::new` that follows it), as one [`Window`]: the guns of [`unit_guns`] and [`attachment_guns`] roll their
//! [`cannon_plan`] from the dice stream (`Sx::die`, recorded in `ledger.dice`), each gunner that scored hits gets its reroll
//! window (the `UNIT_ABILITY_ROLLED` event; the gunner's Jol-Nar commander is asked about each die first, see [`Facts::jolnar`]; a Heart
//! of Ixth is excluded by the adapter), then per gunner `SPACE_CANNON_HITS`, Graviton Laser System binding the loss to non-fighters, and the hits
//! absorbed by the victim ([`Absorb`]: cancellations, SUSTAIN DAMAGE, casualties). Last, the active seat having lost its last
//! non-fighter ship to the guns of the others scores `SpaceCannonTookTheLastNonFighters` for them, and the occurrence is handed
//! to the caller (the string `pending_event_scoring`).
//!
//! The order of gunners is the seat order: the string iterates a map keyed by the player id, so the adapter excludes the tables
//! whose seats are not in id order. Not covered (the adapter returns `None`, the string result stands): a Heart of Ixth holder (reroll
//! questions), a Scramble Frequency or other reaction that changes the dice, neutral seats,
//! irreproducible dice, a Last Bastion, Crimson, Ral Nel or Firmament seat.

use super::cannon_guns::{attachment_guns, opponent_with_ships, unit_guns};
use crate::kernel::commander_walk::Walk;
use super::{cannon_plan, ships_of, Rd};
use crate::choice::{Choice, Pick};
use crate::generated::{Planet, System, Technology, Unit};
use crate::ids::Seat;
use crate::kernel::children::HitTexts;
use crate::kernel::fx;
use crate::kernel::stage::{Child, Illegal, Ran, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::{board as qboard, reach};
use crate::state::{Code, CoreState, EventKind, Feat, Key, Payload, RerollEntry, RerollSet, RngDomain, RollPurpose, RollRec, Slice, Val};

use super::effect_hits::Absorb;

const DIE_SIDES: u32 = 10;

/// The suspension points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Nothing has happened yet: the guns roll.
    Rolling,
    /// The next gunner's `SPACE_CANNON_HITS` and Graviton; then its hits are absorbed.
    Hitting,
    /// The victim of the gunner at `at` absorbs the hits (the child window; its questions are ours).
    Absorbing,
    /// The reroll window of each gunner with hits: its Jol-Nar commander's walk, then the event.
    Rerolling,
    /// The feat, then the end.
    Scoring,
    Done,
}

/// The facts of one offense.
#[derive(Debug, Clone)]
pub struct Facts {
    /// The active seat (activates `system`).
    pub active: Seat,
    pub system: System,
    /// The resolver carries the timing machinery (`ctx.emit` announces).
    pub announce: bool,
    /// `supply::staging_enabled`.
    pub staging: bool,
    /// The unit forms whose card lets them fire into an adjacent system (the adapter reads the card text).
    pub reaching: Vec<Unit>,
    /// The map slice is lent (a galaxy was given): the guns that reach in from adjacent systems can be found. Without it the
    /// map is not read at all.
    pub mapped: bool,
    /// Interned texts: the roll kind `space_cannon`, the loss cause `unit_ability:space_cannon`, the casualty subtype
    /// `assign_casualty`, the commander source `letnevcommander`.
    pub kind: Code,
    pub cause: Code,
    pub casualty: Code,
    pub commander: Code,
    /// The seats whose Jol-Nar commander is in play: such a gunner is asked about each die of its roll before the event (W16).
    pub jolnar: Vec<Seat>,
    /// The text of each unit form that may fire: the state code the staged entry carries and the code of the question renderer's table (the
    /// label of the commander's questions). A form without one is staged under `kind`.
    pub unit_texts: Vec<(Unit, Code, Code)>,
    /// The same for an attachment's cannon (`planet cannon {planet}`), by the planet that carries it.
    pub planet_texts: Vec<(Planet, Code, Code)>,
}

/// One staged roll of a gunner: the unit behind it (or the planet whose attachment fired), the hit number and the faces.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    unit: Option<Unit>,
    planet: Option<Planet>,
    hits_on: u32,
    faces: Vec<u32>,
}

impl Entry {
    fn hits(&self) -> u32 {
        u32::try_from(self.faces.iter().filter(|f| **f >= self.hits_on).count()).unwrap_or(u32::MAX)
    }
}

#[derive(Debug, Clone)]
pub struct SpaceCannonOffense {
    pub f: Facts,
    /// The rolls of the window in order: (hit number, faces), for the dual to compare with the string dice. A redraw a card made
    /// (Scramble Frequency) is one of them, as in the string's dice history; [`Self::first_draws`] leaves them out (the string's rolls of
    /// the cannon's own reason).
    pub drawn: Vec<(u32, Vec<u32>)>,
    redraws: Vec<usize>,
    /// The rolls of each gunner, in the order the string stages them (its units' rolls, then the attachments'): what the event shows the
    /// reactions and what they leave after drawing dice again.
    sets: Vec<(Seat, Vec<Entry>)>,
    /// The feat occurrence the window opened (the active seat lost its last non-fighter ship to the guns): the caller's
    /// event-scoring pause.
    pub scoring: Option<u64>,
    stage: Stage,
    /// The gunners that scored hits, in order, with the hits (after the reroll windows).
    gunners: Vec<(Seat, u32)>,
    at: usize,
    /// Non-fighter ships the active seat had before the guns fired.
    before: usize,
    /// The loss of the gunner at `at` is bound to non-fighter ships (Graviton Laser System).
    bound: bool,
    absorb: Child<Absorb>,
    /// The gunner whose reroll window is open (index into `gunners`), the commander's walk over its dice and the ledger length when it began.
    rerolling: usize,
    walk: Option<Walk>,
    mark: usize,
}

impl SpaceCannonOffense {
    #[must_use]
    pub fn new(f: Facts) -> Self {
        Self { f, drawn: Vec::new(), redraws: Vec::new(), sets: Vec::new(), scoring: None, stage: Stage::Rolling, gunners: Vec::new(), at: 0, before: 0, bound: false, absorb: Child::idle(), rerolling: 0, walk: None, mark: 0 } // mutants:skip: `at`, `before` and `bound` are set before they are read
    }

    /// The gunner whose hits are being absorbed and whether their loss is bound to non-fighters (the facts of the casualty
    /// question's hit detail).
    #[must_use]
    pub fn absorbing(&self) -> Option<(Seat, bool)> {
        (self.stage == Stage::Absorbing).then(|| (self.gunners[self.at].0, self.bound))
    }

    fn rd<'a>(s: &'a CoreState, k: &'a Kernel) -> Rd<'a> {
        Rd::new(s, k).with_blanked(reach::blanked(s))
    }

    /// `non_fighter_ships_of`: the seat's ships in the space area other than fighters.
    fn non_fighters(s: &CoreState, k: &Kernel, owner: Seat, system: System) -> usize {
        let rd = Self::rd(s, k);
        s.board().sys(system).units.iter().filter(|u| u.owner() == owner && rd.is_ship(u.kind()) && !rd.is_fighter(u.kind())).count()
    }

    /// `space_cannon_offense`: the rolls, and the hits each seat scored (in seat order; seats with dice but no hit are
    /// dropped, as the string drops them after staging their dice).
    fn roll(&mut self, sx: &mut Sx<'_>) {
        let (active, system) = (self.f.active, self.f.system);
        let adjacency = if self.f.mapped { qboard::adjacency(sx.state) } else { None };
        let rd = Self::rd(sx.state, sx.kernel);
        let guns = unit_guns(rd, system, active, adjacency.as_ref(), &self.f.reaching);
        let opponent = opponent_with_ships(rd, active, system);
        let list: Vec<(Seat, Unit)> = guns.iter().map(|p| (p.owner(), p.kind())).collect();
        let plan = cannon_plan(rd, system, active, opponent, &list);
        let attachments = attachment_guns(rd, system, active);
        let mut hits: Vec<(Seat, u32)> = Vec::new();
        let mut sets: Vec<(Seat, Vec<Entry>)> = Vec::new();
        let mut note = |owner: Seat, unit: Option<Unit>, planet: Option<Planet>, on: u32, faces: &[u32]| {
            let got = u32::try_from(faces.iter().filter(|f| **f >= on).count()).unwrap_or(u32::MAX);
            match hits.iter_mut().find(|(s, _)| *s == owner) {
                Some((_, n)) => *n += got,
                None => hits.push((owner, got)),
            }
            let entry = Entry { unit, planet, hits_on: on, faces: faces.to_vec() };
            match sets.iter_mut().find(|(s, _)| *s == owner) {
                Some((_, list)) => list.push(entry),
                None => sets.push((owner, vec![entry])),
            }
        };
        let mut drawn = Vec::new();
        for roll in plan {
            let on = u32::try_from(roll.hits_on).unwrap_or(u32::MAX);
            let faces: Vec<u32> = sx.dice(RngDomain::Dice, usize::try_from(roll.dice).unwrap_or(0), DIE_SIDES);
            Self::record(sx, roll.owner, Some(roll.unit), None, system, on, &faces);
            note(roll.owner, Some(roll.unit), None, on, &faces);
            drawn.push((on, faces));
        }
        for (owner, planet, on, count) in attachments {
            let faces: Vec<u32> = sx.dice(RngDomain::Dice, usize::try_from(count).unwrap_or(0), DIE_SIDES);
            Self::record(sx, owner, None, Some(planet), system, on, &faces);
            note(owner, None, Some(planet), on, &faces);
            drawn.push((on, faces));
        }
        self.drawn = drawn;
        self.sets = sets;
        hits.sort_by_key(|(s, _)| s.0);
        self.gunners = hits.into_iter().filter(|(_, n)| *n > 0).collect();
    }

    // mutants:skip: the ledger of dice has no counterpart in the string `GameState` (D2), so the lockstep does not read it
    fn record(sx: &mut Sx<'_>, owner: Seat, unit: Option<Unit>, planet: Option<Planet>, system: System, on: u32, faces: &[u32]) {
        sx.state.ledger_mut().dice.push(RollRec {
            purpose: RollPurpose::SpaceCannon,
            owner: Some(owner),
            unit,
            planet,
            scene: Some(system),
            hits_on: Some(u8::try_from(on).unwrap_or(u8::MAX)),
            faces: faces.iter().map(|f| u8::try_from(*f).unwrap_or(u8::MAX)).collect(),
            rerolled: 0,
        });
    }

    /// The rolls of the window that are its own, without the redraws a card made.
    #[must_use]
    pub fn first_draws(&self) -> Vec<(u32, Vec<u32>)> {
        self.drawn.iter().enumerate().filter(|(i, _)| !self.redraws.contains(i)).map(|(_, d)| d.clone()).collect()
    }

    /// The gunner's rolls as the set the reactions see (`transient.combat.reroll_staging`, staged for the event).
    fn mirror(&self, sx: &mut Sx<'_>, gunner: Seat) {
        let Some((_, entries)) = self.sets.iter().find(|(s, _)| *s == gunner) else { return };
        let rolls: Vec<RerollEntry> = entries
            .iter()
            .map(|e| RerollEntry {
                unit: self.staged_code(e),
                planet: None,
                hits_on: Some(u8::try_from(e.hits_on).unwrap_or(u8::MAX)),
                faces: e.faces.iter().map(|f| u8::try_from(*f).unwrap_or(u8::MAX)).collect(),
                rerolled: 0, // mutants:skip: display only
                deltas: crate::containers::SortedMap::default(),
                unit_types: self.unit_types(e),
                kind: e.unit,
            })
            .collect();
        let set = RerollSet { kind: self.f.kind, system: self.f.system, rolls };
        let list = sx.state.transient_mut().combat.reroll_staging.edit();
        list.retain(|(s, _)| *s != gunner);
        let at = list.partition_point(|(s, _)| *s < gunner);
        list.insert(at, (gunner, set));
    }

    /// After the event: the faces the staged set shows are the gunner's roll now, and the redraws the cards recorded in the ledger since
    /// `mark` are the window's own, in `drawn`.
    fn read_back(&mut self, sx: &mut Sx<'_>, gunner: Seat, mark: usize) {
        let staged = sx.state.transient().combat.reroll_staging.iter().find(|(s, _)| *s == gunner).map(|(_, set)| set.clone());
        if let (Some(set), Some((_, entries))) = (staged, self.sets.iter_mut().find(|(s, _)| *s == gunner)) {
            for (entry, now) in entries.iter_mut().zip(set.rolls.iter()) {
                entry.faces = now.faces.iter().map(|f| u32::from(*f)).collect();
            }
        }
        if sx.state.loaded.has(Slice::Ledger) {
            let redraws: Vec<(u32, Vec<u32>)> = sx
                .state
                .ledger()
                .dice
                .iter()
                .skip(mark)
                .filter(|r| r.purpose == RollPurpose::Reroll)
                .map(|r| (u32::from(r.hits_on.unwrap_or(0)), r.faces.iter().map(|f| u32::from(*f)).collect())) // mutants:skip: a redraw row always has its number
                .collect();
            for redraw in redraws {
                self.drawn.push(redraw);
                self.redraws.push(self.drawn.len() - 1);
            }
        }
    }

    /// The unit text a staged entry carries (the string stages `type_id` of the gun, `planet cannon {planet}` of an attachment); `kind`
    /// where the adapter handed no text for the form.
    /// The unit type behind a unit's entry, as the string stages it (`unit_types`: the type's text once); an attachment's cannon names no unit.
    /// (A live position carries no unit texts: the entry's own code, the roll kind's, stands for the type.)
    fn unit_types(&self, e: &Entry) -> crate::containers::SortedMap<Code, u32, 4> {
        let mut types = crate::containers::SortedMap::default();
        if e.unit.is_some() {
            types.insert(self.staged_code(e), 1);
        }
        types
    }

    fn staged_code(&self, e: &Entry) -> Code {
        match (e.unit, e.planet) {
            (Some(u), _) => self.f.unit_texts.iter().find(|(k, _, _)| *k == u).map_or(self.f.kind, |(_, c, _)| *c),
            (None, Some(p)) => self.f.planet_texts.iter().find(|(k, _, _)| *k == p).map_or(self.f.kind, |(_, c, _)| *c),
            (None, None) => self.f.kind,
        }
    }

    /// The label of an entry's questions in the question renderer's table.
    fn shown_code(&self, e: &Entry) -> Code {
        match (e.unit, e.planet) {
            (Some(u), _) => self.f.unit_texts.iter().find(|(k, _, _)| *k == u).map_or(self.f.kind, |(_, _, c)| *c),
            (None, Some(p)) => self.f.planet_texts.iter().find(|(k, _, _)| *k == p).map_or(self.f.kind, |(_, _, c)| *c),
            (None, None) => self.f.kind,
        }
    }

    fn hits_of(&self, gunner: Seat) -> u32 {
        self.sets.iter().find(|(s, _)| *s == gunner).map_or(0, |(_, entries)| entries.iter().map(Entry::hits).sum()) // mutants:skip: a gunner of the list has a set
    }

    fn ledger_len(sx: &Sx<'_>) -> usize {
        if sx.state.loaded.has(Slice::Ledger) { sx.state.ledger().dice.len() } else { 0 } // mutants:skip: a window lent no ledger reads no rows
    }

    /// The event of a gunner's roll (`open_reroll_windows`' `UNIT_ABILITY_ROLLED`, nothing to ask here): the gunner's dice are staged and
    /// a card played in its windows may draw them again (Scramble Frequency); the faces that remain are read back.
    fn announce_roll(&mut self, sx: &mut Sx<'_>, gunner: Seat) {
        if !self.f.announce {
            return;
        }
        self.mirror(sx, gunner);
        let payload = Payload::new()
            .with(Key::Kind, Val::Code(self.f.kind))
            .with(Key::Player, Val::Seat(gunner))
            .with(Key::System, Val::System(self.f.system))
            .with(Key::Hits, Val::Int(i64::from(self.hits_of(gunner))));
        let mark = Self::ledger_len(sx);
        let _ = sx.emit(fx::event(EventKind::UnitAbilityRolled, payload));
        self.read_back(sx, gunner, mark);
        sx.state.transient_mut().combat.reroll_staging.edit().retain(|(s, _)| *s != gunner);
    }

    /// The reroll window of each gunner with hits (`open_reroll_windows`), one at a time: the gunner's Jol-Nar commander is asked about
    /// each die and the picked dice are drawn again, then the event goes out. Returns the commander's question while one is open. Last,
    /// the hits of every gunner are read from the faces that remain (the string takes them from the staged sets), a gunner left with
    /// none keeps its place.
    fn reroll_windows(&mut self, sx: &mut Sx<'_>) -> Option<Next> {
        loop {
            if let Some(walk) = self.walk.as_mut() {
                if !walk.done() {
                    return Some(match walk.ask(sx.state) {
                        Some(choice) => Next::Continue(choice.ask),
                        None => Next::Done,
                    });
                }
                walk.redraw(sx);
                self.walk = None;
                let (gunner, _) = self.gunners[self.rerolling];
                self.read_back(sx, gunner, self.mark);
                sx.state.transient_mut().combat.reroll_staging.edit().retain(|(s, _)| *s != gunner);
                self.announce_roll(sx, gunner);
                self.rerolling += 1;
                continue;
            }
            let Some(&(gunner, _)) = self.gunners.get(self.rerolling) else { break };
            sx.state.transient_mut().combat.last_reroll_player = Some(gunner);
            if self.f.jolnar.contains(&gunner) {
                self.mirror(sx, gunner);
                let shown: Vec<Code> = self.sets.iter().find(|(s, _)| *s == gunner).map(|(_, entries)| entries.iter().map(|e| self.shown_code(e)).collect()).unwrap_or_default();
                self.mark = Self::ledger_len(sx);
                self.walk = Walk::start(sx.state, gunner, &shown);
                if self.walk.is_none() {
                    sx.state.transient_mut().combat.reroll_staging.edit().retain(|(s, _)| *s != gunner); // mutants:skip: a gunner of the list rolled a die, so the walk exists
                }
                if self.walk.is_some() {
                    continue;
                }
            }
            self.announce_roll(sx, gunner);
            self.rerolling += 1;
        }
        sx.state.transient_mut().combat.last_reroll_player = None;
        for (gunner, hits) in &mut self.gunners {
            if let Some((_, entries)) = self.sets.iter().find(|(s, _)| s == gunner) {
                *hits = entries.iter().map(Entry::hits).sum();
            }
        }
        None
    }

    /// `use_graviton`: the gunner exhausts the card when the loss can be bound to non-fighters and it changes something (the string
    /// also asks for hits to assign; a gunner here has scored at least one).
    fn graviton(sx: &mut Sx<'_>, gunner: Seat, victim: Seat, system: System) -> bool {
        let rd = Self::rd(sx.state, sx.kernel);
        let ships = ships_of(rd, victim, system);
        let non_fighters = ships.iter().filter(|p| !rd.is_fighter(p.kind())).count();
        let ready = sx.state.players().by_seat(gunner).is_some_and(|p| p.tech.owned.contains(Technology::GravitonLaserSystem) && !p.tech.exhausted.contains(Technology::GravitonLaserSystem)); // mutants:skip: `exhaust_technology` below checks ownership again
        if !ready || non_fighters == 0 || non_fighters == ships.len() {
            return false;
        }
        crate::kernel::fx_strategy::exhaust_technology(sx.state, gunner, Technology::GravitonLaserSystem)
    }

    /// The seat the gunner at `at` fires at: the active seat, or (the active seat's own guns) the first opponent with ships.
    fn victim(&self, s: &CoreState, k: &Kernel, gunner: Seat) -> Option<Seat> {
        if gunner == self.f.active { opponent_with_ships(Self::rd(s, k), self.f.active, self.f.system) } else { Some(self.f.active) }
    }

    fn hit(&mut self, sx: &mut Sx<'_>) -> bool {
        let (gunner, hits) = self.gunners[self.at];
        let system = self.f.system;
        let Some(victim) = self.victim(sx.state, sx.kernel, gunner) else { return false }; // mutants:skip: the active seat's guns fire only at a position with an opponent's ship, and nothing before this removes it
        if self.f.announce {
            let payload = Payload::new()
                .with(Key::System, Val::System(system))
                .with(Key::Player, Val::Seat(victim))
                .with(Key::Gunner, Val::Seat(gunner))
                .with(Key::Hits, Val::Int(i64::from(hits)));
            let _ = sx.emit(fx::event(EventKind::SpaceCannonHits, payload));
        }
        let bound = Self::graviton(sx, gunner, victim, system);
        self.bound = bound;
        let left = {
            let rd = Self::rd(sx.state, sx.kernel);
            ships_of(rd, victim, system).iter().any(|p| !rd.is_fighter(p.kind()))
        };
        let texts = HitTexts { staging: self.f.staging, commander: self.f.commander, casualty: self.f.casualty, effect_hit: self.f.cause };
        self.absorb.start(Absorb::unit_ability(victim, gunner, system, u16::try_from(hits).unwrap_or(u16::MAX), texts, self.f.cause, bound, left, self.f.announce));
        true
    }

    /// The feat: the active seat had non-fighter ships before the guns and has none now.
    fn feat(&mut self, sx: &mut Sx<'_>) {
        let (active, system) = (self.f.active, self.f.system);
        if self.before == 0 || Self::non_fighters(sx.state, sx.kernel, active, system) != 0 {
            return;
        }
        let meta = sx.state.meta_mut();
        meta.feat_occurrence_seq += 1;
        let occurrence = meta.feat_occurrence_seq;
        for (gunner, _) in self.gunners.clone() {
            if gunner == active {
                continue;
            }
            if let Some(p) = sx.state.players_mut().by_seat_mut(gunner) {
                let entry = (Feat::SpaceCannonTookTheLastNonFighters, occurrence);
                if !p.misc.event_feats.as_slice().contains(&entry) {
                    p.misc.event_feats.push(entry);
                    p.misc.event_feats.as_mut_slice().sort_unstable();
                }
            }
        }
        self.scoring = Some(occurrence);
    }
}

impl Window for SpaceCannonOffense {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                Stage::Rolling => {
                    self.roll(sx);
                    self.rerolling = 0;
                    self.stage = Stage::Rerolling;
                }
                Stage::Rerolling => {
                    if let Some(next) = self.reroll_windows(sx) {
                        return next;
                    }
                    // Turn Their Fleets to Dust names this step: the count is taken across the absorption.
                    self.before = Self::non_fighters(sx.state, sx.kernel, self.f.active, self.f.system);
                    self.at = 0;
                    self.stage = Stage::Hitting;
                }
                Stage::Hitting => {
                    if self.at >= self.gunners.len() {
                        self.stage = Stage::Scoring;
                    } else if self.hit(sx) {
                        self.stage = Stage::Absorbing;
                    } else {
                        self.at += 1; // mutants:skip: only reached for a gunner with no victim, which `hit` never reports (see there)
                    }
                }
                Stage::Absorbing => match self.absorb.step(sx) {
                    Some(Ran::Asks(ask)) => return Next::Continue(ask),
                    Some(Ran::Pass(next)) => return next,
                    Some(Ran::Done(_)) | None => {
                        self.at += 1;
                        self.stage = Stage::Hitting;
                    }
                },
                Stage::Scoring => {
                    self.feat(sx);
                    self.stage = Stage::Done;
                }
                Stage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        match self.stage {
            Stage::Absorbing => self.absorb.ask_of(s, k),
            Stage::Rerolling => self.walk.as_ref()?.ask(s),
            _ => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match self.stage {
            Stage::Absorbing => self.absorb.answer(sx, pick),
            Stage::Rerolling => self.walk.as_mut().ok_or(Illegal)?.answer(sx.state, pick),
            _ => Err(Illegal),
        }
    }
}
