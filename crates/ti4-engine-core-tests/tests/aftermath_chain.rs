//! Lane M5: the aftermath chain (`driver::windows::chain`), its space combat (`driver::windows::combat`) and the event scoring window
//! (`driver::windows::scoring`) run through the live arms: a whole aftermath held as payloads on the `Pending` stack between the questions. The leaf
//! windows have their own tests; these check what the composers add: the order of the stages, the payload round trip, a clone in the middle, the
//! hand-off to the invasion, the scoring pauses and the bails.

use std::rc::Rc;

use ti4_engine_core::choice::production::{Ask as PAsk, Opt as POpt};
use ti4_engine_core::containers::Cow;
use ti4_engine_core::driver::live::{ask, Env, Live, LiveError};
use ti4_engine_core::driver::windows::chain::{advance_aftermath, ask_aftermath, AftermathChain};
use ti4_engine_core::driver::windows::scoring::EventScoring;
use ti4_engine_core::driver::windows::status::advance_event_scoring;
use ti4_engine_core::kernel::ability::{AftermathGap, CombatTexts, Unknown};
use ti4_engine_core::kernel::stage::{AbilityTable, Held};
use ti4_engine_core::kernel::Registry;
use ti4_engine_core::state::{AFight, AStage, Aftermath, Before, Code, Emit, Feat, NoteHold, PlainKind, WindowKind};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::rig::KitSeeds;
use ti4_engine_core_tests::scenario::plain_systems;

fn sys() -> System {
    plain_systems().into_iter().find(|s| s.data().planets.len() >= 2).expect("a two-planet tile")
}

/// Texts for the unit types in `present`: every code distinct (the core never resolves them).
fn texts(present: &[Unit]) -> CombatTexts {
    let mut next = 100_u16;
    let mut mint = || {
        next += 1;
        Code(next)
    };
    let mut t = CombatTexts::default();
    for p in &mut t.phases {
        *p = mint();
    }
    let mut sorted = present.to_vec();
    sorted.sort_unstable();
    for u in &sorted {
        t.unit_codes.push((*u, mint()));
        t.shown_codes.push((*u, mint()));
    }
    for mask in 1_u32..(1 << sorted.len()) {
        let kinds: Vec<Unit> = sorted.iter().enumerate().filter(|(i, _)| mask & (1 << i) != 0).map(|(_, u)| *u).collect();
        t.groups.push((kinds, mint(), mint()));
    }
    t.barrage_extra = mint();
    t.retreat_cause = mint();
    t.barrage_extra_shown = mint();
    t.barrage_cause = mint();
    t.barrage_kind = mint();
    t.roll_cause = mint();
    t.hacan = mint();
    t.retreat_event = mint();
    t.retreat_pool = mint();
    t.raid = mint();
    t.ac_subtype = mint();
    t.ac_cause = mint();
    t.ambush = mint();
    t.cause_core = mint();
    t.cause_devotion = mint();
    t.casualty = mint();
    t.commander = mint();
    t.opponents = mint();
    for e in &mut t.end {
        *e = mint();
    }
    t
}

fn env<'a>(k: &'a Kernel, seed: u64, present: &[Unit]) -> Env<'a> {
    let mut table = AbilityTable::default();
    ti4_engine_core::driver::children::register(&mut table);
    table.codes.ground_combat = Code(1);
    table.codes.harrow = Code(2);
    // The production step the chain hands over to (lane M2): its roster (every faction's own units are the plain ones), the texts of
    // `UNITS_PRODUCED` and the link to Self-Assembly Routines' window.
    ti4_engine_core_factions::self_assembly::register_links(&mut table);
    table.facts.roster = Some(Box::leak(Box::default()));
    table.codes.production.ability = Code(901);
    table.codes.production.production = Code(902);
    table.codes.production.units = Code(903);
    table.facts.combat = texts(present);
    Env { kernel: k, registry: Cow::new(Registry::default()), table, seeds: Rc::new(KitSeeds(seed)) }
}

/// Seat 0 (active) against seat 1 in the two-planet system; the head has written the sides.
fn arena(mine: usize, theirs: usize) -> (CoreState, Kernel) {
    let (mut s, k) = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec])
        .map_from(&[sys()])
        .active(Some(0))
        .active_system(sys())
        .unit(sys(), None, Unit::DreadnoughtI, 0, mine)
        .unit(sys(), None, Unit::CruiserI, 1, theirs)
        .build();
    s.transient_mut().combat.active_space_combat = Some((sys(), Seat(0), Seat(1)));
    (s, k)
}

fn open_aftermath(core: &mut CoreState) {
    let a = Aftermath::new(Seat(0), sys(), AStage::Fighting, Before::default(), NoteHold::new());
    AftermathChain::new(a).to_pending(core);
}

type Pol = dyn FnMut(&Choice) -> usize;

fn first(_: &Choice) -> usize {
    0
}

/// Run the open aftermath to its end, closing the scoring windows it pauses for: the questions, in order, and how it ended.
fn play(core: &mut CoreState, env: &Env<'_>, pol: &mut Pol) -> (Vec<Choice>, Result<(), LiveError>) {
    let mut asked = Vec::new();
    let mut answer: Option<Pick> = None;
    for _ in 0..800 {
        match advance_aftermath(core, env, answer.take()) {
            Err(e) => return (asked, Err(e)),
            Ok((Live::Asks(a), _)) => {
                let i = pol(&a.choice).min(a.choice.opts.len() - 1);
                answer = Some(a.choice.pick(i).expect("an answer"));
                asked.push(a.choice);
            }
            Ok((Live::Done, _)) => {
                core.pending_mut().close(WindowKind::EventScoring);
                if core.pending().window(WindowKind::Aftermath).is_none() {
                    return (asked, Ok(()));
                }
            }
        }
    }
    panic!("the aftermath does not end");
}

fn plain_kinds(core: &CoreState) -> Vec<PlainKind> {
    core.ledger().events.iter().filter_map(|e| if let Emit::Plain(p) = e { Some(p.kind) } else { None }).collect()
}

