//! Lane M5: the aftermath chain (`driver::windows::chain`), its space combat (`driver::windows::combat`) and the event scoring window
//! (`driver::windows::scoring`) run through the live arms: a whole aftermath held as payloads on the `Pending` stack between the questions. The leaf
//! windows have their own tests; these check what the composers add: the order of the stages, the payload round trip, a clone in the middle, the
//! hand-off to the invasion, the scoring pauses and the bails.

use std::rc::Rc;

use ti4_engine_core::containers::Cow;
use ti4_engine_core::driver::live::{ask, Env, Live, LiveError};
use ti4_engine_core::driver::windows::chain::{advance_aftermath, ask_aftermath, AftermathChain};
use ti4_engine_core::driver::windows::scoring::{advance_event_scoring, EventScoring};
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
    table.facts.combat = texts(present);
    Env { kernel: k, registry: Cow::new(Registry::default()), table, seeds: Rc::new(KitSeeds(seed)) }
}

const SHIPS: [Unit; 3] = [Unit::CarrierI, Unit::DreadnoughtI, Unit::CruiserI];

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
                answer = Some(Pick::Opt(a.choice.opts.as_slice()[i]));
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
                let pick = Pick::Opt(a.choice.opts.as_slice()[0]);
                let mut rest = core.clone();
                let mut p = |_: &Choice| 0;
                let mut run = |c: &mut CoreState| {
                    let mut answer = Some(pick);
                    for _ in 0..800 {
                        match advance_aftermath(c, &env, answer.take()).expect("legal") {
                            (Live::Asks(a), _) => answer = Some(Pick::Opt(a.choice.opts.as_slice()[p(&a.choice)])),
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
                answer = Some(Pick::Opt(a.choice.opts.as_slice()[0]));
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

#[test]
fn a_production_capacity_is_the_counted_gap_of_the_production_step() {
    covers!("driver::windows::chain::AftermathChain");
    let (mut core, k) = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec])
        .map_from(&[sys()])
        .active(Some(0))
        .active_system(sys())
        .unit(sys(), None, Unit::CarrierI, 0, 1)
        .unit(sys(), Some(sys().data().planets[0]), Unit::SpaceDockI, 0, 1)
        .control(sys().data().planets[0], 0)
        .build();
    let env = env(&k, 3, &[]);
    let mut a = Aftermath::new(Seat(0), sys(), AStage::Producing, Before::default(), NoteHold::new());
    a.fight = AFight::Over;
    a.feats_noted = true;
    AftermathChain::new(a).to_pending(&mut core);
    let (_, end) = play(&mut core, &env, &mut first);
    assert_eq!(end, Err(LiveError::Unknown(Unknown::UnportedAftermath(AftermathGap::ProductionStep))));
    assert!(core.pending().window(WindowKind::Aftermath).is_some(), "the position is what it was");
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
            (Live::Asks(a), _) => answer = Some(Pick::Opt(a.choice.opts.as_slice()[0])),
            (Live::Done, _) => {
                if core.pending().window(WindowKind::EventScoring).is_some() {
                    assert!(core.pending().window(WindowKind::Aftermath).is_some(), "the aftermath waits under the scoring window");
                    let ask = ask::<EventScoring>(&core, &env).expect("asks").expect("a question");
                    assert_eq!(ask.choice.actor, Seat(0));
                    let first = Pick::Opt(ask.choice.opts.as_slice()[0]);
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
        advance_event_scoring(&mut core, &env, Some(Pick::Opt(asking.choice.opts.as_slice()[0]))).expect("legal");
        let next = ask::<EventScoring>(&core, &env).expect("open").expect("someone is asked");
        assert_eq!(next.choice.actor == who, again, "capped {capped}: the seat that scored is asked again only without the one-per-player cap");
    }
}
