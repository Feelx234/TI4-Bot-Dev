//! The stage operations of the invasion machine up to the end of the commit step: the opening, the bombardment, the custodians offer, the
//! landings and the public boundary. The ground rounds, the advance and the control gain are `fight.rs`.

use super::InvasionMachine;
use crate::kernel::ability::Unported as Why;
use crate::choice::invasion::Opt as IOpt;
use crate::choice::{Choice, Opt, Pick};
use crate::containers::{Cow, SeatSet, SortedMap};
use crate::generated::{Planet, Technology};
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::economy::Spend;
use crate::query::reach;
use crate::kernel::commander_walk::Walk;
use crate::rules::invasion::bombard::{Bombardment, Cursor, Entry};
use crate::rules::invasion::stages::{self, Custodians, FightRound, NextCombat};
use crate::rules::invasion::{
    bombard_plan, commit::LandOne, custodians_cost, custodians_gate, is_ground_force, landable, roll_bombard, Rd, Troop,
};
use crate::rules::invasion::commit::CommitGround;
use crate::rules::production::pay::PayBill;
use crate::state::{
    ActiveInvasion, CoreState, EventKind, IBill, IBombardCursor, ICustodians, IPlanetBombard, IRound, IStage, IWalk, InvasionDie, InvasionStep, Key, Payload,
    Piece, RerollEntry, RerollSet, RollPurpose, RollRec, Val,
};

/// `RULES_FACTION_EFFECTS`.
pub(super) const RULES_FACTION_EFFECTS: u32 = 16;

pub(super) fn rd<'a>(s: &'a CoreState, k: &'a Kernel) -> Rd<'a> {
    Rd::new(s, k).with_blanked(reach::blanked(s))
}

/// Whether `who` can pay `cost` of `kind` (`production::available(..) >= cost`).
pub(super) fn can_pay(s: &CoreState, k: &Kernel, who: Seat, kind: Spend, cost: i64) -> bool {
    use crate::rules::production::{available, Cx, Held};
    let cx = Cx::new(s, k, crate::tables::Scope(u32::from(k.sources.0)), reach::blanked(s));
    available(&cx, who, kind, &Held::of(&cx, who).extras()) >= cost
}

/// What a bill did when it was stepped.
pub(super) enum Billed {
    Asks(i64),
    Paid(bool),
}

/// Step a bill to its next real question or its end.
pub(super) fn run_bill(sx: &mut Sx<'_>, bill: &mut PayBill) -> Billed {
    match bill.step(sx) {
        Next::Continue(_) => Billed::Asks(bill.paid()),
        _ => Billed::Paid(bill.outcome == Some(true)),
    }
}

impl InvasionMachine {
    // -- the questions --------------------------------------------------------------------------------------------------

    /// The pool the commit question is asked over (`commit_pool`): the ground forces in the space area, then the ships committed as ground
    /// forces for the moment, then a faction's candidates with their origin.
    pub(super) fn commit_pool(&self, s: &CoreState, k: &Kernel) -> Vec<Troop> {
        let (invader, system) = (self.p.invader, self.p.system);
        let r = rd(s, k);
        let mut troops: Vec<Troop> = landable(r, invader, system).into_iter().map(Troop::space).collect();
        for c in k.hooks.ground.temporary_space_commit_candidates(s, k, invader, system).iter() {
            let real = c.owner() == invader && r.data(c.kind()).is_some_and(|d| d.ship) && s.board().sys(system).units.iter().any(|u| u == c);
            if real {
                troops.push(Troop { piece: *c, origin: None, temporary: true });
            }
        }
        for c in k.hooks.ground.commit_candidates(s, k, invader, system, &self.p.extra_commits).iter() {
            let real = c.unit.owner() == invader && is_ground_force(r, c.unit.kind()) && s.board().planet(c.planet).units.iter().any(|u| *u == c.unit);
            if real {
                troops.push(Troop { piece: c.unit, origin: Some((c.system, c.planet)), temporary: false });
            }
        }
        troops
    }

    /// The question of the stage, with the invasion's sequence number in its context (the string's `about_invasion`: a question about this system).
    pub(super) fn question(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        let mut choice = self.stage_question(s, k)?;
        if let Some(active) = s.transient().invasion.active.as_ref()
            && !choice.ctx.space_battle
            && match choice.ctx.target {
                Some(crate::choice::Target::System(t) | crate::choice::Target::Planet { system: t, .. }) => t == self.p.system,
                _ => true,
            }
        {
            choice.ctx.invasion_seq = Some(active.seq);
        }
        Some(choice)
    }

