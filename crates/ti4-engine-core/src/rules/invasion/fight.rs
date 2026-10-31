//! The ground combats of one committed planet, to their end, as one window (lent-bodies lane 9; the string
//! `InvasionWindow::fight_committed_planet`, which the Yin hero uses): every rival is fought while the invader still has a ground force
//! there.
//!
//! For each rival, in the order of the player ids, `GROUND_COMBAT_STARTED` is announced and the combat begins a feat occurrence; then each round is the
//! windows of this area in turn: the start of the round ([`BeginGroundRound`]), the dice ([`GroundRoll`]), their tail with the reroll windows
//! ([`GroundRollTail`]) and the hits with Harrow ([`GroundRoundHits`]); when one side has no ground force left the end is announced
//! (`GROUND_COMBAT_ENDED`: the winner, whether control changes) and the win earns its feats (anomaly, a rival's home system, the leader of the
//! points, a note held). The scoring windows the string opens after a won combat are not here: the adapter declines a run in which the string
//! asked one.
//!
//! Not carried, and declined by the adapter before the window opens: The Thundarian (a step-ended event and its replays), X-89 Bacterial Weapon, Alarum,
//! Proxima Targeting VI, a neutral side, a coexisting planet.

use super::begin::{BeginGroundRound, Facts as BeginFacts};
use super::harrow::Facts as HarrowFacts;
use super::roll::{Facts as RollFacts, GroundRoll};
use super::round::{Facts as RoundFacts, GroundRoundHits};
use super::tail::{Facts as TailFacts, GroundRollTail};
use super::{is_ground_force, Rd};
use crate::choice::{Choice, Pick};
use crate::generated::{Planet, System, Unit};
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Child, Illegal, Ran, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::reach;
use crate::state::{Code, CoreState, EventKind, Feat, InvasionDie, InvasionStep, Key, Payload, Piece, Val};

/// What the adapter vouches for: the same for every combat of the planet.
#[derive(Debug, Clone)]
pub struct FightFacts {
    pub invader: Seat,
    pub system: System,
    pub planet: Planet,
    /// The seats in the order of their player ids (the string picks the first rival of a set of ids).
    pub order: Vec<Seat>,
    /// The resolver carries the timing machinery (the events are announced).
    pub announce: bool,
    /// A copied round agent numbers and announces every round; `numbered`: rules version 16 on.
    pub copied: bool,
    pub numbered: bool,
    /// Per participant (invader, rival): 2 resources can be spent, for the Dunlain Reaper.
    pub can_pay: [bool; 2],
    /// What each seat can spend is read in the round (the payment environment); `None`: `can_pay` stands for every round.
    pub pay_env: Option<crate::kernel::fx_pay::PayEnv>,
    pub dunlain: Code,
    /// The interned texts of the dice: the staging kind `ground`, `combat value N` and the unit types that may roll.
    pub kind: Code,
    pub names: Vec<Code>,
    pub unit_codes: Vec<(crate::generated::Unit, Code)>,
    /// The interned event causes `ground_combat` and `harrow`, and the kind `ground_round` of the step report.
    pub cause: Code,
    pub harrow_cause: Code,
    pub step_kind: Code,
    /// The invader holds the Harrow ability.
    pub harrow: bool,
    /// The seats that hold X-89 Bacterial Weapon (their ground combat hits are doubled).
    pub x89: Vec<Seat>,
    /// The seats that hold Dacxive Animators with the infantry they place ("after you win a ground combat, place 1 infantry from your
    /// reinforcements on that planet"): the unit the seat's buildable infantry is.
    pub dacxive: Vec<(Seat, Unit)>,
    /// The notes held at the start of the tactical action (holder, issuer), for the feat of winning against a note holder.
    pub notes: Vec<(Seat, Seat)>,
}

/// The suspension points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FStage {
    /// The invasion window the string opens for the fight begins a feat occurrence of its own (`InvasionWindow::at_commit_step`).
    Open,
    /// The next rival is picked and the combat announced.
    Pick,
    /// The start of a round.
    Begin,
    /// The dice.
    Roll,
    /// The tail of the roll step.
    Tail,
    /// The hits.
    Hits,
    Done,
}

