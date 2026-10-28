//! The composed space combat (package B1g): the stage machine of `CombatWindow::settle` as one core [`Window`].
//!
//! The string engine runs a combat as a loop over its `Stage` (`settle_stages`) in which every stage is a
//! window of its own. This machine owns that loop: [`Machine::step`] runs from the stage a settle starts in to
//! the next decision the players must make, or to the end of the fight, driving the windows the other files of
//! this directory port (the round opening, the anti-fighter barrage, the roll step with its reroll questions,
//! the move of a retreat, the announcement of the end) and the transitions between them ([`Flow`]).
//!
//! What the string answers from its own rules arrives as [`Env`] (interned texts, the neighbours a retreat may
//! enter, rule gates that cannot change within one settle); everything that can change during a settle is read
//! from the core state: whether the fight is over, who may retreat where, the ships left, who may sustain.
//!
//! The machine stops (the adapter returns `None`, the string result stands) at a position it does not carry:
//! [`Machine::bailed`] names the reason. They are all string-only on purpose and have a test each
//! (`combat.rs`, `the_composed_combat_*`): a Naaz or Letnev round opening, diplomacy, Assault Cannon or other
//! hits at the start of a combat, hits cancelled in a window, a single casualty candidate, a neutral side,
//! Duranium Armor, Devotion, a retreat that moves a flipping unit or a Floating Factory or a Last Bastion
//! seat's fleet, a roll that ends the fight, destructions waiting for their announcement, reactions.
//!
//! A decision that is pending when the machine stops (`Stop::Pending`) is the string's `pending_choice`; the
//! machine builds the same question in [`Machine::pending_ask`] and answers it in [`Machine::answer_pending`].

use super::barrage::{BarrageStep, Facts as BarrageFacts};
use super::begin_round::{BeginRound, Facts as BeginFacts};
use super::end::{finished, winner};
use super::end_announce::{Codes as EndCodes, EndAnnounce, Facts as EndFacts};
use super::flow::{Facts as FlowFacts, Flow, Out, Pend, Phase, Step, MAX_ROUNDS};
use super::retreat::{Facts as RetreatFacts, Retreat};
use super::roll::{Facts as RollFacts, Name, RollStep};
use super::{hit_targets, ships_of, sustain_candidates, Rd};
use crate::choice::{Choice, Pick};
use crate::generated::{System, Unit};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::{board as qboard, reach};
use crate::state::{Code, CoreState};

/// What the string side gives the machine for one settle: texts, gates, the map around the system.
#[derive(Debug, Clone)]
pub struct Env {
    pub attacker: Seat,
    pub defender: Seat,
    pub system: System,
    /// The resolver carries the timing machinery (`ctx.emit` announces).
    pub announce: bool,
    /// `supply::staging_enabled`.
    pub staging: bool,
    /// The feat occurrence of the combat (already begun by the adapter when the first round opens).
    pub occurrence: u64,
    /// The presentation phases: `pre_roll`, `barrage`, `resolving_hits`, `retreating`, `complete`.
    pub phases: [Code; 5],
    /// `round_opening_unported` (Naaz, Letnev, diplomacy, round agents): the opening of a round is the string's.
    pub opening_unported: bool,
    /// `barrage_unported`, or dice the core cannot reproduce.
    pub barrage_unported: bool,
    pub barrage_cause: Code,
    pub barrage_kind: Code,
    pub barrage_extra: Code,
    pub unit_codes: Vec<(Unit, Code)>,
    /// A roll the core cannot carry (`roll_unported`, dice it cannot reproduce).
    pub roll_unported: bool,
    pub munitions_after: [bool; 2],
    pub roll_cause: Code,
    pub hacan: Code,
    /// Every roll group's text (the unit types joined with `+` in id order): (types, state text, shown text).
    pub groups: Vec<(Vec<Unit>, Code, Code)>,
    /// The unit types present in the alphabetical order of their ids (the order a roll names its types in).
    pub alpha: Vec<Unit>,
    pub retreat_cause: Code,
    pub retreat_pool: Code,
    /// A retreat the core does not carry (flip, Voltron, Floating Factory, Last Bastion, a flagship that
    /// raises an event on arrival, Dark Energy Tap).
    pub retreat_unported: bool,
    /// Per side (attacker, defender): the neighbours of the system a fleet may retreat to as far as the movement
    /// rules go, and whether the side may retreat at all (Intercept, the Mahact hero).
    pub neighbors: [Vec<(System, bool)>; 2],
    pub may_retreat: [bool; 2],
    /// Per side: Duranium Armor is owned; the Yin (Devotion) hits-after-a-round module is in the game.
    pub duranium: [bool; 2],
    pub devotion: bool,
    pub end_codes: EndCodes,
}

