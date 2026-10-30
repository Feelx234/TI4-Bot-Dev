//! D4-2d: the composed space combat (`rules::combat::machine::Machine`): the whole fight from the opening of round 1 to the announcement of its end,
//! settle by settle, the way the aftermath's combat window drives it: a settle runs to the next decision (`Stop::Pending`), the decision is put and
//! answered by an `awaiting` machine, the next settle goes on. The rules checked are LRR 78 (the space combat) and 87 (sustain damage): the
//! announcement of retreats (defender first), the roll, the hits (sustain before assignment), the retreat of a fleet, the end.

use ti4_engine_core::choice::combat::{Ask as CAsk, Opt as COpt};
use ti4_engine_core::containers::Opt32;
use ti4_engine_core::rules::combat::flow::{Phase, Step};
use ti4_engine_core::rules::combat::end_announce::Codes as EndCodes;
use ti4_engine_core::rules::combat::machine::{Bail, Env, Fields, Machine, Stop};
use ti4_engine_core::state::{Code, EventKind, Key, Piece};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::rig::Policy;
use ti4_engine_core_tests::scenario::plain_systems;

fn sys() -> System {
    plain_systems()[0]
}

fn there() -> System {
    plain_systems()[1]
}

const KINDS: [Unit; 7] = [Unit::CarrierI, Unit::CruiserI, Unit::DestroyerI, Unit::DestroyerII, Unit::DreadnoughtI, Unit::FighterI, Unit::WarSun];

/// The text tables the string side would hand over: every non-empty set of the unit types used, in id order.
fn groups() -> Vec<(Vec<Unit>, Code, Code)> {
    let mut out = Vec::new();
    for mask in 1u16..(1 << KINDS.len()) {
        let mut kinds: Vec<Unit> = KINDS.iter().enumerate().filter(|(i, _)| mask & (1 << i) != 0).map(|(_, u)| *u).collect();
        kinds.sort_unstable();
        out.push((kinds, Code(1000 + mask), Code(2000 + mask)));
    }
    out
}

fn env() -> Env {
    let mut alpha = KINDS.to_vec();
    alpha.sort_unstable();
    Env {
        attacker: Seat(0),
        defender: Seat(1),
        system: sys(),
        announce: true,
        staging: false,
        occurrence: 0,
        begin_occurrence: true,
        phases: [Code(1), Code(2), Code(3), Code(4), Code(5)],
        opening_unported: false,
        round_seq_listens: false,
        barrage_unported: false,
        barrage_cause: Code(6),
        barrage_kind: Code(7),
        barrage_extra: Code(8),
        unit_codes: KINDS.iter().enumerate().map(|(i, u)| (*u, Code(100 + i as u16))).collect(),
        roll_unported: false,
        munitions_after: [false, false],
        roll_cause: Code(9),
        hacan: Code(10),
        groups: groups(),
        alpha,
        retreat_cause: Code(11),
        retreat_pool: Code(12),
        retreat_unported: false,
        neighbors: [vec![], vec![]],
        may_retreat: [false, false],
        duranium: [false, false],
        devotion: false,
        end_codes: EndCodes { losers: Code(40), retreated: Code(41), reasons: [Code(42), Code(43), Code(44), Code(45)] },
        casualty: Code(13),
        commander: Code(14),
    }
}

fn fields(s: &CoreState) -> Fields {
    let fleet = |seat: Seat| s.board().sys(sys()).units.iter().filter(|p| p.owner() == seat).map(|p| p.kind()).collect::<Vec<_>>();
    Fields {
        step: Step::Opening { round: 1 },
        phase: Phase::Round,
        resuming: false,
        pending_retreats: vec![],
        offered: vec![],
        damaged: vec![],
        fled: vec![],
        at_start: vec![(Seat(0), fleet(Seat(0))), (Seat(1), fleet(Seat(1)))],
        scoring: false,
        outcome: None,
        end_announced: false,
    }
}

fn arena(a: Faction, d: Faction) -> Scenario {
    Scenario::new(&[a, d]).map_from(&[sys(), there()]).tokens(0, 3, 3, 2).tokens(1, 3, 3, 2)
}

/// One whole combat, driven settle by settle.
struct Fight {
    state: CoreState,
    kernel: Kernel,
    env: Env,
    fields: Fields,
    seed: u64,
    events: Vec<(EventKind, ti4_engine_core::state::Payload)>,
    drawn: usize,
    questions: Vec<Choice>,
    stop: Option<Stop>,
    settles: usize,
    /// Stop at a scoring pause instead of closing the scoring window (the caller of a test that looks at the pause).
    pause_at_scoring: bool,
    pauses: usize,
}

