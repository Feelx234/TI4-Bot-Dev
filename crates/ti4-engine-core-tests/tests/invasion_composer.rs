//! Lane M3: the invasion composer (`driver::windows::invasion`) run through the live arm (`advance_invasion`): a whole invasion from the opening to the control
//! gain, held as a payload on the `Pending` stack between the questions. The leaf windows have their own tests (`invasion_windows.rs`); these check what the
//! composer adds: the order of the stages, the payload round trip, a clone in the middle, the pause for a scoring window, the bails.

use std::rc::Rc;

use ti4_engine_core::choice::invasion::Opt as IOpt;
use ti4_engine_core::containers::Cow;
use ti4_engine_core::driver::live::{ask, Env, Live, LiveError};
use ti4_engine_core::driver::windows::invasion::{advance_invasion, ask_invasion, open_at_commit, open_invasion, InvasionMachine};
use ti4_engine_core::kernel::ability::InvasionCodes;
use ti4_engine_core::kernel::stage::{AbilityTable, Held};
use ti4_engine_core::kernel::Registry;
use ti4_engine_core::state::{Code, IStage, NoteHold, Piece, WindowKind};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::rig::KitSeeds;
use ti4_engine_core_tests::scenario::plain_systems;

fn sys() -> System {
    plain_systems().into_iter().find(|s| s.data().planets.len() >= 2).expect("a two-planet tile")
}

fn planets() -> (Planet, Planet) {
    let p = sys().data().planets;
    (p[0], p[1])
}

fn arena() -> Scenario {
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys()])
}

fn codes() -> InvasionCodes {
    let mut c = InvasionCodes::default();
    let mut next = 10_u16;
    let mut mint = || {
        next += 1;
        Code(next)
    };
    c.ground = mint();
    c.bombardment = mint();
    c.ground_round = mint();
    c.space_cannon_defense = mint();
    c.bombardment_cause = mint();
    c.space_cannon_cause = mint();
    c.planets = mint();
    c.dunlain_reaper = mint();
    for p in &mut c.phases {
        *p = mint();
    }
    for n in &mut c.names {
        *n = mint();
    }
    for u in &mut c.units {
        *u = mint();
    }
    c
}

fn env(k: &Kernel, seed: u64) -> Env<'_> {
    let mut table = AbilityTable::default();
    ti4_engine_core::driver::children::register(&mut table);
    table.codes.invasion = codes();
    table.codes.ground_combat = Code(1);
    table.codes.harrow = Code(2);
    Env { kernel: k, registry: Cow::new(Registry::default()), table, seeds: Rc::new(KitSeeds(seed)) }
}

/// How a test answers: the index of the option for a question.
type Pol = dyn FnMut(&Choice) -> usize;

/// Run the open invasion to its end, closing the scoring windows it pauses for; the questions, in order, and how it ended.
fn play(core: &mut CoreState, env: &Env<'_>, pol: &mut Pol) -> (Vec<Choice>, Result<(), LiveError>) {
    let mut asked = Vec::new();
    let mut answer: Option<Pick> = None;
    for _ in 0..600 {
        match advance_invasion(core, env, answer.take()) {
            Err(e) => return (asked, Err(e)),
            Ok((Live::Asks(a), _)) => {
                let i = pol(&a.choice).min(a.choice.opts.len() - 1);
                answer = Some(Pick::Opt(a.choice.opts.as_slice()[i]));
                asked.push(a.choice);
            }
            Ok((Live::Done, _)) => {
                core.pending_mut().close(WindowKind::EventScoring);
                if core.pending().window(WindowKind::Invasion).is_none() {
                    return (asked, Ok(()));
                }
            }
        }
    }
    panic!("the invasion does not end");
}

fn first(_: &Choice) -> usize {
    0
}

fn units_on(s: &CoreState, planet: Planet, seat: u8) -> usize {
    s.board().planet(planet).units.iter().filter(|p| p.owner() == Seat(seat)).count()
}

/// The invader (seat 0) in the space area with `n` infantry, a defender (seat 1) holding `p1` with `d` infantry.
fn conquest(n: usize, d: usize) -> (CoreState, Kernel) {
    let (p1, _) = planets();
    let (mut s, k) = arena().unit(sys(), None, Unit::CarrierI, 0, 1).unit(sys(), Some(p1), Unit::InfantryI, 1, d).control(p1, 1).build();
    carry(&mut s, n);
    (s, k)
}

/// `n` infantry of seat 0 aboard in the space area (the scenario builder places no ground force in space).
fn carry(s: &mut CoreState, n: usize) {
    for _ in 0..n {
        s.board_mut().sys_mut(sys()).units.push(Piece::new(Unit::InfantryI, Seat(0)));
    }
}

