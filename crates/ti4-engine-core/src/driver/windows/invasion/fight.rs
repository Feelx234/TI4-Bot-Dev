//! The tail of the invasion machine: the end of the commit step (the space cannon defense, the forced combats), the ground rounds, the choice of the
//! next fight, the control gain and its chain. The string counterparts are `finish_committing`, `announce_and_defend`, `add_forced_combats`,
//! `begin_ground_round`, `roll_ground_step`, `ground_round_hits`, `finish_ground_round`, `advance_fighting`, `advance_control`, `finish_control_gain`.

use super::ops::{can_pay, rd, run_bill, Billed, RULES_FACTION_EFFECTS};
use super::InvasionMachine;
use crate::kernel::ability::Unported as Why;
use crate::choice::combat::Opt as COpt;
use crate::choice::invasion::Opt as IOpt;
use crate::choice::{Opt, Pick};
use crate::containers::SortedMap;
use crate::generated::{Leader, Planet, Technology, Unit};
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::economy::Spend;
use crate::query::{board as qboard, reach};
use crate::rules::explore_windows::Exploring;
use crate::rules::invasion::advance::{Advance, Facts as AdvanceFacts, Upcoming};
use crate::rules::invasion::begin::{deploy_question, reaper_offered, reaper_replace, REAPER_COST};
use crate::rules::invasion::control::{ControlGained, Facts as ControlFacts};
use crate::rules::invasion::defense::{Facts as DefenseFacts, SpaceCannonDefense};
use crate::rules::invasion::harrow::Facts as HarrowFacts;
use crate::rules::invasion::roll::{Facts as RollFacts, GroundRoll};
use crate::rules::invasion::round::{Facts as RoundFacts, GroundRoundHits};
use crate::rules::invasion::tail::{Facts as TailFacts, GroundRollTail};
use crate::rules::invasion::is_ground_force;
use crate::rules::production::pay::PayBill;
use crate::state::{CoreState, EDeck, ETarget, EventKind, ExploreRun, Feat, IBill, IRound, IStage, InvasionDie, InvasionStep, Key, Payload, Piece, Val};
use crate::tables::{Flags, SystemFlags};

type Bail = Why;

impl InvasionMachine {
    fn event(sx: &mut Sx<'_>, kind: EventKind, payload: Payload) {
        let _ = sx.emit(fx::event(kind, payload));
    }

    fn base(&self, planet: Planet, defender: Seat) -> Payload {
        Payload::new()
            .with(Key::System, Val::System(self.p.system))
            .with(Key::Planet, Val::Planet(planet))
            .with(Key::Attacker, Val::Seat(self.p.invader))
            .with(Key::Defender, Val::Seat(defender))
    }

    fn or_bail(sx: &mut Sx<'_>, r: Result<(), Bail>) {
        if let Err(why) = r {
            Self::bail_flag(sx, why);
        }
    }

    // -- the end of the commit step -------------------------------------------------------------------------------------

    /// `finish_committing`: the announcement and the space cannon defense of every committed planet, the forced combats, then the first fight.
    pub(super) fn finish_committing(&mut self, sx: &mut Sx<'_>) {
        let planets = self.p.report.committed.clone();
        let r = self.defend(sx, &planets).and_then(|()| {
            self.add_forced_combats(sx);
            let planets = self.p.report.committed.clone();
            if planets.is_empty() {
                self.p.stage = IStage::Done;
                Ok(())
            } else {
                self.advance_fighting(sx, &planets, 0)
            }
        });
        Self::or_bail(sx, r);
    }