/// The window fields the string `CombatWindow` keeps between the stages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fields {
    pub step: Step,
    pub phase: Phase,
    pub resuming: bool,
    pub pending_retreats: Vec<Seat>,
    pub offered: Vec<(u32, Seat)>,
    pub damaged: Vec<(Seat, Vec<Unit>)>,
    pub fled: Vec<(Seat, Vec<Unit>)>,
    pub at_start: Vec<(Seat, Vec<Unit>)>,
    /// The anti-fighter barrage scored a feat: the secret's scoring window opens before the round resumes.
    pub scoring: bool,
    /// `Stage::Done(outcome)`: the winner and the rounds fought.
    pub outcome: Option<(Option<Seat>, u32)>,
    pub end_announced: bool,
}

/// Where the machine stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// A decision of the players is pending at `fields.step`.
    Pending,
    /// The scoring window of the barrage opens (`RollingAfterBarrage`).
    Scoring,
    /// The fight is over and announced.
    Over,
    /// A position the machine does not carry.
    Bailed(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Running,
    Rolling,
    Done,
}

#[derive(Debug, Clone)]
pub struct Machine {
    pub env: Env,
    pub fields: Fields,
    pub stop: Option<Stop>,
    /// The dice of the whole settle, in order: (hit number, faces), the barrage's and every roll's.
    pub drawn: Vec<(u32, Vec<u32>)>,
    /// The seats that scored the barrage feat.
    pub feat: Vec<Seat>,
    roll: Option<RollStep>,
    roll_round: u32,
    stage: Stage,
}

impl Machine {
    #[must_use]
    pub fn new(env: Env, fields: Fields) -> Self {
        Self { env, fields, stop: None, drawn: Vec::new(), feat: Vec::new(), roll: None, roll_round: 0, stage: Stage::Running }
    }

    fn side(&self, seat: Seat) -> usize {
        usize::from(seat != self.env.attacker)
    }

    fn rd<'a>(s: &'a CoreState, k: &'a Kernel) -> Rd<'a> {
        Rd::new(s, k).with_blanked(reach::blanked(s))
    }

    fn bail(&mut self, why: &'static str) -> Next {
        self.stop = Some(Stop::Bailed(why));
        self.stage = Stage::Done;
        Next::Done
    }

    fn over(&self, sx: &Sx<'_>) -> bool {
        finished(Self::rd(sx.state, sx.kernel), self.env.system, self.env.attacker, self.env.defender)
    }

    /// `conclude`: the stage `Done` with the winner measured now (Skilled Retreat's draw has none).
    fn conclude(&mut self, sx: &Sx<'_>, rounds: u32) {
        let meta = sx.state.meta();
        let drawn = meta.combat_draw_round.is(meta.combat_round_seq);
        let who = if drawn { None } else { winner(Self::rd(sx.state, sx.kernel), self.env.system, self.env.attacker, self.env.defender) };
        self.fields.outcome = Some((who, rounds));
        self.fields.step = Step::Done;
    }

