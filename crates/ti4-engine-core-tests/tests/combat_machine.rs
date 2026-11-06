//! D4-2d: the composed space combat (`rules::combat::machine::Machine`): the whole fight from the opening of round 1 to the announcement of its end,
//! settle by settle, the way the aftermath's combat window drives it: a settle runs to the next decision (`Stop::Pending`), the decision is put and
//! answered by an `awaiting` machine, the next settle goes on. The rules checked are LRR 78 (the space combat) and 87 (sustain damage): the
//! announcement of retreats (defender first), the roll, the hits (sustain before assignment), the retreat of a fleet, the end.

use ti4_engine_core::choice::combat::{Ask as CAsk, Opt as COpt};
use ti4_engine_core::containers::Opt32;
use ti4_engine_core::rules::combat::flow::{Phase, Step};
use ti4_engine_core::rules::combat::end_announce::Codes as EndCodes;
use ti4_engine_core::rules::combat::machine::{Bail, Env, Fields, Machine, Stop};
use ti4_engine_core::state::{Code, EventKind, Key, LeaderStatus, Piece};
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
fn groups_of(kinds_all: &[Unit]) -> Vec<(Vec<Unit>, Code, Code)> {
    let mut out = Vec::new();
    for mask in 1u16..(1 << kinds_all.len()) {
        let mut kinds: Vec<Unit> = kinds_all.iter().enumerate().filter(|(i, _)| mask & (1 << i) != 0).map(|(_, u)| *u).collect();
        kinds.sort_unstable();
        out.push((kinds, Code(1000 + mask), Code(2000 + mask)));
    }
    out
}

fn env() -> Env {
    env_of(&KINDS)
}

/// The environment of a fight whose units are of these types.
fn env_of(kinds: &[Unit]) -> Env {
    let mut alpha = kinds.to_vec();
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
        unit_codes: kinds.iter().enumerate().map(|(i, u)| (*u, Code(100 + i as u16))).collect(),
        shown_codes: kinds.iter().enumerate().map(|(i, u)| (*u, Code(100 + i as u16))).collect(),
        barrage_extra_shown: Code(99),
        jolnar: [false, false],
        roll_unported: false,
        munitions_after: [false, false],
        munitions_start: [false, false],
        roll_cause: Code(9),
        hacan: Code(10),
        groups: groups_of(kinds),
        alpha,
        retreat_cause: Code(11),
        retreat_event: Code(11),
        retreat_pool: Code(12),
        retreat_unported: false,
        neighbors: [vec![], vec![]],
        may_retreat: [false, false],
        duranium: [false, false],
        devotion: false,
        end_codes: EndCodes { losers: Code(40), retreated: Code(41), reasons: [Code(42), Code(43), Code(44), Code(45)] },
        show_neutral: false,
        casualty: Code(13),
        commander: Code(14),
        det: [false, false],
        wormhole: false,
        ambush: Code(15),
        cause_core: Code(16),
        cause_devotion: Code(17),
        raid: Code(18),
        ac: false,
        ac_subtype: Code(19),
        ac_cause: Code(20),
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
        round_end_produced: None,
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
    // (Six defending cruisers miss the lone attacker in about one round of twenty: enough seeds to see both.)
    for seed in 0..160u64 {
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
    let mut seen = Vec::new();
    for seed in 0..40u64 {
        let mut fight = Fight::new(sc(), env(), seed);
        fight.pause_at_scoring = true;
        let stop = fight.settle(Policy::First);
        let feat = !fight.state.players().by_seat(Seat(0)).unwrap().misc.event_feats.is_empty();
        seen.push(format!("seed {seed}: {stop:?}, defender ships {}, outcome {:?}, feat {feat}", fight.ships(1, sys()), fight.fields.outcome));
        // The string engine (`a_barrage_that_takes_the_last_ship_of_a_side_still_pauses_for_scoring_...`, combat.rs) pauses for the barrage
        // feat although the fighters were the defender's last ships: the machine must pause too (it ended at `Over` before the fix, INT7).
        assert_eq!(stop, if feat { Some(Stop::Scoring) } else { Some(Stop::Over) }, "seed {seed}");
        if stop == Some(Stop::Scoring) {
            fight.fields.scoring = false;
            let again = fight.settle(Policy::First);
            assert_eq!(again, Some(Stop::Over));
        }
        if fight.ships(1, sys()) == 0 && fight.fields.outcome.map(|o| o.1) == Some(0) {
            ended_early = true;
            assert_eq!(fight.fields.outcome.unwrap().0, Some(Seat(0)));
            assert_eq!(fight.state.ledger().dice.iter().filter(|r| r.purpose == ti4_engine_core::state::RollPurpose::SpaceCombat).count(), 0, "no combat dice were rolled");
        }
    }
    assert!(ended_early, "{:#?}", &seen[..seen.len().min(6)]);
}

// ---- bails: the positions the machine does not carry ----------------------------------------------

#[test]
fn every_gate_the_string_side_closes_is_a_documented_bail() {
    covers!("rules::combat::machine::Machine");
    let base = || arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 2).unit(sys(), None, Unit::CruiserI, 1, 2);
    type Gate<'a> = (&'a str, Box<dyn Fn(&mut Env)>);
    let gates: Vec<Gate> = vec![
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

/// Found by the end-of-D1 gate (`dual_run ... driver=dual --game 3`, shard 0 of the dual-drv step): `round_seq` on `COMBAT_ROUND_STARTED` is
/// stamped only while a Letnev round agent (the owner's, or a Yssaril copy) is usable. The string asks at each announcement; the fact in
/// the `Env` is the answer taken when the settle began, and using the agent in one round (it is exhausted, or the Ssruu that copies it)
/// ends it for the later rounds of the same settle. The machine reads the state for every round, the fact is only the upper bound.
#[test]
fn the_round_seq_is_stamped_only_while_a_round_agent_is_usable_in_the_state() {
    covers!("rules::combat::machine::Machine");
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::state::LeaderStatus;
    let stamps = |fight: &Fight| -> Vec<Option<i64>> { fight.events.iter().filter(|(k, _)| *k == EventKind::CombatRoundStarted).map(|(_, p)| p.int(Key::RoundSeq)).collect() };
    let sc = |a: Faction| arena(a, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 3).unit(sys(), None, Unit::CruiserI, 1, 3);
    let mut e = env();
    e.round_seq_listens = true;
    let (mut long_plain, mut long_agent) = (0, 0);
    for seed in 0..12u64 {
        // The fact says an agent listens, the state holds none: no round carries the number.
        let mut plain = Fight::new(sc(Faction::TheEmbersOfMuaat), e.clone(), seed);
        assert_eq!(plain.settle(Policy::First), Some(Stop::Over));
        let seen = stamps(&plain);
        assert!(seen.iter().all(Option::is_none), "seed {seed}: {seen:?}");
        long_plain += usize::from(seen.len() >= 2);
        // The Letnev agent readied: every round of the settle carries its number.
        let mut agent = Fight::new(sc(Faction::TheBaronyOfLetnev), e.clone(), seed);
        agent.state.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::ViscountUnlenn, LeaderStatus::Readied);
        assert_eq!(agent.settle(Policy::First), Some(Stop::Over));
        let seen = stamps(&agent);
        assert!(seen.iter().enumerate().all(|(i, v)| *v == Some(i as i64 + 1)), "seed {seed}: {seen:?}");
        long_agent += usize::from(seen.len() >= 2);
    }
    assert!(long_plain > 0 && long_agent > 0, "some fights went on for more than one round: {long_plain} {long_agent}");
}