#[test]
fn a_space_fight_runs_to_the_end_and_announces_what_the_string_announces() {
    covers!("driver::windows::chain::AftermathChain");
    covers!("driver::windows::combat::CombatLive");
    let (mut core, k) = arena(3, 2);
    let env = env(&k, 7, &[Unit::DreadnoughtI, Unit::CruiserI]);
    open_aftermath(&mut core);
    let (asked, end) = play(&mut core, &env, &mut first);
    assert_eq!(end, Ok(()), "the whole aftermath runs in the core");
    assert!(!asked.is_empty(), "the fight asks a sustain or a casualty");
    assert!(core.pending().window(WindowKind::Combat).is_none() && core.pending().window(WindowKind::Aftermath).is_none());
    let kinds = plain_kinds(&core);
    assert_eq!(kinds, [PlainKind::SpaceCombatResolved, PlainKind::InvasionResolved, PlainKind::ProductionResolved], "the three step ends, after the step");
    assert!(core.transient().combat.active_space_combat.is_none(), "the fight's scratch is cleared");
}

#[test]
fn a_clone_in_the_middle_of_the_window_continues_identically() {
    covers!("driver::windows::chain::AftermathChain");
    let (mut core, k) = arena(3, 3);
    let env = env(&k, 11, &[Unit::DreadnoughtI, Unit::CruiserI]);
    open_aftermath(&mut core);
    let mut answer: Option<Pick> = None;
    let mut questions = 0;
    loop {
        match advance_aftermath(&mut core, &env, answer.take()).expect("legal") {
            (Live::Asks(a), _) => {
                questions += 1;
                let mut copy = core.clone();
                let again = ask_aftermath(&copy, &env).expect("asks").expect("a question");
                // (The subtype's text code is the adapter's: a window rebuilt without the table puts the default; everything else is equal.)
                assert_eq!((again.choice.actor, again.choice.ask, &again.choice.opts), (a.choice.actor, a.choice.ask, &a.choice.opts), "the copy asks the same question");
                let pick = a.choice.pick(0).expect("an answer");
                let mut rest = core.clone();
                let p = |_: &Choice| 0;
                let run = |c: &mut CoreState| {
                    let mut answer = Some(pick);
                    for _ in 0..800 {
                        match advance_aftermath(c, &env, answer.take()).expect("legal") {
                            (Live::Asks(a), _) => answer = Some(a.choice.pick(p(&a.choice)).expect("an answer")),
                            (Live::Done, _) => {
                                c.pending_mut().close(WindowKind::EventScoring);
                                if c.pending().window(WindowKind::Aftermath).is_none() {
                                    return;
                                }
                            }
                        }
                    }
                    panic!("does not end");
                };
                run(&mut rest);
                run(&mut copy);
                assert_eq!(rest, copy, "after question {questions}");
                answer = Some(pick);
            }
            (Live::Done, _) => {
                core.pending_mut().close(WindowKind::EventScoring);
                if core.pending().window(WindowKind::Aftermath).is_none() {
                    break;
                }
            }
        }
    }
    assert!(questions >= 2);
}

#[test]
fn the_payload_on_the_stack_is_the_chain_after_every_question() {
    covers!("driver::windows::chain::AftermathChain");
    let (mut core, k) = arena(3, 3);
    let env = env(&k, 5, &[Unit::DreadnoughtI, Unit::CruiserI]);
    open_aftermath(&mut core);
    let mut answer: Option<Pick> = None;
    for _ in 0..400 {
        match advance_aftermath(&mut core, &env, answer.take()).expect("legal") {
            (Live::Asks(a), _) => {
                let held = AftermathChain::from_pending(&core, &k).expect("open");
                let mut copy = core.clone();
                held.to_pending(&mut copy);
                assert_eq!(core, copy, "to_pending(from_pending(c)) is the payload");
                answer = Some(a.choice.pick(0).expect("an answer"));
            }
            (Live::Done, _) => {
                core.pending_mut().close(WindowKind::EventScoring);
                if core.pending().window(WindowKind::Aftermath).is_none() {
                    return;
                }
            }
        }
    }
    panic!("does not end");
}

#[test]
fn an_undefended_system_goes_straight_on_to_the_invasion() {
    covers!("driver::windows::chain::AftermathChain");
    let (p1, _) = (sys().data().planets[0], 0);
    let (mut core, k) = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec])
        .map_from(&[sys()])
        .active(Some(0))
        .active_system(sys())
        .tokens(0, 3, 6, 3)
        .unit(sys(), None, Unit::CarrierI, 0, 1)
        .unit(sys(), Some(p1), Unit::InfantryI, 1, 1)
        .control(p1, 1)
        .build();
    for _ in 0..4 {
        core.board_mut().sys_mut(sys()).units.push(ti4_engine_core::state::Piece::new(Unit::InfantryI, Seat(0)));
    }
    let env = env(&k, 3, &[]);
    open_aftermath(&mut core);
    let (asked, end) = play(&mut core, &env, &mut first);
    assert_eq!(end, Ok(()));
    assert!(asked.iter().any(|c| matches!(c.ask, Ask::Invasion(_))), "the invasion's questions come through the aftermath arm: {:?}", asked.iter().map(|c| c.ask).collect::<Vec<_>>());
    assert_eq!(core.board().planet(p1).control, Some(Seat(0)), "four infantry take a planet held by one");
    let kinds = plain_kinds(&core);
    assert_eq!(&kinds[kinds.len() - 2..], [PlainKind::InvasionResolved, PlainKind::ProductionResolved]);
    assert!(!kinds.contains(&PlainKind::SpaceCombatResolved), "a fight of no rounds announces nothing");
}