    fn stage_question(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        let (invader, system) = (self.p.invader, self.p.system);
        match &self.p.stage {
            IStage::Bombarding if self.p.bombard.walk.is_some() => self.walk_of(s)?.ask(s),
            IStage::Bombarding if self.p.bombard.cursor.is_some() => self.bomb_window(s, true).ask_of(s, k),
            IStage::Custodians(ICustodians::Asking) => Custodians::at_question(invader, system).ask_of(s, k),
            IStage::Custodians(ICustodians::Paying(b)) => PayBill::resume(b.who, spend(b), i64::from(b.cost), i64::from(b.paid)).ask_of(s, k),
            IStage::Committing => {
                let pool = self.commit_pool(s, k);
                (!pool.is_empty()).then(|| CommitGround::new(invader, system).question_with(s, k, Some(&pool), &[]))
            }
            IStage::Fighting { planets, index, round, defender } => {
                let planet = planets.get(usize::from(*index)).copied()?;
                match round {
                    IRound::Asking => Some(FightRound::new(invader, system, planet).question()),
                    IRound::Reaper { at, bill: None, asked: true, source } => {
                        let who = if *at == 0 { invader } else { *defender };
                        Some(crate::rules::invasion::begin::deploy_question(s, system, planet, who, *source))
                    }
                    IRound::Reaper { bill: Some(b), .. } => PayBill::resume(b.who, spend(b), i64::from(b.cost), i64::from(b.paid)).ask_of(s, k),
                    IRound::Reaper { .. } => None,
                }
            }
            IStage::ChoosingNextCombat { planet, remaining, .. } => {
                let next = remaining.first().copied()?;
                Some(NextCombat::new(invader, system, *planet, next).question())
            }
            _ => None,
        }
    }

    // -- the opening ----------------------------------------------------------------------------------------------------

    /// `AftermathWindow::next_step`'s invasion branch and `InvasionWindow::new_with_notes`: the public boundary, `INVASION_BEGAN`, the rolled
    /// bombardment.
    pub(super) fn begin(&mut self, sx: &mut Sx<'_>) -> Result<(), Why> {
        let (invader, system) = (self.p.invader, self.p.system);
        let codes = sx.table.codes.invasion;
        let seq = u64::from(sx.state.meta().activation_seq);
        sx.state.transient_mut().invasion.active = Some(Cow::new(ActiveInvasion {
            system,
            invader,
            seq,
            phase: codes.phases[0],
            planet: None,
            defender: None,
            ground_round: 0,
            last_step: None,
        }));
        // Two cards read "at the start of an invasion": the window opens before the invasion does.
        let payload = Payload::new().with(Key::Player, Val::Seat(invader)).with(Key::System, Val::System(system));
        let _ = sx.emit(fx::event(EventKind::InvasionBegan, payload));
        let meta = sx.state.meta_mut();
        meta.feat_occurrence_seq += 1;
        self.p.bombard.occurrence = meta.feat_occurrence_seq;
        // A stale set left by a synchronous API can only name rolls that are not this bombardment's.
        let combat = &mut sx.state.transient_mut().combat;
        combat.reroll_staging.edit().clear();
        combat.last_reroll_player = None;
        let planned = bombard_plan(rd(sx.state, sx.kernel), system, invader);
        let dice = roll_bombard(&planned, sx.state.rng_mut(), sx.seeds);
        let multiplier = x89_multiplier(sx.state, invader);
        let mut staged: Vec<RerollEntry> = Vec::new();
        let mut at = 0_usize;
        for entry in &planned {
            let mut groups: Vec<u32> = Vec::new();
            for _ in &entry.rolls {
                let Some(d) = dice.get(at) else { return Err(Why::BombardmentDiceMismatch) };
                at += 1;
                let hits = u32::try_from(d.faces.iter().filter(|f| **f >= d.hits_on).count()).unwrap_or(u32::MAX) * multiplier;
                if hits > 0 {
                    groups.push(hits);
                }
                let faces: Vec<u8> = d.faces.iter().map(|f| u8::try_from(*f).unwrap_or(u8::MAX)).collect();
                sx.state.ledger_mut().dice.push(RollRec {
                    purpose: RollPurpose::Bombardment,
                    owner: Some(invader),
                    unit: Some(d.unit),
                    planet: Some(d.planet),
                    scene: Some(system),
                    hits_on: Some(u8::try_from(d.hits_on).unwrap_or(u8::MAX)),
                    faces: faces.iter().copied().collect(),
                    rerolled: 0,
                });
                let unit = codes.unit(d.unit);
                let mut unit_types = SortedMap::<crate::state::Code, u32, 4>::default();
                *unit_types.get_or_insert_with(unit, || 0) += 1;
                staged.push(RerollEntry {
                    unit,
                    planet: Some(d.planet),
                    hits_on: Some(u8::try_from(d.hits_on).unwrap_or(u8::MAX)),
                    faces,
                    rerolled: 0,
                    deltas: SortedMap::default(),
                    unit_types,
                });
            }
            let before: Vec<Piece> = sx.state.board().planet(entry.planet).units.iter().copied().collect();
            self.p.bombard.plan.push(IPlanetBombard { planet: entry.planet, before, dice: Vec::new(), groups, held: u16::try_from(entry.held).unwrap_or(u16::MAX), victims: entry.victims });
        }
        if staged.iter().any(|r| !r.faces.is_empty()) {
            let combat = &mut sx.state.transient_mut().combat;
            let staging = combat.reroll_staging.edit();
            staging.retain(|(s, _)| *s != invader);
            let at = staging.partition_point(|(s, _)| *s < invader);
            staging.insert(at, (invader, RerollSet { kind: codes.bombardment, system, rolls: staged }));
            combat.last_reroll_player = Some(invader);
        }
        self.p.stage = IStage::Bombarding;
        Ok(())
    }