/// Found by the end-of-D1 gate (`dual_run ... driver=dual --game 18`): a technology gained at the start of the combat swaps the owner's
/// Destroyer I to a Destroyer II; the settle's unit texts were interned for the board as it stood when the settle began, so the
/// barrage dice of the new type were labelled `extra barrage` (the relic's label) where the string names `destroyer2`.
#[test]
fn a_barrage_by_a_unit_type_the_settle_has_no_text_for_is_a_bail_not_an_extra_barrage_label() {
    covers!("rules::combat::machine::Machine");
    let sc = || arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::DestroyerII, 0, 1).unit(sys(), None, Unit::CruiserI, 1, 2);
    // The texts of the board the settle began on: no Destroyer II among them.
    let mut e = env();
    e.unit_codes.retain(|(u, _)| *u != Unit::DestroyerII);
    let mut fight = Fight::new(sc(), e, 1);
    assert_eq!(fight.settle(Policy::First), Some(Stop::Bailed(Bail::Unported)));
    assert_eq!(fight.count(EventKind::UnitAbilityRolled), 0, "no barrage was played under a wrong label");
    // With the text the same position is fought (and the dice carry the unit's own label).
    let mut fight = Fight::new(sc(), env(), 1);
    assert_eq!(fight.settle(Policy::First), Some(Stop::Over));
    let dice = &fight.state.transient().combat.presentation.barrage_dice;
    assert!(!dice.is_empty() && dice.iter().all(|d| d.unit != Code(8)), "{dice:?}");
}

#[test]
fn staged_destructions_and_cancellations_and_duranium_armor_and_devotion_are_bails_too() {
    covers!("rules::combat::machine::Machine");
    let base = || arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 2).unit(sys(), None, Unit::CruiserI, 1, 2);
    // Devotion (the Yin module) is in the game: since W2 the machine asks its hits after a round (a table without a Yin seat has
    // none, so the fight runs to its end); it is no longer a gate.
    let mut e = env();
    e.devotion = true;
    let mut fight = Fight::new(base(), e, 3);
    assert_eq!(fight.settle(Policy::First), Some(Stop::Over));
    // Destructions waiting for their announcement: the flow goes on with them pending (the string leaves them pending across the windows of a round) and
    // they are announced at the end of the round (it was a bail of the non-live machine until the bench stops lane).
    let mut fight = Fight::new(base(), env(), 3);
    fight.state.transient_mut().combat.pending_destructions.push(ti4_engine_core::state::PendingDestruction { system: sys(), owner: Seat(1), unit: Unit::CruiserI, reason: Code(1), during_space_combat: true });
    assert_eq!(fight.settle(Policy::First), Some(Stop::Over));
    assert!(fight.state.transient().combat.pending_destructions.is_empty(), "announced at the end of a round");
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

