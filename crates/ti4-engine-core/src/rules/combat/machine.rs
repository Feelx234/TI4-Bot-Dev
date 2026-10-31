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
//! hits at the start of a combat, a neutral side,
//! Duranium Armor, Devotion, a retreat that moves a flipping unit or a Floating Factory or a Last Bastion
//! seat's fleet, a roll that ends the fight, destructions waiting for their announcement, reactions.
//!
//! A decision that is pending when the machine stops (`Stop::Pending`) is the string's `pending_choice`; the
//! machine builds the same question in [`Machine::pending_ask`] and answers it in [`Machine::answer_pending`].

use super::assault_cannon::{AssaultCannon, Facts as AcFacts};
use super::barrage::{BarrageStep, Facts as BarrageFacts};
use super::begin_round::{BeginRound, Facts as BeginFacts};
use super::end::{finished, winner};
use super::effect_hits::Absorb;
use super::end_announce::{Codes as EndCodes, EndAnnounce, Facts as EndFacts};
use super::flow::{Facts as FlowFacts, Flow, Out, Pend, Phase, Step, MAX_ROUNDS};
use super::assign::{announce_loss, remove_ship};
use super::produce::{Facts as ProduceFacts, ProduceHits};
use super::retreat::{Facts as RetreatFacts, Retreat};
use super::sustain_commander::SustainCommander;
use super::sustain_offer::{emit_used, reflective_hits, Offer};
use super::roll::{Facts as RollFacts, Name, RollStep};
use super::{apply_sustain, cancel_per_sustain, hit_targets, ships_of, sustain_candidates, sustain_opts, Rd};
use crate::choice::combat::{Ask as CAsk, Opt as COpt, Subtype as CSub};
use crate::choice::generic::Rule;
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Owed, OwedKind, Pick, Subtype, Target};
use crate::containers::SmallList;
use crate::kernel::fx;
use crate::state::{EventKind, Key, Payload, Piece, Val};
use crate::generated::{System, Unit};
use crate::ids::Seat;
use crate::kernel::hooks::CombatMoment;
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
    /// The feat occurrence of the combat (already begun when the combat had one).
    pub occurrence: u64,
    /// The combat has none yet: the first round begins it, after the opening (a fight that ends in the opening, a card
    /// played at its start, never does).
    pub begin_occurrence: bool,
    /// The presentation phases: `pre_roll`, `barrage`, `resolving_hits`, `retreating`, `complete`.
    pub phases: [Code; 5],
    /// `round_opening_unported` (Naaz, Letnev, diplomacy): the opening of a round is the string's.
    pub opening_unported: bool,
    /// A Letnev round agent (own or a Yssaril copy) listens: `COMBAT_ROUND_STARTED` carries `round_seq`.
    pub round_seq_listens: bool,
    /// `barrage_unported`, or dice the core cannot reproduce.
    pub barrage_unported: bool,
    pub barrage_cause: Code,
    pub barrage_kind: Code,
    pub barrage_extra: Code,
    pub unit_codes: Vec<(Unit, Code)>,
    /// The same unit types' texts in the question renderer's table, the relic's `extra barrage`, and the sides (attacker, defender) the
    /// Jol-Nar commander is in play for (it rerolls the barrage's dice).
    pub shown_codes: Vec<(Unit, Code)>,
    pub barrage_extra_shown: Code,
    pub jolnar: [bool; 2],
    /// A roll the core cannot carry (`roll_unported`, dice it cannot reproduce).
    pub roll_unported: bool,
    pub munitions_after: [bool; 2],
    /// Munitions Reserves is paid at the opening of a round (a holder below `RULES_MUNITIONS_AFTER_ROLL`), per side.
    pub munitions_start: [bool; 2],
    pub roll_cause: Code,
    pub hacan: Code,
    /// Every roll group's text (the unit types joined with `+` in id order): (types, state text, shown text).
    pub groups: Vec<(Vec<Unit>, Code, Code)>,
    /// The unit types present in the alphabetical order of their ids (the order a roll names its types in).
    pub alpha: Vec<Unit>,
    /// The cause of a ship a retreat could not hold: the state table's text (what the staged destruction carries) and the renderer's
    /// (what the `SHIP_DESTROYED` announcement of the loss carries).
    pub retreat_cause: Code,
    pub retreat_event: Code,
    pub retreat_pool: Code,
    /// A retreat the core does not carry (the Last Bastion, a flagship that raises an event on arrival). The dual-form units, the
    /// Voltron and the Floating Factory are carried by the retreat window since package W3.
    pub retreat_unported: bool,
    /// Per side (attacker, defender): the neighbours of the system a fleet may retreat to as far as the movement
    /// rules go, and whether the side may retreat at all (Intercept, the Mahact hero).
    pub neighbors: [Vec<(System, bool)>; 2],
    pub may_retreat: [bool; 2],
    /// Per side: Dark Energy Tap is owned ("your ships can retreat into adjacent systems that do not contain other players' units, even
    /// if you have no units or planets there": the holder's destinations are the union of the two rules).
    pub det: [bool; 2],
    /// Per side: Duranium Armor is owned; the Yin (Devotion) hits-after-a-round module is in the game.
    pub duranium: [bool; 2],
    /// A Yin seat is in the game: the hits Devotion makes after a round are asked (the window `ProduceHits` at `RoundEnded`).
    pub devotion: bool,
    /// The system holds a wormhole now (Dimensional Splicer), and the interned texts of the hits the modules produce.
    pub wormhole: bool,
    pub ambush: Code,
    pub cause_core: Code,
    pub cause_devotion: Code,
    /// The interned subtype `raid_formation_damage` of an Argent producer's excess barrage hits.
    pub raid: Code,
    /// A side holds Assault Cannon (and none is a neutral side), and the interned subtype and cause of its loss.
    pub ac: bool,
    pub ac_subtype: Code,
    pub ac_cause: Code,
    pub end_codes: EndCodes,
    /// The interned subtype text of the casualty question (`"assign_casualty"`).
    pub casualty: Code,
    /// The interned source of the trade good of the Letnev commander (`"letnevcommander"`).
    pub commander: Code,
    /// The decisions the rules answer for a neutral side are put as questions and answered by the machine itself (the lockstep with the
    /// string engine, whose neutral answer is a question it answers at once, compares them); off, nothing is asked.
    pub show_neutral: bool,
}