#[test]
fn an_invasion_lands_fights_and_takes_the_planet_that_was_held() {
    covers!("driver::windows::invasion::InvasionMachine");
    let (p1, _) = planets();
    let (mut core, k) = conquest(6, 1);
    let env = env(&k, 3);
    open_invasion(&mut core, Seat(0), sys(), NoteHold::new());
    let (asked, end) = play(&mut core, &env, &mut first);
    assert_eq!(end, Ok(()), "the whole invasion runs in the core");
    assert!(asked.iter().any(|c| matches!(c.ask, Ask::Invasion(ti4_engine_core::choice::invasion::Ask::CommitGround { .. }))), "the invader commits");
    assert!(asked.iter().any(|c| matches!(c.ask, Ask::Invasion(ti4_engine_core::choice::invasion::Ask::FightRound { .. }))), "and fights rounds");
    assert_eq!(core.board().planet(p1).control, Some(Seat(0)), "six infantry against one take the planet");
    assert_eq!(units_on(&core, p1, 1), 0);
    assert!(units_on(&core, p1, 0) > 0);
    assert!(core.transient().invasion.active.is_none(), "the public boundary goes with the window");
    assert!(core.pending().window(WindowKind::Invasion).is_none());
}

#[test]
fn a_clone_in_the_middle_of_the_window_continues_identically() {
    covers!("driver::windows::invasion::InvasionMachine");
    let (mut core, k) = conquest(6, 2);
    let env = env(&k, 11);
    open_invasion(&mut core, Seat(0), sys(), NoteHold::new());
    let mut answer: Option<Pick> = None;
    let mut questions = 0;
    loop {
        match advance_invasion(&mut core, &env, answer.take()).expect("legal") {
            (Live::Asks(a), _) => {
                questions += 1;
                // A copy of the position at this question stands for a checkpoint: it asks the same question and ends in the same position.
                let mut copy = core.clone();
                let again = ask_invasion(&copy, &env).expect("asks").expect("a question");
                assert_eq!(again.choice, a.choice, "the copy asks the same question");
                let pick = Pick::Opt(a.choice.opts.as_slice()[0]);
                let mut rest_a = core.clone();
                let (qa, ea) = play_from(&mut rest_a, &env, pick);
                let (qb, eb) = play_from(&mut copy, &env, pick);
                assert_eq!((qa.len(), ea.is_ok()), (qb.len(), eb.is_ok()));
                assert_eq!(rest_a, copy, "after question {questions}");
                answer = Some(pick);
            }
            (Live::Done, _) => {
                core.pending_mut().close(WindowKind::EventScoring);
                if core.pending().window(WindowKind::Invasion).is_none() {
                    break;
                }
            }
        }
    }
    assert!(questions >= 3, "a commit, a fight and more");
}

fn play_from(core: &mut CoreState, env: &Env<'_>, pick: Pick) -> (Vec<Choice>, Result<(), LiveError>) {
    let mut asked = Vec::new();
    let mut answer = Some(pick);
    for _ in 0..600 {
        match advance_invasion(core, env, answer.take()) {
            Err(e) => return (asked, Err(e)),
            Ok((Live::Asks(a), _)) => {
                answer = Some(Pick::Opt(a.choice.opts.as_slice()[0]));
                asked.push(a.choice);
            }
            Ok((Live::Done, _)) => {
                core.pending_mut().close(WindowKind::EventScoring);
                if core.pending().window(WindowKind::Invasion).is_none() {
                    return (asked, Ok(()));
                }
            }
        }
    }
    panic!("the invasion does not end");
}

#[test]
fn the_payload_on_the_stack_is_the_machine_after_every_question() {
    covers!("driver::windows::invasion::InvasionMachine");
    let (mut core, k) = conquest(5, 2);
    let env = env(&k, 5);
    open_invasion(&mut core, Seat(0), sys(), NoteHold::new());
    let mut answer: Option<Pick> = None;
    for _ in 0..200 {
        match advance_invasion(&mut core, &env, answer.take()).expect("legal") {
            (Live::Asks(a), _) => {
                let machine = InvasionMachine::from_pending(&core, &k).expect("open");
                let mut again = core.clone();
                machine.to_pending(&mut again);
                assert_eq!(again.pending(), core.pending(), "to_pending(from_pending(c)) is the payload");
                assert_eq!(ask::<InvasionMachine>(&core, &env).expect("asks").map(|x| x.choice), Some(a.choice.clone()), "the payload alone says what is asked");
                answer = Some(Pick::Opt(a.choice.opts.as_slice()[0]));
            }
            (Live::Done, _) => {
                core.pending_mut().close(WindowKind::EventScoring);
                if core.pending().window(WindowKind::Invasion).is_none() {
                    return;
                }
            }
        }
    }
    panic!("the invasion does not end");
}

/// The table of an environment that explores: the texts of the log row and of the events, the links of the children.
fn exploring_env(k: &Kernel, seed: u64) -> Env<'_> {
    let mut env = env(k, seed);
    let kit = ti4_engine_core_tests::windows::explore_table();
    env.table.codes.explore = kit.codes.explore;
    env.table.facts.explore_words = kit.facts.explore_words;
    env
}

fn stack_deck(core: &mut CoreState, deck: ExploreType, cards: &[Explore]) {
    core.decks_mut().explore[deck as usize].set(cards.to_vec());
    core.decks_mut().explore_present |= 1 << deck as u8;
}