    /// `CombatWindow::retreats`: where `seat` may retreat to, in the order of the map.
    #[must_use]
    pub fn retreats(&self, s: &CoreState, k: &Kernel, seat: Seat) -> Vec<System> {
        let side = self.side(seat);
        // Intercept bars a retreat for one round: read as the rounds go by.
        let barred = s.players().by_seat(seat).is_some_and(|p| p.combat_timers.retreat_barred_round.is(s.meta().combat_round_seq));
        if !self.env.may_retreat[side] || barred {
            return Vec::new();
        }
        let rd = Self::rd(s, k);
        self.env.neighbors[side]
            .iter()
            .filter(|(_, enter)| *enter)
            .map(|(n, _)| *n)
            .filter(|n| {
                let cell = s.board().sys(*n);
                if cell.units.iter().any(|u| u.owner() != seat && rd.is_ship(u.kind())) {
                    return false;
                }
                cell.units.iter().any(|u| u.owner() == seat) || qboard::controls_a_planet_in(s, *n, seat)
            })
            .collect()
    }

    /// The presentation at the top of every turn of the loop.
    fn present(&self, sx: &mut Sx<'_>) {
        let p = &self.env.phases;
        let phase = match &self.fields.step {
            Step::Opening { .. } | Step::Announcing { .. } => p[0],
            Step::AfterBarrage { .. } => p[1],
            Step::Queue { .. } | Step::Rolling { .. } => p[2],
            Step::Retreating { .. } => p[3],
            Step::Done => p[4],
        };
        let mut remaining = std::collections::BTreeMap::new();
        if let Step::Queue { queue, .. } = &self.fields.step {
            for pending in queue {
                *remaining.entry(pending.player).or_insert(0_u32) += pending.hits;
            }
        }
        let edit = sx.state.transient_mut().combat.presentation.edit();
        edit.phase = phase;
        edit.remaining_hits.clear();
        for (seat, hits) in remaining {
            edit.remaining_hits.insert(seat, hits);
        }
    }

    fn fleet_at(&self, sx: &Sx<'_>, seat: Seat, system: System) -> Vec<Unit> {
        ships_of(Self::rd(sx.state, sx.kernel), seat, system).iter().map(|p| p.kind()).collect()
    }

    /// The facts of one transition of the flow, read from the core state.
    fn flow_facts(&mut self, sx: &Sx<'_>, over: bool) -> Option<FlowFacts> {
        let env = &self.env;
        let rd = Self::rd(sx.state, sx.kernel);
        let (mut asking_can_retreat, mut leaver_destinations, mut front_alive, mut offered_before, mut sustainers) = (false, 0, 0, false, false);
        match &self.fields.step {
            Step::Announcing { asking, .. } => asking_can_retreat = !self.retreats(sx.state, sx.kernel, *asking).is_empty(),
            Step::Retreating { leaving, .. } => {
                if let Some(first) = leaving.first() {
                    leaver_destinations = self.retreats(sx.state, sx.kernel, *first).len();
                }
            }
            Step::Queue { assigning, round, queue } => {
                if let Some(front) = queue.first() {
                    front_alive = ships_of(rd, front.player, env.system).len();
                    offered_before = self.fields.offered.contains(&(*round, front.player));
                    sustainers = !sustain_candidates(rd, front.player, env.system, true, false, env.staging).is_empty();
                    let producer_picks = front.producer_assigns && *assigning;
                    if front.hits > 0 && front_alive > 0 {
                        let cancel = sx.state.players().by_seat(front.player).and_then(|p| p.combat_timers.cancel_hits_round);
                        let cancelled = cancel.is_some_and(|(r, h)| r == sx.state.meta().combat_round_seq && h > 0);
                        let candidates = hit_targets(rd, front.player, env.system, false, front.non_fighters_only).len();
                        if cancelled || (*assigning && candidates == 1) || (super::is_neutral(sx.state, front.player) && !producer_picks) {
                            return None;
                        }
                    }
                } else if self.fields.phase == Phase::Round {
                    // Duranium Armor repairs after both sides absorbed; hits Devotion makes after a round.
                    let damaged = |seat: Seat| self.fields.damaged.iter().any(|(s, l)| *s == seat && !l.is_empty());
                    if (damaged(env.attacker) && env.duranium[0]) || (damaged(env.defender) && env.duranium[1]) {
                        return None;
                    }
                }
            }
            _ => {}
        }
        if matches!(self.fields.step, Step::Retreating { ref leaving, .. } if leaving.is_empty()) && !over && env.devotion {
            return None;
        }
        Some(FlowFacts {
            attacker: env.attacker,
            defender: env.defender,
            system: env.system,
            announce: env.announce,
            over,
            asking_can_retreat,
            leaver_destinations,
            front_alive,
            offered_before,
            sustainers,
            pending_retreats: self.fields.pending_retreats.clone(),
            phase: self.fields.phase,
            resuming: self.fields.resuming,
        })
    }