#[test]
fn the_ships_a_retreat_cannot_hold_are_announced_at_once_with_the_retreat_cause() {
    covers!("rules::combat::machine::Machine");
    // Package W10: a fleet of destroyers (no capacity) with fighters retreats; the fighters are stranded and lost. With staging the loss is
    // staged and the machine announces it as SHIP_DESTROYED with the renderer's text of the cause (it bailed before).
    let mut e = env();
    e.staging = true;
    e.may_retreat = [true, true];
    e.neighbors = [vec![(there(), true)], vec![(there(), true)]];
    e.retreat_cause = Code(11);
    e.retreat_event = Code(51);
    let sc = || arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), None, Unit::DestroyerI, 1, 2).unit(sys(), None, Unit::FighterI, 1, 4).unit(there(), None, Unit::CruiserI, 1, 1);
    let mut stranded_seeds = 0;
    for seed in 0..60u64 {
        let mut fight = Fight::new(sc(), e.clone(), seed);
        let stop = fight.settle(Policy::Last);
        assert_eq!(stop, Some(Stop::Over), "seed {seed}: the strand no longer bails");
        let lost: Vec<_> = fight.events.iter().filter(|(k, p)| *k == EventKind::ShipDestroyed && p.code(Key::Cause) == Some(Code(51))).collect();
        if fight.fields.fled.is_empty() {
            assert!(lost.is_empty(), "seed {seed}: no retreat, no strand");
            continue;
        }
        let fighters_before = 4;
        let fighters_there = fight.state.board().sys(there()).units.iter().filter(|p| p.owner() == Seat(1) && p.kind() == Unit::FighterI).count();
        assert_eq!(fighters_there, 0, "seed {seed}: fighters cannot retreat on their own");
        // Fighters lost to the barrage and the rounds before the retreat are not stranded: the stranded are the ones still there then.
        assert!(lost.len() <= fighters_before);
        assert!(lost.iter().all(|(_, p)| p.unit(Key::Unit) == Some(Unit::FighterI) && p.seat(Key::Player) == Some(Seat(1))));
        assert!(fight.state.transient().combat.pending_destructions.is_empty(), "nothing is left staged: the announcement drained it");
        stranded_seeds += usize::from(!lost.is_empty());
    }
    assert!(stranded_seeds > 0, "the seeds cover a retreat that stranded fighters");
}

#[test]
fn a_roll_that_empties_a_fleet_concludes_the_fight_with_the_round_of_the_roll_and_announces_its_end() {
    covers!("rules::combat::machine::Machine");
    // Package W10: a Crown of Thalnos reroll that destroys the lone ship of the roller ends the fight inside the roll step (the machine
    // bailed with `RollEndsFight` before): nothing is queued, the fight is concluded in round 1 and announced once.
    let (mut ended, mut went_on) = (0, 0);
    for seed in 0..60u64 {
        let sc = || arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), None, Unit::CruiserI, 1, 1);
        let mut fight = Fight::new(sc(), env(), seed);
        fight.state.players_mut().by_seat_mut(Seat(0)).unwrap().cards.relics.push(ti4_engine_core::generated::Relic::TheCrownOfThalnos);
        let stop = fight.settle(Policy::First);
        assert_eq!(stop, Some(Stop::Over), "seed {seed}: no bail");
        let (winner, rounds) = fight.fields.outcome.expect("an outcome");
        assert_eq!(fight.count(EventKind::SpaceCombatEnded), 1);
        let first_round_over = rounds == 1 && winner == Some(Seat(1)) && fight.ships(0, sys()) == 0;
        let crown_kill = fight.state.board().sys(sys()).units.iter().all(|p| p.owner() != Seat(0)) && fight.count(EventKind::HitsToAssign) == 0;
        if crown_kill && first_round_over {
            ended += 1;
            assert_eq!(fight.count(EventKind::SpaceCombatRoundEnded), 0, "the round never ended: the fight was over inside the roll step");
        } else {
            went_on += 1;
        }
    }
    assert!(ended > 0 && went_on > 0, "the seeds cover both: {ended} ended inside the roll, {went_on} went on");
}