/// A first claim of a planet nobody holds: the control chain, then the exploration of the planet (the deck among its traits now, the card
/// and the log row), kept as data in the payload between the questions.
#[test]
fn a_first_claim_explores_the_planet_natively() {
    covers!("driver::windows::invasion::InvasionMachine");
    let (p1, _) = planets();
    let (mut core, k) = arena().unit(sys(), None, Unit::CarrierI, 0, 1).build();
    carry(&mut core, 2);
    let deck = ti4_engine_core::query::explore::traits_of(ti4_engine_core::tables::Scope(u32::MAX), p1).as_slice().first().copied().expect("a planet with a trait");
    stack_deck(&mut core, deck, &[Explore::CulturalRelicFragment1, Explore::HazardousRelicFragment1, Explore::IndustrialRelicFragment1]);
    let env = exploring_env(&k, 1);
    open_invasion(&mut core, Seat(0), sys(), NoteHold::new());
    let (_, end) = play(&mut core, &env, &mut first);
    assert!(end.is_ok(), "{end:?}");
    assert_eq!(core.board().planet(p1).control, Some(Seat(0)), "the planet is taken");
    let log: Vec<_> = core.explore().log.iter().collect();
    assert_eq!(log.len(), 1, "one card is explored");
    assert_eq!((log[0].planet, log[0].player, log[0].deck), (Some(p1), Seat(0), deck));
    assert!(!log[0].outcome.0.eq(&0), "the row is closed with the adapter's words");
    assert!(core.pending().window(WindowKind::Invasion).is_none());
}

/// Two planets taken for the first time in one invasion are explored one after the other, in the order of the chain, and a planet taken from
/// its holder is not explored at all.
#[test]
fn two_first_claims_are_explored_in_order_and_a_conquest_is_not_explored() {
    covers!("driver::windows::invasion::InvasionMachine");
    let (p1, p2) = planets();
    let (mut core, k) = arena().unit(sys(), None, Unit::CarrierI, 0, 1).build();
    carry(&mut core, 2);
    for deck in [ExploreType::Cultural, ExploreType::Hazardous, ExploreType::Industrial] {
        stack_deck(&mut core, deck, &[Explore::CulturalRelicFragment1, Explore::HazardousRelicFragment1, Explore::IndustrialRelicFragment1]);
    }
    let env = exploring_env(&k, 1);
    open_invasion(&mut core, Seat(0), sys(), NoteHold::new());
    // One infantry to each planet: the first commit question is answered with p1, the second with p2.
    let mut wanted = vec![p1, p2];
    let mut pick = move |c: &Choice| -> usize {
        let want = wanted.first().copied();
        let at = c.opts.as_slice().iter().position(|o| matches!(o, Opt::Invasion(IOpt::Commit { planet, .. }) if Some(*planet) == want));
        if at.is_some() {
            wanted.remove(0);
        }
        at.unwrap_or(0)
    };
    let (_, end) = play(&mut core, &env, &mut pick);
    assert!(end.is_ok(), "{end:?}");
    let explored: Vec<_> = core.explore().log.iter().map(|r| r.planet).collect();
    assert_eq!(explored, vec![Some(p1), Some(p2)], "both planets, in the order of the chain");
    assert_eq!((core.board().planet(p1).control, core.board().planet(p2).control), (Some(Seat(0)), Some(Seat(0))));
    // A conquest explores nothing.
    let (mut held, k) = conquest(6, 1);
    for deck in [ExploreType::Cultural, ExploreType::Hazardous, ExploreType::Industrial] {
        stack_deck(&mut held, deck, &[Explore::CulturalRelicFragment1]);
    }
    let env = exploring_env(&k, 1);
    open_invasion(&mut held, Seat(0), sys(), NoteHold::new());
    let (_, end) = play(&mut held, &env, &mut first);
    assert!(end.is_ok(), "{end:?}");
    assert!(held.explore().log.is_empty(), "a planet taken from its holder is not explored");
}

/// A table that gives the exploration no texts for its log row: the arm stops with a counted bail that names the exploration, and the
/// planet stays unclaimed.
#[test]
fn a_first_claim_in_a_table_with_no_texts_is_a_counted_bail() {
    covers!("driver::windows::invasion::InvasionMachine");
    let (p1, _) = planets();
    let (mut core, k) = arena().unit(sys(), None, Unit::CarrierI, 0, 1).build();
    carry(&mut core, 2);
    let env = env(&k, 1);
    open_invasion(&mut core, Seat(0), sys(), NoteHold::new());
    let (_, end) = play(&mut core, &env, &mut first);
    let Err(LiveError::Unknown(why)) = end else { panic!("a first claim bails: {end:?}") };
    assert!(format!("{why:?}").contains("Exploration(NoTexts)"), "{why:?}");
    assert_eq!(core.board().planet(p1).control, None);
}