    /// Whether the invader holds the Jol-Nar commander (Aglnlan Oln unlocked): it may reroll any of the dice of a unit ability.
    fn holds_commander(s: &CoreState, who: Seat) -> bool {
        s.players().by_seat(who).is_some_and(|p| p.leaders.status.get(&crate::generated::Leader::AgnlanOln) == Some(&crate::state::LeaderStatus::Unlocked))
    }

    /// The commander's walk at the position the payload keeps (`None`: no walk is open or the dice are gone).
    fn walk_of(&self, s: &CoreState) -> Option<Walk> {
        let w = self.p.bombard.walk.as_ref()?;
        Walk::resume(s, self.p.invader, &w.shown, usize::from(w.at), &w.picks)
    }

    /// The label of each staged entry in the question renderer's table: the entry's unit text is the state's text of its unit type.
    fn shown(s: &CoreState, invader: Seat, codes: &crate::kernel::ability::InvasionCodes) -> Vec<crate::state::Code> {
        let Some((_, set)) = s.transient().combat.reroll_staging.iter().find(|(who, _)| *who == invader) else { return Vec::new() };
        set.rolls
            .iter()
            .map(|e| codes.units.iter().position(|c| *c == e.unit).map(|i| codes.shown_units[i]).filter(|c| *c != crate::state::Code(0)).unwrap_or(e.unit))
            .collect()
    }

    /// The walk begins when the invader holds the commander and rolled a die (`reroll_window`: `has_dice`). `true`: a question is open.
    fn begin_walk(&mut self, sx: &mut Sx<'_>) -> bool {
        if !Self::holds_commander(sx.state, self.p.invader) || Walk::start(sx.state, self.p.invader, &[]).is_none() {
            return false;
        }
        self.p.bombard.walk = Some(IWalk { shown: Self::shown(sx.state, self.p.invader, &sx.table.codes.invasion), ..IWalk::default() });
        true
    }

    /// The answer to the commander's question about one die; the last one draws the picked dice again and closes the walk.
    fn answer_walk(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let mut walk = self.walk_of(sx.state).ok_or(Illegal)?;
        walk.answer(sx.state, pick)?;
        if walk.done() {
            walk.redraw(sx);
            self.p.bombard.walk = None;
        } else {
            let shown = self.p.bombard.walk.take().map(|w| w.shown).unwrap_or_default();
            self.p.bombard.walk = Some(IWalk { at: u16::try_from(walk.at()).unwrap_or(u16::MAX), picks: walk.picks().to_vec(), shown });
        }
        Ok(())
    }