/// Found by the end-of-D1 gate run (golden game 3): the string asks at every announcement whether a Letnev round agent is still usable
/// (the owner's readied one, or Ssruu copying it); using it exhausts it. The machine used to keep the fact it was built with for all
/// the rounds of its call, so it stamped `round_seq` on rounds the string did not.
#[test]
fn the_round_seq_is_stamped_only_while_a_letnev_round_agent_is_still_usable() {
    let seqs = |fight: &Fight| fight.events.iter().filter(|(k, _)| *k == EventKind::CombatRoundStarted).filter(|(_, p)| p.int(Key::RoundSeq).is_some()).count();
    let rounds = |fight: &Fight| fight.count(EventKind::CombatRoundStarted);
    // The fact says "listens" but nobody holds a usable agent any more: no round carries a sequence number.
    let mut stale = env();
    stale.round_seq_listens = true;
    let mut fight = Fight::new(skirmish(), stale, 3);
    assert_eq!(fight.settle(Policy::First), Some(Stop::Over));
    assert!(rounds(&fight) >= 1);
    assert_eq!(seqs(&fight), 0, "no usable agent: no round_seq");
    // A Letnev seat that holds Viscount Unlenn readied (rules version 17): every round carries it.
    let mut live = env();
    live.round_seq_listens = true;
    let mut fight = Fight::new(arena(Faction::TheBaronyOfLetnev, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 3).unit(sys(), None, Unit::CruiserI, 1, 3), live, 3);
    fight.state.meta_mut().rules_version = 17;
    fight.state.players_mut().edit(0).leaders.status.insert(Leader::ViscountUnlenn, LeaderStatus::Readied);
    assert_eq!(fight.settle(Policy::First), Some(Stop::Over));
    assert_eq!(seqs(&fight), rounds(&fight), "a readied owner agent listens in every round");
    assert!(rounds(&fight) >= 1);
}

// ---- the stages a settle can stop in (D4A4) -------------------------------------------------------------------------------------------

/// Settle once from the opening of round 1 and say which stage each question was asked in.
fn first_question(sc: Scenario, env: Env, seed: u64) -> (Rig<Machine>, Option<Ask>) {
    let (state, kernel) = sc.build();
    let fields = fields(&state);
    let mut rig = Rig::from_parts(state, kernel, Machine::new(env, fields)).seeded(seed);
    rig.run();
    let ask = match rig.status() {
        Some(ti4_engine_core::driver::Status::Asking(c)) => Some(c.ask.clone()),
        _ => None,
    };
    (rig, ask)
}

#[test]
fn the_machine_waits_in_the_rolling_stage_when_the_roll_asks_about_munitions_reserves() {
    covers!("rules::combat::machine::Machine");
    let sc = || arena(Faction::TheBaronyOfLetnev, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 4).unit(sys(), None, Unit::CruiserI, 1, 1).goods(0, 3);
    let mut found = false;
    for seed in 0..40u64 {
        let mut e = env();
        e.munitions_after = [true, false];
        let (rig, ask) = first_question(sc(), e, seed);
        if ask == Some(Ask::Combat(CAsk::MunitionsRoll)) {
            assert_eq!(rig.asked().actor, Seat(0));
            assert_eq!(rig.stages.last().map(String::as_str), Some("Rolling"), "{:?}", rig.stages);
            rig.assert_options_sound();
            found = true;
            break;
        }
    }
    assert!(found, "a roll with misses was found among 40 seeds");
}

#[test]
fn the_machine_waits_in_the_producing_stage_while_the_opponent_picks_the_ship_assault_cannon_destroys() {
    covers!("rules::combat::machine::Machine");
    let sc = || {
        arena(Faction::TheEmbersOfMuaat, Faction::TheArborec)
            .tech(0, ti4_engine_core::generated::Technology::AssaultCannon)
            .unit(sys(), None, Unit::CruiserI, 0, 3)
            .unit(sys(), None, Unit::CruiserI, 1, 1)
            .unit(sys(), None, Unit::DestroyerI, 1, 2)
    };
    let mut e = env();
    e.ac = true;
    let (rig, ask) = first_question(sc(), e, 1);
    assert_eq!(ask, Some(Ask::Combat(CAsk::AssignHit)));
    assert_eq!(rig.asked().actor, Seat(1), "the victim chooses which of their non-fighter ships is destroyed");
    assert_eq!(rig.stages.last().map(String::as_str), Some("Producing"), "{:?}", rig.stages);
    rig.assert_options_sound();
}