impl Fight {
    fn new(sc: Scenario, env: Env, seed: u64) -> Self {
        let (state, kernel) = sc.build();
        let fields = fields(&state);
        Self { state, kernel, env, fields, seed, events: vec![], drawn: 0, questions: vec![], stop: None, settles: 0, pause_at_scoring: false, pauses: 0 }
    }

    fn answer_all(rig: &mut Rig<Machine>, policy: Policy, questions: &mut Vec<Choice>) {
        let mut n = 0u64;
        while let Some(ti4_engine_core::driver::Status::Asking(c)) = rig.status().cloned() {
            questions.push(c.clone());
            let len = c.opts.len();
            let i = match policy {
                Policy::First => 0,
                Policy::Last => len.saturating_sub(1),
                Policy::Mix(s) => ((s + n * 7) % len.max(1) as u64) as usize,
            };
            let p = c.pick(i).unwrap_or(Pick::Number(0));
            if let Err(e) = rig.try_answer(p) {
                panic!("{e:?}\n{}", rig.transcript());
            }
            n += 1;
            assert!(n < 300, "runaway\n{}", rig.transcript());
        }
    }

    /// Run settles until the fight stops for good (`Over`, a bail or a scoring pause the caller handles).
    fn settle(&mut self, policy: Policy) -> Option<Stop> {
        for _ in 0..200 {
            self.settles += 1;
            let mut rig = Rig::from_parts(self.state.clone(), self.kernel.clone(), Machine::new(self.env.clone(), self.fields.clone())).seeded(self.seed);
            rig.run();
            Self::answer_all(&mut rig, policy, &mut self.questions);
            self.collect(&rig);
            let m = rig.driver.window.clone();
            self.state = rig.state.clone();
            self.fields = m.fields.clone();
            self.env = m.env.clone();
            self.drawn += m.drawn.len();
            self.stop = m.stop;
            match m.stop {
                Some(Stop::Pending) => self.decide(policy),
                Some(Stop::Scoring) if !self.pause_at_scoring => {
                    // The scoring window of the secret objectives closes (the caller's job), the round goes on.
                    self.pauses += 1;
                    self.fields.scoring = false;
                }
                other => return other,
            }
            if matches!(self.stop, Some(Stop::Bailed(_))) {
                return self.stop;
            }
        }
        panic!("a combat that never ends");
    }

    fn decide(&mut self, policy: Policy) {
        let mut rig = Rig::from_parts(self.state.clone(), self.kernel.clone(), Machine::awaiting(self.env.clone(), self.fields.clone())).seeded(self.seed);
        rig.run();
        Self::answer_all(&mut rig, policy, &mut self.questions);
        self.collect(&rig);
        let m = rig.driver.window.clone();
        self.state = rig.state.clone();
        self.fields = m.fields.clone();
        if let Some(Stop::Bailed(b)) = m.stop {
            self.stop = Some(Stop::Bailed(b));
        }
    }

    fn collect(&mut self, rig: &Rig<Machine>) {
        self.events.extend(rig.driver.emitted.iter().map(|e| (e.kind, e.payload.clone())));
    }

    fn count(&self, kind: EventKind) -> usize {
        self.events.iter().filter(|(k, _)| *k == kind).count()
    }

    fn ships(&self, seat: u8, system: System) -> usize {
        self.state.board().sys(system).units.iter().filter(|p| p.owner() == Seat(seat)).count()
    }
}

// ---- whole fights ------------------------------------------------------------------------------

fn skirmish() -> Scenario {
    arena(Faction::TheEmbersOfMuaat, Faction::TheArborec)
        .unit(sys(), None, Unit::DreadnoughtI, 0, 2)
        .unit(sys(), None, Unit::CruiserI, 0, 2)
        .unit(sys(), None, Unit::FighterI, 0, 2)
        .unit(sys(), None, Unit::DreadnoughtI, 1, 1)
        .unit(sys(), None, Unit::DestroyerI, 1, 3)
        .unit(sys(), None, Unit::CruiserI, 1, 1)
}

