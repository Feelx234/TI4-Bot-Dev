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