#[test]
fn the_hero_opening_skips_the_bombardment_and_asks_for_the_commitment() {
    covers!("driver::windows::invasion::InvasionMachine");
    let (mut core, k) = conquest(4, 1);
    let env = env(&k, 2);
    open_at_commit(&mut core, Seat(0), sys(), NoteHold::new());
    assert!(matches!(InvasionMachine::from_pending(&core, &k).expect("open").p.stage, IStage::Custodians(_)));
    let (asked, end) = play(&mut core, &env, &mut first);
    assert_eq!(end, Ok(()));
    assert!(matches!(asked[0].ask, Ask::Invasion(ti4_engine_core::choice::invasion::Ask::CommitGround { .. })), "the first question is the commitment");
    assert!(asked[0].opts.as_slice().iter().any(|o| matches!(o, Opt::Invasion(IOpt::DoneCommitting))));
}

#[test]
fn the_public_boundary_follows_the_stage_the_question_is_asked_in() {
    covers!("driver::windows::invasion::InvasionMachine");
    use ti4_engine_core::choice::invasion::Ask as IAsk;
    let (p1, _) = planets();
    let (mut core, k) = conquest(6, 2);
    let env = env(&k, 9);
    let phases = codes().phases;
    open_invasion(&mut core, Seat(0), sys(), NoteHold::new());
    let mut answer: Option<Pick> = None;
    let (mut rounds_seen, mut commits_seen) = (0, 0);
    for _ in 0..200 {
        match advance_invasion(&mut core, &env, answer.take()).expect("legal") {
            (Live::Asks(a), _) => {
                let active = core.transient().invasion.active.clone().expect("an invasion is public while it runs");
                assert_eq!((active.system, active.invader), (sys(), Seat(0)));
                match a.choice.ask {
                    Ask::Invasion(IAsk::CommitGround { .. }) => {
                        commits_seen += 1;
                        assert_eq!((active.phase, active.planet, active.defender), (phases[2], None, None), "landing");
                    }
                    Ask::Invasion(IAsk::FightRound { planet, .. }) => {
                        rounds_seen += 1;
                        assert_eq!((active.phase, active.planet, active.defender), (phases[3], Some(planet), Some(Seat(1))), "ground battle");
                        assert_eq!(planet, p1);
                        assert_eq!(active.ground_round, rounds_seen - 1, "the round counter counts the rounds begun on this planet");
                        if rounds_seen > 1 {
                            let step = active.last_step.as_ref().expect("the last round is reported");
                            assert_eq!((step.planet, step.round), (p1, rounds_seen - 1));
                        }
                    }
                    _ => {}
                }
                answer = Some(Pick::Opt(a.choice.opts.as_slice()[0]));
            }
            (Live::Done, _) => {
                core.pending_mut().close(WindowKind::EventScoring);
                if core.pending().window(WindowKind::Invasion).is_none() {
                    assert!(commits_seen > 0 && rounds_seen > 0);
                    return;
                }
            }
        }
    }
    panic!("the invasion does not end");
}

/// W16: the invader's Jol-Nar commander is asked about each die of the bombardment by the composer (it was a counted bail,
/// `Unported::JolNarCommanderReroll`). The walk is held in the payload between the questions: a copy taken at every question asks the same
/// one and ends in the same position, whatever it answers.
#[test]
fn the_commander_of_the_invader_is_asked_about_each_die_of_the_bombardment_and_the_walk_survives_a_clone() {
    covers!("driver::windows::invasion::InvasionMachine");
    covers!("kernel::commander_walk::Walk");
    use ti4_engine_core::choice::combat::{Ask as CAsk, RerollSource};
    let (p1, _) = planets();
    let mut walked = 0;
    for seed in 0..12_u64 {
        let (mut core, k) = arena().unit(sys(), None, Unit::DreadnoughtI, 0, 3).unit(sys(), Some(p1), Unit::InfantryI, 1, 2).control(p1, 1).build();
        carry(&mut core, 3);
        core.players_mut().edit(0).leaders.status.insert(ti4_engine_core::generated::Leader::AgnlanOln, ti4_engine_core::state::LeaderStatus::Unlocked);
        let env = env(&k, seed);
        open_invasion(&mut core, Seat(0), sys(), NoteHold::new());
        let mut answer: Option<Pick> = None;
        let mut asks = Vec::new();
        for _ in 0..200 {
            match advance_invasion(&mut core, &env, answer.take()).expect("legal") {
                (Live::Asks(a), _) => {
                    let is_walk = matches!(a.choice.ask, Ask::Combat(CAsk::RerollDie { source: RerollSource::Commander, .. }));
                    // Reroll every other die, keep the rest; the other questions take their first option.
                    let pick = Pick::Opt(a.choice.opts.as_slice()[if is_walk { usize::from(asks.len() % 2 == 1) } else { 0 }]);
                    let mut copy = core.clone();
                    let again = ask_invasion(&copy, &env).expect("asks").expect("a question");
                    assert_eq!(again.choice, a.choice, "seed {seed}: the copy asks the same question");
                    let mut rest_a = core.clone();
                    let (qa, ea) = play_from(&mut rest_a, &env, pick);
                    let (qb, eb) = play_from(&mut copy, &env, pick);
                    assert_eq!((qa.len(), ea.is_ok()), (qb.len(), eb.is_ok()));
                    assert_eq!(rest_a, copy, "seed {seed}: after question {}", asks.len());
                    asks.push(a.choice.clone());
                    answer = Some(pick);
                }
                (Live::Done, _) => {
                    core.pending_mut().close(WindowKind::EventScoring);
                    if core.pending().window(WindowKind::Invasion).is_none() {
                        break;
                    }
                }
            }
        }
        let walk: Vec<&Choice> = asks.iter().filter(|c| matches!(c.ask, Ask::Combat(CAsk::RerollDie { .. }))).collect();
        assert_eq!(walk.len(), 3, "seed {seed}: one question for each die of the three dreadnoughts (BOMBARDMENT 5, one die each)");
        assert!(walk.iter().all(|c| c.actor == Seat(0)), "the invader is asked");
        // With no renderer table the label is the staged entry's own code: the state's text of the unit type.
        assert!(walk.iter().all(|c| matches!(c.ask, Ask::Combat(CAsk::RerollDie { unit, .. }) if unit == codes().units[Unit::DreadnoughtI as usize])), "seed {seed}: the labels");
        // The walk comes first: before the planet's hits are rolled into a fight and before any commitment.
        assert!(matches!(asks[0].ask, Ask::Combat(CAsk::RerollDie { .. })));
        assert!(core.transient().combat.reroll_staging.is_empty() && core.transient().combat.last_reroll_player.is_none(), "the staging is spent");
        walked += walk.len();
    }
    assert_eq!(walked, 36);
}