#[test]
fn the_machine_waits_in_the_barraging_stage_while_raid_formation_picks_the_ship_to_damage() {
    covers!("rules::combat::machine::Machine");
    let sc = || {
        arena(Faction::TheArgentFlight, Faction::TheArborec)
            .unit(sys(), None, Unit::DestroyerII, 0, 6)
            .unit(sys(), None, Unit::DreadnoughtI, 1, 1)
            .unit(sys(), None, Unit::WarSun, 1, 1)
            .unit(sys(), None, Unit::FighterI, 1, 1)
    };
    let mut found = false;
    for seed in 0..60u64 {
        let (rig, ask) = first_question(sc(), env(), seed);
        if matches!(ask, Some(Ask::Combat(CAsk::RaidDamage { .. }))) {
            assert_eq!(rig.asked().actor, Seat(0), "the Argent player picks the type to damage");
            assert_eq!(rig.stages.last().map(String::as_str), Some("Barraging"), "{:?}", rig.stages);
            rig.assert_options_sound();
            found = true;
            break;
        }
    }
    assert!(found, "a barrage with more hits than fighters was found among 60 seeds");
}

// ---- a neutral side (W11) ----------------------------------------------------------------------------------

const NEUTRAL_KINDS: [Unit; 6] = [Unit::CruiserI, Unit::DreadnoughtI, Unit::FighterI, Unit::NeutralDestroyer, Unit::NeutralDreadnought, Unit::NeutralFighter];

/// Seat 0 (six dreadnoughts and three cruisers) against the neutral force, an owner that is no seat (`Seat(2)`): a fighter, a destroyer and two
/// dreadnoughts, the lowest on the reference card first the fighter.
fn neutral_fight(show: bool, seed: u64) -> Fight {
    let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::DreadnoughtI, 0, 6).unit(sys(), None, Unit::CruiserI, 0, 3);
    let (mut state, kernel) = sc.build();
    for kind in [Unit::NeutralDreadnought, Unit::NeutralDestroyer, Unit::NeutralFighter, Unit::NeutralDreadnought] {
        state.board_mut().sys_mut(sys()).units.push(Piece::new(kind, Seat(2)));
    }
    let mut env = env_of(&NEUTRAL_KINDS);
    env.defender = Seat(2);
    env.show_neutral = show;
    let mut fight = Fight { state, kernel, env, fields: Fields { step: Step::Opening { round: 1 }, phase: Phase::Round, resuming: false, pending_retreats: vec![], offered: vec![], damaged: vec![], fled: vec![], at_start: vec![], scoring: false, outcome: None, end_announced: false, round_end_produced: None }, seed, events: vec![], drawn: 0, questions: vec![], stop: None, settles: 0, pause_at_scoring: false, pauses: 0 };
    let fleet = |seat: Seat| fight.state.board().sys(sys()).units.iter().filter(|p| p.owner() == seat).map(|p| p.kind()).collect::<Vec<_>>();
    fight.fields.at_start = vec![(Seat(0), fleet(Seat(0))), (Seat(2), fleet(Seat(2)))];
    fight
}

/// The ships the neutral force lost, in order.
fn neutral_losses(fight: &Fight) -> Vec<Unit> {
    fight.events.iter().filter(|(k, p)| *k == EventKind::ShipDestroyed && p.seat(Key::Player) == Some(Seat(2))).filter_map(|(_, p)| p.unit(Key::Unit)).collect()
}

#[test]
fn the_rules_answer_for_a_neutral_side_nobody_is_asked_and_the_lowest_on_the_card_goes_first() {
    covers!("rules::combat::machine::Machine");
    let mut seen = 0;
    for seed in 1..=6u64 {
        let mut fight = neutral_fight(false, seed);
        assert_eq!(fight.settle(Policy::First), Some(Stop::Over), "seed {seed}");
        assert!(fight.questions.iter().all(|c| c.actor != Seat(2)), "seed {seed}: the neutral side is never asked");
        let lost = neutral_losses(&fight);
        // Each loss is the unit lowest on the card among those standing: the fighter, then the destroyer, then the dreadnoughts.
        let order = [Unit::NeutralFighter, Unit::NeutralDestroyer, Unit::NeutralDreadnought, Unit::NeutralDreadnought];
        assert_eq!(lost, order[..lost.len()].to_vec(), "seed {seed}: {lost:?}");
        seen += lost.len();
        // A dreadnought sustains before it is lost: damage was taken (the sustain is always taken).
        if lost.contains(&Unit::NeutralDreadnought) {
            assert!(fight.events.iter().any(|(k, p)| *k == EventKind::SustainDamageUsed && p.seat(Key::Player) == Some(Seat(2))), "seed {seed}: the neutral side sustains");
        }
        assert_eq!(fight.state.board().sys(sys()).units.iter().filter(|p| p.owner() == Seat(2)).count() + lost.len(), 4, "seed {seed}: nothing else was lost");
    }
    assert!(seen > 10, "ships of the neutral force were lost: {seen}");
}

#[test]
fn a_neutral_side_may_not_retreat() {
    covers!("rules::combat::machine::Machine");
    let fight = neutral_fight(false, 1);
    let machine = Machine::new(fight.env.clone(), fight.fields.clone());
    let k = &fight.kernel;
    assert!(machine.retreats(&fight.state, k, Seat(2)).is_empty(), "the neutral force never retreats");
}