    /// `announce_and_defend`: `GROUND_COMMITMENT_FINISHED`, then the guns of each committed planet roll at the invader's ground forces.
    fn defend(&mut self, sx: &mut Sx<'_>, planets: &[Planet]) -> Result<(), Bail> {
        let (invader, system) = (self.p.invader, self.p.system);
        let codes = sx.table.codes.invasion;
        let f = DefenseFacts {
            invader,
            system,
            planets: planets.to_vec(),
            announce: true,
            cause: codes.space_cannon_cause,
            kind: codes.space_cannon_defense,
            planets_key: codes.planets,
            unit_codes: Unit::ALL.iter().map(|u| (*u, codes.unit(*u))).collect(),
        };
        let mut w = SpaceCannonDefense::new(f);
        let _ = w.step(sx);
        Ok(())
    }

    /// `add_forced_combats`: the planets a faction rule forces the invader to fight on (Titans' Coalescence), the window of lane 12. Its question
    /// (Slumberstate Computing, a Thunder's Edge breakthrough) is out of the agreed content: a counted bail.
    pub(super) fn add_forced_combats(&mut self, sx: &mut Sx<'_>) {
        let mut w = crate::rules::invasion::forced::ForcedCombats::new(self.p.invader, self.p.system, self.p.report.committed.clone());
        if matches!(w.step(sx), crate::kernel::Next::Continue(_)) {
            Self::bail_flag(sx, Why::Slumberstate);
            return;
        }
        for planet in w.added {
            if !self.p.report.committed.contains(&planet) {
                self.p.report.committed.push(planet);
            }
        }
    }

    // -- the ground round -----------------------------------------------------------------------------------------------

    fn copied_round(s: &CoreState) -> bool {
        let status = |p: &crate::state::Player, card: Leader| p.leaders.status.get(&card).copied();
        let ready = Some(crate::state::LeaderStatus::Readied);
        let owner = s.meta().rules_at_least(RULES_FACTION_EFFECTS)
            && s.players().iter().any(|p| p.faction == crate::generated::Faction::TheFederationOfSol && status(p, Leader::EvelynDeLouis) == ready);
        owner
            || s.players().iter().any(|b| {
                b.faction == crate::generated::Faction::TheYssarilTribes
                    && status(b, Leader::Ssruu) == ready
                    && s.players().iter().any(|l| l.seat != b.seat && matches!(status(l, Leader::EvelynDeLouis), Some(crate::state::LeaderStatus::Readied | crate::state::LeaderStatus::Exhausted)))
            })
    }

    /// What the leaf windows of the round do not carry (the gates of the string adapters, read from the position).
    fn round_unported(&self, s: &CoreState, k: &Kernel, defender: Seat) -> Option<Bail> {
        let sides = [self.p.invader, defender];
        let owned = |who: Seat, t: Technology| s.players().by_seat(who).is_some_and(|p| p.tech.owned.contains(t));
        let status = |p: &crate::state::Player, card: Leader| p.leaders.status.get(&card).copied();
        let thundarian = s.players().iter().any(|p| status(p, Leader::TheThundarian) == Some(crate::state::LeaderStatus::Readied))
            || s.players().iter().any(|b| b.faction == crate::generated::Faction::TheYssarilTribes && status(b, Leader::Ssruu) == Some(crate::state::LeaderStatus::Readied));
        if thundarian && s.players().iter().any(|p| p.faction == crate::generated::Faction::TheNomad) {
            return Some(Why::Thundarian);
        }
        if sides.iter().any(|w| owned(*w, Technology::DacxiveAnimators)) {
            return Some(Why::Dacxive);
        }
        let _ = k;
        None
    }