#[derive(Debug, Clone)]
pub struct CommittedFight {
    pub f: FightFacts,
    stage: FStage,
    defender: Option<Seat>,
    occurrence: u64,
    before: Vec<Piece>,
    rolled: (u32, u32),
    dice: Vec<crate::rules::invasion::tail::Die>,
    begin: Child<BeginGroundRound>,
    roll: Child<GroundRoll>,
    tail: Child<GroundRollTail>,
    hits: Child<GroundRoundHits>,
    /// The combats fought, by the rival.
    pub fought: Vec<Seat>,
    /// A feat of a won combat was earned: the string opens a scoring window for it.
    pub scoring: Vec<u64>,
    /// A pool or a unit the roll window cannot name, or no link to the payment window: the adapter declines the run.
    pub unsupported: bool,
}

impl CommittedFight {
    #[must_use]
    pub fn new(f: FightFacts) -> Self {
        Self {
            f,
            stage: FStage::Open,
            defender: None,
            occurrence: 0,
            before: Vec::new(),
            rolled: (0, 0),
            dice: Vec::new(),
            begin: Child::idle(),
            roll: Child::idle(),
            tail: Child::idle(),
            hits: Child::idle(),
            fought: Vec::new(),
            scoring: Vec::new(),
            unsupported: false,
        }
    }

    fn rd<'a>(s: &'a CoreState, k: &'a Kernel) -> Rd<'a> {
        Rd::new(s, k).with_blanked(reach::blanked(s))
    }

    /// The seats with a ground force on the planet.
    fn owners(&self, sx: &Sx<'_>) -> Vec<Seat> {
        let rd = Self::rd(sx.state, sx.kernel);
        let cell = sx.state.board().planet(self.f.planet);
        self.f.order.iter().copied().filter(|seat| cell.units.as_slice().iter().any(|p| p.owner() == *seat && is_ground_force(rd, p.kind()))).collect()
    }

    fn event(&self, sx: &mut Sx<'_>, kind: EventKind, payload: Payload) {
        if self.f.announce {
            let _ = sx.emit(fx::event(kind, payload));
        }
    }

    fn base_payload(&self, defender: Seat) -> Payload {
        Payload::new()
            .with(Key::System, Val::System(self.f.system))
            .with(Key::Planet, Val::Planet(self.f.planet))
            .with(Key::Attacker, Val::Seat(self.f.invader))
            .with(Key::Defender, Val::Seat(defender))
    }

    /// The next combat of the planet, or the end.
    fn pick(&mut self, sx: &mut Sx<'_>) {
        let owners = self.owners(sx);
        let invader = self.f.invader;
        let Some(defender) = owners.iter().copied().find(|s| *s != invader).filter(|_| owners.contains(&invader)) else {
            self.stage = FStage::Done;
            return;
        };
        self.event(sx, EventKind::GroundCombatStarted, self.base_payload(defender));
        let owners = self.owners(sx);
        if !(owners.contains(&invader) && owners.contains(&defender)) {
            // A handler at the start of the combat removed a side's last ground force: look at the planet again.
            return;
        }
        let meta = sx.state.meta_mut();
        meta.feat_occurrence_seq += 1;
        self.occurrence = meta.feat_occurrence_seq;
        self.defender = Some(defender);
        self.fought.push(defender);
        self.start_round(sx);
    }

    fn start_round(&mut self, sx: &mut Sx<'_>) {
        let Some(defender) = self.defender else { return };
        let f = &self.f;
        let facts = BeginFacts { system: f.system, planet: f.planet, invader: f.invader, defender, copied: f.copied, numbered: f.numbered, can_pay: f.can_pay, pay_env: f.pay_env, dunlain: f.dunlain };
        self.begin.start(BeginGroundRound::new(facts));
        self.stage = FStage::Begin;
        let _ = sx;
    }