/// With the decisions shown (the lockstep with the string engine) the neutral side's sustain and casualty are put as questions of
/// the neutral seat; the only answer the machine takes is the rules' own: the first sustain option, the lowest unit on the card.
#[test]
fn a_shown_neutral_decision_takes_only_the_answer_the_rules_give() {
    covers!("rules::combat::machine::Machine");
    use ti4_engine_core::rules::combat::NEUTRAL_LOWEST_FIRST;
    let mut fight = neutral_fight(true, 2);
    let (mut sustains, mut casualties, mut refused) = (0, 0, 0);
    for _ in 0..200 {
        let mut rig = Rig::from_parts(fight.state.clone(), fight.kernel.clone(), Machine::new(fight.env.clone(), fight.fields.clone())).seeded(fight.seed);
        rig.run();
        while let Some(ti4_engine_core::driver::Status::Asking(c)) = rig.status().cloned() {
            assert_eq!(c.actor, Seat(2), "only the neutral side's decisions are put inside a settle");
            let expected = match c.ask {
                Ask::Combat(CAsk::CancelHit { .. }) => {
                    sustains += 1;
                    assert!(matches!(c.opts.as_slice()[0], Opt::Combat(COpt::Sustain { .. })));
                    0
                }
                Ask::Combat(CAsk::AssignHit) => {
                    casualties += 1;
                    let kind = NEUTRAL_LOWEST_FIRST.iter().find(|k| c.opts.iter().any(|o| matches!(o, Opt::Combat(COpt::Destroy { unit, .. }) if unit == *k))).copied().unwrap();
                    c.opts.iter().position(|o| matches!(o, Opt::Combat(COpt::Destroy { unit, .. }) if *unit == kind)).unwrap()
                }
                other => panic!("{other:?}"),
            };
            // Any other option is refused; the question stays.
            for i in (0..c.opts.len()).filter(|i| *i != expected) {
                if let Some(p) = c.pick(i) {
                    if rig.try_answer(p).is_err() {
                        refused += 1;
                    } else {
                        panic!("a pick other than the rules' answer was taken: {i} for {expected}");
                    }
                }
            }
            rig.try_answer(c.pick(expected).unwrap()).unwrap();
        }
        let m = rig.driver.window.clone();
        fight.state = rig.state.clone();
        fight.fields = m.fields.clone();
        fight.collect(&rig);
        match m.stop {
            Some(Stop::Pending) => fight.decide(Policy::First),
            Some(Stop::Scoring) => fight.fields.scoring = false,
            Some(Stop::Over) => break,
            other => panic!("{other:?}"),
        }
    }
    assert!(sustains > 0 && casualties > 0, "{sustains} sustains and {casualties} casualties were put");
    assert!(refused > 0, "other options were refused");
}

/// W11: the hits a Reflective Shielding staged while the use of SUSTAIN DAMAGE was announced are absorbed by the answer to the sustain
/// (the producer of the cancelled hit chooses its losses), before the machine goes on; the sustain still cancels its hit.
#[test]
fn the_hits_a_reflective_shielding_staged_are_absorbed_inside_the_answer_to_the_sustain() {
    covers!("rules::combat::machine::Machine");
    use ti4_engine_core::state::CombatPend;
    let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 2).unit(sys(), None, Unit::DreadnoughtI, 1, 1).unit(sys(), None, Unit::DestroyerI, 1, 1);
    let (mut state, kernel) = sc.build();
    state.transient_mut().combat.pending_reflective_hits = Some((sys(), Seat(0), 2));
    let mut fields = fields(&state);
    fields.step = Step::Queue { assigning: false, round: 1, queue: vec![CombatPend { player: Seat(1), hits: 3, producer: Seat(0), non_fighters_only: false, producer_assigns: false }] };
    let mut rig = Rig::from_parts(state, kernel, Machine::awaiting(env(), fields)).seeded(1);
    rig.run();
    let sustain = rig.asked().clone();
    assert_eq!(sustain.actor, Seat(1), "the dreadnought's owner is asked to sustain");
    assert!(matches!(sustain.ask, Ask::Combat(CAsk::CancelHit { .. })));
    rig.pick(0);
    // The two reflected hits land on the producer of the hit (seat 0): two identical cruisers, one option.
    assert_eq!(rig.asked().actor, Seat(0), "the producer absorbs the reflected hits");
    assert_eq!(rig.driver.window.reflecting_producer(), Some(Seat(1)), "the machine says whose hits it absorbs (the sustaining seat produced them)");
    assert!(matches!(rig.asked().ask, Ask::Combat(CAsk::AssignHit)));
    rig.pick(0);
    assert!(rig.is_done(), "the second reflected hit finds a single ship: no question");
    let m = rig.driver.window.clone();
    let left = rig.state.board().sys(sys()).units.iter().filter(|p| p.owner() == Seat(0)).count();
    assert_eq!(left, 0, "two hits, two cruisers");
    assert!(rig.state.transient().combat.pending_reflective_hits.is_none());
    match m.fields.step {
        Step::Queue { assigning: false, ref queue, .. } => assert_eq!(queue[0].hits, 2, "the sustain cancelled one of the three hits"),
        other => panic!("{other:?}"),
    }
}