pub use crate::state::CombatFields as Fields;

/// Why the machine stopped at a position it does not carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bail {
    /// A roll that emptied a fleet mid-way (the fight ends inside the roll step).
    RollEndsFight,
    /// A position outside what the machine carries (a gate of the `Env`, a cancellation, Duranium Armor, Devotion, staged destructions, a retreat the core does not move).
    Unported,
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
    Bailed(Bail),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Running,
    Rolling,
    /// The hits the modules produce (`ProduceHits`) wait on a question.
    Producing,
    /// The barrage waits on a question (Raid Formation).
    Barraging,
    /// The pending decision (or the commander's offer inside its answer) waits on the players.
    Asking,
    /// A neutral side's decision is put (`Env::show_neutral`) and answered by the rules.
    Auto,
    Done,
}

/// What the rules answer to a neutral side's decision.
#[derive(Debug, Clone, Copy)]
enum Taken {
    Sustain { opt: Opt, site: crate::choice::combat::SustainSite, unit: Unit },
    Destroy { opt: Opt, doomed: Piece },
}

/// A sustain being answered: the commander's offer runs before the hits are counted.
#[derive(Debug, Clone)]
struct Sustaining {
    /// Rear Admiral Farran's payment after a sustain (rules version 17 and later; the window did not ask before).
    commander: Option<SustainCommander>,
    offer: Offer,
    unit: Unit,
    round: u32,
    queue: Vec<Pend>,
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
    /// The hits the modules produce at a moment of the round being played, and where they came from.
    produce: Option<(ProduceHits, CombatMoment, u32)>,
    /// The anti-fighter barrage being played (an Argent producer's Raid Formation may ask) and whether it finished while the round
    /// resumes after it.
    barrage_w: Option<BarrageStep>,
    barrage_done: bool,
    /// Assault Cannon being resolved at the start of the combat, and whether it was (the opening goes on after it).
    ac_w: Option<AssaultCannon>,
    after_ac: bool,
    /// The round whose end-of-round hits were produced already (the flow goes on to its end).
    round_end_produced: Option<u32>,
    stage: Stage,
    /// Built for the answer to the decision the settle stopped at (`Machine::awaiting`).
    awaiting: bool,
    answered: bool,
    sustaining: Option<Sustaining>,
    /// The neutral side's decision being shown (`Env::show_neutral`).
    auto: Option<Choice>,
    /// The hits a Reflective Shielding played in the window of a use of SUSTAIN DAMAGE staged, being absorbed.
    reflecting: Option<Box<Absorb>>,
    /// The last unit that sustained: where it stood and how it stands now.
    pub last_sustain: Option<(crate::choice::combat::SustainSite, Piece)>,
}

impl Machine {
    #[must_use]
    pub fn new(env: Env, fields: Fields) -> Self {
        Self {
            env,
            fields,
            stop: None,
            drawn: Vec::new(),
            feat: Vec::new(),
            roll: None,
            roll_round: 0,
            produce: None,
            barrage_w: None,
            barrage_done: false,
            ac_w: None,
            after_ac: false,
            round_end_produced: None,
            stage: Stage::Running,
            awaiting: false,
            answered: false,
            sustaining: None,
            auto: None,
            reflecting: None,
            last_sustain: None,
        }
    }

    /// A machine that puts the decision the settle stopped at (`fields.step`) and applies the answer to it,
    /// nothing more: the settle that follows is a machine of its own.
    #[must_use]
    pub fn awaiting(env: Env, fields: Fields) -> Self {
        let mut m = Self::new(env, fields);
        m.awaiting = true;
        m
    }

    fn side(&self, seat: Seat) -> usize {
        usize::from(seat != self.env.attacker)
    }