    /// One transition of the flow (announcing, retreating, the hit queue), applied to the fields. `true`: the
    /// loop stops (a decision is pending or the fight is over).
    fn transition(&mut self, sx: &mut Sx<'_>) -> Option<bool> {
        if !sx.state.transient().combat.pending_destructions.is_empty() {
            return None;
        }
        let over = self.over(sx);
        let facts = self.flow_facts(sx, over)?;
        let before = self.fields.step.clone();
        let mut flow = Flow::new(facts, before.clone());
        let _ = flow.step(sx);
        let out: Out = flow.out.clone()?;
        if let Step::Queue { round, queue, .. } = &before {
            if out.offered_now {
                if let Some(front) = queue.first() {
                    self.fields.offered.push((*round, front.player));
                }
            }
        }
        // Both sides absorbed: the damage notes of the round are spent.
        if matches!(&before, Step::Queue { queue, .. } if queue.is_empty()) && self.fields.phase == Phase::Round && out.phase == Phase::Round {
            self.fields.damaged.clear();
        }
        self.fields.phase = out.phase;
        self.fields.resuming = out.resuming;
        self.fields.pending_retreats = out.pending_retreats;
        let rounds = match &before {
            Step::Announcing { round, .. } => round.saturating_sub(1),
            Step::Queue { round, .. } | Step::Retreating { round, .. } | Step::Opening { round } | Step::Rolling { round } => *round,
            _ => 0,
        };
        self.fields.step = out.next;
        if self.fields.step == Step::Done {
            self.conclude(sx, rounds);
            return Some(out.returned);
        }
        Some(out.returned)
    }

    fn begin(&self, sx: &mut Sx<'_>, round: u32) {
        let e = &self.env;
        let mut w = BeginRound::new(BeginFacts { attacker: e.attacker, defender: e.defender, system: e.system, round, phase: e.phases[0], announce: e.announce });
        let _ = w.step(sx);
    }

    fn barrage(&mut self, sx: &mut Sx<'_>, round: u32) {
        let e = &self.env;
        let facts = BarrageFacts {
            attacker: e.attacker,
            defender: e.defender,
            system: e.system,
            round,
            occurrence: e.occurrence,
            staging: e.staging,
            announce: e.announce,
            phase: e.phases[1],
            cause: e.barrage_cause,
            kind: e.barrage_kind,
            extra: e.barrage_extra,
            unit_codes: e.unit_codes.clone(),
        };
        let mut w = BarrageStep::new(facts);
        let _ = w.step(sx);
        self.drawn.extend(w.drawn.iter().cloned());
        self.feat = w.feat.clone();
    }

    /// `Stage::Opening`: the string `open_round` (everything before the announcement of retreats).
    fn open_round(&mut self, sx: &mut Sx<'_>, round: u32) -> Option<bool> {
        let resumed = std::mem::take(&mut self.fields.resuming);
        if !resumed {
            if self.env.opening_unported {
                return None;
            }
            self.begin(sx, round);
        }
        if round == 1 {
            if self.env.barrage_unported {
                return None;
            }
            self.barrage(sx, round);
            if !self.feat.is_empty() {
                self.fields.scoring = true;
                self.fields.step = Step::AfterBarrage { round };
                self.stop = Some(Stop::Scoring);
                return Some(true);
            }
            if self.over(sx) {
                self.conclude(sx, round);
                return Some(true);
            }
            sx.state.transient_mut().combat.presentation.edit().phase = self.env.phases[0];
        }
        let (a, d, system) = (self.env.attacker, self.env.defender, self.env.system);
        let mut start = ships_of(Self::rd(sx.state, sx.kernel), a, system);
        start.extend(ships_of(Self::rd(sx.state, sx.kernel), d, system));
        sx.state.transient_mut().combat.presentation.edit().round_start = start;
        self.fields.step = Step::Announcing { round, asking: d, announced: Vec::new() };
        Some(false)
    }

