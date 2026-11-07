//! The dice of a ground combat round (LRR 42.1, 42.2; the two `roll_ground` calls of the string
//! `roll_ground_step`), as a [`Window`] that never asks.
//!
//! Attacker first, then defender: [`super::ground_pools`] groups the side's ground forces by combat value; each
//! pool with dice rolls them from `Sx::die` (the dice stream), recorded in `ledger.dice`; the side's reroll staging
//! (`reroll_staging`, kind `ground`, one entry per pool named `combat value N`) and `last_reroll_player` are set
//! for the reroll windows that follow; the hits are the dice at or above the value.
//!
//! X-89 Bacterial Weapon (`Facts::x89`, package W10): "double the hits produced by your units' BOMBARDMENT and ground combat rolls" -- the
//! hits of that side are twice the dice at or above the value (the staged dice stay raw faces).
//!
//! Not covered (the adapter returns `None`, the string result stands): a
//! combat value outside `1..=NAMED`, test faces or a pending stream restore, nested activity.

use super::{ground_pools, Rd};
use crate::choice::{Choice, Pick};
use crate::containers::SortedMap;
use crate::generated::{Planet, System, Unit};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::reach;
use crate::state::{Code, CoreState, RerollEntry, RerollSet, RngDomain, RollPurpose, RollRec};

const DIE_SIDES: u32 = 10;

/// The pool names the adapter interns (`combat value 1` ..= `combat value NAMED`).
pub const NAMED: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    Done,
}

/// The facts of one round's roll.
#[derive(Debug, Clone)]
pub struct Facts {
    pub attacker: Seat,
    pub defender: Seat,
    pub system: System,
    pub planet: Planet,
    /// Interned state texts: the staging kind `ground`, `combat value N` for N in `1..=NAMED` (index N - 1).
    pub kind: Code,
    pub names: Vec<Code>,
    /// The interned unit-type text of every unit type that may roll (`RerollEntry.unit_types`).
    pub unit_codes: Vec<(Unit, Code)>,
    /// Per side (attacker, defender): X-89 Bacterial Weapon doubles its hits.
    pub x89: [bool; 2],
}

#[derive(Debug, Clone)]
pub struct GroundRoll {
    pub f: Facts,
    /// The hits of the attacker and the defender (X-89 doubled).
    pub hits: (u32, u32),
    /// The rolls in order: (hit number, faces), for the dual to compare with the string dice.
    pub drawn: Vec<(u32, Vec<u32>)>,
    /// A pool the window cannot name (the adapter drops the comparison).
    pub unsupported: bool,
    stage: Stage,
}

impl GroundRoll {
    #[must_use]
    pub fn new(f: Facts) -> Self {
        Self { f, hits: (0, 0), drawn: Vec::new(), unsupported: false, stage: Stage::Start }
    }

    fn roll_side(&mut self, sx: &mut Sx<'_>, side: Seat) -> u32 {
        let (system, planet) = (self.f.system, self.f.planet);
        let pools = ground_pools(Rd::new(sx.state, sx.kernel).with_blanked(reach::blanked(sx.state)), side, system, planet);
        let mut hits = 0_u32;
        let mut rolls: Vec<RerollEntry> = Vec::new();
        for pool in pools {
            let count = usize::try_from(pool.dice).unwrap_or(0);
            if count == 0 {
                continue;
            }
            let on = u32::try_from(pool.value).unwrap_or(u32::MAX);
            let faces: Vec<u32> = sx.dice_on(RngDomain::Dice, usize::try_from(count).unwrap_or(0), DIE_SIDES, on);
            sx.state.ledger_mut().dice.push(RollRec {
                purpose: RollPurpose::GroundCombat,
                owner: Some(side),
                unit: None,
                planet: Some(planet),
                scene: Some(system),
                hits_on: Some(u8::try_from(on).unwrap_or(u8::MAX)),
                faces: faces.iter().map(|f| u8::try_from(*f).unwrap_or(u8::MAX)).collect(),
                rerolled: 0,
            });
            hits += u32::try_from(faces.iter().filter(|f| **f >= on).count()).unwrap_or(u32::MAX);
            let name = usize::try_from(pool.value).ok().filter(|v| (1..=NAMED).contains(v)).and_then(|v| self.f.names.get(v - 1).copied());
            let mut unit_types = SortedMap::<Code, u32, 4>::default();
            for (unit, n) in &pool.types {
                match self.f.unit_codes.iter().find(|(u, _)| u == unit) {
                    Some((_, code)) => *unit_types.get_or_insert_with(*code, || 0) += n,
                    None => self.unsupported = true,
                }
            }
            let Some(unit) = name else {
                self.unsupported = true;
                continue;
            };
            rolls.push(RerollEntry {
                unit,
                planet: Some(planet),
                hits_on: Some(u8::try_from(on).unwrap_or(u8::MAX)),
                faces: faces.iter().map(|f| u8::try_from(*f).unwrap_or(u8::MAX)).collect(),
                rerolled: 0,
                deltas: SortedMap::default(),
                unit_types,
                kind: None,
            });
            self.drawn.push((on, faces));
        }
        if rolls.iter().any(|r| !r.faces.is_empty()) {
            let combat = &mut sx.state.transient_mut().combat;
            let staging = combat.reroll_staging.edit();
            staging.retain(|(s, _)| *s != side);
            let at = staging.partition_point(|(s, _)| *s < side);
            staging.insert(at, (side, RerollSet { kind: self.f.kind, system, rolls }));
            combat.last_reroll_player = Some(side);
        }
        hits
    }
}

impl Window for GroundRoll {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Done {
            return Next::Done;
        }
        let a = self.roll_side(sx, self.f.attacker);
        let d = self.roll_side(sx, self.f.defender);
        self.hits = (a * if self.f.x89[0] { 2 } else { 1 }, d * if self.f.x89[1] { 2 } else { 1 });
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
