//! The "Roll Dice" step of a space combat round without a reroll question (string `CombatWindow::roll_round`,
//! LRR 78.5): the presentation, both sides' dice (the attacker first, group by group in ascending combat
//! value, drawn with `Sx::die` and recorded in `ledger.dice`), the round's hits (free and 0.0.1-forced, J.N.S.
//! Hylarim's extra hits), `combat_round_hits`, `combat_round_dice` and the assignment queue (forced hits
//! first, the attacker's hits against the defender before the defender's).
//!
//! Not covered (the adapter returns `None`, the string result stands; see `dual_window::roll_unported`): a
//! reroll question (Crown of Thalnos relic or law, Heart of Ixth, Munitions Reserves after the roll, War
//! Funding, Wrath of Kenara), a readied The Thundarian / Yssaril agent (roll-step reaction), neutral sides,
//! test faces, a pending stream restore, a roll that empties a fleet, and any hit a module produces after
//! the roll.

use super::{fleet_groups, record_rolls, ships_of, FleetRoll, Rd, DIE_SIDES};
use crate::choice::{Choice, Pick};
use crate::generated::Unit;
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::reach;
use crate::state::{Code, CombatDie, CoreState, RngDomain, RollPurpose};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    Done,
}

/// The facts of one roll step.
#[derive(Debug, Clone)]
pub struct Facts {
    pub attacker: Seat,
    pub defender: Seat,
    pub system: crate::generated::System,
    /// The presentation phase `resolving_hits`.
    pub phase: Code,
    /// The interned unit text of every roll group the sides will roll: (seat, hit number, tag, text), the text
    /// being the group's unit types joined with `+` in id order.
    pub names: Vec<(Seat, i64, u8, Code)>,
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

#[derive(Debug, Clone)]
pub struct RollStep {
    pub f: Facts,
    /// The rolls of the step in order: (hit number, faces), for the dual to compare with the string dice.
    pub drawn: Vec<(u32, Vec<u32>)>,
    pub queue: Vec<Queued>,
    /// Each side's damaged ships at the time the hits were queued (`damaged_before_round`).
    pub damaged: Vec<(Seat, Vec<Unit>)>,
    stage: Stage,
}

impl RollStep {
    #[must_use]
    pub fn new(f: Facts) -> Self {
        Self { f, drawn: Vec::new(), queue: Vec::new(), damaged: Vec::new(), stage: Stage::Start }
    }

    fn name(&self, side: Seat, value: i64, tag: u8) -> Code {
        self.f.names.iter().find(|(s, v, t, _)| *s == side && *v == value && *t == tag).map_or(self.f.phase, |n| n.3)
    }
}

/// One side's staged entry: (hit number, tag, text, faces).
type Entry = (i64, u8, Code, Vec<u32>);

impl Window for RollStep {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Done {
            return Next::Done;
        }
        let (a, d, system) = (self.f.attacker, self.f.defender, self.f.system);
        {
            let rd = Rd::new(sx.state, sx.kernel).with_blanked(reach::blanked(sx.state));
            let mut round_start = ships_of(rd, a, system);
            round_start.extend(ships_of(rd, d, system));
            let p = sx.state.transient_mut().combat.presentation.edit();
            p.phase = self.f.phase;
            p.round_start = round_start;
        }
        let round_seq = sx.state.meta().combat_round_seq;
        let mut sides: Vec<(Seat, Vec<Entry>)> = Vec::new();
        for side in [a, d] {
            let (groups, munitions) = {
                let rd = Rd::new(sx.state, sx.kernel).with_blanked(reach::blanked(sx.state));
                let munitions = sx.state.players().by_seat(side).is_some_and(|p| p.combat_timers.munitions_round.is(round_seq));
                (fleet_groups(rd, side, system), munitions)
            };
            let mut rolls: Vec<FleetRoll> = Vec::new();
            let mut entries: Vec<Entry> = Vec::new();
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
                entries.push((value, tag, self.name(side, value, tag), last));
            }
            record_rolls(sx.state.ledger_mut(), side, Some(system), &rolls, RollPurpose::SpaceCombat);
            for r in &rolls {
                self.drawn.push((r.hits_on, r.faces.clone()));
            }
            sides.push((side, entries));
        }
        // hits: (free, forced) per side
        let mut totals: Vec<(Seat, u32, u32)> = Vec::new();
        let mut dice: Vec<CombatDie> = Vec::new();
        for (side, entries) in &sides {
            let (mut free, mut forced) = (0_u32, 0_u32);
            for (value, tag, name, faces) in entries {
                let on = u32::try_from(*value).unwrap_or(u32::MAX);
                let mut hits = u32::try_from(faces.iter().filter(|f| **f >= on).count()).unwrap_or(0);
                if *tag == 1 {
                    hits += 2 * u32::try_from(faces.iter().filter(|f| **f >= 9).count()).unwrap_or(0);
                }
                if *tag == 2 {
                    forced += hits;
                } else {
                    free += hits;
                }
                for face in faces {
                    dice.push(CombatDie { player: *side, unit: *name, roll: *face, target: on, hit: on > 0 && *face >= on });
                }
            }
            totals.push((*side, free, forced));
        }
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