    fn roll_facts(&self, defender: Seat) -> RollFacts {
        let f = &self.f;
        RollFacts { attacker: f.invader, defender, system: f.system, planet: f.planet, kind: f.kind, names: f.names.clone(), unit_codes: f.unit_codes.clone(), x89: [f.x89.contains(&f.invader), f.x89.contains(&defender)] }
    }

    /// The report the hits window fills (`end_ground_round`): the units before the dice and the dice as the board shows them.
    fn template(&self) -> InvasionStep {
        let dice = self
            .dice
            .iter()
            .map(|(side, group, face, target, hit)| InvasionDie { planet: self.f.planet, player: *side, group: *group, face: *face, target: *target, hit: *hit })
            .collect();
        InvasionStep { planet: self.f.planet, kind: self.f.step_kind, round: 0, before: self.before.clone(), after: Vec::new(), dice, hits: Default::default(), harrow_hits: 0 }
    }

    /// A round is over: another follows, or the combat ends (`finish_ground_round`).
    fn finish_round(&mut self, sx: &mut Sx<'_>) {
        let Some(defender) = self.defender else { return };
        let invader = self.f.invader;
        let owners = self.owners(sx);
        let (invader_survives, defender_survives) = (owners.contains(&invader), owners.contains(&defender));
        if invader_survives && defender_survives {
            return self.start_round(sx);
        }
        let (winner, loser) = if invader_survives { (invader, defender) } else { (defender, invader) };
        // Dacxive Animators: only a side with forces left has won (both wiped out is nobody's win).
        if owners.contains(&winner)
            && let Some((_, infantry)) = self.f.dacxive.iter().find(|(seat, _)| *seat == winner)
        {
            let scope = crate::tables::Scope(u32::from(sx.kernel.sources.0));
            if crate::query::fleet::remaining_all(sx.state, scope, winner, *infantry) >= 1 {
                fx::place_unit(sx.state, winner, self.f.system, Some(self.f.planet), *infantry);
            }
        }
        let control_changed = winner == invader && owners.contains(&winner) && sx.state.board().planet(self.f.planet).control != Some(invader);
        let noted = self.win_feats(sx, winner, loser);
        if noted {
            self.scoring.push(self.occurrence);
        }
        let rival = if loser == invader { winner } else { loser };
        let mut payload = self.base_payload(rival);
        if owners.contains(&winner) {
            payload.set(Key::Winner, Val::Seat(winner));
        }
        payload.set(Key::ControlChanged, Val::Flag(control_changed));
        self.event(sx, EventKind::GroundCombatEnded, payload);
        self.defender = None;
        self.stage = FStage::Pick;
    }

    /// `note_ground_combat_win_feats`: the feats a won ground combat earns; whether there was one.
    fn win_feats(&self, sx: &mut Sx<'_>, winner: Seat, loser: Seat) -> bool {
        use crate::tables::{Flags, SystemFlags};
        let rd = Self::rd(sx.state, sx.kernel);
        let mut earned: Vec<Feat> = Vec::new();
        let anomaly = rd.scope().system(self.f.system).is_some_and(|d| d.has(SystemFlags::NEBULA) || d.has(SystemFlags::SUPERNOVA) || d.has(SystemFlags::SCAR) || d.has(SystemFlags::ASTEROID_FIELD) || d.has(SystemFlags::GRAVITY_RIFT));
        if anomaly {
            earned.push(Feat::WonInAnAnomaly);
        }
        if rd.s.players().iter().any(|p| p.seat != winner && p.home_system == Some(self.f.system)) {
            earned.push(Feat::WonInARivalHome);
        }
        let most = rd.s.players().iter().map(|p| p.econ.victory_points).max().unwrap_or(0);
        if rd.s.players().by_seat(loser).is_some_and(|p| p.econ.victory_points == most) {
            earned.push(Feat::WonAgainstThePointsLeader);
        }
        if self.f.notes.contains(&(winner, loser)) {
            earned.push(Feat::WonAgainstANoteHolder);
        }
        let noted = !earned.is_empty();
        if let Some(p) = sx.state.players_mut().by_seat_mut(winner) {
            for feat in earned {
                let entry = (feat, self.occurrence);
                if !p.misc.event_feats.as_slice().contains(&entry) {
                    p.misc.event_feats.push(entry);
                    p.misc.event_feats.as_mut_slice().sort_unstable();
                }
            }
        }
        noted
    }
}