    /// `begin_ground_round`: the round counter, "round started" for a copied round agent, the faction effects of the start (the hooks), then the
    /// Dunlain Reaper's offers.
    fn start_round(&mut self, sx: &mut Sx<'_>, planet: Planet, defender: Seat) -> Result<(), Bail> {
        if let Some(why) = self.round_unported(sx.state, sx.kernel, defender) {
            return Err(why);
        }
        let (invader, system) = (self.p.invader, self.p.system);
        if let Some(active) = sx.state.transient_mut().invasion.active.as_mut()
            && active.planet == Some(planet)
            && active.defender == Some(defender)
        {
            active.edit().ground_round += 1;
        }
        let copied = Self::copied_round(sx.state);
        self.p.round_copied = copied;
        let numbered = sx.state.meta().rules_at_least(RULES_FACTION_EFFECTS);
        if numbered && !copied {
            sx.state.meta_mut().combat_round_seq = sx.state.meta().combat_round_seq.saturating_add(1);
        }
        if copied {
            sx.state.meta_mut().combat_round_seq = sx.state.meta().combat_round_seq.saturating_add(1);
            let payload = self.base(planet, defender).with(Key::RoundSeq, Val::Int(i64::from(sx.state.meta().combat_round_seq)));
            Self::event(sx, EventKind::GroundCombatRoundStarted, payload);
        }
        for who in [invader, defender] {
            let (k, mut cx) = (sx.kernel, sx.cx());
            k.hooks.ground.ground_combat_round_started(&mut cx, who, system, planet);
        }
        let source = sx.table.codes.invasion.dunlain_reaper;
        self.set_round(IRound::Reaper { at: 0, bill: None, asked: false, source });
        self.reaper(sx, planet, defender)
    }

    fn set_round(&mut self, round: IRound) {
        if let IStage::Fighting { round: r, .. } = &mut self.p.stage {
            *r = round;
        }
    }

    /// Letnev's Dunlain Reaper, participant by participant: a question and a payment before the dice.
    fn reaper(&mut self, sx: &mut Sx<'_>, planet: Planet, defender: Seat) -> Result<(), Bail> {
        let source = sx.table.codes.invasion.dunlain_reaper;
        loop {
            let IStage::Fighting { round: IRound::Reaper { at, .. }, .. } = self.p.stage.clone() else { return Ok(()) };
            if at >= 2 {
                return self.round_body(sx, planet, defender);
            }
            let who = if at == 0 { self.p.invader } else { defender };
            let pay = can_pay(sx.state, sx.kernel, who, Spend::Resources, REAPER_COST);
            if reaper_offered(sx.state, sx.kernel, planet, who, pay) {
                self.set_round(IRound::Reaper { at, bill: None, asked: true, source });
                return Ok(());
            }
            self.set_round(IRound::Reaper { at: at + 1, bill: None, asked: false, source });
        }
    }

    /// The answer to the Reaper's question, or to a step of its payment.
    fn answer_reaper(&mut self, sx: &mut Sx<'_>, pick: Pick, planet: Planet, defender: Seat) -> Result<(), Illegal> {
        let IStage::Fighting { round: IRound::Reaper { at, bill, source, .. }, .. } = self.p.stage.clone() else { return Err(Illegal) };
        let who = if at == 0 { self.p.invader } else { defender };
        let mut pay = match (bill, pick) {
            (Some(b), pick) => {
                let mut pay = PayBill::resume(b.who, Spend::Resources, i64::from(b.cost), i64::from(b.paid));
                pay.answer(sx, pick)?;
                pay
            }
            (None, Pick::Opt(Opt::Decline)) => {
                self.set_round(IRound::Reaper { at: at + 1, bill: None, asked: false, source });
                let r = self.reaper(sx, planet, defender);
                Self::or_bail(sx, r);
                return Ok(());
            }
            (None, Pick::Opt(Opt::Combat(COpt::DeployMech))) => PayBill::new(who, Spend::Resources, REAPER_COST),
            _ => return Err(Illegal),
        };
        match run_bill(sx, &mut pay) {
            Billed::Asks(paid) => {
                let b = IBill { who, influence: false, cost: u16::try_from(REAPER_COST).unwrap_or(2), paid: u16::try_from(paid).unwrap_or(u16::MAX) };
                self.set_round(IRound::Reaper { at, bill: Some(b), asked: true, source });
            }
            Billed::Paid(paid) => {
                if paid {
                    reaper_replace(sx.state, planet, who);
                }
                self.set_round(IRound::Reaper { at: at + 1, bill: None, asked: false, source });
                let r = self.reaper(sx, planet, defender);
                Self::or_bail(sx, r);
            }
        }
        Ok(())
    }