/// W16: the same bombardment without the commander asks nothing (a walk needs the ability, not only dice), and with it the questions are
/// labelled by the renderer's text of the unit type when the table holds one.
#[test]
fn a_bombardment_without_the_commander_is_not_walked_and_the_labels_are_the_renderers() {
    covers!("driver::windows::invasion::InvasionMachine");
    use ti4_engine_core::choice::combat::{Ask as CAsk, RerollSource};
    let (p1, _) = planets();
    for seed in 0..6_u64 {
        let (mut core, k) = arena().unit(sys(), None, Unit::DreadnoughtI, 0, 3).unit(sys(), Some(p1), Unit::InfantryI, 1, 2).control(p1, 1).build();
        carry(&mut core, 3);
        let env = env(&k, seed);
        open_invasion(&mut core, Seat(0), sys(), NoteHold::new());
        let (asked, end) = play(&mut core, &env, &mut first);
        assert_eq!(end, Ok(()));
        assert!(asked.iter().all(|c| !matches!(c.ask, Ask::Combat(CAsk::RerollDie { .. }))), "seed {seed}: no commander, no walk");
    }
    // With the commander and a table that holds the renderer's texts, the label is that text.
    let (mut core, k) = arena().unit(sys(), None, Unit::DreadnoughtI, 0, 2).unit(sys(), Some(p1), Unit::InfantryI, 1, 2).control(p1, 1).build();
    carry(&mut core, 3);
    core.players_mut().edit(0).leaders.status.insert(ti4_engine_core::generated::Leader::AgnlanOln, ti4_engine_core::state::LeaderStatus::Unlocked);
    let mut env = env(&k, 1);
    env.table.codes.invasion.shown_units[Unit::DreadnoughtI as usize] = Code(777);
    open_invasion(&mut core, Seat(0), sys(), NoteHold::new());
    let (asked, _) = play(&mut core, &env, &mut first);
    let labels: Vec<Code> = asked.iter().filter_map(|c| if let Ask::Combat(CAsk::RerollDie { source: RerollSource::Commander, unit, .. }) = c.ask { Some(unit) } else { None }).collect();
    assert_eq!(labels, vec![Code(777), Code(777)]);
}

/// A closure that commits to `wanted` in the order given (the first commit question is answered with the first planet, and so on; any other question with
/// its first option).
fn committing(mut wanted: Vec<Planet>) -> impl FnMut(&Choice) -> usize {
    move |c: &Choice| {
        let want = wanted.first().copied();
        let at = c.opts.as_slice().iter().position(|o| matches!(o, Opt::Invasion(IOpt::Commit { planet, .. }) if Some(*planet) == want));
        if at.is_some() {
            wanted.remove(0);
        }
        at.unwrap_or(0)
    }
}