    /// The one-shot windows after the bombardment rolls (`announce_bombard_rerolls`): the reaction windows of `UNIT_ABILITY_ROLLED` (Scramble
    /// Frequency), then every planet's groups are recomputed from the possibly rerolled dice and the staging is spent.
    pub(super) fn announce_rerolls(&mut self, sx: &mut Sx<'_>) -> Result<(), Why> {
        let invader = self.p.invader;
        let codes = sx.table.codes.invasion;
        let has = |s: &CoreState| s.transient().combat.reroll_staging.iter().any(|(who, _)| *who == invader);
        if !has(sx.state) {
            return Ok(());
        }
        let (system, hits) = {
            let set = sx.state.transient().combat.reroll_staging.iter().find(|(who, _)| *who == invader).map(|(_, set)| set).ok_or(Why::BombardmentDiceMismatch)?;
            (set.system, staged_hits(set))
        };
        let payload = Payload::new()
            .with(Key::Kind, Val::Code(codes.bombardment_cause))
            .with(Key::Player, Val::Seat(invader))
            .with(Key::System, Val::System(system))
            .with(Key::Hits, Val::Int(i64::from(hits)));
        let _ = sx.emit(fx::event(EventKind::UnitAbilityRolled, payload));
        let multiplier = x89_multiplier(sx.state, invader);
        let set = sx.state.transient().combat.reroll_staging.iter().find(|(who, _)| *who == invader).map(|(_, set)| set.clone());
        if let Some(set) = set {
            for entry in &mut self.p.bombard.plan {
                entry.dice = set
                    .rolls
                    .iter()
                    .filter(|roll| roll.planet == Some(entry.planet))
                    .flat_map(|roll| {
                        let target = roll.hits_on.map_or(u32::MAX, u32::from);
                        roll.faces.iter().enumerate().map(move |(i, face)| (roll, target, i, *face))
                    })
                    .map(|(roll, target, i, face)| {
                        let delta = i64::from(roll.deltas.get(&u8::try_from(i).unwrap_or(u8::MAX)).copied().unwrap_or(0));
                        InvasionDie { planet: entry.planet, player: invader, group: roll.unit, face: u32::from(face), target, hit: i64::from(face) + delta >= i64::from(target) }
                    })
                    .collect();
                entry.groups = set
                    .rolls
                    .iter()
                    .filter(|roll| roll.planet == Some(entry.planet))
                    .map(|roll| entry_hits(roll) * multiplier)
                    .filter(|hits| *hits > 0)
                    .collect();
            }
        }
        let combat = &mut sx.state.transient_mut().combat;
        combat.reroll_staging.edit().retain(|(who, _)| *who != invader);
        combat.last_reroll_player = None;
        let _ = codes;
        Ok(())
    }

    // -- the bombardment ------------------------------------------------------------------------------------------------

    /// The core bombardment window at the payload's position: in `Bombarding` (or `Choosing` at the pause), or about to be answered.
    pub(super) fn bomb_window(&self, s: &CoreState, asking: bool) -> Bombardment {
        let b = &self.p.bombard;
        let plan = b
            .plan
            .iter()
            .map(|e| {
                let hits: u32 = e.groups.iter().sum();
                let mut counts = SortedMap::<Seat, u32, 4>::default();
                counts.insert(self.p.invader, hits);
                Entry {
                    planet: e.planet,
                    template: InvasionStep {
                        planet: e.planet,
                        kind: crate::state::Code(0),
                        round: 0,
                        before: e.before.clone(),
                        after: Vec::new(),
                        dice: e.dice.clone(),
                        hits: counts,
                        harrow_hits: 0,
                    },
                    groups: e.groups.iter().map(|g| usize::try_from(*g).unwrap_or(usize::MAX)).collect(),
                    held: usize::from(e.held),
                    victims: e.victims,
                }
            })
            .collect();
        let _ = s;
        let mut w = Bombardment::new(self.p.invader, self.p.system, b.occurrence, true, crate::state::Code(0), plan, usize::from(b.index));
        if let Some(c) = &b.cursor {
            w = w.at_cursor(
                Cursor {
                    planet_index: usize::from(c.planet_index),
                    planet: c.planet,
                    groups: c.groups.iter().map(|g| usize::try_from(*g).unwrap_or(usize::MAX)).collect(),
                    next_group: usize::from(c.next_group),
                    taken: usize::from(c.taken),
                    held: usize::from(c.held),
                    victims: c.victims,
                },
                asking,
            );
        }
        w
    }