    /// The rest of the round: the dice, their tail, the hits and the end of the round.
    fn round_body(&mut self, sx: &mut Sx<'_>, planet: Planet, defender: Seat) -> Result<(), Bail> {
        let (invader, system) = (self.p.invader, self.p.system);
        let codes = sx.table.codes.invasion;
        // Decided when the round began: the agent that numbered it exhausts at that very start.
        let copied = self.p.round_copied;
        let owns_x89 = |who: Seat| sx.state.players().by_seat(who).is_some_and(|p| p.tech.owned.contains(Technology::X89BacterialWeapon));
        let x89 = [owns_x89(invader), owns_x89(defender)];
        let before: Vec<Piece> = sx.state.board().planet(planet).units.iter().copied().collect();
        // The dice, then the end of the "Roll Dice" step (the reroll windows open at the announcements).
        let mut roll = GroundRoll::new(RollFacts {
            attacker: invader,
            defender,
            system,
            planet,
            kind: codes.ground,
            names: codes.names.to_vec(),
            unit_codes: Unit::ALL.iter().map(|u| (*u, codes.unit(*u))).collect(),
            x89,
        });
        let _ = roll.step(sx);
        if roll.unsupported {
            return Err(Why::RollCannotName);
        }
        let mut tail = GroundRollTail::new(TailFacts { invader, defender, system, planet, rolled: roll.hits, bump: !copied, announce: true, x89 });
        let _ = tail.step(sx);
        // Hits a faction adds to a roll once the dice are final (Valkyrie Particle Weave).
        let extra = |who: Seat, hits: u32, other: u32| sx.kernel.hooks.ground.ground_rolls_extra_hits(sx.state, sx.kernel, who, system, planet, hits as usize, other as usize);
        if extra(invader, tail.hits.0, tail.hits.1) != 0 || extra(defender, tail.hits.1, tail.hits.0) != 0 {
            return Err(Why::HitsAddedToARoll);
        }
        let dice: Vec<InvasionDie> = tail.dice.iter().map(|(side, group, face, target, hit)| InvasionDie { planet, player: *side, group: *group, face: *face, target: *target, hit: *hit }).collect();
        let template = InvasionStep { planet, kind: codes.ground_round, round: 0, before, after: Vec::new(), dice, hits: SortedMap::default(), harrow_hits: 0 };
        let harrow = sx.state.players().by_seat(invader).is_some_and(|p| p.faction.data().abilities.contains(&crate::generated::Ability::Harrow));
        let harrow = harrow.then_some(HarrowFacts { invader, defender, system, planet, announce: true, cause: sx.table.codes.harrow });
        let mut hits = GroundRoundHits::new(RoundFacts { invader, defender, system, planet, hits: tail.hits, announce: true, cause: sx.table.codes.ground_combat, template, harrow });
        let _ = hits.step(sx);
        self.finish_ground_round(sx, planet, defender);
        Ok(())
    }