#[test]
fn a_whole_combat_is_fought_to_a_winner_under_every_answer_policy() {
    covers!("rules::combat::machine::Machine");
    let mut winners = [0usize; 3];
    for seed in 0..14u64 {
        for policy in [Policy::First, Policy::Last, Policy::Mix(seed)] {
            let mut fight = Fight::new(skirmish(), env(), seed);
            let stop = fight.settle(policy);
            assert_eq!(stop, Some(Stop::Over), "seed {seed} {policy:?}");
            let (winner, rounds) = fight.fields.outcome.expect("the outcome is recorded");
            assert!(rounds >= 1, "at least one round was fought");
            let (a, d) = (fight.ships(0, sys()), fight.ships(1, sys()));
            match winner {
                Some(Seat(0)) => assert!(a > 0 && d == 0, "{a} {d}"),
                Some(Seat(1)) => assert!(d > 0 && a == 0, "{a} {d}"),
                None => assert!(a == 0 && d == 0, "no winner means nobody is left: {a} {d}"),
                Some(other) => panic!("{other:?}"),
            }
            winners[winner.map_or(2, |s| usize::from(s.0))] += 1;
            assert_eq!(fight.fields.step, Step::Done);
            assert!(fight.fields.end_announced);
            assert_eq!(fight.count(EventKind::SpaceCombatEnded), 1, "the end is announced exactly once");
            assert_eq!(fight.count(EventKind::SpaceCombatStarted), 1);
            assert_eq!(fight.count(EventKind::CombatRoundStarted) as u32, rounds, "one announcement per round");
            assert_eq!(fight.count(EventKind::SpaceCombatRoundEnded) as u32, rounds, "every round ends");
            let end = fight.events.iter().find(|(k, _)| *k == EventKind::SpaceCombatEnded).unwrap();
            assert_eq!(end.1.int(Key::Rounds), Some(i64::from(rounds)));
            assert_eq!(end.1.code(Key::Reason), Some(Code(44)), "one side was destroyed");
            assert_eq!(end.1.seat(Key::Winner), winner);
            // Conservation: units are only lost, never gained or moved to other places.
            for (seat, before) in [(0u8, 6usize), (1, 5)] {
                assert!(fight.ships(seat, sys()) <= before);
            }
            assert_eq!(fight.ships(0, there()) + fight.ships(1, there()), 0, "nobody retreated");
        }
    }
    assert!(winners[0] > 0 && winners[1] > 0, "the seeds cover both winners: {winners:?}");
}

#[test]
fn the_defender_is_asked_about_a_retreat_before_the_attacker_and_a_fleet_that_leaves_ends_the_fight() {
    covers!("rules::combat::machine::Machine");
    let mut e = env();
    e.may_retreat = [true, true];
    e.neighbors = [vec![(there(), true)], vec![(there(), true)]];
    let sc = || arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), None, Unit::CruiserI, 1, 6).unit(there(), None, Unit::CruiserI, 1, 1);
    let (mut left, mut stayed) = (0, 0);
    for seed in 0..40u64 {
        let mut fight = Fight::new(sc(), e.clone(), seed);
        // The defender is asked first; answer "announce a retreat" whenever it is asked, "stay" for the attacker (the attacker cannot retreat to a neighbour with the defender's ships).
        fight.state.transient_mut().combat.last_reroll_player = None;
        let stop = fight.settle(Policy::Last);
        assert_eq!(stop, Some(Stop::Over), "seed {seed}");
        let first = fight.questions.iter().find(|q| matches!(q.ask, Ask::Combat(CAsk::AnnounceRetreat { .. }))).expect("the defender is asked in round 1");
        assert_eq!(first.actor, Seat(1), "the defender announces first");
        assert_eq!(first.opts.as_slice(), &[c(COpt::Stay), c(COpt::Announce)]);
        assert!(matches!(first.ask, Ask::Combat(CAsk::AnnounceRetreat { forced: false, ships: 6, .. })));
        assert_eq!(fight.count(EventKind::RetreatDeclared), 1, "the defender announced (the last option) and the attacker cannot retreat: its neighbour holds enemy ships");
        let declared = fight.events.iter().find(|(k, _)| *k == EventKind::RetreatDeclared).unwrap();
        assert_eq!(declared.1.seat(Key::Player), Some(Seat(1)));
        let end = fight.events.iter().find(|(k, _)| *k == EventKind::SpaceCombatEnded).unwrap();
        if fight.fields.fled.is_empty() {
            stayed += 1;
            assert_eq!(fight.ships(0, sys()), 0, "seed {seed}: the lone attacker died before the retreat");
        } else {
            left += 1;
            assert_eq!(end.1.code(Key::Reason), Some(Code(42)), "retreat");
            assert!(fight.ships(1, sys()) == 0 && fight.ships(1, there()) >= 2, "the fleet moved to the neighbour");
            assert!(fight.state.board().sys(there()).cmd.contains(Seat(1)), "a command token marks the retreat");
            assert_eq!(fight.fields.fled[0].0, Seat(1));
            assert_eq!(end.1.seat(Key::Winner), Some(Seat(0)).filter(|_| fight.ships(0, sys()) > 0));
        }
    }
    assert!(left > 0 && stayed > 0, "the seeds cover a retreat and a fleet that died first: {left} {stayed}");
}