    /// The window with the interned codes the table gives (the step kind and cause `bombardment`).
    fn bomb_window_with(&self, sx: &Sx<'_>, asking: bool) -> Bombardment {
        let code = sx.table.codes.invasion.bombardment;
        let mut w = self.bomb_window(sx.state, asking);
        w.cause = sx.table.codes.invasion.bombardment_cause;
        for e in &mut w.plan {
            e.template.kind = code;
        }
        w
    }

    /// Read the window back into the payload: where it stopped, what it killed, what it queued.
    fn sync_bombard(&mut self, w: &Bombardment) {
        self.p.bombard.index = u8::try_from(w.index).unwrap_or(u8::MAX);
        self.p.bombard.cursor = w.cursor.as_ref().filter(|_| w.paused()).map(|c| IBombardCursor {
            planet_index: u8::try_from(c.planet_index).unwrap_or(u8::MAX),
            planet: c.planet,
            groups: c.groups.iter().map(|g| u32::try_from(*g).unwrap_or(u32::MAX)).collect(),
            next_group: u8::try_from(c.next_group).unwrap_or(u8::MAX),
            taken: u16::try_from(c.taken).unwrap_or(u16::MAX),
            held: u16::try_from(c.held).unwrap_or(u16::MAX),
            victims: c.victims,
        });
        self.p.report.bombardment_kills = self.p.report.bombardment_kills.saturating_add(u16::try_from(w.kills).unwrap_or(u16::MAX));
        self.p.scoring.extend(w.scoring.iter().copied().map(|(o, c)| (o, c)));
    }

    /// The `Bombarding`/`ChoosingBombardment` arms of `settle`.
    pub(super) fn bombard_settle(&mut self, sx: &mut Sx<'_>) {
        let mut w = self.bomb_window_with(sx, false);
        let _ = w.step(sx);
        self.sync_bombard(&w);
        if !w.paused() {
            self.p.bombard.cursor = None;
            self.p.stage = IStage::Custodians(ICustodians::Check);
        }
    }

    /// `answer_bombardment_target`: the invader names whose ground forces take the next group of hits.
    fn answer_bombard(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let mut w = self.bomb_window_with(sx, true);
        w.answer(sx, pick)?;
        self.sync_bombard(&w);
        // The answer is not settled: the machine's next chunk finishes or pauses the bombardment (the string's next `settle`).
        if matches!(w.string_stage(), crate::rules::invasion::bombard::Where::Bombarding) {
            self.p.bombard.cursor = None;
        }
        Ok(())
    }

    // -- the custodians offer -------------------------------------------------------------------------------------------

    /// `custodians_removable`: the token is still there, this is Mecatol Rex, a ground force can land, and the invader can pay (or need not).
    pub(super) fn custodians_removable(&self, s: &CoreState, k: &Kernel) -> bool {
        let r = rd(s, k);
        let cost = custodians_cost(r, self.p.invader);
        custodians_gate(r, self.p.invader, self.p.system) && (cost == 0 || can_pay(s, k, self.p.invader, Spend::Influence, cost))
    }

    fn answer_custodians(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let mut c = Custodians::at_question(self.p.invader, self.p.system);
        c.answer(sx, pick)?;
        if c.removed {
            self.p.report.custodians_removed = true;
            self.p.stage = IStage::Committing;
        } else if let Some(cost) = c.owes {
            // 27.3: pay six influence, take the token, gain a victory point.
            let mut bill = PayBill::new(self.p.invader, Spend::Influence, cost);
            self.after_custodians_bill(sx, &mut bill);
        } else {
            self.p.stage = IStage::Committing;
        }
        Ok(())
    }

    fn after_custodians_bill(&mut self, sx: &mut Sx<'_>, bill: &mut PayBill) {
        match run_bill(sx, bill) {
            Billed::Asks(paid) => {
                self.p.stage = IStage::Custodians(ICustodians::Paying(IBill { who: self.p.invader, influence: true, cost: u16::try_from(bill.cost).unwrap_or(u16::MAX), paid: u16::try_from(paid).unwrap_or(u16::MAX) }));
            }
            Billed::Paid(paid) => {
                if paid {
                    stages::lift(sx.state, self.p.invader);
                    self.p.report.custodians_removed = true;
                }
                self.p.stage = IStage::Committing;
            }
        }
    }