    /// 42.3: the fight ends when one side has no ground forces left on the planet. Both surviving starts the next round.
    fn finish_ground_round(&mut self, sx: &mut Sx<'_>, planet: Planet, defender: Seat) {
        let IStage::Fighting { planets, index, .. } = self.p.stage.clone() else { return };
        let (invader, system) = (self.p.invader, self.p.system);
        let owners = self.owners(sx.state, sx.kernel, planet);
        let (inv_s, def_s) = (owners.contains(invader), owners.contains(defender));
        if inv_s && def_s {
            self.p.stage = IStage::Fighting { planets, index, defender, round: IRound::Asking };
            return;
        }
        let (winner, loser) = if inv_s { (invader, defender) } else { (defender, invader) };
        // Only a side with forces left has won (both wiped out is nobody's win).
        // (Dacxive Animators is a gate of the round start.)
        self.return_temp_commits(sx, planet);
        let owners_after = self.owners(sx.state, sx.kernel, planet);
        let control_changed = winner == invader && owners.contains(winner) && owners_after.contains(invader) && qboard::control(sx.state, planet) != Some(invader);
        let noted = match self.p.ground_occurrence.take() {
            Some(occurrence) => {
                let noted = self.win_feats(sx, winner, loser, occurrence);
                if noted {
                    self.p.scoring.push((occurrence, true));
                }
                noted
            }
            None => false,
        };
        let rival = if loser == invader { winner } else { loser };
        let mut payload = self.base(planet, rival);
        if owners.contains(winner) {
            payload.set(Key::Winner, Val::Seat(winner));
        }
        payload.set(Key::ControlChanged, Val::Flag(control_changed));
        Self::event(sx, EventKind::GroundCombatEnded, payload);
        let _ = system;
        if self.p.combat_only {
            self.p.stage = IStage::Done;
            return;
        }
        if noted {
            self.p.stage = IStage::Advancing { planets, index: index.saturating_add(1) };
        } else {
            let r = self.advance_fighting(sx, &planets, usize::from(index) + 1);
            Self::or_bail(sx, r);
        }
    }

    /// `note_ground_combat_win_feats`: the feats a won ground combat earns; whether there was one.
    fn win_feats(&self, sx: &mut Sx<'_>, winner: Seat, loser: Seat, occurrence: u64) -> bool {
        let r = rd(sx.state, sx.kernel);
        let mut earned: Vec<Feat> = Vec::new();
        let anomaly = r.scope().system(self.p.system).is_some_and(|d| d.has(SystemFlags::NEBULA) || d.has(SystemFlags::SUPERNOVA) || d.has(SystemFlags::SCAR) || d.has(SystemFlags::ASTEROID_FIELD) || d.has(SystemFlags::GRAVITY_RIFT));
        if anomaly {
            earned.push(Feat::WonInAnAnomaly);
        }
        if r.s.players().iter().any(|p| p.seat != winner && p.home_system == Some(self.p.system)) {
            earned.push(Feat::WonInARivalHome);
        }
        let most = r.s.players().iter().map(|p| p.econ.victory_points).max().unwrap_or(0);
        if r.s.players().by_seat(loser).is_some_and(|p| p.econ.victory_points == most) {
            earned.push(Feat::WonAgainstThePointsLeader);
        }
        if self.p.notes.as_slice().contains(&(winner, loser)) {
            earned.push(Feat::WonAgainstANoteHolder);
        }
        let noted = !earned.is_empty();
        if let Some(p) = sx.state.players_mut().by_seat_mut(winner) {
            for feat in earned {
                let entry = (feat, occurrence);
                if !p.misc.event_feats.as_slice().contains(&entry) {
                    p.misc.event_feats.push(entry);
                    p.misc.event_feats.as_mut_slice().sort_unstable();
                }
            }
        }
        noted
    }

    /// Matriarch's fighters have been ground forces only for this invasion's combat: the surviving copies return to the space area.
    fn return_temp_commits(&mut self, sx: &mut Sx<'_>, planet: Planet) {
        let Some(at) = self.p.temp_commits.iter().position(|(p, _)| *p == planet) else { return };
        let (_, committed) = self.p.temp_commits.remove(at);
        let mut returned = Vec::new();
        for unit in committed {
            let cell = sx.state.board_mut().planet_mut(planet);
            if let Some(i) = cell.units.as_slice().iter().position(|u| *u == unit) {
                cell.units.remove(i);
                returned.push(unit);
            }
        }
        if !returned.is_empty() {
            let system = self.p.system;
            sx.state.board_mut().sys_mut(system).flags.set(crate::state::SysFlags::TOUCHED, true);
            for u in returned {
                sx.state.board_mut().sys_mut(system).units.push(u);
            }
        }
    }