/// A planet whose attackers all died is passed over and the next planet of the chain is still taken (the walk goes one planet on, not two); the one
/// taken is announced and, nobody having held it, explored.
#[test]
fn a_planet_the_invader_lost_the_fight_for_is_passed_over_and_the_next_one_is_taken() {
    covers!("driver::windows::invasion::InvasionMachine");
    let (p1, p2) = planets();
    let (mut core, k) = arena().unit(sys(), None, Unit::CarrierI, 0, 1).unit(sys(), Some(p1), Unit::InfantryI, 1, 12).control(p1, 1).build();
    carry(&mut core, 8);
    for deck in [ExploreType::Cultural, ExploreType::Hazardous, ExploreType::Industrial] {
        stack_deck(&mut core, deck, &[Explore::CulturalRelicFragment1, Explore::HazardousRelicFragment1, Explore::IndustrialRelicFragment1]);
    }
    open_invasion(&mut core, Seat(0), sys(), NoteHold::new());
    let env = exploring_env(&k, 1);
    let (_, end) = play(&mut core, &env, &mut committing(vec![p1, p2, p2, p2, p2, p2, p2, p2]));
    assert!(end.is_ok(), "{end:?}");
    assert_eq!(core.board().planet(p1).control, Some(Seat(1)), "the lone attacker lost");
    assert_eq!(core.board().planet(p2).control, Some(Seat(0)), "the second planet is taken after the first was passed over");
    assert_eq!(core.explore().log.iter().map(|r| r.planet).collect::<Vec<_>>(), vec![Some(p2)], "and explored once");
}

/// Two planets conquered one after the other: the chain goes on to the second.
#[test]
fn two_planets_conquered_in_one_invasion_are_both_taken() {
    covers!("driver::windows::invasion::InvasionMachine");
    let (p1, p2) = planets();
    let (mut core, k) = arena().unit(sys(), None, Unit::CarrierI, 0, 1).unit(sys(), Some(p1), Unit::InfantryI, 1, 1).control(p1, 1).unit(sys(), Some(p2), Unit::InfantryI, 1, 1).control(p2, 1).build();
    carry(&mut core, 8);
    open_invasion(&mut core, Seat(0), sys(), NoteHold::new());
    let env = exploring_env(&k, 3);
    let (_, end) = play(&mut core, &env, &mut committing(vec![p1, p2, p1, p2, p1, p2, p1, p2]));
    assert!(end.is_ok(), "{end:?}");
    assert_eq!((core.board().planet(p1).control, core.board().planet(p2).control), (Some(Seat(0)), Some(Seat(0))));
}

/// A planet taken from the holder whose home system it is leaves the chain open for the next planet: both are taken.
#[test]
fn losing_a_home_planet_does_not_end_the_chain_of_control() {
    covers!("driver::windows::invasion::InvasionMachine");
    let (p1, p2) = planets();
    let (mut core, k) = arena().unit(sys(), None, Unit::CarrierI, 0, 1).unit(sys(), Some(p1), Unit::InfantryI, 1, 1).control(p1, 1).unit(sys(), Some(p2), Unit::InfantryI, 1, 1).control(p2, 1).build();
    core.players_mut().edit(1).home_system = Some(sys());
    carry(&mut core, 8);
    open_invasion(&mut core, Seat(0), sys(), NoteHold::new());
    let mut wanted = vec![p1, p2, p1, p2];
    let mut pick = move |c: &Choice| -> usize {
        let want = wanted.first().copied();
        let at = c.opts.as_slice().iter().position(|o| matches!(o, Opt::Invasion(IOpt::Commit { planet, .. }) if Some(*planet) == want));
        if at.is_some() {
            wanted.remove(0);
        }
        at.unwrap_or(0)
    };
    let env = exploring_env(&k, 3);
    let (_, end) = play(&mut core, &env, &mut pick);
    assert!(end.is_ok(), "{end:?}");
    assert_eq!((core.board().planet(p1).control, core.board().planet(p2).control), (Some(Seat(0)), Some(Seat(0))), "both home-system planets change hands");
    let lost = core.players().by_seat(Seat(1)).unwrap().misc.event_feats.as_slice().iter().filter(|(feat, _)| *feat == ti4_engine_core::state::Feat::LostAHomePlanet).count();
    assert_eq!(lost, 2, "each home planet lost is a feat of the holder, noted against its own occurrence");
}

/// The play facts of a live table (the unit roster the production windows read), for the tests of Integrated Economy.
fn with_plays(env: &mut Env<'_>) {
    use ti4_engine_core::kernel::ability::{PlayFacts, PlaysProvider, SharedPlay};
    let roster: &'static ti4_engine_core::query::tech::UnitRoster = Box::leak(Box::default());
    env.table.facts.plays_provider = Some(PlaysProvider(std::sync::Arc::new(move |c, _| {
        Some(PlayFacts {
            scope: 0,
            blanked: false,
            action_card: Code(0),
            supply_base: Code(0),
            note_order: |a, b| a.cmp(&b),
            seats: Vec::new(),
            name_group: Vec::new(),
            has_effect: Vec::new(),
            skimmer: false,
            roster: Some(roster),
            rules_version: c.meta().rules_version,
            tech_order: |a, b| a.cmp(&b),
            shared: SharedPlay::default(),
        })
    })));
}