    // -- committing -----------------------------------------------------------------------------------------------------

    fn answer_commit(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match pick {
            Pick::Opt(Opt::Invasion(IOpt::DoneCommitting)) => {
                self.finish_committing(sx);
                return Ok(());
            }
            Pick::Opt(Opt::Invasion(IOpt::Commit { .. })) => {}
            _ => return Err(Illegal),
        }
        let pool = self.commit_pool(sx.state, sx.kernel);
        let mut w = LandOne::new(self.p.invader, self.p.system, true).with_pool(pool);
        let _ = w.step(sx);
        w.answer(sx, pick)?;
        if w.unsupported {
            return {
                crate::driver::windows::invasion::InvasionMachine::bail_flag(sx, Why::ShipFormFlip);
                Ok(())
            };
        }
        if let Some(t) = w.landed {
            if let Pick::Opt(Opt::Invasion(IOpt::Commit { planet, .. })) = pick {
                if t.temporary {
                    match self.p.temp_commits.iter_mut().find(|(p, _)| *p == planet) {
                        Some((_, v)) => v.push(t.piece),
                        None => self.p.temp_commits.push((planet, vec![t.piece])),
                    }
                }
                if let Some(origin) = t.origin {
                    self.p.extra_commits.push(origin);
                }
                if !self.p.report.committed.contains(&planet) {
                    self.p.report.committed.push(planet);
                }
            }
        }
        // Committing settles when nothing is left to land.
        if matches!(self.p.stage, IStage::Committing) && self.commit_pool(sx.state, sx.kernel).is_empty() {
            self.finish_committing(sx);
        }
        Ok(())
    }

    // -- answers --------------------------------------------------------------------------------------------------------

    pub(super) fn resolve(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match self.p.stage.clone() {
            IStage::Bombarding if self.p.bombard.walk.is_some() => self.answer_walk(sx, pick),
            IStage::Bombarding if self.p.bombard.cursor.is_some() => self.answer_bombard(sx, pick),
            IStage::Custodians(ICustodians::Asking) => self.answer_custodians(sx, pick),
            IStage::Custodians(ICustodians::Paying(b)) => {
                let mut bill = PayBill::resume(b.who, spend(&b), i64::from(b.cost), i64::from(b.paid));
                bill.answer(sx, pick)?;
                self.after_custodians_bill(sx, &mut bill);
                Ok(())
            }
            IStage::Committing => self.answer_commit(sx, pick),
            IStage::Fighting { .. } => self.answer_fight(sx, pick),
            IStage::ChoosingNextCombat { .. } => self.answer_next_combat(sx, pick),
            _ => Err(Illegal),
        }
    }

    // -- the settle chunk -----------------------------------------------------------------------------------------------

    /// `InvasionWindow::settle`: one chunk. `Err(reason)`: a position the native invasion does not run.
    pub(super) fn settle(&mut self, sx: &mut Sx<'_>) -> Result<(), Why> {
        loop {
            match self.p.stage.clone() {
                IStage::Begin => self.begin(sx)?,
                IStage::Bombarding => {
                    if self.p.bombard.cursor.is_none() && !self.p.bombard.announced {
                        // The invader's Jol-Nar commander is asked about each die before the event goes out.
                        if !self.p.bombard.walked {
                            self.p.bombard.walked = true;
                            if self.begin_walk(sx) {
                                return Ok(());
                            }
                        }
                        if self.p.bombard.walk.is_some() {
                            return Ok(());
                        }
                        self.p.bombard.announced = true;
                        self.announce_rerolls(sx)?;
                    }
                    self.bombard_settle(sx);
                    // A pause waits for the invader's choice.
                    if self.p.bombard.cursor.is_some() {
                        return Ok(());
                    }
                }
                IStage::Advancing { planets, index } => {
                    self.advance_fighting(sx, &planets, usize::from(index))?;
                    return Ok(());
                }
                IStage::ChoosingNextCombat { planets, index, remaining, .. } if remaining.is_empty() => {
                    self.p.stage = IStage::Advancing { planets, index: index.saturating_add(1) };
                }
                IStage::FinalizingControl { planets, index, planet, previous } => {
                    self.finish_control_gain(sx, planet, previous)?;
                    self.advance_control(sx, &planets, usize::from(index))?;
                    return Ok(());
                }
                IStage::Custodians(ICustodians::Check | ICustodians::Asking) => {
                    if self.custodians_removable(sx.state, sx.kernel) {
                        self.p.stage = IStage::Custodians(ICustodians::Asking);
                        return Ok(());
                    }
                    self.p.stage = IStage::Committing;
                }
                IStage::Committing if self.commit_pool(sx.state, sx.kernel).is_empty() => {
                    self.add_forced_combats(sx);
                    let planets = self.p.report.committed.clone();
                    if planets.is_empty() {
                        self.p.stage = IStage::Done;
                    } else {
                        self.advance_fighting(sx, &planets, 0)?;
                    }
                    return Ok(());
                }
                _ => return Ok(()),
            }
        }
    }