    // -- the answers ----------------------------------------------------------------------------------------------------

    /// `answer_fight`: the invader asked for a round, or the Reaper is being answered.
    pub(super) fn answer_fight(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let IStage::Fighting { planets, index, defender, round } = self.p.stage.clone() else { return Err(Illegal) };
        let planet = planets.get(usize::from(index)).copied().ok_or(Illegal)?;
        match round {
            IRound::Asking => {
                if !matches!(pick, Pick::Opt(Opt::Invasion(IOpt::FightRound { .. }))) {
                    return Err(Illegal);
                }
                let r = self.start_round(sx, planet, defender);
                Self::or_bail(sx, r);
                Ok(())
            }
            IRound::Reaper { .. } => self.answer_reaper(sx, pick, planet, defender),
        }
    }

    /// `answer_next_combat`: the winner starts another combat against the next coexister, or declines.
    pub(super) fn answer_next_combat(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let IStage::ChoosingNextCombat { planets, index, planet, mut remaining } = self.p.stage.clone() else { return Err(Illegal) };
        match pick {
            Pick::Opt(Opt::Invasion(IOpt::LeaveCoexisting { .. })) => {
                self.p.stage = IStage::Advancing { planets, index: index.saturating_add(1) };
            }
            Pick::Opt(Opt::Invasion(IOpt::FightNext { .. })) => {
                if remaining.is_empty() {
                    return Err(Illegal);
                }
                let defender = remaining.remove(0);
                // The chosen coexister stops coexisting for the duration: they are now a side in a combat.
                let cell = sx.state.board_mut().planet_mut(planet);
                cell.coexist.remove(defender);
                cell.flags.set(crate::state::PlanetFlags::COEXIST_ENTRY, true);
                if self.start_ground_combat(sx, planet, defender) {
                    self.p.ground_occurrence = Some(Self::begin_feat(sx));
                    self.p.stage = IStage::Fighting { planets, index, defender, round: IRound::Asking };
                } else {
                    // A start-of-combat handler removed a side's last ground force.
                    self.p.stage = IStage::Advancing { planets, index };
                }
            }
            _ => return Err(Illegal),
        }
        Ok(())
    }

    fn begin_feat(sx: &mut Sx<'_>) -> u64 {
        let meta = sx.state.meta_mut();
        meta.feat_occurrence_seq += 1;
        meta.feat_occurrence_seq
    }

    /// Announce the start of a ground combat and report whether both sides still have ground forces after whatever reacted to it.
    fn start_ground_combat(&mut self, sx: &mut Sx<'_>, planet: Planet, defender: Seat) -> bool {
        let payload = self.base(planet, defender);
        Self::event(sx, EventKind::GroundCombatStarted, payload);
        let owners = self.owners(sx.state, sx.kernel, planet);
        owners.contains(self.p.invader) && owners.contains(defender)
    }

    // -- the next fight, the control ---------------------------------------------------------------------------------------

    /// Move to the next planet that still needs a fight, or finish and take control.
    pub(super) fn advance_fighting(&mut self, sx: &mut Sx<'_>, planets: &[Planet], mut index: usize) -> Result<(), Bail> {
        loop {
            let mut adv = Advance::new(AdvanceFacts { invader: self.p.invader, system: self.p.system, planets: planets.to_vec(), index, coexisted: self.p.report.coexisted.clone() });
            let _ = adv.step(sx);
            match adv.pick.unwrap_or(Upcoming::Exhausted) {
                Upcoming::Fight { index: at, defender } => {
                    index = at;
                    let planet = planets[index];
                    if !self.start_ground_combat(sx, planet, defender) {
                        // A handler at the start of the combat removed a side's last ground force: look at the planet again.
                        continue;
                    }
                    self.p.ground_occurrence = Some(Self::begin_feat(sx));
                    self.p.stage = IStage::Fighting { planets: planets.to_vec(), index: u8::try_from(index).unwrap_or(u8::MAX), defender, round: IRound::Asking };
                    return Ok(());
                }
                Upcoming::Coexisters { index: at, remaining } => {
                    self.p.stage = IStage::ChoosingNextCombat { planets: planets.to_vec(), index: u8::try_from(at).unwrap_or(u8::MAX), planet: planets[at], remaining };
                    return Ok(());
                }
                Upcoming::Exhausted => break,
            }
        }
        let committed = self.p.report.committed.clone();
        let temps: Vec<Planet> = self.p.temp_commits.iter().map(|(p, _)| *p).collect();
        for planet in temps {
            self.return_temp_commits(sx, planet);
        }
        self.advance_control(sx, &committed, 0)
    }