fn c(o: COpt) -> Opt {
    Opt::Combat(o)
}

#[test]
fn a_rout_leaves_the_attacker_no_choice_but_to_announce_and_intercept_bars_the_retreat() {
    covers!("rules::combat::machine::Machine");
    let mut e = env();
    e.may_retreat = [true, true];
    e.neighbors = [vec![(there(), true)], vec![(there(), true)]];
    let sc = || arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 3).unit(sys(), None, Unit::CruiserI, 1, 3).unit(there(), None, Unit::CruiserI, 0, 1);
    // Rout on the attacker's side: the defender played Rout this round, so the attacker must announce (no "stay").
    let mut fight = Fight::new(sc(), e.clone(), 1);
    fight.state.players_mut().by_seat_mut(Seat(1)).unwrap().card_timers.rout_round = Opt32::some(1);
    let _ = fight.settle(Policy::First);
    let forced = fight.questions.iter().find(|q| q.actor == Seat(0) && matches!(q.ask, Ask::Combat(CAsk::AnnounceRetreat { .. })));
    if let Some(q) = forced {
        assert!(matches!(q.ask, Ask::Combat(CAsk::AnnounceRetreat { forced: true, .. })));
        assert_eq!(q.opts.as_slice(), &[c(COpt::Announce)], "a Rout leaves no stay");
    }
    // Intercept: nobody can retreat this round, nobody is asked.
    let mut barred = Fight::new(sc(), e, 1);
    barred.state.players_mut().by_seat_mut(Seat(1)).unwrap().card_timers.retreat_barred_round = Opt32::some(1);
    barred.state.players_mut().by_seat_mut(Seat(0)).unwrap().card_timers.retreat_barred_round = Opt32::some(1);
    let _ = barred.settle(Policy::First);
    let asked = barred.questions.iter().filter(|q| matches!(q.ask, Ask::Combat(CAsk::AnnounceRetreat { .. }))).count();
    let in_round_one = barred.questions.first().is_none_or(|q| !matches!(q.ask, Ask::Combat(CAsk::AnnounceRetreat { .. })));
    assert!(in_round_one, "round 1: the retreat is barred, so the first question is not an announcement ({asked} later)");
}

// ---- the opening: barrage and its feat ------------------------------------------------------------

#[test]
fn the_barrage_of_round_one_can_end_the_fight_before_a_roll_and_scores_the_feat_with_a_scoring_pause() {
    covers!("rules::combat::machine::Machine");
    let sc = || arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::DestroyerI, 0, 1).unit(sys(), None, Unit::FighterI, 1, 1).unit(sys(), None, Unit::CruiserI, 1, 1);
    let (mut paused, mut plain) = (0, 0);
    for seed in 0..40u64 {
        let mut fight = Fight::new(sc(), env(), seed);
        fight.pause_at_scoring = true;
        let stop = fight.settle(Policy::First);
        if stop == Some(Stop::Scoring) {
            paused += 1;
            assert_eq!(fight.fields.step, Step::AfterBarrage { round: 1 });
            assert!(fight.fields.scoring);
            let feats = fight.state.players().by_seat(Seat(0)).unwrap().misc.event_feats.as_slice().to_vec();
            assert_eq!(feats.len(), 1, "the feat was recorded against the combat's occurrence");
            assert_eq!(fight.state.meta().feat_occurrence_seq, 1);
            assert_eq!(fight.ships(1, sys()), 1, "the fighter is gone, the cruiser is not");
            assert_eq!(fight.count(EventKind::CombatRoundStarted), 1);
            // The scoring window closes: the round goes on at the announcement of retreats.
            fight.fields.scoring = false;
            let again = fight.settle(Policy::First);
            assert_eq!(again, Some(Stop::Over));
        } else {
            plain += 1;
            assert_eq!(stop, Some(Stop::Over));
            assert!(fight.state.players().by_seat(Seat(0)).unwrap().misc.event_feats.is_empty(), "seed {seed}: no feat without the last fighter");
        }
    }
    assert!(paused > 0 && plain > 0, "the seeds cover both: {paused} {plain}");
}