/// Lane L: a planet taken by invasion while the invader owns Integrated Economy no longer bails (`Unported(IntegratedEconomy)`): the production on the
/// planet gained is asked in the control gain (the body of Peace Accords' too, `strategic::perform::integrated_economy`), then the invasion ends.
#[test]
fn a_planet_taken_with_integrated_economy_asks_the_production_and_the_invasion_goes_on() {
    use ti4_engine_core::choice::production::{Ask as PAsk, Opt as POpt};
    covers!("driver::windows::invasion::InvasionMachine");
    let (p1, p2) = planets();
    let target = if p1.data().resources > 0 { p1 } else { p2 };
    assert!(target.data().resources > 0, "a planet with resources on the tile");
    let (mut core, k) = conquest(6, 1);
    // The taken planet must be the one with resources: hold it with the defender.
    core.board_mut().planet_mut(p1).control = None;
    core.board_mut().planet_mut(target).control = Some(Seat(1));
    core.board_mut().planet_mut(target).units.push(Piece::new(Unit::InfantryI, Seat(1)));
    core.board_mut().planet_mut(p1).units.clear();
    if p1 != target {
        core.board_mut().planet_mut(p1).control = None;
    }
    let edit = core.players_mut().edit(0);
    edit.tech.owned.insert(Technology::IntegratedEconomy);
    edit.econ.trade_goods = 8;
    let mut env = env(&k, 3);
    with_plays(&mut env);
    open_invasion(&mut core, Seat(0), sys(), NoteHold::new());
    let mut stop = |c: &Choice| -> usize { c.opts.as_slice().iter().position(|o| matches!(o, Opt::Production(POpt::Stop))).unwrap_or(0) };
    let (asked, end) = play(&mut core, &env, &mut stop);
    assert_eq!(end, Ok(()), "the invasion is native with Integrated Economy");
    assert_eq!(core.board().planet(target).control, Some(Seat(0)), "the planet is taken");
    assert!(asked.iter().any(|c| matches!(c.ask, Ask::Production(PAsk::Economy(_)))), "the production on the planet gained is asked: {:?}", asked.iter().map(|c| c.ask).collect::<Vec<_>>());
}

/// Lane L: a FIRST claim of a planet by an invader who owns Integrated Economy: the production is asked after the capture and the planet is explored after it (the
/// order of `technology::control_gained`, then the exploration), not instead of it.
#[test]
fn a_first_claim_with_integrated_economy_produces_and_then_explores() {
    use ti4_engine_core::choice::production::{Ask as PAsk, Opt as POpt};
    covers!("driver::windows::invasion::InvasionMachine");
    let (p1, _) = planets();
    assert!(p1.data().resources > 0, "the first planet of the tile has resources to produce with");
    let (mut core, k) = arena().unit(sys(), None, Unit::CarrierI, 0, 1).build();
    carry(&mut core, 2);
    let deck = ti4_engine_core::query::explore::traits_of(ti4_engine_core::tables::Scope(u32::MAX), p1).as_slice().first().copied().expect("a planet with a trait");
    stack_deck(&mut core, deck, &[Explore::CulturalRelicFragment1, Explore::HazardousRelicFragment1, Explore::IndustrialRelicFragment1]);
    let edit = core.players_mut().edit(0);
    edit.tech.owned.insert(Technology::IntegratedEconomy);
    edit.econ.trade_goods = 8;
    let mut env = exploring_env(&k, 1);
    with_plays(&mut env);
    open_invasion(&mut core, Seat(0), sys(), NoteHold::new());
    let mut stop = |c: &Choice| -> usize { c.opts.as_slice().iter().position(|o| matches!(o, Opt::Production(POpt::Stop))).unwrap_or(0) };
    let (asked, end) = play(&mut core, &env, &mut stop);
    assert!(end.is_ok(), "{end:?}");
    assert_eq!(core.board().planet(p1).control, Some(Seat(0)), "the planet is taken");
    assert!(asked.iter().any(|c| matches!(c.ask, Ask::Production(PAsk::Economy(_)))), "the production is asked");
    assert_eq!(core.explore().log.iter().count(), 1, "and the first claim is explored after it");
}

// ---- X-89 Bacterial Weapon in a native game (silent gap N-01 / F-technology-x89c4) ---------------------------------------------------
//
// The deck holds the Codex IV printing (`X89BacterialWeaponOmegaOmega`, id `x89c4`); the core used to read the base printing, which is in no game, so a seat
// that researched the card never had its hits doubled nor its bombarded planets exhausted. The window tests hand the flag in; these give the technology.

/// The defender infantry left on `p1`, the planet's exhaustion and who holds it after a whole invasion with `holder` (if any) owning the card.
fn x89_run(scenario: Scenario, holder: Option<u8>, seed: u64, carried: usize, dreadnoughts: bool) -> (usize, usize, bool, Option<Seat>) {
    let (p1, _) = planets();
    let scenario = if dreadnoughts { scenario.unit(sys(), None, Unit::DreadnoughtI, 0, 6) } else { scenario.unit(sys(), None, Unit::CarrierI, 0, 1) };
    let scenario = match holder {
        Some(seat) => scenario.tech(seat, Technology::X89BacterialWeaponOmegaOmega),
        None => scenario,
    };
    let (mut core, k) = scenario.build();
    carry(&mut core, carried);
    let env = env(&k, seed);
    open_invasion(&mut core, Seat(0), sys(), NoteHold::new());
    let (_, end) = play(&mut core, &env, &mut first);
    assert_eq!(end, Ok(()), "seed {seed}");
    let exhausted = core.board().planet(p1).flags.has(ti4_engine_core::state::PlanetFlags::EXHAUSTED);
    (units_on(&core, p1, 1), units_on(&core, p1, 0), exhausted, core.board().planet(p1).control)
}