/// The string mirrors its timing log into `Game::events` when the step returns: what the invasion emitted (the landing, the control) is in the
/// queue, in the order of the emissions, before the aftermath's own plain events (`INVASION_RESOLVED`). Found by `native_play --shadow` (sol, 4
/// players, seed 1: the native events of a first claim were `INVASION_RESOLVED` alone).
#[test]
fn the_events_an_invasion_emitted_are_mirrored_into_the_queue_before_its_resolved_line() {
    covers!("driver::windows::chain::AftermathChain");
    let (p1, _) = (sys().data().planets[0], 0);
    let (mut core, k) = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec])
        .map_from(&[sys()])
        .active(Some(0))
        .active_system(sys())
        .tokens(0, 3, 6, 3)
        .unit(sys(), None, Unit::CarrierI, 0, 1)
        .unit(sys(), Some(p1), Unit::InfantryI, 1, 1)
        .control(p1, 1)
        .build();
    for _ in 0..4 {
        core.board_mut().sys_mut(sys()).units.push(ti4_engine_core::state::Piece::new(Unit::InfantryI, Seat(0)));
    }
    let env = env(&k, 3, &[]);
    open_aftermath(&mut core);
    let (_, end) = play(&mut core, &env, &mut first);
    assert_eq!(end, Ok(()));
    let line: Vec<String> = core
        .ledger()
        .events
        .iter()
        .map(|e| match e {
            Emit::Typed(t) => format!("typed {:?}", t.kind),
            Emit::Plain(p) => format!("plain {:?}", p.kind),
        })
        .collect();
    let at = |what: &str| line.iter().position(|l| l == what).unwrap_or_else(|| panic!("{what} is not in the queue: {line:?}"));
    assert!(at("typed UnitsCommitted") < at("typed PlanetControlGained"), "{line:?}");
    assert!(at("typed PlanetControlGained") < at("plain InvasionResolved"), "the typed events come before the aftermath's own line: {line:?}");
    assert_eq!(line.iter().filter(|l| *l == "typed PlanetControlGained").count(), 1, "one planet changed hands, mirrored once: {line:?}");
}

/// The string's `AftermathWindow::new` settles the combat window, and so presents the finished (empty) fight, unless the head's space cannon offense
/// paused for a scoring window; after such a pause the window it built is already done and nothing is presented, whether a side holds the system
/// or nobody does. Lane H3 found the pause case (the Argent commander's lockstep); integration 13 found that the same rule must not apply without a
/// pause (a tactical action that moved no ship: the whole-game shadow, 35 of 96 games); lane S found that a side that holds the system does not
/// change it (the string, jolnar 5 players seed 1: the guns of the defender destroyed the activator's only ship): the case `(1, true)` was `true`.
#[test]
fn an_empty_system_presents_the_finished_fight_unless_the_head_paused() {
    covers!("driver::windows::chain::AftermathChain");
    for (ships, paused, presented) in [(0_usize, false, true), (0, true, false), (1, false, true), (1, true, false)] {
        let (mut core, k) = {
            let mut sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys()]).active(Some(0)).active_system(sys()).tokens(0, 3, 6, 3);
            if ships > 0 {
                sc = sc.unit(sys(), None, Unit::CarrierI, 0, 1);
            }
            sc.build()
        };
        let env = env(&k, 3, &[]);
        let untouched = core.transient().combat.presentation.phase;
        let mut a = Aftermath::new(Seat(0), sys(), AStage::Fighting, Before::default(), NoteHold::new());
        if paused {
            a.event_scoring = Some((5, ti4_engine_core::state::PauseLimit::OnePerPlayer));
        }
        AftermathChain::new(a).to_pending(&mut core);
        let (_, end) = play(&mut core, &env, &mut first);
        assert_eq!(end, Ok(()), "{ships} ships, paused {paused}");
        let complete = env.table.facts.combat.phases[4];
        assert_eq!(core.transient().combat.presentation.phase == complete, presented, "{ships} ships, paused {paused}: the finished fight is presented");
        assert!(presented || core.transient().combat.presentation.phase == untouched, "{ships} ships, paused {paused}: nothing is written after the head's pause");
    }
}

#[test]
fn the_skip_to_the_commit_step_opens_the_invasion_at_its_commit_step() {
    covers!("driver::windows::chain::AftermathChain");
    let (p1, _) = (sys().data().planets[0], 0);
    let (mut core, k) = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec])
        .map_from(&[sys()])
        .active(Some(0))
        .active_system(sys())
        .unit(sys(), None, Unit::CarrierI, 0, 1)
        .unit(sys(), Some(p1), Unit::InfantryI, 1, 1)
        .control(p1, 1)
        .build();
    core.board_mut().sys_mut(sys()).units.push(ti4_engine_core::state::Piece::new(Unit::InfantryI, Seat(0)));
    let env = env(&k, 3, &[]);
    let mut a = Aftermath::new(Seat(0), sys(), AStage::Invading, Before::default(), NoteHold::new());
    a.feats_noted = true;
    a.fight = AFight::Over;
    AftermathChain::new(a).to_pending(&mut core);
    let (_, end) = play(&mut core, &env, &mut first);
    assert_eq!(end, Ok(()));
    assert!(!plain_kinds(&core).contains(&PlainKind::SpaceCombatResolved));
    assert!(core.ledger().events.iter().all(|e| !matches!(e, Emit::Typed(t) if t.kind == ti4_engine_core::state::EventKind::InvasionBegan)), "no INVASION_BEGAN: the skip starts at the commit step");
}

/// Seat 0 (the active one) holds a space dock on the first planet of the system, a carrier in the space and trade goods to build with.
fn dock_arena() -> (CoreState, Kernel) {
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec])
        .map_from(&[sys()])
        .active(Some(0))
        .active_system(sys())
        .unit(sys(), None, Unit::CarrierI, 0, 1)
        .unit(sys(), Some(sys().data().planets[0]), Unit::SpaceDockI, 0, 1)
        .control(sys().data().planets[0], 0)
        .goods(0, 6)
        .tokens(0, 3, 3, 2)
        .build()
}

fn open_producing(core: &mut CoreState) {
    let mut a = Aftermath::new(Seat(0), sys(), AStage::Producing, Before::default(), NoteHold::new());
    a.fight = AFight::Over;
    a.feats_noted = true;
    AftermathChain::new(a).to_pending(core);
}

/// Stop at once; pay with trade goods when something is built.
fn stop_producing(c: &Choice) -> usize {
    let at = |f: &dyn Fn(&Opt) -> bool| c.opts.as_slice().iter().position(f);
    match c.ask {
        Ask::Production(PAsk::Choose(_)) => at(&|o| matches!(o, Opt::Production(POpt::Stop))).unwrap_or(0),
        Ask::Production(PAsk::Pay(_)) => at(&|o| matches!(o, Opt::Production(POpt::TradeGood { .. }))).unwrap_or(0),
        _ => 0,
    }
}