    fn rd<'a>(s: &'a CoreState, k: &'a Kernel) -> Rd<'a> {
        Rd::new(s, k).with_blanked(reach::blanked(s))
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
        let barred = s.players().by_seat(seat).is_some_and(|p| p.card_timers.retreat_barred_round.is(s.meta().combat_round_seq));
        if !self.env.may_retreat[side] || barred {
            return Vec::new();
        }
        let rd = Self::rd(s, k);
        let mut spots: Vec<System> = self.env.neighbors[side]
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
            .collect();
        // Dark Energy Tap: the holder alone adds the neighbours with no other player's unit at all (ships or ground forces), after the others.
        if self.env.det[side] {
            for (n, _) in self.env.neighbors[side].iter().filter(|(_, enter)| *enter) {
                let foreign = s.board().sys(*n).units.iter().any(|u| u.owner() != seat)
                    || super::planets_of_sorted(s, *n).iter().any(|p| s.board().planet(*p).units.iter().any(|u| u.owner() != seat));
                if !foreign && !spots.contains(n) {
                    spots.push(*n);
                }
            }
        }
        spots
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
        let edit = sx.state.transient_mut().combat.presentation.edit();
        edit.phase = phase;
        edit.remaining_hits.clear();
        if let Step::Queue { queue, .. } = &self.fields.step {
            for pending in queue {
                let held = edit.remaining_hits.get(&pending.player).copied().unwrap_or(0);
                edit.remaining_hits.insert(pending.player, held + pending.hits);
            }
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
                    // (Hits the round's pool cancels are spent by the flow window itself, before anything else is asked; a neutral side's
                    // automatic answer comes after the flow, in `transition`.)
                    let _ = assigning;
                }
            }
            _ => {}
        }
        let _ = over;
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
        if self.lone_casualty(sx) {
            return Some(false);
        }
        // Both sides absorbed: Duranium Armor repairs now, "after you assign hits to your units".
        if matches!(&self.fields.step, Step::Queue { queue, .. } if queue.is_empty()) && self.fields.phase == Phase::Round {
            self.duranium(sx);
        }
        let over = self.over(sx);
        // After a round (retreats included): the hits Devotion makes, produced once the fight is not decided.
        if let Step::Retreating { round, leaving } = &self.fields.step {
            if leaving.is_empty() && !over && self.env.devotion && self.round_end_produced != Some(*round) {
                let round = *round;
                self.start_produce(CombatMoment::RoundEnded, round);
                return Some(false);
            }
        }
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
        if out.returned && self.neutral_due(sx) {
            if self.env.show_neutral {
                self.auto = self.pending_choice(sx.state, sx.kernel);
                if self.auto.is_none() {
                    return None;
                }
            } else if !self.neutral_answer(sx) {
                return None;
            }
            return Some(false);
        }
        Some(out.returned)
    }

    /// Whether the decision pending at `fields.step` is a neutral side's, which the rules answer.
    fn neutral_due(&self, sx: &Sx<'_>) -> bool {
        let Step::Queue { assigning, queue, .. } = &self.fields.step else { return false };
        let Some(front) = queue.first() else { return false };
        front.hits > 0 && super::is_neutral(sx.state, front.player) && !(front.producer_assigns && *assigning)
    }

    /// What the rules answer to the neutral side's decision at `fields.step` (Thunder's Edge rules 5 and 7; string `neutral_answer`): a
    /// sustain is always taken (the first option), a hit goes to the unit lowest on the reference card.
    fn neutral_taken(&self, sx: &Sx<'_>) -> Option<Taken> {
        let Step::Queue { assigning, .. } = &self.fields.step else { return None };
        let front = *match &self.fields.step {
            Step::Queue { queue, .. } => queue.first()?,
            _ => return None,
        };
        let system = self.env.system;
        let rd = Self::rd(sx.state, sx.kernel);
        if *assigning {
            let candidates = Self::candidates(rd, &front, system);
            let kinds: Vec<Unit> = candidates.iter().map(|(_, p)| p.kind()).collect();
            let at = super::neutral_casualty(&kinds)?;
            let key = (candidates[at].1.kind(), candidates[at].1.sustained());
            // The option is the first unit of its (type, damage): the string finds it by those two.
            let (index, doomed) = candidates.iter().find(|(_, p)| (p.kind(), p.sustained()) == key).copied()?;
            return Some(Taken::Destroy { opt: Opt::Combat(COpt::Destroy { index, unit: key.0, damaged: key.1 }), doomed });
        }
        let own = u16::try_from(ships_of(rd, front.player, system).len()).unwrap_or(u16::MAX);
        let ask = CAsk::CancelHit {
            system,
            hits: u16::try_from(front.hits).unwrap_or(u16::MAX),
            non_fighters_first: front.non_fighters_only && ships_of(rd, front.player, system).iter().any(|p| !rd.is_fighter(p.kind())),
            staging: self.env.staging,
            ability: false,
            in_combat: true,
            window: Some(own),
        };
        let opt = sustain_opts(rd, front.player, ask).as_slice().first().copied()?;
        let Opt::Combat(COpt::Sustain { site, unit, .. }) = opt else { return None };
        Some(Taken::Sustain { opt, site, unit })
    }

    /// The option the rules take at the neutral side's pending `choice`.
    fn neutral_choice_taken(&self, sx: &Sx<'_>, _choice: &Choice) -> Option<Opt> {
        match self.neutral_taken(sx)? {
            Taken::Destroy { opt, .. } | Taken::Sustain { opt, .. } => Some(opt),
        }
    }

    /// A decision pending for a neutral side is answered by the rules, not by a player: applied, nothing asked. A hit a producer assigns
    /// (Devotion) is the producer's to place. `true`: it answered.
    fn neutral_answer(&mut self, sx: &mut Sx<'_>) -> bool {
        let Step::Queue { assigning, round, mut queue } = self.fields.step.clone() else { return false };
        let Some(front) = queue.first().copied() else { return false };
        let system = self.env.system;
        if front.hits == 0 || !super::is_neutral(sx.state, front.player) || (front.producer_assigns && assigning) {
            return false;
        }
        let Some(taken) = self.neutral_taken(sx) else { return false };
        match taken {
            Taken::Destroy { doomed, .. } => {
                remove_ship(sx, system, front.player, doomed);
                announce_loss(sx, system, front.player, doomed.kind(), self.env.end_cause(), true, self.env.announce);
                queue[0].hits = front.hits.saturating_sub(1);
            }
            Taken::Sustain { site, unit, .. } => {
                let rd = Self::rd(sx.state, sx.kernel);
                let cancels = u32::try_from(Self::cancels(sx.state, rd, front.player)).unwrap_or(u32::MAX);
                if let Some((after, piece)) = apply_sustain(rd, front.player, system, site) {
                    *sx.state = after;
                    super::sustain_offer::remember_target(sx.state, system, self.env.staging, site, piece);
                    let offer = Offer {
                        player: front.player,
                        producer: front.producer,
                        system,
                        ability: false,
                        in_combat: true,
                        non_fighters_first: false,
                        source: self.env.commander,
                        staging: self.env.staging,
                        announce: self.env.announce,
                    };
                    let _ = emit_used(sx, &offer, unit);
                }
                queue[0].hits = front.hits.saturating_sub(cancels);
            }
        }
        self.fields.step = Step::Queue { assigning: false, round, queue };
        true
    }

    /// A single possible casualty is not a decision (string `hit_queue_head_string`): the ship is lost, the hit
    /// counted, and the queue goes back to the sustain offer. `true`: that happened.
    fn lone_casualty(&mut self, sx: &mut Sx<'_>) -> bool {
        let Step::Queue { assigning: true, round, queue } = self.fields.step.clone() else { return false };
        let env_system = self.env.system;
        let Some(front) = queue.first().copied() else { return false };
        let rd = Self::rd(sx.state, sx.kernel);
        if front.hits == 0 || ships_of(rd, front.player, env_system).is_empty() {
            return false;
        }
        let cancelled = sx.state.players().by_seat(front.player).and_then(|p| p.card_timers.cancel_hits_round).is_some_and(|(r, h)| r == sx.state.meta().combat_round_seq && h > 0);
        let targets = hit_targets(rd, front.player, env_system, false, front.non_fighters_only);
        let [only] = targets.as_slice() else { return false };
        if cancelled {
            return false;
        }
        let only = *only;
        remove_ship(sx, env_system, front.player, only);
        announce_loss(sx, env_system, front.player, only.kind(), self.env.end_cause(), true, self.env.announce);
        let mut queue = queue;
        queue[0].hits = front.hits.saturating_sub(1);
        self.fields.step = Step::Queue { assigning: false, round, queue };
        true
    }

    fn begin(&mut self, sx: &mut Sx<'_>, round: u32) {
        let e = &self.env;
        // The string asks at each announcement whether a Letnev round agent (the owner's, or a Yssaril copy) is still usable;
        // using one exhausts it (or the Ssruu that copies it), so the answer taken when the machine was built goes stale.
        // The fact stays the upper bound (it is what the string saw), the state says what holds now.
        let listens = e.round_seq_listens && super::begin_round::letnev_round_agent_usable(&*sx.state);
        let mut w = BeginRound::new(BeginFacts { attacker: e.attacker, defender: e.defender, system: e.system, round, phase: e.phases[0], announce: e.announce, round_seq_listens: listens, munitions_start: e.munitions_start });
        let _ = w.step(sx);
        // A unit that took its ship form joins the fleet the end announcement measures the losses against.
        for (owner, form) in w.flipped {
            if let Some((_, fleet)) = self.fields.at_start.iter_mut().find(|(s, _)| *s == owner) {
                fleet.push(form);
                fleet.sort_unstable();
            }
        }
    }

    /// Start the barrage step (string `barrage_step`); it runs in the loop, where its question (Raid Formation) can stop it.
    fn start_barrage(&mut self, round: u32) {
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
            raid: Some(e.raid),
            commander: e.jolnar,
            shown: e.shown_codes.clone(),
            extra_shown: e.barrage_extra_shown,
            hit_texts: crate::kernel::children::HitTexts { staging: e.staging, commander: e.commander, casualty: e.casualty, effect_hit: e.barrage_cause },
        };
        self.barrage_w = Some(BarrageStep::new(facts));
    }

    /// `Stage::Opening`: the string `open_round` (everything before the announcement of retreats).
    fn open_round(&mut self, sx: &mut Sx<'_>, round: u32) -> Option<bool> {
        let resumed = std::mem::take(&mut self.fields.resuming);
        if !resumed && !self.after_ac {
            if self.env.opening_unported {
                return None;
            }
            self.begin(sx, round);
        }
        if round == 1 {
            if !resumed && self.env.ac && !self.after_ac {
                // Assault Cannon: "at the start of a space combat" the opponent of a holder destroys a non-fighter ship.
                let e = &self.env;
                self.ac_w = Some(AssaultCannon::new(AcFacts { attacker: e.attacker, defender: e.defender, system: e.system, subtype: e.ac_subtype, cause: e.ac_cause, announce: e.announce }));
                return Some(false);
            }
            self.after_ac = false;
            // The string checks the fight after the round's opening (Assault Cannon, or a card played at its start, a
            // Skilled Retreat) before the barrage.
            if !resumed && self.over(sx) {
                self.conclude(sx, round);
                return Some(true);
            }
            if !resumed {
                // "At the start of a space combat": the hits the modules produce come before the barrage; the round resumes there.
                self.start_produce(CombatMoment::CombatStart, round);
                return Some(false);
            }
            if self.env.barrage_unported {
                return None;
            }
            if std::mem::take(&mut self.env.begin_occurrence) {
                let meta = sx.state.meta_mut();
                meta.feat_occurrence_seq += 1;
                self.env.occurrence = meta.feat_occurrence_seq;
            }
            if !self.barrage_done {
                self.start_barrage(round);
                // The round resumes here once the barrage is played.
                self.fields.resuming = true;
                return Some(false);
            }
            self.barrage_done = false;
            if !self.feat.is_empty() {
                // (The turn of the loop that resumed this round presented the opening; the barrage's presentation stands.)
                sx.state.transient_mut().combat.presentation.edit().phase = self.env.phases[1];
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
            round,
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

    /// Duranium Armor (string `duranium_armor`): "after you assign hits to your units, repair 1 of your damaged units that did not use
    /// SUSTAIN DAMAGE during this combat round". `damaged` lists the types of the side's ships that were damaged when the round's hits were
    /// queued; a unit of such a type still damaged now cannot be one that sustained this round, and damaged units of one type are
    /// interchangeable, so the first is repaired. The round's note of each side is spent.
    fn duranium(&mut self, sx: &mut Sx<'_>) {
        let (system, sides) = (self.env.system, [self.env.attacker, self.env.defender]);
        for (at, side) in sides.into_iter().enumerate() {
            let before: Vec<Unit> = self.fields.damaged.iter().find(|(s, _)| *s == side).map(|(_, kinds)| kinds.clone()).unwrap_or_default();
            self.fields.damaged.retain(|(s, _)| *s != side);
            if before.is_empty() || !self.env.duranium[at] {
                continue;
            }
            let units = sx.state.board_mut().sys_mut(system).units.as_mut_slice();
            if let Some(unit) = units.iter_mut().find(|u| u.owner() == side && u.sustained() && before.contains(&u.kind())) {
                *unit = unit.with_sustained(false);
            }
        }
    }

    /// Start the window of the hits the modules produce at `moment` of `round` (string `produce_hits`).
    fn start_produce(&mut self, moment: CombatMoment, round: u32) {
        let e = &self.env;
        let facts = ProduceFacts {
            attacker: e.attacker,
            defender: e.defender,
            system: e.system,
            moment,
            announce: e.announce,
            wormhole: e.wormhole,
            ambush: e.ambush,
            cause_core: e.cause_core,
            cause_devotion: e.cause_devotion,
        };
        self.produce = Some((ProduceHits::new(facts), moment, round));
    }

    /// The hits queued for assignment from what each side produced (string `produced_queue`): forced non-fighter hits first, like a
    /// round's own, then the producer-assigned ones. `produced` is `[attacker, defender]`.
    fn produced_queue(&self, produced: &[crate::kernel::hooks::ProducedHits; 2]) -> Vec<Pend> {
        let (a, d) = (self.env.attacker, self.env.defender);
        let mut queue: Vec<Pend> = [
            (d, produced[0].non_fighter, a, true),
            (d, produced[0].any_ship, a, false),
            (a, produced[1].non_fighter, d, true),
            (a, produced[1].any_ship, d, false),
        ]
        .into_iter()
        .filter(|(_, hits, _, _)| *hits > 0)
        .map(|(player, hits, producer, non_fighters_only)| Pend { player, hits: u32::try_from(hits).unwrap_or(u32::MAX), producer, non_fighters_only, producer_assigns: false })
        .collect();
        queue.extend(
            [(d, produced[0].producer_assigned, a), (a, produced[1].producer_assigned, d)]
                .into_iter()
                .filter(|(_, hits, _)| *hits > 0)
                .map(|(player, hits, producer)| Pend { player, hits: u32::try_from(hits).unwrap_or(u32::MAX), producer, non_fighters_only: false, producer_assigns: true }),
        );
        queue
    }

    /// The window finished: its hits are the queue of the moment (`open_start_hits`, `open_round_end_hits`), or the round goes on.
    fn finish_produce(&mut self) {
        let Some((w, moment, round)) = self.produce.take() else { return };
        self.drawn.extend(w.drawn.iter().cloned());
        let queue = self.produced_queue(&w.produced);
        match moment {
            CombatMoment::CombatStart => {
                if queue.is_empty() {
                    // The round resumes at the barrage, as after a queue of start hits.
                    self.fields.resuming = true;
                } else {
                    self.fields.phase = Phase::CombatStart;
                    self.fields.step = Step::Queue { assigning: false, round, queue };
                }
            }
            _ => {
                self.round_end_produced = Some(round);
                if !queue.is_empty() {
                    self.fields.phase = Phase::RoundEnd;
                    // Hits produced after the round are their own "before you assign hits" moment: the once-per-round guard on the
                    // ordinary assignment must not swallow their announcement.
                    self.fields.offered.retain(|(seen, _)| *seen != round);
                    self.fields.step = Step::Queue { assigning: false, round, queue };
                }
            }
        }
    }

    /// The roll finished: its queue is the round's. `Some(true)`: a side's window ended the fight (the step queued nothing; the
    /// fight concludes with the round count of the roll).
    fn finish_roll(&mut self, sx: &mut Sx<'_>) -> Option<bool> {
        let w = self.roll.take()?;
        if w.ended {
            self.drawn.extend(w.drawn.iter().cloned());
            self.conclude(sx, self.roll_round);
            return Some(true);
        }
        if self.over(sx) {
            return None;
        }
        self.drawn.extend(w.drawn.iter().cloned());
        let queue: Vec<Pend> = w.queue.iter().map(|q| Pend { player: q.player, hits: q.hits, producer: q.producer, non_fighters_only: q.non_fighters_only, producer_assigns: false }).collect();
        self.fields.damaged = w.damaged.clone();
        self.fields.step = Step::Queue { assigning: false, round: self.roll_round, queue };
        Some(false)
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
        // The ships the destination could not hold are lost and announced at once (string `retreat`: `announce_staged_destructions`).
        self.announce_staged(sx);
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

    /// `announce_staged_destructions`: every destruction an effect staged is announced as `SHIP_DESTROYED`, in staging order, until
    /// nothing is left (an announcement can stage more).
    fn announce_staged(&self, sx: &mut Sx<'_>) {
        while !sx.state.transient().combat.pending_destructions.is_empty() {
            let staged = std::mem::take(&mut sx.state.transient_mut().combat.pending_destructions).as_slice().to_vec();
            for d in staged {
                let cause = if d.reason == self.env.retreat_cause { self.env.retreat_event } else { d.reason };
                announce_loss(sx, d.system, d.owner, d.unit, cause, d.during_space_combat, self.env.announce);
            }
        }
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
            if let Some(choice) = &self.auto {
                self.stage = Stage::Auto;
                return Some(Next::Continue(choice.ask));
            }
            if let Some(ac) = &mut self.ac_w {
                match ac.step(sx) {
                    Next::Continue(ask) => {
                        self.stage = Stage::Producing;
                        return Some(Next::Continue(ask));
                    }
                    _ => {
                        self.stage = Stage::Running;
                        self.ac_w = None;
                        self.after_ac = true;
                        continue;
                    }
                }
            }
            if let Some(barrage) = &mut self.barrage_w {
                match barrage.step(sx) {
                    Next::Continue(ask) => {
                        self.stage = Stage::Barraging;
                        return Some(Next::Continue(ask));
                    }
                    _ => {
                        self.stage = Stage::Running;
                        if let Some(done) = self.barrage_w.take() {
                            self.drawn.extend(done.drawn.iter().cloned());
                            self.feat = done.feat.clone();
                        }
                        self.barrage_done = true;
                        self.fields.resuming = true;
                        continue;
                    }
                }
            }
            if let Some((produce, _, _)) = &mut self.produce {
                match produce.step(sx) {
                    Next::Continue(ask) => {
                        self.stage = Stage::Producing;
                        return Some(Next::Continue(ask));
                    }
                    _ => {
                        self.stage = Stage::Running;
                        self.finish_produce();
                        continue;
                    }
                }
            }
            if let Some(roll) = &mut self.roll {
                match roll.step(sx) {
                    Next::Continue(ask) => {
                        self.stage = Stage::Rolling;
                        return Some(Next::Continue(ask));
                    }
                    _ => {
                        self.stage = Stage::Running;
                        let ended = self.finish_roll(sx).or_else(|| {
                            self.stop = Some(Stop::Bailed(Bail::RollEndsFight));
                            None
                        })?;
                        if ended {
                            // A roll that ended the fight: the string concludes it in the roll step and returns, without another turn of
                            // the loop (no presentation), then announces the end.
                            self.end_announce(sx);
                            self.stop = Some(Stop::Over);
                            return Some(Next::Done);
                        }
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
                    // The round resumes here once the barrage is played: the string scores the barrage feat (and pauses for it)
                    // even when the barrage took the last ships of a side; the end of the fight is read after that pause.
                    // (Not right after Assault Cannon: the string checks the fight inside the opening, after the cannon, and concludes it with the
                    // round it opened.)
                    if !self.barrage_done && !self.after_ac && self.over(sx) {
                        self.conclude(sx, round - 1);
                        // The string returns here without another turn of the loop (no presentation): the phase of the stage stands.
                        self.end_announce(sx);
                        self.stop = Some(Stop::Over);
                        return Some(Next::Done);
                    }
                    if self.open_round(sx, round)? && self.stop == Some(Stop::Scoring) {
                        return Some(Next::Done);
                    }
                }
                Step::Rolling { round } => {
                    if self.over(sx) {
                        self.conclude(sx, round - 1);
                        // As above: the string's `Rolling` arm returns after `conclude`.
                        self.end_announce(sx);
                        self.stop = Some(Stop::Over);
                        return Some(Next::Done);
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
        if self.awaiting {
            return self.step_awaiting(sx);
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
                    self.stop = Some(Stop::Bailed(Bail::Unported));
                }
                self.stage = Stage::Done;
                Next::Done
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        if self.awaiting {
            if let Some(reflecting) = &self.reflecting {
                return reflecting.ask_of(s, k);
            }
            if let Some(sus) = &self.sustaining {
                return sus.commander.as_ref().and_then(|c| c.ask_of(s, k));
            }
            return if self.answered { None } else { self.pending_choice(s, k) };
        }
        if let Some(choice) = &self.auto {
            return Some(choice.clone());
        }
        if let Some(ac) = &self.ac_w {
            return ac.ask_of(s, k);
        }
        if let Some(b) = &self.barrage_w {
            return b.ask_of(s, k);
        }
        if let Some((p, _, _)) = &self.produce {
            return p.ask_of(s, k);
        }
        self.roll.as_ref().and_then(|r| r.ask_of(s, k))
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.awaiting {
            if let Some(reflecting) = self.reflecting.as_mut() {
                return reflecting.answer(sx, pick);
            }
            if let Some(sus) = self.sustaining.as_mut() {
                return sus.commander.as_mut().ok_or(Illegal)?.answer(sx, pick);
            }
            if self.answered {
                return Err(Illegal);
            }
            return self.answer_pending(sx, pick);
        }
        if let Some(choice) = self.auto.take() {
            // The rules' own answer: the pick must be the one asked for (the string answers its question with it).
            let Pick::Opt(opt) = pick else { return Err(Illegal) };
            if !choice.opts.as_slice().contains(&opt) {
                self.auto = Some(choice);
                return Err(Illegal);
            }
            self.stage = Stage::Running;
            let expected = self.neutral_choice_taken(sx, &choice);
            if expected != Some(opt) || !self.neutral_answer(sx) {
                return Err(Illegal);
            }
            return Ok(());
        }
        if let Some(ac) = self.ac_w.as_mut() {
            return ac.answer(sx, pick);
        }
        if let Some(b) = self.barrage_w.as_mut() {
            return b.answer(sx, pick);
        }
        if let Some((p, _, _)) = self.produce.as_mut() {
            return p.answer(sx, pick);
        }
        match self.roll.as_mut() {
            Some(r) => r.answer(sx, pick),
            None => Err(Illegal),
        }
    }
}

impl Machine {
    #[must_use]
    pub fn bailed(&self) -> Option<Bail> {
        match self.stop {
            Some(Stop::Bailed(why)) => Some(why),
            _ => None,
        }
    }

    /// The producer of the hits a Reflective Shielding staged while the machine absorbs them (the questions it asks belong to it).
    #[must_use]
    pub fn reflecting_producer(&self) -> Option<Seat> {
        self.reflecting.as_ref().map(|a| a.producer())
    }

    #[must_use]
    pub const fn max_rounds() -> u32 {
        MAX_ROUNDS
    }
}

// -- the decisions the settle stops at (string `pending_choice` / `resolve`) ----------------------------------

impl Machine {
    /// The hits one use of SUSTAIN DAMAGE cancels: one, or two with Non-Euclidean Shielding from rules version 17.
    fn cancels(s: &CoreState, rd: Rd<'_>, player: Seat) -> usize {
        if s.meta().rules_at_least(super::RULES_FACTION_BUGFIXES) { cancel_per_sustain(rd, player) } else { 1 }
    }

    fn bail(&mut self) {
        self.stop = Some(Stop::Bailed(Bail::Unported));
        self.answered = true;
    }

    fn step_awaiting(&mut self, sx: &mut Sx<'_>) -> Next {
        if let Some(sus) = self.sustaining.as_mut() {
            if let Some(commander) = sus.commander.as_mut()
                && let Next::Continue(ask) = commander.step(sx)
            {
                self.stage = Stage::Asking;
                return Next::Continue(ask);
            }
            // The commander has answered (or there is none): the use is announced, the hits it cancels counted.
            let Some(sus) = self.sustaining.take() else { return Next::Done };
            let _ = emit_used(sx, &sus.offer, sus.unit);
            self.reflecting = reflective_hits(sx, &sus.offer);
            let cancels = Self::cancels(sx.state, Self::rd(sx.state, sx.kernel), sus.offer.player);
            let mut queue = sus.queue;
            if let Some(front) = queue.first_mut() {
                front.hits = front.hits.saturating_sub(u32::try_from(cancels).unwrap_or(u32::MAX));
            }
            self.fields.step = Step::Queue { assigning: false, round: sus.round, queue };
            self.answered = true;
        }
        if let Some(reflecting) = self.reflecting.as_mut() {
            if let Next::Continue(ask) = reflecting.step(sx) {
                self.stage = Stage::Asking;
                return Next::Continue(ask);
            }
            self.reflecting = None;
        }
        if self.answered {
            self.stage = Stage::Done;
            return Next::Done;
        }
        match self.pending_choice(sx.state, sx.kernel) {
            Some(choice) => {
                self.stage = Stage::Asking;
                Next::Continue(choice.ask)
            }
            None => {
                self.bail();
                self.stage = Stage::Done;
                Next::Done
            }
        }
    }

    fn ctx(&self, rule: Rule, subtype: Subtype, owed: Option<u32>) -> Ctx {
        Ctx {
            origin: Origin::Rule(rule),
            subtype,
            optional: false,
            target: Some(Target::System(self.env.system)),
            outstanding: owed
                .map(|n| Owed { kind: OwedKind::UnitsToRemove, amount: i64::from(n), paid: 0 })
                .into_iter()
                .collect::<SmallList<Owed, 4>>(),
            space_battle: false,
            invasion_seq: None,
        }
    }

    /// The ships a hit may be assigned to with their index in the owner's ship list (`casualty_candidates`): every
    /// ship, or only the non-fighters when the hit is bound to them and one is left.
    fn candidates(rd: Rd<'_>, front: &Pend, system: System) -> Vec<(u16, Piece)> {
        let units: Vec<(u16, Piece)> = ships_of(rd, front.player, system).into_iter().enumerate().map(|(i, p)| (i as u16, p)).collect();
        if front.non_fighters_only && units.iter().any(|(_, p)| !rd.is_fighter(p.kind())) {
            units.into_iter().filter(|(_, p)| !rd.is_fighter(p.kind())).collect()
        } else {
            units
        }
    }

    /// The decision the settle stopped at, as the string `pending_choice` puts it. `None`: nothing is asked or the
    /// position is one the machine does not carry (a producer-assigned hit).
    #[must_use]
    pub fn pending_choice(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        let env = &self.env;
        let rd = Self::rd(s, k);
        let ships = |seat: Seat, at: System| u16::try_from(ships_of(rd, seat, at).len()).unwrap_or(u16::MAX);
        match &self.fields.step {
            Step::Announcing { asking, .. } => {
                if self.retreats(s, k, *asking).is_empty() {
                    return None;
                }
                let forced = *asking == env.attacker
                    && s.players().by_seat(env.defender).is_some_and(|p| p.card_timers.rout_round.is(s.meta().combat_round_seq));
                let opts: Opts = if forced { [Opt::Combat(COpt::Announce)].into_iter().collect() } else { [Opt::Combat(COpt::Stay), Opt::Combat(COpt::Announce)].into_iter().collect() };
                Some(Choice {
                    actor: *asking,
                    ask: Ask::Combat(CAsk::AnnounceRetreat { system: env.system, forced, ships: ships(*asking, env.system) }),
                    ctx: self.ctx(Rule::R78_9, Subtype::Combat(CSub::AnnounceRetreat), None),
                    opts,
                })
            }
            Step::Retreating { leaving, .. } => {
                let player = *leaving.first()?;
                let destinations = self.retreats(s, k, player);
                if destinations.len() < 2 {
                    return None;
                }
                let opts: Opts = destinations
                    .iter()
                    .map(|d| Opt::Combat(COpt::RetreatTo { system: *d, there: ships(player, *d) }))
                    .collect();
                Some(Choice {
                    actor: player,
                    ask: Ask::Combat(CAsk::RetreatTo { system: env.system, fleet: ships(player, env.system) }),
                    ctx: self.ctx(Rule::R78_7, Subtype::Combat(CSub::RetreatTo), None),
                    opts,
                })
            }
            Step::Queue { assigning: false, queue, .. } => {
                let front = queue.first()?;
                let own = ships(front.player, env.system);
                let ask = CAsk::CancelHit {
                    system: env.system,
                    hits: u16::try_from(front.hits).unwrap_or(u16::MAX),
                    non_fighters_first: front.non_fighters_only && ships_of(rd, front.player, env.system).iter().any(|p| !rd.is_fighter(p.kind())),
                    staging: env.staging,
                    ability: false,
                    in_combat: true,
                    window: Some(own),
                };
                let opts = sustain_opts(rd, front.player, ask);
                if opts.as_slice().is_empty() {
                    return None;
                }
                Some(Choice {
                    actor: front.player,
                    ask: Ask::Combat(ask),
                    ctx: self.ctx(Rule::R82, Subtype::Combat(CSub::SustainDamage), Some(front.hits)),
                    opts,
                })
            }
            Step::Queue { assigning: true, queue, .. } => {
                let front = queue.first()?;
                if front.producer_assigns {
                    return None;
                }
                let candidates = Self::candidates(rd, front, env.system);
                if candidates.len() < 2 {
                    return None;
                }
                let mut seen: Vec<(Unit, bool)> = Vec::new();
                let mut opts = Opts::default();
                for (index, piece) in candidates {
                    let key = (piece.kind(), piece.sustained());
                    if seen.contains(&key) {
                        continue;
                    }
                    seen.push(key);
                    opts.push(Opt::Combat(COpt::Destroy { index, unit: key.0, damaged: key.1 }));
                }
                Some(Choice {
                    actor: front.player,
                    ask: Ask::Combat(CAsk::AssignHit),
                    ctx: self.ctx(Rule::R78_4, Subtype::Combat(CSub::Casualty(env.casualty)), Some(front.hits)),
                    opts,
                })
            }
            _ => None,
        }
    }

    /// The answer to the decision the settle stopped at (string `resolve`, without the settle that follows).
    fn answer_pending(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let choice = self.pending_choice(sx.state, sx.kernel).ok_or(Illegal)?;
        let Pick::Opt(opt) = pick else { return Err(Illegal) };
        if !choice.opts.as_slice().contains(&opt) {
            return Err(Illegal);
        }
        let env_system = self.env.system;
        match self.fields.step.clone() {
            Step::Announcing { round, asking, mut announced } => {
                if opt == Opt::Combat(COpt::Announce) {
                    announced.push(asking);
                    if self.env.announce {
                        let payload = Payload::new()
                            .with(Key::System, Val::System(env_system))
                            .with(Key::Player, Val::Seat(asking))
                            .with(Key::Round, Val::Int(i64::from(round)));
                        let _ = sx.emit(fx::event(EventKind::RetreatDeclared, payload));
                    }
                }
                if asking == self.env.defender && !announced.contains(&self.env.defender) {
                    self.fields.step = Step::Announcing { round, asking: self.env.attacker, announced };
                } else {
                    if !announced.is_empty() {
                        self.fields.pending_retreats = announced;
                    }
                    self.fields.step = Step::Rolling { round };
                }
            }
            Step::Retreating { round, leaving } => {
                let Opt::Combat(COpt::RetreatTo { system, .. }) = opt else { return Err(Illegal) };
                if self.retreat_fleet(sx, round, &leaving, system).is_none() {
                    self.bail();
                    return Ok(());
                }
            }
            Step::Queue { assigning: false, round, mut queue } => {
                let front = *queue.first().ok_or(Illegal)?;
                match opt {
                    Opt::Decline => self.fields.step = Step::Queue { assigning: true, round, queue },
                    Opt::Combat(COpt::Sustain { site, unit, .. }) => {
                        let rd = Self::rd(sx.state, sx.kernel);
                        match apply_sustain(rd, front.player, env_system, site) {
                            None => {
                                // No such unit sustains: nothing happens, but the use still cancels its hits.
                                let cancels = Self::cancels(sx.state, rd, front.player);
                                if let Some(f) = queue.first_mut() {
                                    f.hits = f.hits.saturating_sub(u32::try_from(cancels).unwrap_or(u32::MAX));
                                }
                                self.fields.step = Step::Queue { assigning: false, round, queue };
                            }
                            Some((after, piece)) => {
                                *sx.state = after;
                                super::sustain_offer::remember_target(sx.state, env_system, self.env.staging, site, piece);
                                self.last_sustain = Some((site, piece));
                                let offer = Offer {
                                    player: front.player,
                                    producer: front.producer,
                                    system: env_system,
                                    ability: false,
                                    in_combat: true,
                                    non_fighters_first: false,
                                    source: self.env.commander,
                                    staging: self.env.staging,
                                    announce: self.env.announce,
                                };
                                let commander = sx.state.meta().rules_at_least(super::RULES_FACTION_BUGFIXES).then(|| SustainCommander::new(front.player, self.env.commander));
                                self.sustaining = Some(Sustaining { commander, offer, unit, round, queue });
                                return Ok(());
                            }
                        }
                    }
                    _ => return Err(Illegal),
                }
            }
            Step::Queue { assigning: true, round, mut queue } => {
                let front = *queue.first().ok_or(Illegal)?;
                let Opt::Combat(COpt::Destroy { index, .. }) = opt else { return Err(Illegal) };
                let units = ships_of(Self::rd(sx.state, sx.kernel), front.player, env_system);
                if let Some(doomed) = units.get(usize::from(index)).copied() {
                    remove_ship(sx, env_system, front.player, doomed);
                    announce_loss(sx, env_system, front.player, doomed.kind(), self.env.end_cause(), true, self.env.announce);
                }
                if let Some(f) = queue.first_mut() {
                    f.hits = f.hits.saturating_sub(1);
                }
                self.fields.step = Step::Queue { assigning: false, round, queue };
            }
            _ => return Err(Illegal),
        }
        self.answered = true;
        Ok(())
    }
}

impl Env {
    /// The cause of a ship lost in a combat round (`"space_combat"`).
    #[must_use]
    pub fn end_cause(&self) -> Code {
        self.roll_cause
    }
}