    /// The names of the roll groups of both sides, read from the core state.
    fn names(&self, sx: &Sx<'_>) -> Option<Vec<Name>> {
        let rd = Self::rd(sx.state, sx.kernel);
        let mut names = Vec::new();
        for seat in [self.env.attacker, self.env.defender] {
            for ((value, tag), group) in super::fleet_groups(rd, seat, self.env.system) {
                let mut types: Vec<(Unit, u32)> = group.types.iter().map(|(u, n)| (*u, *n)).collect();
                let mut kinds: Vec<Unit> = types.iter().map(|(u, _)| *u).collect();
                kinds.sort_unstable();
                types.sort_by_key(|(u, _)| self.env.alpha.iter().position(|a| a == u));
                let (_, state, shown) = self.env.groups.iter().find(|(k, _, _)| *k == kinds)?;
                names.push(Name { seat, value, tag, state: *state, shown: *shown, types });
            }
        }
        Some(names)
    }

    fn start_roll(&mut self, sx: &mut Sx<'_>, round: u32) -> Option<()> {
        let e = &self.env;
        if e.roll_unported {
            return None;
        }
        let facts = RollFacts {
            attacker: e.attacker,
            defender: e.defender,
            system: e.system,
            phase: e.phases[2],
            names: self.names(sx)?,
            munitions_after: e.munitions_after,
            announce: e.announce,
            cause: e.roll_cause,
            hacan: e.hacan,
        };
        self.roll = Some(RollStep::new(facts));
        self.roll_round = round;
        None.or(Some(()))
    }

    /// The roll finished: its queue is the round's, and a roll that ended the fight is the string's.
    fn finish_roll(&mut self, sx: &mut Sx<'_>) -> Option<()> {
        let w = self.roll.take()?;
        if self.over(sx) {
            return None;
        }
        self.drawn.extend(w.drawn.iter().cloned());
        let queue: Vec<Pend> = w.queue.iter().map(|q| Pend { player: q.player, hits: q.hits, producer: q.producer, non_fighters_only: q.non_fighters_only, producer_assigns: false }).collect();
        self.fields.damaged = w.damaged.clone();
        self.fields.step = Step::Queue { assigning: false, round: self.roll_round, queue };
        Some(())
    }

    fn retreat_fleet(&mut self, sx: &mut Sx<'_>, round: u32, leaving: &[Seat], destination: System) -> Option<()> {
        let e = &self.env;
        let player = *leaving.first()?;
        if e.retreat_unported {
            return None;
        }
        let there = |me: &Self, sx: &Sx<'_>| me.fleet_at(sx, player, destination);
        let before = there(self, sx);
        let facts = RetreatFacts { player, system: e.system, destination, staging: e.staging, cause: e.retreat_cause, pool: e.retreat_pool };
        let mut w = Retreat::new(facts);
        let _ = w.step(sx);
        if !sx.state.transient().combat.pending_destructions.is_empty() {
            return None;
        }
        let mut now = there(self, sx);
        for kind in before {
            if let Some(at) = now.iter().position(|k| *k == kind) {
                now.remove(at);
            }
        }
        self.fields.fled.push((player, now));
        self.fields.step = Step::Retreating { round, leaving: leaving[1..].to_vec() };
        Some(())
    }

    fn end_announce(&mut self, sx: &mut Sx<'_>) {
        if std::mem::replace(&mut self.fields.end_announced, true) {
            return;
        }
        let Some((who, rounds)) = self.fields.outcome else { return };
        let e = &self.env;
        let meta = sx.state.meta();
        let facts = EndFacts {
            attacker: e.attacker,
            defender: e.defender,
            system: e.system,
            rounds,
            winner: who,
            declared_draw: meta.combat_draw_round.is(meta.combat_round_seq),
            fled: self.fields.fled.clone(),
            at_start: self.fields.at_start.clone(),
            codes: e.end_codes,
            announce: e.announce,
        };
        let mut w = EndAnnounce::new(facts);
        let _ = w.step(sx);
    }