/// Stage 4 of the hand-over of lane M5 to lane M2: the `Producing` stage of a seat with a production capacity in the system opens ONE use of
/// PRODUCTION (`UseOpen::Tactical`) above the aftermath's entry, the aftermath arm hands the answers to it, and `PRODUCTION_RESOLVED` is written once
/// the use has left the stack. (It was the counted gap `AftermathGap::ProductionStep` before the merge of the two lanes.)
#[test]
fn a_production_capacity_hands_the_step_to_the_production_use_and_announces_it_resolved() {
    covers!("driver::windows::chain::AftermathChain");
    covers!("driver::windows::production::ProductionMachine");
    let (mut core, k) = dock_arena();
    let env = env(&k, 3, &[]);
    open_producing(&mut core);
    let mut asked_while_open = Vec::new();
    let mut emitted = Vec::new();
    let mut answer: Option<Pick> = None;
    for _ in 0..200 {
        let (live, out) = advance_aftermath(&mut core, &env, answer.take()).expect("the aftermath runs natively");
        emitted.extend(out.emitted.iter().map(|e| e.kind));
        match live {
            Live::Asks(a) => {
                // The question belongs to the use above the entry: the chain stays under it.
                assert!(core.pending().window(WindowKind::Production).is_some() && core.pending().window(WindowKind::Aftermath).is_some());
                let again = ask_aftermath(&core, &env).expect("asks").expect("a question");
                assert_eq!(again.choice, a.choice, "the aftermath's own question is the use's");
                assert!(plain_kinds(&core).is_empty(), "PRODUCTION_RESOLVED is not written while the use asks");
                let i = stop_producing(&a.choice);
                answer = Some(a.choice.pick(i).expect("an answer"));
                asked_while_open.push(a.choice);
            }
            Live::Done => {
                if core.pending().window(WindowKind::Aftermath).is_none() {
                    break;
                }
            }
        }
    }
    assert!(core.pending().window(WindowKind::Aftermath).is_none() && core.pending().window(WindowKind::Production).is_none(), "both left the stack");
    assert!(matches!(asked_while_open[0].ask, Ask::Production(PAsk::Choose(_))), "the first question is what to build");
    assert_eq!(core.meta().production_seq, 1, "the use has a number");
    assert_eq!(emitted.first(), Some(&ti4_engine_core::state::EventKind::ProductionUsed), "the use announced itself when it started");
    assert!(!emitted.contains(&ti4_engine_core::state::EventKind::UnitsProduced) || core.board().sys(sys()).units.as_slice().iter().all(|p| p.kind() != Unit::DestroyerI), "nothing was built");
    assert_eq!(plain_kinds(&core), [PlainKind::ProductionResolved], "only the production step was run, and announced once, when the use had left the stack");
}

/// A question asked from inside the step (a reaction to `PRODUCTION_USED`) suspends the step: the events the invasion step emitted before it are the
/// string's too (`INVASION_BEGAN` comes before `PRODUCTION_USED`), and the step's call that is resumed must not have lost them.
#[test]
fn the_events_an_earlier_step_of_the_chain_emitted_survive_a_reaction_question_of_a_later_one() {
    covers!("driver::windows::chain::AftermathChain");
    use ti4_engine_core::choice::reaction::{AbilityId, AbilityKind, Relation};
    use ti4_engine_core::kernel::resolver::AbilityDef;
    use ti4_engine_core::kernel::stage::{Slot, Spec};
    use ti4_engine_core::state::EventKind;
    let (mut core, k) = dock_arena();
    let mut env = env(&k, 3, &[]);
    let id = AbilityId { kind: AbilityKind::Ability, key: Code(1) };
    let mut resolver = ti4_engine_core::kernel::Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0)));
    resolver.register([AbilityDef::new(id, Seat(1), EventKind::ProductionUsed, Relation::After).with_optional(true).with_arg(1)]);
    env.registry = Cow::new(resolver.registry().clone());
    env.table.add(Spec::new(Slot(1), AbilityKind::Ability, EventKind::ProductionUsed, Relation::After, |_| Ok(())).optional());
    let mut a = Aftermath::new(Seat(0), sys(), AStage::Invading, Before::default(), NoteHold::new());
    a.fight = AFight::Over;
    a.feats_noted = true;
    a.holds = true;
    AftermathChain::new(a).to_pending(&mut core);
    // At the question itself: the invasion's event is in the queue, the aftermath's own line (`INVASION_RESOLVED`) is not (it is drained when the step ends).
    let Live::Asks(q) = advance_aftermath(&mut core, &env, None).expect("runs").0 else { panic!("the reaction is asked") };
    assert!(matches!(q.choice.ask, Ask::Reaction(_)) && q.nested.is_some(), "a question from inside an emission");
    assert!(core.ledger().events.iter().any(|e| matches!(e, Emit::Typed(t) if t.kind == EventKind::InvasionBegan)), "mirrored at the suspension");
    assert!(plain_kinds(&core).is_empty(), "the aftermath's plain log waits for the end of the step");
    let (asked, end) = play(&mut core, &env, &mut |c: &Choice| {
        if matches!(c.ask, Ask::Reaction(_)) { c.opts.as_slice().iter().position(|o| *o == Opt::Decline).unwrap_or(0) } else { stop_producing(c) }
    });
    assert_eq!(end, Ok(()));
    assert!(asked.iter().any(|c| matches!(c.ask, Ask::Reaction(_))), "the reaction to the use of production is asked from inside the step");
    let typed: Vec<EventKind> = core.ledger().events.iter().filter_map(|e| if let Emit::Typed(t) = e { Some(t.kind) } else { None }).collect();
    let at = |kind| typed.iter().position(|k| *k == kind).unwrap_or_else(|| panic!("{kind:?} in {typed:?}"));
    assert!(at(EventKind::InvasionBegan) < at(EventKind::ProductionUsed), "{typed:?}");
}