fn defended(n: usize) -> Scenario {
    let (p1, _) = planets();
    arena().unit(sys(), Some(p1), Unit::InfantryI, 1, n).control(p1, 1)
}

#[test]
fn x89_doubles_the_hits_of_the_holders_bombardment_and_exhausts_the_bombarded_planet_in_a_native_game() {
    covers!("driver::windows::invasion::InvasionMachine");
    let mut hit_somewhere = false;
    for seed in 0..24_u64 {
        let (plain_left, _, plain_exhausted, _) = x89_run(defended(40), None, seed, 0, true);
        let (x89_left, _, x89_exhausted, _) = x89_run(defended(40), Some(0), seed, 0, true);
        let (killed_plain, killed_x89) = (40 - plain_left, 40 - x89_left);
        assert_eq!(killed_x89, 2 * killed_plain, "seed {seed}: the same dice, twice the hits");
        hit_somewhere |= killed_plain > 0;
        assert!(!plain_exhausted, "seed {seed}: without the card the planet stays ready");
        assert!(x89_exhausted, "seed {seed}: the bombarded planet is exhausted");
    }
    assert!(hit_somewhere, "some seed hits");
}

#[test]
fn x89_held_by_the_defender_leaves_the_invaders_bombardment_alone() {
    covers!("driver::windows::invasion::InvasionMachine");
    for seed in 0..8_u64 {
        let plain = x89_run(defended(40), None, seed, 0, true);
        let rival = x89_run(defended(40), Some(1), seed, 0, true);
        assert_eq!((plain.0, plain.2), (rival.0, rival.2), "seed {seed}: the invader's rolls are the invader's card");
    }
}

#[test]
fn x89_doubles_the_ground_hits_of_the_holder_on_either_side_in_a_native_game() {
    covers!("driver::windows::invasion::InvasionMachine");
    // Aggregates over the seeds: the same dice stream, so only the doubled hits can move the totals.
    let total = |holder: Option<u8>| {
        let (mut defenders, mut invaders) = (0, 0);
        for seed in 0..24_u64 {
            let (d, i, _, _) = x89_run(defended(6), holder, seed, 6, false);
            defenders += d;
            invaders += i;
        }
        (defenders, invaders)
    };
    let (none, invader, defender) = (total(None), total(Some(0)), total(Some(1)));
    assert!(invader.0 < none.0, "the invader's doubled hits leave fewer defenders: {invader:?} against {none:?}");
    assert!(invader.1 > none.1, "and more of the invader's own: {invader:?} against {none:?}");
    assert!(defender.1 < none.1, "the defender's doubled hits leave fewer invaders: {defender:?} against {none:?}");
}

/// Dacxive Animators in the composed invasion: no longer a counted bail (`Unported(Dacxive)`). The holder that wins a ground combat has one more
/// infantry on the planet than the same fight (same dice) without the technology; a seat that does not hold it gets nothing.
#[test]
fn dacxive_animators_places_an_infantry_for_the_winner_of_the_composed_fight() {
    covers!("driver::windows::invasion::InvasionMachine");
    let (p1, _) = planets();
    let run = |holder: Option<u8>, n: usize, d: usize, seed: u64| {
        let (mut core, k) = conquest(n, d);
        if let Some(seat) = holder {
            core.players_mut().by_seat_mut(Seat(seat)).expect("seated").tech.owned.insert(Technology::DacxiveAnimators);
        }
        let mut env = env(&k, seed);
        with_plays(&mut env);
        open_invasion(&mut core, Seat(0), sys(), NoteHold::new());
        let (_, end) = play(&mut core, &env, &mut first);
        assert_eq!(end, Ok(()), "holder {holder:?} seed {seed}: the invasion runs in the core");
        (units_on(&core, p1, 0), units_on(&core, p1, 1))
    };
    let (mut invader_won, mut defender_won) = (0, 0);
    for seed in 0..12 {
        // Invader holds it: one more infantry when it won.
        let (without, with) = (run(None, 3, 3, seed), run(Some(0), 3, 3, seed));
        if without.0 > 0 {
            invader_won += 1;
            assert_eq!((with.0, with.1), (without.0 + 1, without.1), "seed {seed}: the invader that won gets an infantry");
        } else {
            assert_eq!(with, without, "seed {seed}: a lost combat places nothing");
        }
        // Defender holds it: one more infantry when the defender won, none when the invader did.
        let with = run(Some(1), 3, 3, seed);
        if without.1 > 0 {
            defender_won += 1;
            assert_eq!(with, (without.0, without.1 + 1), "seed {seed}: the defender that won gets an infantry");
        } else {
            assert_eq!(with, without, "seed {seed}: the holder lost, nothing placed");
        }
    }
    assert!(invader_won > 0 && defender_won > 0, "the seeds cover both winners ({invader_won}, {defender_won})");
}