    /// `establish_control` (49.5): whoever has ground forces left takes the planet, exhausted. `Some(previous)` when the planet changed hands.
    fn establish_control(&mut self, sx: &mut Sx<'_>, planet: Planet) -> Result<Option<Option<Seat>>, Bail> {
        let (invader, system) = (self.p.invader, self.p.system);
        let r = rd(sx.state, sx.kernel);
        let holds = sx.state.board().planet(planet).units.iter().any(|u| u.owner() == invader && is_ground_force(r, u.kind()));
        if !holds {
            return Ok(None); // 49.5d
        }
        // A coexisting invader holds no control of the planet (coexistence 2, 3.1).
        if qboard::is_coexisting(sx.state, system, planet, invader) {
            return Ok(None);
        }
        let previous = qboard::control(sx.state, planet);
        if previous == Some(invader) {
            return Ok(None); // 49.5c
        }
        sx.state.board_mut().planet_mut(planet).control = Some(invader);
        sx.state.board_mut().sys_mut(system).flags.set(crate::state::SysFlags::TOUCHED, true);
        fx::exhaust_planet(sx.state, planet);
        // Everything that reads "when you gain control of a planet" fires here, where control actually changes hands: the Minister of
        // Exploration pays a trade good (staged, as the string stages it), and the Crown of Emphidia changes hands when a planet in its
        // owner's home system is taken.
        let blanked = reach::blanked(sx.state);
        if crate::query::laws::holds_ministry(sx.state, blanked, invader, crate::query::laws::MINISTER_EXPLORATION) {
            fx::gain_trade_goods_staged(sx.state, invader, 1, sx.table.codes.invasion.minister);
        }
        let crown = crate::generated::Agenda::TheCrownOfEmphidia;
        if let Some(crate::keys::Outcome::Seat(owner)) = crate::query::laws::elected(sx.state, blanked, crown)
            && crate::query::objectives::home_system_of(sx.state, owner) == Some(system)
        {
            fx::steal_throne_card(sx.state, crown, invader);
        }
        Ok(Some(previous))
    }

    /// Establish and finish control one planet at a time so a control-loss occurrence is not delayed past later captures.
    pub(super) fn advance_control(&mut self, sx: &mut Sx<'_>, planets: &[Planet], mut index: usize) -> Result<(), Bail> {
        let (invader, system) = (self.p.invader, self.p.system);
        while index < planets.len() {
            let planet = planets[index];
            let Some(previous) = self.establish_control(sx, planet)? else {
                index += 1;
                continue;
            };
            self.p.report.captured.push((planet, previous));
            let lost_home = previous.is_some_and(|holder| sx.state.players().by_seat(holder).is_some_and(|p| p.home_system == Some(system)));
            if lost_home {
                let holder = previous.unwrap_or(invader);
                let occurrence = Self::begin_feat(sx);
                if let Some(p) = sx.state.players_mut().by_seat_mut(holder) {
                    let entry = (Feat::LostAHomePlanet, occurrence);
                    if !p.misc.event_feats.as_slice().contains(&entry) {
                        p.misc.event_feats.push(entry);
                        p.misc.event_feats.as_mut_slice().sort_unstable();
                    }
                }
                self.p.scoring.push((occurrence, false));
                self.p.stage = IStage::FinalizingControl { planets: planets.to_vec(), index: u8::try_from(index + 1).unwrap_or(u8::MAX), planet, previous };
                return Ok(());
            }
            if let Some(run) = self.finish_control_gain(sx, planet, previous)? {
                // A first claim explores its planet before the chain goes on to the next one.
                self.p.stage = IStage::Exploring { planets: planets.to_vec(), index: u8::try_from(index + 1).unwrap_or(u8::MAX), planet, run: Box::new(run) };
                return Ok(());
            }
            index += 1;
        }
        self.p.stage = IStage::Done;
        Ok(())
    }