/// Lane S: the live machine (the native loop) drains the hits a Reflective Shielding staged when the answer to the sustain is taken, as the string does
/// after every use of SUSTAIN DAMAGE: the producer of the cancelled hit chooses its losses (a question of the tape) and nothing stays staged. Before, only
/// the machine that applies a single answer (`Machine::awaiting`, the dual's) drained them, and a live game left the hits on the position (sardakk, 4 players,
/// seed 1, step 919: the staged hits were still there at the end of the turn).
#[test]
fn the_live_machine_absorbs_the_hits_a_reflective_shielding_staged_when_the_sustain_is_answered() {
    covers!("rules::combat::machine::Machine");
    use ti4_engine_core::state::CombatPend;
    let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 2).unit(sys(), None, Unit::DreadnoughtI, 1, 1).unit(sys(), None, Unit::DestroyerI, 1, 1);
    let (mut state, kernel) = sc.build();
    state.transient_mut().combat.pending_reflective_hits = Some((sys(), Seat(0), 2));
    let mut fields = fields(&state);
    fields.step = Step::Queue { assigning: false, round: 1, queue: vec![CombatPend { player: Seat(1), hits: 3, producer: Seat(0), non_fighters_only: false, producer_assigns: false }] };
    let mut rig = Rig::from_parts(state, kernel, Machine::new(env(), fields).live()).seeded(1);
    rig.run();
    let sustain = rig.asked().clone();
    assert_eq!(sustain.actor, Seat(1), "the dreadnought's owner is asked to sustain");
    assert!(matches!(sustain.ask, Ask::Combat(CAsk::CancelHit { .. })));
    rig.pick(0);
    assert_eq!(rig.asked().actor, Seat(0), "the producer absorbs the reflected hits: the question is the tape's");
    assert!(matches!(rig.asked().ask, Ask::Combat(CAsk::AssignHit)));
    rig.pick(0);
    assert!(rig.state.transient().combat.pending_reflective_hits.is_none(), "nothing stays staged");
    let left = rig.state.board().sys(sys()).units.iter().filter(|p| p.owner() == Seat(0)).count();
    assert_eq!(left, 0, "two hits, two cruisers");
}

/// W11: the machine continues a settle from a queue whose front is the neutral side's: `hits` hits are answered by the rules (one
/// sustain/loss each), then the round is announced over.
fn neutral_queue(assigning: bool, hits: u32, producer_assigns: bool) -> (Fight, Machine) {
    use ti4_engine_core::state::CombatPend;
    let mut fight = neutral_fight(false, 7);
    fight.fields.step = Step::Queue { assigning, round: 1, queue: vec![CombatPend { player: Seat(2), hits, producer: Seat(0), non_fighters_only: false, producer_assigns }] };
    let machine = Machine::new(fight.env.clone(), fight.fields.clone());
    (fight, machine)
}

#[test]
fn each_hit_on_the_neutral_side_is_one_answer_of_the_rules() {
    covers!("rules::combat::machine::Machine");
    for (assigning, hits) in [(true, 1u32), (true, 2), (false, 1)] {
        let (fight, machine) = neutral_queue(assigning, hits, false);
        let mut rig = Rig::from_parts(fight.state.clone(), fight.kernel.clone(), machine).seeded(3);
        rig.run();
        let before_round_end: Vec<_> = rig.driver.emitted.iter().take_while(|e| e.kind != EventKind::SpaceCombatRoundEnded).collect();
        let lost = before_round_end.iter().filter(|e| e.kind == EventKind::ShipDestroyed && e.payload.seat(Key::Player) == Some(Seat(2))).count();
        let sustained = before_round_end.iter().filter(|e| e.kind == EventKind::SustainDamageUsed && e.payload.seat(Key::Player) == Some(Seat(2))).count();
        if assigning {
            // After a loss the queue goes back to the sustain offer: a dreadnought takes the next hit.
            assert_eq!(lost + sustained, hits as usize, "assigning {hits}: one answer per hit");
            assert!(lost >= 1);
        } else {
            // The fighter and the destroyer cannot sustain; the first dreadnought can: one hit, one sustain, nothing lost.
            assert_eq!((lost, sustained), (0, 1), "sustaining {hits}");
        }
    }
}

#[test]
fn a_hit_the_producer_assigns_is_not_answered_by_the_rules_for_the_neutral_side() {
    covers!("rules::combat::machine::Machine");
    let (fight, machine) = neutral_queue(true, 1, true);
    let mut rig = Rig::from_parts(fight.state.clone(), fight.kernel.clone(), machine).seeded(3);
    rig.run();
    let m = rig.driver.window.clone();
    assert_eq!(m.stop, Some(Stop::Pending), "the producer places the hit (Devotion): the decision is left pending");
    assert_eq!(rig.state.board().sys(sys()).units.iter().filter(|p| p.owner() == Seat(2)).count(), 4, "nothing was taken");
}

