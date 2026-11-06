//! Space cannon defense (LRR 49.2, 77.2 after the ground forces are committed): the string `finish_committing`'s
//! announcement and `space_cannon_defense`, as one [`Window`] that never asks.
//!
//! `GROUND_COMMITMENT_FINISHED` (system, player, the planets committed to), then planet by planet: the guns of
//! the other players standing there roll at the invader's ground forces. The plan is [`defense_plan`]: the printed
//! SPACE CANNON of every unit that has one (a PDS another player disabled this invasion does not fire), Plasma
//! Scoring's extra die for the owner's easiest gun, a galvanized unit's extra die, Assail lowering the number.
//! The dice come from `Sx::die` (the dice stream) and are recorded in `ledger.dice`; the hits go through the hits
//! step ([`super::hits`]: window, apply, announce); the `InvasionStep` of the planet is recorded in
//! `active_invasion.last_step` when any die was rolled.
//!
//! No gun fires in an entropic scar (an ability cannot be used from or against one, rules 2 and 4), and none fires at the units of a
//! seat that owns L4 Disruptors ("During an invasion, units cannot use SPACE CANNON against your units"). An attachment that gives its
//! planet SPACE CANNON (Titans' Geoform) rolls for the planet's controller "as if it were a unit", after the guns: Disable, Plasma Scoring
//! and the other unit modifiers do not apply to those dice, and they are not part of the step's dice (only their hits count).
//!
//! Not covered (the adapter returns `None`, the string result stands): a Research Team or Viper EX-23 answer, forced combats, nested activity.

use super::hits::{self as step, Batch};
use super::Rd;
use crate::containers::{SeatSet, SmallList};
use crate::choice::{Choice, Pick};
use crate::generated::{Planet, System, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::hooks::{CombatCtx, CombatUnit};
use crate::kernel::seam::{data as seam_data, Clamp, RollKind, RollSite, RowKind, Source};
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{fx, hits, Kernel, Next};
use crate::query::reach;
use crate::state::{Code, CoreState, EventKind, InvasionDie, InvasionStep, Key, List, Payload, Piece, RollPurpose, RollRec, RoundKind, RngDomain, Val};

const DIE_SIDES: u32 = 10;

/// One gun's roll as planned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GunRoll {
    pub owner: Seat,
    pub unit: Unit,
    pub hits_on: i64,
    pub dice: usize,
}

fn has_plasma_scoring(rd: Rd<'_>, owner: Seat) -> bool {
    seam_data::COMBAT_ROWS
        .iter()
        .filter(|r| matches!(r.kind, RowKind::ExtraDice(_)) && r.source == Source::Technology(crate::generated::Technology::PlasmaScoring))
        .any(|r| r.times(rd.q(), owner, SeatSet::EMPTY, None, 0) > 0)
}