/// The string mirrors its timing log into `Game::events`, every `emit` line of it: the event a reaction emits INSIDE another (a card played at
/// `after PRODUCTION_USED`) follows its parent in the queue. The chain used to mirror the top-level emissions of its sub-windows only, so the card's
/// event was missing from the aftermath's step (found by `native_play --shadow`, War Machine at `PRODUCTION_USED`: native `INVASION_RESOLVED` where the
/// string had `ACTION_CARD_PLAYED`, 25 of 96 games).
#[test]
fn an_event_a_reaction_emits_inside_another_is_mirrored_after_its_parent_by_the_aftermath_chain() {
    covers!("driver::windows::chain::AftermathChain");
    use ti4_engine_core::choice::reaction::{AbilityId, AbilityKind, Relation};
    use ti4_engine_core::kernel::resolver::AbilityDef;
    use ti4_engine_core::kernel::stage::{Slot, Spec};
    use ti4_engine_core::state::{EventKind, Payload};
    let (mut core, k) = dock_arena();
    let mut env = env(&k, 3, &[]);
    let id = AbilityId { kind: AbilityKind::Ability, key: Code(1) };
    let mut resolver = ti4_engine_core::kernel::Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0)));
    resolver.register([AbilityDef::new(id, Seat(1), EventKind::ProductionUsed, Relation::After).with_optional(true).with_arg(1)]);
    env.registry = Cow::new(resolver.registry().clone());
    env.table.add(
        Spec::new(Slot(1), AbilityKind::Ability, EventKind::ProductionUsed, Relation::After, |e| {
            // Two events of ONE kind: each is mirrored as it ended, with its own payload (the seat), not as the last one of that kind.
            for seat in [Seat(0), Seat(1)] {
                e.emit(ti4_engine_core::kernel::fx::event(EventKind::ActionCardPlayed, Payload::new().with(ti4_engine_core::state::Key::Player, ti4_engine_core::state::Val::Seat(seat))))?;
            }
            Ok(())
        })
        .optional(),
    );
    open_producing(&mut core);
    let (asked, end) = play(&mut core, &env, &mut |c: &Choice| {
        if matches!(c.ask, Ask::Reaction(_)) { c.opts.as_slice().iter().position(|o| *o != Opt::Decline).unwrap_or(0) } else { stop_producing(c) }
    });
    assert_eq!(end, Ok(()));
    assert!(asked.iter().any(|c| matches!(c.ask, Ask::Reaction(_))), "the reaction to the use of production is asked");
    let typed: Vec<EventKind> = core.ledger().events.iter().filter_map(|e| if let Emit::Typed(t) = e { Some(t.kind) } else { None }).collect();
    let at = |kind| typed.iter().position(|k| *k == kind).unwrap_or_else(|| panic!("{kind:?} in {typed:?}"));
    assert_eq!(at(EventKind::ActionCardPlayed), at(EventKind::ProductionUsed) + 1, "the card follows its parent: {typed:?}");
    let players: Vec<Option<Seat>> = core
        .ledger()
        .events
        .iter()
        .filter_map(|e| if let Emit::Typed(t) = e { Some(t) } else { None })
        .filter(|t| t.kind == EventKind::ActionCardPlayed)
        .map(|t| t.payload.seat(ti4_engine_core::state::Key::Player))
        .collect();
    assert_eq!(players, [Some(Seat(0)), Some(Seat(1))], "each nested event is mirrored with its own payload, in order");
}

/// The chain hands its sub-windows the position's initiative order (the order the ruling of S1 offers the other seats in): a seat that holds the first
/// strategy card is first of the others.
#[test]
fn the_chain_syncs_the_resolver_with_the_initiative_order_of_the_position() {
    covers!("driver::windows::chain::AftermathChain");
    let (mut core, _k) = dock_arena();
    core.strategy_mut().initiative_overrides.insert(Seat(1), 1);
    core.strategy_mut().initiative_overrides.insert(Seat(0), 2);
    assert!(core.pending().resolver.initiative.is_empty(), "nothing set the order before");
    ti4_engine_core::driver::windows::chain::sync_initiative(&mut core);
    assert_eq!(core.pending().resolver.initiative.as_slice(), &[Seat(1), Seat(0)]);
}

/// The string asks AI Development Algorithm's question INSIDE the step (before it emits `PRODUCTION_USED`) and mirrors its timing log when the step
/// ends, before the aftermath's own line: `INVASION_BEGAN`, `PRODUCTION_USED`, `INVASION_RESOLVED`. The core had `INVASION_RESOLVED` first (found by
/// `native_play --shadow`, nomad 4 players: the question is a tape question of the use, and the step that answers it must not flush the line early).
#[test]
fn the_production_used_event_follows_a_nested_question_of_the_use_before_the_aftermaths_own_line() {
    covers!("driver::windows::chain::AftermathChain");
    use ti4_engine_core::generated::Technology as T;
    use ti4_engine_core::state::EventKind;
    let (mut core, k) = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec])
        .map_from(&[sys()])
        .active(Some(0))
        .active_system(sys())
        .unit(sys(), None, Unit::CarrierI, 0, 1)
        .unit(sys(), Some(sys().data().planets[0]), Unit::SpaceDockI, 0, 1)
        .control(sys().data().planets[0], 0)
        .goods(0, 6)
        .tokens(0, 3, 3, 2)
        .tech(0, T::AIDevelopmentAlgorithm)
        .tech(0, T::CarrierII)
        .build();
    let env = env(&k, 3, &[]);
    let mut a = Aftermath::new(Seat(0), sys(), AStage::Invading, Before::default(), NoteHold::new());
    a.fight = AFight::Over;
    a.feats_noted = true;
    a.holds = true;
    AftermathChain::new(a).to_pending(&mut core);
    let (asked, end) = play(&mut core, &env, &mut |c: &Choice| {
        if matches!(c.ask, Ask::Technology(_)) { c.opts.as_slice().iter().position(|o| *o == Opt::Decline).unwrap_or(0) } else { stop_producing(c) }
    });
    assert_eq!(end, Ok(()));
    assert!(asked.iter().any(|c| matches!(c.ask, Ask::Technology(_))), "AI Development Algorithm asks for the discount");
    let queue: Vec<String> = core
        .ledger()
        .events
        .iter()
        .map(|e| match e {
            Emit::Typed(t) => format!("{:?}", t.kind),
            Emit::Plain(p) => format!("{:?}", p.kind),
        })
        .collect();
    let at = |name: &str| queue.iter().position(|k| k == name).unwrap_or_else(|| panic!("{name} in {queue:?}"));
    assert!(at("InvasionBegan") < at(&format!("{:?}", EventKind::ProductionUsed)), "{queue:?}");
    assert!(at(&format!("{:?}", EventKind::ProductionUsed)) < at("InvasionResolved"), "{queue:?}");
}