    // -- the public boundary --------------------------------------------------------------------------------------------

    /// `update_public_boundary`: the phase, the planet and the defender the UI reads. The step records the leaf windows wrote stay.
    pub(super) fn publish(&self, sx: &mut Sx<'_>) {
        let phases = sx.table.codes.invasion.phases;
        let Some(active) = sx.state.transient_mut().invasion.active.as_mut() else { return };
        let (phase, planet, defender): (usize, Option<Planet>, Option<Seat>) = match &self.p.stage {
            IStage::Begin | IStage::Bombarding => {
                let b = &self.p.bombard;
                (0, b.cursor.as_ref().map(|c| c.planet).or_else(|| b.plan.get(usize::from(b.index)).map(|e| e.planet)), None)
            }
            IStage::Custodians(_) => (1, None, None),
            IStage::Committing => (2, None, None),
            IStage::Fighting { planets, index, defender, .. } => (3, planets.get(usize::from(*index)).copied(), Some(*defender)),
            IStage::Advancing { planets, index } => (4, planets.get(usize::from(*index)).copied(), None),
            IStage::ChoosingNextCombat { planet, .. } => (5, Some(*planet), None),
            IStage::FinalizingControl { planet, .. } => (6, Some(*planet), None),
            IStage::Done => (7, None, None),
        };
        let active = active.edit();
        if active.planet != planet || active.defender != defender {
            active.ground_round = 0;
        }
        if active.last_step.as_ref().is_some_and(|step| Some(step.planet) != planet) && planet.is_some() {
            active.last_step = None;
        }
        active.phase = phases[phase];
        active.planet = planet;
        active.defender = defender;
    }

    // -- the helpers the other files use ---------------------------------------------------------------------------------

    pub(super) fn bail_flag(sx: &mut Sx<'_>, why: Why) {
        if sx.tape.unknown.get().is_none() {
            sx.tape.unknown.set(Some(crate::kernel::ability::Unknown::Unported(why)));
        }
    }

    /// The seats with a ground force on `planet` here.
    pub(super) fn owners(&self, s: &CoreState, k: &Kernel, planet: Planet) -> SeatSet {
        let r = rd(s, k);
        let mut set = SeatSet::EMPTY;
        for u in s.board().planet(planet).units.iter() {
            if crate::rules::invasion::is_ground_force_here(r, u.owner(), self.p.system, planet, *u) {
                set.insert(u.owner());
            }
        }
        set
    }
}

fn spend(b: &IBill) -> Spend {
    if b.influence { Spend::Influence } else { Spend::Resources }
}

/// `X-89 Bacterial Weapon` doubles the hits of the seat's bombardment.
pub(super) fn x89_multiplier(s: &CoreState, who: Seat) -> u32 {
    if s.players().by_seat(who).is_some_and(|p| p.tech.owned.contains(Technology::X89BacterialWeapon)) { 2 } else { 1 }
}

/// `RerollEntry::hits`: a die hits when its face plus its delta reaches the number.
pub(super) fn entry_hits(e: &RerollEntry) -> u32 {
    let Some(on) = e.hits_on else { return 0 };
    let n = e
        .faces
        .iter()
        .enumerate()
        .filter(|(i, face)| i64::from(**face) + e.deltas.get(&u8::try_from(*i).unwrap_or(u8::MAX)).map_or(0, |d| i64::from(*d)) >= i64::from(on))
        .count();
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// `combat::staged_hits`.
pub(super) fn staged_hits(set: &RerollSet) -> u32 {
    set.rolls.iter().map(entry_hits).sum()
}