    /// Run the loop. `None`: a position the machine does not carry (the reason is in `stop`).
    fn run(&mut self, sx: &mut Sx<'_>) -> Option<Next> {
        loop {
            if let Some(roll) = &mut self.roll {
                match roll.step(sx) {
                    Next::Continue(ask) => {
                        self.stage = Stage::Rolling;
                        return Some(Next::Continue(ask));
                    }
                    _ => {
                        self.stage = Stage::Running;
                        self.finish_roll(sx).or_else(|| {
                            self.stop = Some(Stop::Bailed("a roll that ends the fight"));
                            None
                        })?;
                        continue;
                    }
                }
            }
            self.present(sx);
            match self.fields.step.clone() {
                Step::Done => {
                    self.end_announce(sx);
                    self.stop = Some(Stop::Over);
                    return Some(Next::Done);
                }
                Step::AfterBarrage { round } => {
                    if self.stop == Some(Stop::Scoring) || self.fields.scoring {
                        self.stop = Some(Stop::Scoring);
                        return Some(Next::Done);
                    }
                    self.fields.step = Step::Announcing { round, asking: self.env.defender, announced: Vec::new() };
                }
                Step::Opening { round } => {
                    if self.over(sx) {
                        self.conclude(sx, round - 1);
                        continue;
                    }
                    if self.open_round(sx, round)? && self.stop == Some(Stop::Scoring) {
                        return Some(Next::Done);
                    }
                }
                Step::Rolling { round } => {
                    if self.over(sx) {
                        self.conclude(sx, round - 1);
                        continue;
                    }
                    self.start_roll(sx, round)?;
                }
                Step::Retreating { round, leaving } => {
                    if let Some(first) = leaving.first() {
                        let destinations = self.retreats(sx.state, sx.kernel, *first);
                        if let [only] = destinations.as_slice() {
                            self.retreat_fleet(sx, round, &leaving, *only)?;
                            continue;
                        }
                    }
                    if self.transition(sx)? {
                        if self.fields.step == Step::Done {
                            // The string returns here without another turn of the loop (no presentation).
                            self.end_announce(sx);
                            self.stop = Some(Stop::Over);
                            return Some(Next::Done);
                        }
                        self.stop = Some(Stop::Pending);
                        return Some(Next::Done);
                    }
                }
                Step::Announcing { .. } | Step::Queue { .. } => {
                    if self.transition(sx)? {
                        if self.fields.step == Step::Done {
                            // The string returns here without another turn of the loop (no presentation).
                            self.end_announce(sx);
                            self.stop = Some(Stop::Over);
                            return Some(Next::Done);
                        }
                        self.stop = Some(Stop::Pending);
                        return Some(Next::Done);
                    }
                }
            }
        }
    }
}

impl Window for Machine {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Done {
            return Next::Done;
        }
        match self.run(sx) {
            Some(next) => {
                if matches!(next, Next::Done) {
                    self.stage = Stage::Done;
                }
                next
            }
            None => {
                if self.stop.is_none() {
                    self.stop = Some(Stop::Bailed("a position the machine does not carry"));
                }
                self.stage = Stage::Done;
                Next::Done
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        self.roll.as_ref().and_then(|r| r.ask_of(s, k))
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match self.roll.as_mut() {
            Some(r) => r.answer(sx, pick),
            None => Err(Illegal),
        }
    }
}

/// Unused outside the adapter: keeps the bail helper reachable for the tests.
impl Machine {
    #[must_use]
    pub fn bailed(&self) -> Option<&'static str> {
        match self.stop {
            Some(Stop::Bailed(why)) => Some(why),
            _ => None,
        }
    }

    #[allow(dead_code)]
    fn unused(&mut self) -> Next {
        self.bail("unused")
    }

    #[must_use]
    pub const fn max_rounds() -> u32 {
        MAX_ROUNDS
    }
}