/// A unit built in the step is made (and announced) before `PRODUCTION_RESOLVED`; a clone in the middle of the use continues identically.
#[test]
fn a_unit_built_in_the_aftermath_production_step_is_announced_before_it_is_resolved() {
    covers!("driver::windows::chain::AftermathChain");
    let (mut core, k) = dock_arena();
    let env = env(&k, 3, &[]);
    open_producing(&mut core);
    let mut built = false;
    let mut answer: Option<Pick> = None;
    let mut cloned_ok = false;
    let mut emitted = Vec::new();
    for _ in 0..200 {
        let (live, out) = advance_aftermath(&mut core, &env, answer.take()).expect("legal");
        emitted.extend(out.emitted.iter().map(|e| e.kind));
        match live {
            Live::Asks(a) => {
                let mut copy = core.clone();
                let from_copy = ask_aftermath(&copy, &env).expect("asks").expect("a question");
                cloned_ok |= from_copy.choice == a.choice;
                let _ = advance_aftermath(&mut copy, &env, None);
                let i = match a.choice.ask {
                    Ask::Production(PAsk::Choose(_)) if !built => {
                        built = true;
                        a.choice.opts.as_slice().iter().position(|o| matches!(o, Opt::Production(POpt::Build { unit: Unit::DestroyerI, .. }))).expect("a destroyer can be built")
                    }
                    _ => stop_producing(&a.choice),
                };
                answer = a.choice.pick(i);
            }
            Live::Done => {
                if core.pending().window(WindowKind::Aftermath).is_none() {
                    break;
                }
            }
        }
    }
    assert!(built && cloned_ok);
    let made = core.board().sys(sys()).units.as_slice().iter().filter(|p| p.owner() == Seat(0) && p.kind() == Unit::DestroyerI).count();
    assert_eq!(made, 1, "the destroyer stands in the system");
    assert_eq!(emitted, [ti4_engine_core::state::EventKind::ProductionUsed, ti4_engine_core::state::EventKind::UnitsProduced], "UNITS_PRODUCED comes with the end of the use");
    assert_eq!(plain_kinds(&core), [PlainKind::ProductionResolved], "and PRODUCTION_RESOLVED after it, once");
}

/// A seat with no capacity in the system asks nothing and ends the step at once, with no number and no production events.
#[test]
fn a_system_without_a_production_capacity_ends_the_step_at_once() {
    covers!("driver::windows::chain::AftermathChain");
    let (mut core, k) = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec])
        .map_from(&[sys()])
        .active(Some(0))
        .active_system(sys())
        .unit(sys(), None, Unit::CarrierI, 0, 1)
        .build();
    let env = env(&k, 3, &[]);
    open_producing(&mut core);
    let (asked, end) = play(&mut core, &env, &mut first);
    assert_eq!(end, Ok(()));
    assert!(asked.is_empty());
    assert_eq!(core.meta().production_seq, 0);
    assert_eq!(plain_kinds(&core), [PlainKind::ProductionResolved]);
}

#[test]
fn the_contact_pause_is_a_counted_gap() {
    covers!("driver::windows::chain::AftermathChain");
    let (mut core, k) = arena(1, 0);
    let env = env(&k, 1, &[]);
    let mut a = Aftermath::new(Seat(0), sys(), AStage::Negotiating { invade: false }, Before::default(), NoteHold::new());
    a.fight = AFight::Over;
    AftermathChain::new(a).to_pending(&mut core);
    let (_, end) = play(&mut core, &env, &mut first);
    assert_eq!(end, Err(LiveError::Unknown(Unknown::UnportedAftermath(AftermathGap::Negotiating))));
}

#[test]
fn a_feat_of_the_fight_opens_the_scoring_window_and_the_aftermath_waits_for_it() {
    covers!("driver::windows::chain::AftermathChain");
    covers!("driver::windows::scoring::EventScoring");
    let (mut core, k) = arena(3, 1);
    // Seat 0 holds Destroy Their Greatest Ship; a capital ship of seat 1 is destroyed in the fight.
    core.players_mut().edit(0).cards.secrets.push(SecretObjective::DestroyTheirGreatestShip);
    let env = env(&k, 9, &[Unit::DreadnoughtI, Unit::CruiserI, Unit::WarSun]);
    core.board_mut().sys_mut(sys()).units.push(ti4_engine_core::state::Piece::new(Unit::WarSun, Seat(1)));
    open_aftermath(&mut core);
    let before = AftermathChain::from_pending(&core, &k).expect("open");
    let _ = before;
    // The snapshot the head takes: seat 1 had a war sun.
    if let Some(ti4_engine_core::state::Window::Aftermath(a)) = core.pending_mut().window_mut(WindowKind::Aftermath) {
        a.before_combat = Before { sides: [Seat(0), Seat(1)].into_iter().collect(), capitals: [(Seat(0), 0), (Seat(1), 1)].into_iter().collect(), notes: NoteHold::new() };
    }
    let mut answer: Option<Pick> = None;
    let mut scored = false;
    for _ in 0..800 {
        match advance_aftermath(&mut core, &env, answer.take()).expect("legal") {
            (Live::Asks(a), _) => answer = Some(a.choice.pick(0).expect("an answer")),
            (Live::Done, _) => {
                if core.pending().window(WindowKind::EventScoring).is_some() {
                    assert!(core.pending().window(WindowKind::Aftermath).is_some(), "the aftermath waits under the scoring window");
                    let ask = ask::<EventScoring>(&core, &env).expect("asks").expect("a question");
                    assert_eq!(ask.choice.actor, Seat(0));
                    let first = ask.choice.pick(0).expect("an answer");
                    let (live, _) = advance_event_scoring(&mut core, &env, Some(first)).expect("legal");
                    assert!(matches!(live, Live::Done), "the one seat with something to score is done");
                    assert!(plain_kinds(&core).contains(&PlainKind::ObjectiveScored));
                    scored = true;
                    assert!(core.pending().window(WindowKind::EventScoring).is_none());
                }
                if core.pending().window(WindowKind::Aftermath).is_none() {
                    break;
                }
            }
        }
    }
    assert!(scored, "the war sun was destroyed: the feat was recorded");
    assert!(core.players().by_seat(Seat(0)).is_some_and(|p| p.misc.event_feats.iter().any(|(f, _)| *f == Feat::DestroyedACapitalShip)));
    assert_eq!(core.players().by_seat(Seat(0)).map(|p| p.econ.victory_points), Some(1));
}

