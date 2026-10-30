//! The space cannon offense of the aftermath prelude (LRR 77.1; the string `combat::space_cannon_offense` and the loop of
//! `AftermathWindow::new` that follows it), as one [`Window`]: the guns of [`unit_guns`] and [`attachment_guns`] roll their
//! [`cannon_plan`] from the dice stream (`Sx::die`, recorded in `ledger.dice`), each gunner that scored hits gets its reroll
//! window (the `UNIT_ABILITY_ROLLED` event; nothing to ask unless a Jol-Nar commander or the Heart of Ixth is in play, which the
//! adapter excludes), then per gunner `SPACE_CANNON_HITS`, Graviton Laser System binding the loss to non-fighters, and the hits
//! absorbed by the victim ([`Absorb`]: cancellations, SUSTAIN DAMAGE, casualties). Last, the active seat having lost its last
//! non-fighter ship to the guns of the others scores `SpaceCannonTookTheLastNonFighters` for them, and the occurrence is handed
//! to the caller (the string `pending_event_scoring`).
//!
//! The order of gunners is the seat order: the string iterates a map keyed by the player id, so the adapter excludes the tables
//! whose seats are not in id order. Not covered (the adapter returns `None`, the string result stands): a Jol-Nar commander or a
//! Heart of Ixth holder (reroll questions), a Scramble Frequency or other reaction that changes the dice, neutral seats,
//! irreproducible dice, a Last Bastion, Crimson, Ral Nel or Firmament seat.

use super::cannon_guns::{attachment_guns, opponent_with_ships, unit_guns};
use super::{cannon_plan, ships_of, Rd};
use crate::choice::{Choice, Pick};
use crate::generated::{Planet, System, Technology, Unit};
use crate::ids::Seat;
use crate::kernel::children::HitTexts;
use crate::kernel::fx;
use crate::kernel::stage::{Child, Illegal, Ran, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::{board as qboard, reach};
use crate::state::{Code, CoreState, EventKind, Feat, Key, Payload, RngDomain, RollPurpose, RollRec, Val};

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
}

#[derive(Debug, Clone)]
pub struct SpaceCannonOffense {
    pub f: Facts,
    /// The rolls of the window in order: (hit number, faces), for the dual to compare with the string dice.
    pub drawn: Vec<(u32, Vec<u32>)>,
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
}

impl SpaceCannonOffense {
    #[must_use]
    pub fn new(f: Facts) -> Self {
        Self { f, drawn: Vec::new(), scoring: None, stage: Stage::Rolling, gunners: Vec::new(), at: 0, before: 0, bound: false, absorb: Child::idle() }
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
        let mut note = |owner: Seat, on: u32, faces: &[u32]| {
            let got = u32::try_from(faces.iter().filter(|f| **f >= on).count()).unwrap_or(u32::MAX);
            match hits.iter_mut().find(|(s, _)| *s == owner) {
                Some((_, n)) => *n += got,
                None => hits.push((owner, got)),
            }
        };
        let mut drawn = Vec::new();
        for roll in plan {
            let on = u32::try_from(roll.hits_on).unwrap_or(u32::MAX);
            let faces: Vec<u32> = (0..roll.dice).map(|_| sx.die(RngDomain::Dice, DIE_SIDES)).collect();
            Self::record(sx, roll.owner, Some(roll.unit), None, system, on, &faces);
            note(roll.owner, on, &faces);
            drawn.push((on, faces));
        }
        for (owner, planet, on, count) in attachments {
            let faces: Vec<u32> = (0..count).map(|_| sx.die(RngDomain::Dice, DIE_SIDES)).collect();
            Self::record(sx, owner, None, Some(planet), system, on, &faces);
            note(owner, on, &faces);
            drawn.push((on, faces));
        }
        self.drawn = drawn;
        hits.sort_by_key(|(s, _)| s.0);
        self.gunners = hits.into_iter().filter(|(_, n)| *n > 0).collect();
    }

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

    /// The reroll window of each gunner with hits (`open_reroll_windows`: nothing to ask here, the event other seats react
    /// to).
    fn reroll_windows(&mut self, sx: &mut Sx<'_>) {
        for (gunner, hits) in self.gunners.clone() {
            sx.state.transient_mut().combat.last_reroll_player = Some(gunner);
            if self.f.announce {
                let payload = Payload::new()
                    .with(Key::Kind, Val::Code(self.f.kind))
                    .with(Key::Player, Val::Seat(gunner))
                    .with(Key::System, Val::System(self.f.system))
                    .with(Key::Hits, Val::Int(i64::from(hits)));
                let _ = sx.emit(fx::event(EventKind::UnitAbilityRolled, payload));
            }
        }
        sx.state.transient_mut().combat.last_reroll_player = None;
    }

    /// `use_graviton`: the gunner exhausts the card when the loss can be bound to non-fighters and it changes something.
    fn graviton(sx: &mut Sx<'_>, gunner: Seat, victim: Seat, system: System, hits: u32) -> bool {
        let rd = Self::rd(sx.state, sx.kernel);
        let ships = ships_of(rd, victim, system);
        let non_fighters = ships.iter().filter(|p| !rd.is_fighter(p.kind())).count();
        let ready = sx.state.players().by_seat(gunner).is_some_and(|p| p.tech.owned.contains(Technology::GravitonLaserSystem) && !p.tech.exhausted.contains(Technology::GravitonLaserSystem));
        if !ready || hits == 0 || non_fighters == 0 || non_fighters == ships.len() {
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
        let Some(victim) = self.victim(sx.state, sx.kernel, gunner) else { return false };
        if self.f.announce {
            let payload = Payload::new()
                .with(Key::System, Val::System(system))
                .with(Key::Player, Val::Seat(victim))
                .with(Key::Gunner, Val::Seat(gunner))
                .with(Key::Hits, Val::Int(i64::from(hits)));
            let _ = sx.emit(fx::event(EventKind::SpaceCannonHits, payload));
        }
        let bound = Self::graviton(sx, gunner, victim, system, hits);
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
                    self.reroll_windows(sx);
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
                        self.at += 1;
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
        if self.stage == Stage::Absorbing { self.absorb.ask_of(s, k) } else { None }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage == Stage::Absorbing { self.absorb.answer(sx, pick) } else { Err(Illegal) }
    }
}