impl Window for CommittedFight {
    type Stage = FStage;

    fn stage(&self) -> FStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                FStage::Done => return Next::Done,
                FStage::Open => {
                    sx.state.meta_mut().feat_occurrence_seq += 1;
                    self.stage = FStage::Pick;
                }
                FStage::Pick => self.pick(sx),
                FStage::Begin => match self.begin.step(sx) {
                    Some(Ran::Asks(ask)) => return Next::Continue(ask),
                    Some(Ran::Pass(next)) => return next,
                    Some(Ran::Done(_)) | None => {
                        let Some(defender) = self.defender else { return Next::Done };
                        self.before = sx.state.board().planet(self.f.planet).units.as_slice().to_vec();
                        self.roll.start(GroundRoll::new(self.roll_facts(defender)));
                        self.stage = FStage::Roll;
                    }
                },
                FStage::Roll => match self.roll.step(sx) {
                    Some(Ran::Asks(ask)) => return Next::Continue(ask),
                    Some(Ran::Pass(next)) => return next,
                    Some(Ran::Done(rolled)) => {
                        if rolled.unsupported {
                            self.unsupported = true;
                            self.stage = FStage::Done;
                            continue;
                        }
                        self.rolled = rolled.hits;
                        let Some(defender) = self.defender else { return Next::Done };
                        let f = &self.f;
                        let facts = TailFacts { invader: f.invader, defender, system: f.system, planet: f.planet, rolled: self.rolled, bump: !f.copied, announce: f.announce, x89: [f.x89.contains(&f.invader), f.x89.contains(&defender)] };
                        self.tail.start(GroundRollTail::new(facts));
                        self.stage = FStage::Tail;
                    }
                    None => self.stage = FStage::Done,
                },
                FStage::Tail => match self.tail.step(sx) {
                    Some(Ran::Asks(ask)) => return Next::Continue(ask),
                    Some(Ran::Pass(next)) => return next,
                    Some(Ran::Done(tail)) => {
                        self.rolled = tail.hits;
                        self.dice = tail.dice.clone();
                        let Some(defender) = self.defender else { return Next::Done };
                        let f = &self.f;
                        let harrow = f.harrow.then_some(HarrowFacts { invader: f.invader, defender, system: f.system, planet: f.planet, announce: f.announce, cause: f.harrow_cause });
                        let facts = RoundFacts { invader: f.invader, defender, system: f.system, planet: f.planet, hits: self.rolled, announce: f.announce, cause: f.cause, template: self.template(), harrow };
                        self.hits.start(GroundRoundHits::new(facts));
                        self.stage = FStage::Hits;
                    }
                    None => self.stage = FStage::Done,
                },
                FStage::Hits => match self.hits.step(sx) {
                    Some(Ran::Asks(ask)) => return Next::Continue(ask),
                    Some(Ran::Pass(next)) => return next,
                    Some(Ran::Done(_)) | None => self.finish_round(sx),
                },
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        match self.stage {
            FStage::Begin => self.begin.ask_of(s, k),
            FStage::Roll => self.roll.ask_of(s, k),
            FStage::Tail => self.tail.ask_of(s, k),
            FStage::Hits => self.hits.ask_of(s, k),
            FStage::Open | FStage::Pick | FStage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match self.stage {
            FStage::Begin => self.begin.answer(sx, pick),
            FStage::Roll => self.roll.answer(sx, pick),
            FStage::Tail => self.tail.answer(sx, pick),
            FStage::Hits => self.hits.answer(sx, pick),
            FStage::Open | FStage::Pick | FStage::Done => Err(Illegal),
        }
    }
}