// -- the event scoring window on its own ----------------------------------------------------------------

use ti4_engine_core::driver::windows::scoring::{event_scoring_window, feat_of, open_event_scoring};
use ti4_engine_core::state::ObjectiveRef;

const DTGS: SecretObjective = SecretObjective::DestroyTheirGreatestShip;

/// Two seats, both holding Destroy Their Greatest Ship with its feat recorded for occurrence 5.
fn scoring_table() -> (CoreState, Kernel) {
    let (mut s, k) = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).build();
    for seat in 0..2 {
        s.players_mut().edit(seat).cards.secrets.push(DTGS);
        s.players_mut().edit(seat).misc.event_feats.push((Feat::DestroyedACapitalShip, 5));
    }
    (s, k)
}

fn offers(core: &CoreState, env: &Env<'_>) -> Option<(Seat, Vec<Opt>)> {
    ask::<EventScoring>(core, env).expect("open").map(|a| (a.choice.actor, a.choice.opts.as_slice().to_vec()))
}

#[test]
fn the_window_asks_the_seats_in_initiative_order_and_a_decline_ends_a_seats_turn() {
    covers!("driver::windows::scoring::EventScoring");
    let (mut core, k) = scoring_table();
    let env = env(&k, 1, &[]);
    open_event_scoring(&mut core, 5, true);
    let (first, opts) = offers(&core, &env).expect("someone has something to score");
    assert_eq!(opts.len(), 2, "the secret and the decline");
    let (live, _) = advance_event_scoring(&mut core, &env, Some(Pick::Opt(Opt::Decline))).expect("legal");
    let (second, _) = offers(&core, &env).expect("the other seat is next");
    assert_ne!(first, second, "a decline ends the whole turn of the first seat");
    assert!(matches!(live, Live::Asks(_)));
    let (live, _) = advance_event_scoring(&mut core, &env, Some(Pick::Opt(Opt::Decline))).expect("legal");
    assert!(matches!(live, Live::Done), "nobody is left");
    assert!(core.pending().window(WindowKind::EventScoring).is_none());
    assert!(plain_kinds(&core).is_empty(), "two declines announce nothing");
}

#[test]
fn a_score_is_announced_pays_the_point_and_uses_the_seats_one_score_of_the_occurrence() {
    covers!("driver::windows::scoring::EventScoring");
    let (mut core, k) = scoring_table();
    let env = env(&k, 1, &[]);
    open_event_scoring(&mut core, 5, true);
    let (who, opts) = offers(&core, &env).expect("asks");
    let before = core.players().by_seat(who).map(|p| p.econ.victory_points).unwrap();
    advance_event_scoring(&mut core, &env, Some(Pick::Opt(opts[0]))).expect("legal");
    assert_eq!(plain_kinds(&core), [PlainKind::ObjectiveScored]);
    assert_eq!(core.players().by_seat(who).map(|p| p.econ.victory_points), Some(before + 1));
    assert!(core.objectives().scored_feats.iter().any(|(s, o)| *s == who && *o == 5), "the one score of the occurrence is spent");
    assert!(!core.players().by_seat(who).unwrap().cards.secrets.contains(&DTGS), "the secret left the hand");
    // The same seat is not asked again (one per player and event); the other seat is.
    let (next, _) = offers(&core, &env).expect("the other seat");
    assert_ne!(next, who);
}

#[test]
fn a_secret_already_scored_or_of_another_phase_or_without_its_feat_is_not_offered() {
    covers!("driver::windows::scoring::CoreOffers");
    let (mut core, k) = scoring_table();
    let env = env(&k, 1, &[]);
    ti4_engine_core::kernel::fx::record_score(&mut core, Seat(0), ObjectiveRef::Secret(DTGS));
    core.players_mut().edit(1).misc.event_feats.clear();
    // seat 0 scored it already, seat 1 has no feat: nothing is on offer.
    open_event_scoring(&mut core, 5, true);
    assert_eq!(offers(&core, &env), None);
    // A feat of another occurrence does not count.
    core.players_mut().edit(1).misc.event_feats.push((Feat::DestroyedACapitalShip, 6));
    assert_eq!(offers(&core, &env), None);
    // A status-timed secret is never an event secret.
    core.players_mut().edit(1).misc.event_feats.push((Feat::DestroyedACapitalShip, 5));
    core.players_mut().edit(1).cards.secrets.clear();
    core.players_mut().edit(1).cards.secrets.push(SecretObjective::EstablishAPerimeter);
    assert_eq!(offers(&core, &env), None);
    assert_eq!(feat_of(SecretObjective::EstablishAPerimeter), None);
}

#[test]
fn a_secret_made_public_is_offered_to_a_holder_of_the_feat_from_its_rules_version_only() {
    covers!("driver::windows::scoring::CoreOffers");
    for (version, offered) in [(23_u32, false), (24, true)] {
        let (mut core, k) = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).rules(version).build();
        core.players_mut().edit(1).misc.event_feats.push((Feat::DestroyedACapitalShip, 5));
        core.objectives_mut().revealed.push(ObjectiveRef::Secret(DTGS));
        let env = env(&k, 1, &[]);
        open_event_scoring(&mut core, 5, false);
        assert_eq!(offers(&core, &env).map(|(who, _)| who), offered.then_some(Seat(1)), "rules version {version}");
    }
}