/// The rolls of `guns` (in firing order) at `invader`'s ground forces on `planet`: `invasion::space_cannon_defense`'s
/// per-gun math. Guns without a SPACE CANNON or without dice are skipped.
#[must_use]
pub fn defense_plan(rd: Rd<'_>, system: System, planet: Planet, invader: Seat, guns: &[Piece]) -> Vec<GunRoll> {
    let activation = rd.s.meta().activation_seq;
    let printed = |p: &Piece| rd.data(p.kind()).and_then(|d| d.space_cannon).map(|r| i64::from(r.hits_on)).filter(|v| *v > 0);
    // Plasma Scoring: one of an owner's guns rolls a die more, the one that hits most easily.
    let mut picks: Vec<(Seat, i64)> = Vec::new();
    for g in guns {
        let Some(value) = printed(g) else { continue };
        if !has_plasma_scoring(rd, g.owner()) {
            continue;
        }
        match picks.iter_mut().find(|(s, _)| *s == g.owner()) {
            Some((_, best)) => *best = (*best).min(value),
            None => picks.push((g.owner(), value)),
        }
    }
    let mut out = Vec::new();
    for g in guns {
        let Some(data) = rd.data(g.kind()) else { continue };
        let Some(value) = printed(g) else { continue };
        if data.base_type == UnitBaseType::Pds
            && rd.s.players().iter().any(|p| p.seat != g.owner() && p.round_marks.contains(RoundKind::DisableInvasion, activation))
        {
            continue;
        }
        let dice = data.space_cannon.map_or(0, |r| usize::from(r.dice));
        if dice == 0 {
            continue;
        }
        let plasma = match picks.iter().position(|(s, v)| *s == g.owner() && *v == value) {
            Some(at) => {
                picks.remove(at);
                1
            }
            None => 0,
        };
        let mut foes = SeatSet::EMPTY;
        foes.insert(invader);
        let site = RollSite {
            unit: CombatUnit { owner: g.owner(), system: Some(system), planet: Some(planet), unit: g.kind(), ctx: CombatCtx::Space },
            kind: RollKind::CannonAssail,
            round: rd.s.meta().combat_round_seq,
            unit_index: 0,
            foes,
        };
        let assail = rd.k.hooks.seam.hit_shift.eval(rd.q(), &site, 0, Clamp::None).value;
        out.push(GunRoll { owner: g.owner(), unit: g.kind(), hits_on: (value - assail).max(1), dice: dice + plasma + usize::from(g.galvanized()) });
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    Done,
}

/// The facts of one commitment's defense.
#[derive(Debug, Clone)]
pub struct Facts {
    pub invader: Seat,
    pub system: System,
    /// The planets committed to, in the string's order.
    pub planets: Vec<Planet>,
    /// The resolver carries the timing machinery (`ctx.emit` announces).
    pub announce: bool,
    /// Interned texts: the event cause `space_cannon_defense` and the payload key `planets` (event codes); the
    /// step kind `space_cannon_defense` and each firing unit type (`InvasionDie.group`) are state texts.
    pub cause: Code,
    pub kind: Code,
    pub planets_key: Code,
    /// The interned unit-type text of every unit that may fire (`InvasionDie.group`).
    pub unit_codes: Vec<(Unit, Code)>,
}

#[derive(Debug, Clone)]
pub struct SpaceCannonDefense {
    pub f: Facts,
    /// The rolls of the window in order: (hit number, faces), for the dual to compare with the string dice.
    pub drawn: Vec<(u32, Vec<u32>)>,
    stage: Stage,
}

impl SpaceCannonDefense {
    #[must_use]
    pub fn new(f: Facts) -> Self {
        Self { f, drawn: Vec::new(), stage: Stage::Start }
    }

    fn code(&self, unit: Unit) -> Code {
        self.f.unit_codes.iter().find(|(u, _)| *u == unit).map_or(self.f.kind, |(_, c)| *c)
    }

    fn flush(&self, sx: &mut Sx<'_>) {
        if self.f.announce {
            hits::flush_staged(sx);
        } else {
            let _ = fx::drain_staged(sx.state);
        }
    }

    fn defend(&mut self, sx: &mut Sx<'_>, planet: Planet) {
        let (invader, system) = (self.f.invader, self.f.system);
        // An entropic scar silences every gun (rule 2: not from a scar; rule 4: not against one), and L4 Disruptors every gun against its owner.
        let scope = crate::tables::Scope(u32::from(sx.kernel.sources.0));
        if crate::query::tech::is_scar(scope, system) || sx.state.players().by_seat(invader).is_some_and(|p| p.tech.owned.contains(crate::generated::Technology::L4Disruptors)) {
            return;
        }
        let guns: Vec<Piece> = sx
            .state
            .board()
            .planet(planet)
            .units
            .iter()
            .filter(|u| u.owner() != invader && !sx.kernel.hooks.direct.crimson_abilities_lost(sx.state, u.owner(), system))
            .copied()
            .collect();
        let before: Vec<Piece> = sx.state.board().planet(planet).units.iter().copied().collect();
        let rd = Rd::new(sx.state, sx.kernel).with_blanked(reach::blanked(sx.state));
        let plan = defense_plan(rd, system, planet, invader, &guns);
        let mut dice: Vec<InvasionDie> = Vec::new();
        let mut total = 0_u32;
        for roll in plan {
            let on = u32::try_from(roll.hits_on).unwrap_or(u32::MAX);
            let faces: Vec<u32> = sx.dice(RngDomain::Dice, usize::try_from(roll.dice).unwrap_or(0), DIE_SIDES);
            sx.state.ledger_mut().dice.push(RollRec {
                purpose: RollPurpose::SpaceCannon,
                owner: Some(roll.owner),
                unit: Some(roll.unit),
                planet: Some(planet),
                scene: Some(system),
                hits_on: Some(u8::try_from(on).unwrap_or(u8::MAX)),
                faces: faces.iter().map(|f| u8::try_from(*f).unwrap_or(u8::MAX)).collect(),
                rerolled: 0,
            });
            total += u32::try_from(faces.iter().filter(|f| **f >= on).count()).unwrap_or(u32::MAX);
            for face in &faces {
                dice.push(InvasionDie { planet, player: roll.owner, group: self.code(roll.unit), face: *face, target: on, hit: *face >= on });
            }
            self.drawn.push((on, faces));
        }
        // The SPACE CANNON an attachment gives the planet, rolled for its controller (not against itself).
        if let Some(owner) = sx.state.board().planet(planet).control.filter(|owner| *owner != invader) {
            let attached: Vec<crate::generated::Roll> = crate::query::board::attachments(sx.state, planet).as_slice().iter().filter_map(|a| a.data().space_cannon).collect();
            for cannon in attached {
                let on = u32::from(cannon.hits_on);
                let faces: Vec<u32> = sx.dice(RngDomain::Dice, usize::from(cannon.dice), DIE_SIDES);
                sx.state.ledger_mut().dice.push(RollRec {
                    purpose: RollPurpose::SpaceCannon,
                    owner: Some(owner),
                    unit: None,
                    planet: Some(planet),
                    scene: Some(system),
                    hits_on: Some(cannon.hits_on),
                    faces: faces.iter().map(|f| u8::try_from(*f).unwrap_or(u8::MAX)).collect(),
                    rerolled: 0,
                });
                total += u32::try_from(faces.iter().filter(|f| **f >= on).count()).unwrap_or(u32::MAX);
                self.drawn.push((on, faces));
            }
        }
        let b = Batch { victim: invader, system, planet, cause: self.f.cause, unit_ability: true, announce: self.f.announce };
        let remaining = step::open(sx, &b, total);
        step::apply(sx.state, sx.kernel, &b, remaining);
        self.flush(sx);
        if dice.is_empty() {
            return;
        }
        let after: Vec<Piece> = sx.state.board().planet(planet).units.iter().copied().collect();
        let mut produced = crate::containers::SortedMap::<Seat, u32, 4>::default();
        for d in dice.iter().filter(|d| d.hit) {
            *produced.get_or_insert_with(d.player, || 0) += 1;
        }
        if let Some(active) = sx.state.transient_mut().invasion.active.as_mut() {
            active.edit().last_step = Some(InvasionStep { planet, kind: self.f.kind, round: 0, before, after, dice, hits: produced, harrow_hits: 0 });
        }
    }
}

impl Window for SpaceCannonDefense {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Done {
            return Next::Done;
        }
        if self.f.announce {
            let mut planets = SmallList::new();
            for p in &self.f.planets {
                planets.push(*p);
            }
            let mut payload = Payload::new().with(Key::System, Val::System(self.f.system)).with(Key::Player, Val::Seat(self.f.invader));
            payload.set_list(Key::Other(self.f.planets_key), List::Planets(planets));
            let _ = sx.emit(fx::event(EventKind::GroundCommitmentFinished, payload));
        }
        for planet in self.f.planets.clone() {
            self.defend(sx, planet);
        }
        self.stage = Stage::Done;
        Next::Done
    }

    fn ask_of(&self, _s: &CoreState, _k: &Kernel) -> Option<Choice> {
        None
    }

    fn answer(&mut self, _sx: &mut Sx<'_>, _pick: Pick) -> Result<(), Illegal> {
        Err(Illegal)
    }
}