    /// The exploration of a first claim, kept as data in the payload (`IStage::Exploring`): run it to its next question or its end, then the
    /// chain of control goes on with the next planet. A run the window does not carry is a counted bail of the native path.
    pub(super) fn explore_first_claim(&mut self, sx: &mut Sx<'_>, planets: Vec<Planet>, index: u8, planet: Planet, run: ExploreRun) -> Result<(), Bail> {
        let mut exploring = Exploring::hosted(run).with_one(crate::driver::children::one_link());
        if let Next::Continue(_) = exploring.step(sx) {
            self.p.stage = IStage::Exploring { planets, index, planet, run: Box::new(exploring.into_run()) };
            return Ok(());
        }
        if let Some(why) = exploring.unsupported() {
            return Err(Why::Exploration(why));
        }
        if exploring.explored() { // mutants:skip: `report.explored` is read by the string dual adapter's report only (`engine/invasion.rs` `into_report`), no core driver reads it
            self.p.report.explored.push(planet);
        }
        self.advance_control(sx, &planets, usize::from(index))
    }

    /// Whether a first claim of `planet` explores anything: a printed trait, or an attachment that gives one (`traits_now`).
    fn explorable(&self, s: &CoreState, k: &Kernel, planet: Planet) -> bool {
        let scope = crate::tables::Scope(u32::from(k.sources.0));
        !crate::query::explore::traits_of(scope, planet).is_empty() || qboard::attachments(s, planet).iter().any(|a| !a.data().traits.is_empty())
    }

    /// `finish_control_gain`: Assimilate, `PLANET_CONTROL_GAINED`, the destruction of the rival structures, then what follows a capture. `Some(run)`:
    /// a first claim of a planet with a trait to explore into (35.1): the exploration to run next.
    pub(super) fn finish_control_gain(&mut self, sx: &mut Sx<'_>, planet: Planet, previous: Option<Seat>) -> Result<Option<ExploreRun>, Bail> {
        let (invader, system) = (self.p.invader, self.p.system);
        if sx.state.players().by_seat(invader).is_some_and(|p| p.tech.owned.contains(Technology::IntegratedEconomy)) {
            return Err(Why::IntegratedEconomy);
        }
        let infiltrate_fix = sx.state.meta().rules_at_least(1); // mutants:skip: only the latest rules version is played, which is past version 2 as well
        let roster = sx.table.facts.roster.or_else(|| sx.table.facts.plays.as_ref().and_then(|p| p.roster));
        let mut w = ControlGained::new(ControlFacts { invader, system, planet, previous, infiltrate_fix, announce: true }, roster);
        let _ = w.step(sx);
        if w.unsupported {
            return Err(Why::AssimilateRoster);
        }
        // 35.1: a planet nobody controlled is explored (when it has a trait to explore into: the deck is asked among the traits now).
        if previous.is_none() && self.explorable(sx.state, sx.kernel, planet) {
            return Ok(Some(ExploreRun::new(invader, ETarget::Planet(planet), EDeck::Choose, true)));
        }
        Ok(None)
    }
}

#[allow(dead_code, reason = "the deploy question is built by `ops::question`")]
fn _uses(s: &CoreState, k: &Kernel) {
    let _ = (deploy_question, can_pay, qboard::control, s, k);
}