#[test]
fn a_score_that_ends_the_game_announces_it_and_closes_the_window_at_once() {
    covers!("driver::windows::scoring::EventScoring");
    let (mut core, k) = scoring_table();
    let env = env(&k, 1, &[]);
    open_event_scoring(&mut core, 5, true);
    let (who, opts) = offers(&core, &env).expect("asks");
    core.players_mut().edit(usize::from(who.0)).econ.victory_points = 9;
    let (live, _) = advance_event_scoring(&mut core, &env, Some(Pick::Opt(opts[0]))).expect("legal");
    assert!(matches!(live, Live::Done));
    assert_eq!(plain_kinds(&core), [PlainKind::ObjectiveScored, PlainKind::GameFinished]);
    assert!(core.meta().finished);
    assert!(core.pending().window(WindowKind::EventScoring).is_none(), "the window is dropped before GAME_FINISHED");
}

#[test]
fn the_window_of_an_occurrence_lists_every_seat_in_initiative_order_with_the_cap_of_a_combat() {
    covers!("driver::windows::scoring::EventScoring");
    let (core, _k) = scoring_table();
    let ti4_engine_core::state::Window::EventScoring(w) = event_scoring_window(&core, 5, true) else { panic!("an event scoring window") };
    assert_eq!((w.pending.len(), w.occurrence, w.one_per_player), (2, Some(5), true));
    let ti4_engine_core::state::Window::EventScoring(any) = event_scoring_window(&core, 5, false) else { panic!() };
    assert!(!any.one_per_player);
}

#[test]
fn the_question_shows_the_points_and_a_second_secret_of_the_same_event_waits_for_the_cap() {
    covers!("driver::windows::scoring::CoreOffers");
    for (capped, again) in [(true, false), (false, true)] {
        let (mut core, k) = scoring_table();
        let env = env(&k, 1, &[]);
        // Seat 0 (or 1) also holds Demonstrate Your Power, whose feat was recorded for the same occurrence.
        for seat in 0..2 {
            core.players_mut().edit(seat).cards.secrets.push(SecretObjective::DemonstrateYourPower);
            core.players_mut().edit(seat).misc.event_feats.push((Feat::HeldThreeShipsAfterASpaceCombat, 5));
            core.players_mut().edit(seat).econ.victory_points = 3;
        }
        open_event_scoring(&mut core, 5, capped);
        let asking = ask::<EventScoring>(&core, &env).expect("open").expect("asks");
        assert_eq!(asking.choice.ask, Ask::Objectives(ti4_engine_core::choice::objectives::Ask::Score { vp: 3 }), "the question shows the points held");
        assert_eq!(asking.choice.opts.len(), 3, "two secrets and the decline");
        let who = asking.choice.actor;
        advance_event_scoring(&mut core, &env, Some(asking.choice.pick(0).expect("an answer"))).expect("legal");
        let next = ask::<EventScoring>(&core, &env).expect("open").expect("someone is asked");
        assert_eq!(next.choice.actor == who, again, "capped {capped}: the seat that scored is asked again only without the one-per-player cap");
    }
}

/// H4: the Jol-Nar commander's flag of the fight's environment is `promissory::has_commander_ability`, not only the own unlocked card: a faceup Alliance
/// of the Jol-Nar held by a side, a durable grant, Mahact's Imperia (the string rerolls the barrage's dice for each of them).
#[test]
fn the_jolnar_commander_flag_of_the_fight_follows_every_route_of_the_commander_ability() {
    use ti4_engine_core::driver::windows::combat::env_of;
    use ti4_engine_core::generated::{Leader, PromissoryNote};
    use ti4_engine_core::keys::MarkKey;
    use ti4_engine_core::state::{CombatFields, CombatStep, HitPhase, LeaderStatus, NoteKey};
    let fields = CombatFields {
        step: CombatStep::Opening { round: 1 },
        phase: HitPhase::Round,
        resuming: false,
        pending_retreats: vec![],
        offered: vec![],
        damaged: vec![],
        fled: vec![],
        at_start: vec![],
        scoring: false,
        outcome: None,
        end_announced: false,
        round_end_produced: None,
    };
    let flags = |s: &CoreState, k: &Kernel| env_of(s, k, &CombatTexts::default(), &fields, 0).expect("a fight is open").jolnar;
    let (mut s, k) = arena(1, 1);
    // A third seat plays the Jol-Nar with its commander unlocked; nobody in the fight benefits yet.
    let (mut s3, k3) = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec, Faction::TheUniversitiesOfJolNar])
        .map_from(&[sys()])
        .active(Some(0))
        .active_system(sys())
        .unit(sys(), None, Unit::DreadnoughtI, 0, 1)
        .unit(sys(), None, Unit::CruiserI, 1, 1)
        .build();
    s3.transient_mut().combat.active_space_combat = Some((sys(), Seat(0), Seat(1)));
    assert_eq!(flags(&s, &k), [false, false]);
    s3.players_mut().by_seat_mut(Seat(2)).unwrap().leaders.status.insert(Leader::AgnlanOln, LeaderStatus::Unlocked);
    assert_eq!(flags(&s3, &k3), [false, false], "the commander of another seat helps nobody");
    // The Alliance of the Jol-Nar, faceup in the hand of the defender.
    let alliance = NoteKey { note: PromissoryNote::Alliance, faction: Faction::TheUniversitiesOfJolNar };
    s3.cards_mut().promissory.edit().insert(alliance, Seat(1));
    assert_eq!(flags(&s3, &k3), [false, false], "facedown it conveys nothing");
    s3.cards_mut().promissory_faceup.edit().insert(alliance, ());
    assert_eq!(flags(&s3, &k3), [false, true], "faceup, the defender has the Jol-Nar commander's ability");
    // The own unlocked commander of the attacker, and a durable grant.
    s.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::AgnlanOln, LeaderStatus::Unlocked);
    assert_eq!(flags(&s, &k), [true, false]);
    let (mut g, kg) = arena(1, 1);
    g.marks_mut().tail.edit().insert(MarkKey::CommanderAbility(Seat(1), Leader::AgnlanOln), ti4_engine_core::keys::MarkVal::Atom(ti4_engine_core::keys::Atom::Seat(Seat(1))));
    assert_eq!(flags(&g, &kg), [false, true], "a durable grant is the ability too");
}