#[test]
fn a_combat_whose_other_side_dies_to_the_barrage_alone_is_over_with_no_round_fought() {
    covers!("rules::combat::machine::Machine");
    // Only fighters on the defender's side: one hit of the barrage can empty it; the fight is over before any roll, with zero rounds.
    let sc = || arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::DestroyerII, 0, 3).unit(sys(), None, Unit::FighterI, 1, 1);
    let mut ended_early = false;
    for seed in 0..40u64 {
        let mut fight = Fight::new(sc(), env(), seed);
        fight.pause_at_scoring = true;
        let stop = fight.settle(Policy::First);
        if stop == Some(Stop::Scoring) {
            fight.fields.scoring = false;
            let again = fight.settle(Policy::First);
            assert_eq!(again, Some(Stop::Over));
            if fight.ships(1, sys()) == 0 && fight.fields.outcome.map(|o| o.1) == Some(0) {
                ended_early = true;
                assert_eq!(fight.fields.outcome.unwrap().0, Some(Seat(0)));
                assert_eq!(fight.state.ledger().dice.iter().filter(|r| r.purpose == ti4_engine_core::state::RollPurpose::SpaceCombat).count(), 0, "no combat dice were rolled");
            }
        }
    }
    assert!(ended_early);
}

// ---- bails: the positions the machine does not carry ----------------------------------------------

#[test]
fn every_gate_the_string_side_closes_is_a_documented_bail() {
    covers!("rules::combat::machine::Machine");
    let base = || arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 2).unit(sys(), None, Unit::CruiserI, 1, 2);
    let gates: Vec<(&str, Box<dyn Fn(&mut Env)>)> = vec![
        ("opening", Box::new(|e| e.opening_unported = true)),
        ("barrage", Box::new(|e| e.barrage_unported = true)),
        ("roll", Box::new(|e| e.roll_unported = true)),
    ];
    for (name, gate) in gates {
        let mut e = env();
        gate(&mut e);
        let mut fight = Fight::new(base(), e, 2);
        let stop = fight.settle(Policy::First);
        assert_eq!(stop, Some(Stop::Bailed(Bail::Unported)), "{name}");
    }
    // A unit-type group the text table does not know: the machine cannot name the roll.
    let mut e = env();
    e.groups.clear();
    let mut fight = Fight::new(base(), e, 2);
    assert_eq!(fight.settle(Policy::First), Some(Stop::Bailed(Bail::Unported)), "no text for the roll group");
}

#[test]
fn staged_destructions_and_cancellations_and_duranium_armor_and_devotion_are_bails_too() {
    covers!("rules::combat::machine::Machine");
    let base = || arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 2).unit(sys(), None, Unit::CruiserI, 1, 2);
    // Devotion (the Yin module) is in the game: the end of a round is the string's.
    let mut e = env();
    e.devotion = true;
    let mut fight = Fight::new(base(), e, 3);
    assert_eq!(fight.settle(Policy::First), Some(Stop::Bailed(Bail::Unported)));
    // Destructions waiting for their announcement: the flow stops at a transition.
    let mut fight = Fight::new(base(), env(), 3);
    fight.state.transient_mut().combat.pending_destructions.push(ti4_engine_core::state::PendingDestruction { system: sys(), owner: Seat(1), unit: Unit::CruiserI, reason: Code(1), during_space_combat: true });
    assert_eq!(fight.settle(Policy::First), Some(Stop::Bailed(Bail::Unported)));
    // Hits a card cancelled this round and a neutral side: also the string's.
    let mut e = env();
    e.duranium = [true, true];
    let mut dur = Fight::new(base(), e, 4);
    let stop = dur.settle(Policy::First);
    assert!(matches!(stop, Some(Stop::Bailed(Bail::Unported)) | Some(Stop::Over)), "Duranium Armor bails when a side has damaged ships at the end of a round: {stop:?}");
}

#[test]
fn a_fight_that_is_over_at_the_opening_is_announced_without_a_roll() {
    covers!("rules::combat::machine::Machine");
    let mut fight = Fight::new(arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 1), env(), 0);
    fight.fields.step = Step::Opening { round: 2 };
    let stop = fight.settle(Policy::First);
    assert_eq!(stop, Some(Stop::Over));
    assert_eq!(fight.fields.outcome, Some((Some(Seat(0)), 1)));
    assert_eq!(fight.count(EventKind::SpaceCombatEnded), 1);
    assert_eq!(fight.drawn, 0);
}

#[test]
fn a_machine_that_was_stopped_stays_stopped() {
    covers!("rules::combat::machine::Machine");
    let (state, kernel) = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 1).build();
    let f = fields(&state);
    let mut rig = Rig::from_parts(state, kernel, Machine::new(env(), Fields { step: Step::Done, outcome: Some((Some(Seat(0)), 3)), ..f })).seeded(1);
    rig.run();
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.stop, Some(Stop::Over));
    assert_eq!(rig.driver.window.bailed(), None);
    assert!(rig.driver.window.fields.end_announced);
    assert_eq!(Machine::max_rounds(), 50);
}