/// W11: Assault Cannon taking the last ship of a side ends the fight inside the opening of round one: the fight is concluded with the round
/// it opened (`rounds` = 1), as the string `open_round` does right after the cannon, not with the previous round.
#[test]
fn an_assault_cannon_that_takes_the_last_ship_concludes_the_fight_with_round_one() {
    covers!("rules::combat::machine::Machine");
    let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).tech(0, Technology::AssaultCannon).unit(sys(), None, Unit::CruiserI, 0, 3).unit(sys(), None, Unit::CruiserI, 1, 1);
    let mut e = env();
    e.ac = true;
    let mut fight = Fight::new(sc, e, 3);
    assert_eq!(fight.settle(Policy::First), Some(Stop::Over));
    assert_eq!(fight.fields.outcome, Some((Some(Seat(0)), 1)));
    assert_eq!(fight.count(EventKind::ShipDestroyed), 1);
    assert_eq!(fight.drawn, 0, "no die was rolled");
}

/// W11: a lone ship is lost without a question, unless a card cancelled hits earlier in the round: the flow spends that pool first and the
/// ship survives.
#[test]
fn a_lone_ship_is_not_lost_to_a_hit_a_card_cancelled() {
    covers!("rules::combat::machine::Machine");
    use ti4_engine_core::state::CombatPend;
    let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), None, Unit::CruiserI, 1, 1);
    let (mut state, kernel) = sc.build();
    let round = state.meta().combat_round_seq;
    state.players_mut().by_seat_mut(Seat(1)).unwrap().card_timers.cancel_hits_round = Some((round, 1));
    let mut f = fields(&state);
    f.step = Step::Queue { assigning: true, round: 1, queue: vec![CombatPend { player: Seat(1), hits: 1, producer: Seat(0), non_fighters_only: false, producer_assigns: false }] };
    let mut rig = Rig::from_parts(state, kernel, Machine::new(env(), f)).seeded(1);
    rig.run();
    let lost = rig.driver.emitted.iter().take_while(|e| e.kind != EventKind::SpaceCombatRoundEnded).filter(|e| e.kind == EventKind::ShipDestroyed).count();
    assert_eq!(lost, 0, "the cancelled hit took nothing");
}

/// H4: the round whose end-of-round hits (Devotion) were produced is a field of the window, not of the machine: the live loop rebuilds the machine at
/// every call, so a guard kept in the object would be lost at the next question and the hits asked again in the same round.
#[test]
fn the_round_whose_end_hits_were_produced_is_kept_in_the_window_fields() {
    covers!("rules::combat::machine::Machine");
    let base = || arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 3).unit(sys(), None, Unit::CruiserI, 1, 3);
    let mut e = env();
    e.devotion = true;
    // A round that ends with the hits produced: the window remembers which round it was.
    let mut fight = Fight::new(base(), e.clone(), 3);
    assert_eq!(fight.fields.round_end_produced, None);
    fight.fields.step = Step::Retreating { round: 1, leaving: vec![] };
    let (state, kernel) = (fight.state.clone(), fight.kernel.clone());
    let mut rig = Rig::from_parts(state, kernel, Machine::new(e.clone(), fight.fields.clone())).seeded(3);
    rig.run();
    Fight::answer_all(&mut rig, Policy::First, &mut Vec::new());
    assert_eq!(rig.driver.window.fields.round_end_produced, Some(1), "the produced round is written to the fields the loop keeps");
    // Without Devotion in the game nothing is produced and nothing is remembered.
    let mut off = Fight::new(base(), env(), 3);
    off.fields.step = Step::Retreating { round: 1, leaving: vec![] };
    let (state, kernel) = (off.state.clone(), off.kernel.clone());
    let mut rig = Rig::from_parts(state, kernel, Machine::new(env(), off.fields.clone())).seeded(3);
    rig.run();
    Fight::answer_all(&mut rig, Policy::First, &mut Vec::new());
    assert_eq!(rig.driver.window.fields.round_end_produced, None);
    // A machine rebuilt from fields that name the round does not produce again: the field survives the rebuild (`Machine::new(env, fields)`).
    let mut again = Fight::new(base(), e.clone(), 3);
    again.fields.step = Step::Retreating { round: 1, leaving: vec![] };
    again.fields.round_end_produced = Some(1);
    let (state, kernel) = (again.state.clone(), again.kernel.clone());
    let mut rig = Rig::from_parts(state, kernel, Machine::new(e, again.fields.clone())).seeded(3);
    rig.run();
    Fight::answer_all(&mut rig, Policy::First, &mut Vec::new());
    assert_eq!(rig.driver.window.fields.round_end_produced, Some(1));
}
