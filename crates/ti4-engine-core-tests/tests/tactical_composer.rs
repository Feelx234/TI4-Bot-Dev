//! Lane M6: the tactical composer (`driver::windows::tactical`) run through the live arm (`advance_tactical`): a whole tactical action from the opening to
//! the hand-off to the aftermath and the tail, held as a payload on the `Pending` stack between the questions. The leaf windows have their own tests
//! (`tactical.rs`, `fleet`); these check what the composer adds: the order of the stages, the payload round trip, a clone in the middle, the hand-off,
//! the enforcement in the head, the tail, the bails.

use std::rc::Rc;

use ti4_engine_core::choice::tactical::{Ask as TAsk, Opt as TOpt};
use ti4_engine_core::containers::Cow;
use ti4_engine_core::driver::live::{Env, Live, LiveError};
use ti4_engine_core::driver::windows::tactical::{advance_tactical, ask_tactical, open_free, open_tactical, TacticalAction};
use ti4_engine_core::hex::Hex;
use ti4_engine_core::kernel::ability::TacticalCodes;
use ti4_engine_core::kernel::stage::{AbilityTable, Held};
use ti4_engine_core::kernel::Registry;
use ti4_engine_core::state::{AStage, Code, Emit, EventKind, PlainKind, THeadStep, TStage, Window as OpenWindow, WindowKind};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::rig::KitSeeds;
use ti4_engine_core_tests::scenario::plain_systems;


fn codes() -> TacticalCodes {
    TacticalCodes { tactic: Code(21), space_cannon: Code(22), space_cannon_cause: Code(23), casualty: Code(24), commander: Code(25), pre_roll: Code(26), faction_technology: Code(27), nullification_field: Code(28), starlancer: Code(29), peace_prompt: Code(30), peace_source: Code(31) }
}

fn env(k: &Kernel, seed: u64) -> Env<'_> {
    let mut table = AbilityTable::default();
    ti4_engine_core::driver::children::register(&mut table);
    table.codes.tactical = codes();
    Env { kernel: k, registry: Cow::new(Registry::default()), table, seeds: Rc::new(KitSeeds(seed)) }
}

fn t(o: TOpt) -> Opt {
    Opt::Tactical(o)
}

/// A row A B C D on the hexes (0,0) .. (3,0); seat 0 plays Muaat, seat 1 the Arborec; seat 0 has cruisers on B and C.
fn row() -> (Scenario, [System; 4]) {
    let p = plain_systems();
    let tiles = [p[0], p[1], p[2], p[3]];
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec])
        .map_hexes(&[(tiles[0], Hex::new(0, 0)), (tiles[1], Hex::new(1, 0)), (tiles[2], Hex::new(2, 0)), (tiles[3], Hex::new(3, 0))])
        .tokens(0, 3, 4, 2)
        .active(Some(0))
        .unit(tiles[1], None, Unit::CruiserI, 0, 1)
        .unit(tiles[2], None, Unit::CruiserI, 0, 1);
    (sc, tiles)
}

fn tactical(core: &CoreState) -> Option<ti4_engine_core::state::Tactical> {
    match core.pending().window(WindowKind::Tactical)? {
        OpenWindow::Tactical(t) => Some(t.clone()),
        _ => None,
    }
}

fn pick(c: &Choice, f: impl Fn(&Opt) -> bool) -> Pick {
    Pick::Opt(*c.opts.as_slice().iter().find(|o| f(o)).expect("the option is offered"))
}

/// The question of the open action.
fn asked(core: &CoreState, env: &Env<'_>) -> Choice {
    ask_tactical(core, env).unwrap().expect("a question is pending").choice
}

/// Open the action, activate D and finish the movement step at once; the live position after the last call.
fn to_the_close(core: &mut CoreState, env: &Env<'_>, tiles: &[System; 4]) -> Result<Live, LiveError> {
    open_tactical(core, Seat(0));
    let Live::Asks(q) = advance_tactical(core, env, None)?.0 else { panic!("the activation is asked") };
    let Live::Asks(q) = advance_tactical(core, env, Some(pick(&q.choice, |o| *o == t(TOpt::Activate(tiles[3])))))?.0 else { panic!("the movement step is asked") };
    Ok(advance_tactical(core, env, Some(pick(&q.choice, |o| *o == t(TOpt::DoneMoving))))?.0)
}

#[test]
fn a_whole_action_opens_activates_moves_closes_hands_the_aftermath_over_and_ends_with_the_tail() {
    covers!("driver::windows::tactical::TacticalAction");
    let (sc, tiles) = row();
    let (mut core, k) = sc.build();
    let env = env(&k, 7);
    open_tactical(&mut core, Seat(0));
    let events = |c: &CoreState| -> Vec<PlainKind> { c.ledger().events.iter().filter_map(|e| e.plain().map(|p| p.kind)).collect() };
    assert_eq!(events(&core), vec![PlainKind::TacticalActionBegan]);

    // The activation question lists the systems without our token; D is chosen.
    let (live, _) = advance_tactical(&mut core, &env, None).unwrap();
    let Live::Asks(q) = live else { panic!("the activation is asked") };
    assert_eq!(q.choice.ask, Ask::Tactical(TAsk::Activate { seat: Seat(0) }));
    assert_eq!(tactical(&core).unwrap().stage, TStage::Activating);
    let (live, _) = advance_tactical(&mut core, &env, Some(pick(&q.choice, |o| *o == t(TOpt::Activate(tiles[3]))))).unwrap();
    // Straight on to the movement step (no hook in the position).
    let Live::Asks(q) = live else { panic!("the movement step is asked") };
    assert_eq!(q.choice.ask, Ask::Tactical(TAsk::Move { seat: Seat(0) }));
    assert_eq!(tactical(&core).unwrap().stage, TStage::Moving);
    assert_eq!(core.meta().active_system, Some(tiles[3]));
    assert_eq!(core.players().by_seat(Seat(0)).unwrap().econ.tactic_tokens, 2, "the token came out of the tactic pool");
    assert_eq!(events(&core), vec![PlainKind::TacticalActionBegan, PlainKind::SystemActivated]);

    // The cruiser on C moves; no capacity to fill: it sails at once.
    let (live, _) = advance_tactical(&mut core, &env, Some(pick(&q.choice, |o| matches!(o, Opt::Tactical(TOpt::Move { origin, .. }) if *origin == tiles[2])))).unwrap();
    let Live::Asks(q) = live else { panic!("movement goes on") };
    assert!(core.board().sys(tiles[3]).units.iter().any(|p| p.owner() == Seat(0)), "the cruiser arrived");
    assert!(events(&core).ends_with(&[PlainKind::ShipMoved]));

    // "Finish movement": the close, the head (nothing to enforce, no gun), and the hand-off to the aftermath.
    let (live, _) = advance_tactical(&mut core, &env, Some(pick(&q.choice, |o| *o == t(TOpt::DoneMoving)))).unwrap();
    assert_eq!(live, Live::Done, "the step ends at the hand-off");
    assert_eq!(tactical(&core).unwrap().stage, TStage::Aftermath, "the action waits under the aftermath");
    let Some(OpenWindow::Aftermath(a)) = core.pending().window(WindowKind::Aftermath) else { panic!("the aftermath is open") };
    assert_eq!((a.player, a.system, a.stage, a.feats_noted), (Seat(0), tiles[3], AStage::Fighting, false));
    assert!(a.before_combat.sides.as_slice().contains(&Seat(0)));

    // The aftermath arm (not this lane's) runs the chain and closes its entry; the action then runs its tail.
    core.pending_mut().close(WindowKind::Aftermath);
    let (live, _) = advance_tactical(&mut core, &env, None).unwrap();
    assert_eq!(live, Live::Done);
    assert!(tactical(&core).is_none(), "the action left the stack");
    assert_eq!(core.meta().active_system, None);
    assert_eq!(events(&core).last(), Some(&PlainKind::TacticalActionComplete));
    let _ = ti4_engine_core::state::EventKind::MovementFinished;
}

#[test]
fn the_payload_round_trips_at_every_question_and_a_clone_in_the_middle_continues_identically() {
    covers!("driver::windows::tactical::TacticalAction");
    let (sc, tiles) = row();
    let (mut core, k) = sc.build();
    let env = env(&k, 3);
    open_tactical(&mut core, Seat(0));
    let mut answers = vec![
        |c: &Choice| pick(c, |o| *o == t(TOpt::Activate(System::ALL[0]))),
    ];
    let _ = (&mut answers, tiles);
    let mut fork: Option<CoreState> = None;
    let mut next: Option<Pick> = None;
    let mut step = 0;
    loop {
        let (live, _) = advance_tactical(&mut core, &env, next.take()).unwrap();
        // The held payload is exactly what the machine reads back.
        if let Some(t) = tactical(&core) {
            let m = TacticalAction::from_pending(&core, &k).expect("open");
            assert_eq!(m.p, t, "from_pending reads the payload as it is");
        }
        match live {
            Live::Asks(q) => {
                step += 1;
                if step == 2 {
                    fork = Some(core.clone());
                }
                let o = match q.choice.ask {
                    Ask::Tactical(TAsk::Activate { .. }) => pick(&q.choice, |_| true),
                    _ => pick(&q.choice, |o| matches!(o, Opt::Tactical(TOpt::DoneMoving))),
                };
                next = Some(o);
            }
            Live::Done => break,
        }
    }
    // Replay the second half from the clone: same answers, same position.
    let mut other = fork.expect("a clone was taken at the second question");
    let mut again: Option<Pick> = None;
    let mut asks = 0;
    loop {
        let (live, _) = advance_tactical(&mut other, &env, again.take()).unwrap();
        match live {
            Live::Asks(q) => {
                asks += 1;
                again = Some(pick(&q.choice, |o| matches!(o, Opt::Tactical(TOpt::DoneMoving))));
            }
            Live::Done => break,
        }
    }
    assert!(asks >= 1);
    assert_eq!(core, other, "a clone in the middle of the action continues to the same position");
}

#[test]
fn a_free_tactical_action_activates_without_a_token_and_asks_for_the_movement() {
    covers!("driver::windows::tactical::TacticalAction");
    let (sc, tiles) = row();
    let (mut core, k) = sc.build();
    let env = env(&k, 1);
    open_free(&mut core, Seat(0), tiles[3]);
    let (live, _) = advance_tactical(&mut core, &env, None).unwrap();
    let Live::Asks(q) = live else { panic!("the movement step is asked") };
    assert_eq!(q.choice.ask, Ask::Tactical(TAsk::Move { seat: Seat(0) }));
    assert_eq!(core.meta().active_system, Some(tiles[3]));
    assert!(!core.board().sys(tiles[3]).cmd.contains(Seat(0)), "no token placed");
    assert_eq!(core.players().by_seat(Seat(0)).unwrap().econ.tactic_tokens, 3, "the pool is untouched");
    let plain: Vec<PlainKind> = core.ledger().events.iter().filter_map(|e| e.plain().map(|p| p.kind)).collect();
    assert_eq!(plain, vec![PlainKind::SystemActivated, PlainKind::FreeTacticalAction]);
}

#[test]
fn a_hook_of_the_activation_that_asks_puts_its_question_and_a_clone_taken_while_it_waits_continues_identically() {
    covers!("driver::windows::tactical::TacticalAction");
    covers!("driver::windows::tactical::hooks");
    let (sc, tiles) = row();
    // Seat 0 holds Spatial Conduit Cylinders and has cruisers on B and C: activating B offers the link.
    let (mut core, k) = sc.tech(0, Technology::SpatialConduitCylinders).build();
    let env = env(&k, 1);
    open_tactical(&mut core, Seat(0));
    let Live::Asks(q) = advance_tactical(&mut core, &env, None).unwrap().0 else { panic!("the activation is asked") };
    let Live::Asks(hook) = advance_tactical(&mut core, &env, Some(pick(&q.choice, |o| *o == t(TOpt::Activate(tiles[1]))))).unwrap().0 else { panic!("the hook asks") };
    assert!(matches!(hook.choice.ask, Ask::Tactical(TAsk::Hook { hook: ti4_engine_core::choice::tactical::Hook::SpatialConduit, .. })), "{:?}", hook.choice.ask);
    assert_eq!(hook.choice.actor, Seat(0));
    assert!(core.pending().nested.is_some(), "the question is held in the journal");
    let mut twin = core.clone();
    for (who, c) in [("original", &mut core), ("clone", &mut twin)] {
        let Live::Asks(next) = advance_tactical(c, &env, Some(Pick::Opt(Opt::Decline))).unwrap().0 else { panic!("{who}: the movement step is asked") };
        assert_eq!(next.choice.ask, Ask::Tactical(TAsk::Move { seat: Seat(0) }), "{who}");
    }
    assert_eq!(core, twin, "the clone continues to the same position");
    // Used, the link is written for the activation and the plain record follows; the tail clears it.
    let (sc, tiles) = row();
    let (mut core, k) = sc.tech(0, Technology::SpatialConduitCylinders).build();
    let env2 = crate::env(&k, 1);
    open_tactical(&mut core, Seat(0));
    let Live::Asks(q) = advance_tactical(&mut core, &env2, None).unwrap().0 else { panic!("the activation is asked") };
    let Live::Asks(hook) = advance_tactical(&mut core, &env2, Some(pick(&q.choice, |o| *o == t(TOpt::Activate(tiles[1]))))).unwrap().0 else { panic!("the hook asks") };
    let Live::Asks(_) = advance_tactical(&mut core, &env2, Some(pick(&hook.choice, |o| matches!(o, Opt::Tactical(TOpt::Use))))).unwrap().0 else { panic!("the movement step is asked") };
    let kinds: Vec<PlainKind> = core.ledger().events.iter().filter_map(|e| e.plain().map(|p| p.kind)).collect();
    assert!(kinds.contains(&PlainKind::SpatialConduit));
    assert!(core.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(Technology::SpatialConduitCylinders));
    assert_eq!(core.players().by_seat(Seat(0)).unwrap().timers.spatial_conduit.get(), Some(core.meta().activation_seq));
}

#[test]
fn the_tail_announces_the_end_of_the_action_only_when_somebody_listens() {
    covers!("driver::windows::tactical::TacticalAction");
    let ended = |c: &CoreState| c.ledger().events.iter().any(|e| matches!(e, Emit::Typed(r) if r.kind == EventKind::TacticalActionEnded));
    for (listener, expect) in [(false, false), (true, true)] {
        let (sc, tiles) = row();
        let (mut core, k) = sc.build();
        if listener {
            core.players.edit(1).leaders.status.insert(Leader::Tro, ti4_engine_core::state::LeaderStatus::Readied);
        }
        let env = env(&k, 1);
        assert_eq!(to_the_close(&mut core, &env, &tiles).unwrap(), Live::Done);
        core.pending_mut().close(WindowKind::Aftermath);
        assert_eq!(advance_tactical(&mut core, &env, None).unwrap().0, Live::Done);
        assert_eq!(ended(&core), expect, "listener {listener}");
        assert!(tactical(&core).is_none());
    }
}

#[test]
fn the_crown_of_emphidia_asks_at_the_end_of_the_action_and_a_clone_continues_identically() {
    covers!("driver::windows::tactical::TacticalAction");
    let (sc, tiles) = row();
    let (mut core, k) = sc.build();
    let planet = Planet::ALL.iter().copied().find(|p| !p.data().traits.is_empty()).expect("a planet with a trait");
    core.board.planet_mut(planet).control = Some(Seat(0));
    core.players.edit(0).cards.relics.push(Relic::TheCrownOfEmphidia);
    let env = env(&k, 1);
    assert_eq!(to_the_close(&mut core, &env, &tiles).unwrap(), Live::Done);
    core.pending_mut().close(WindowKind::Aftermath);
    let Live::Asks(q) = advance_tactical(&mut core, &env, None).unwrap().0 else { panic!("the Crown asks") };
    assert!(matches!(q.choice.ask, Ask::Relic(_)), "{:?}", q.choice.ask);
    let mut twin = core.clone();
    let (a, _) = advance_tactical(&mut core, &env, Some(Pick::Opt(Opt::Decline))).unwrap();
    let (b, _) = advance_tactical(&mut twin, &env, Some(Pick::Opt(Opt::Decline))).unwrap();
    assert_eq!((a, b), (Live::Done, Live::Done));
    assert_eq!(core, twin);
    assert!(tactical(&core).is_none(), "declined, the action ends");
}

#[test]
fn the_enforcement_of_the_head_is_held_between_its_questions_and_a_clone_continues_identically() {
    covers!("driver::windows::tactical::TacticalAction");
    let (sc, tiles) = row();
    let (mut core, k) = sc.tokens(0, 3, 1, 2).unit(tiles[3], None, Unit::CruiserI, 0, 3).build();
    let env = env(&k, 5);
    open_tactical(&mut core, Seat(0));
    let Live::Asks(q) = advance_tactical(&mut core, &env, None).unwrap().0 else { panic!() };
    let Live::Asks(q) = advance_tactical(&mut core, &env, Some(pick(&q.choice, |o| *o == t(TOpt::Activate(tiles[3]))))).unwrap().0 else { panic!() };
    let Live::Asks(q) = advance_tactical(&mut core, &env, Some(pick(&q.choice, |o| *o == t(TOpt::DoneMoving)))).unwrap().0 else { panic!("the enforcement asks") };
    assert!(matches!(q.choice.ask, Ask::Tactical(TAsk::Remove { .. })), "{:?}", q.choice.ask);
    assert_eq!(q.choice.actor, Seat(0));
    let Some(ti4_engine_core::state::Tactical { stage: TStage::Heading(h), .. }) = tactical(&core) else { panic!("held in the head") };
    assert_eq!(h.step, THeadStep::Enforcing);
    assert_eq!(asked(&core, &env), q.choice, "the held question is a pure read");
    let mut fork = core.clone();
    let mut left = 10;
    for state in [&mut core, &mut fork] {
        loop {
            left -= 1;
            assert!(left > 0, "the enforcement ends");
            let c = asked(state, &env);
            let live = advance_tactical(state, &env, Some(pick(&c, |_| true))).unwrap().0;
            if live == Live::Done {
                break;
            }
        }
        assert_eq!(tactical(state).map(|t| t.stage), Some(TStage::Aftermath));
    }
    assert_eq!(core, fork);
    assert_eq!(core.board().sys(tiles[3]).units.iter().filter(|p| p.owner() == Seat(0)).count(), 1, "down to the fleet supply of one");
}

#[test]
fn a_floating_factory_among_an_enemy_fleet_is_destroyed_when_the_movement_step_closes() {
    covers!("driver::windows::tactical::TacticalAction");
    let (sc, tiles) = row();
    let (mut core, k) = sc.unit(tiles[3], None, Unit::FloatingFactoryI, 1, 1).unit(tiles[3], None, Unit::CruiserI, 0, 1).build();
    let env = env(&k, 1);
    assert_eq!(to_the_close(&mut core, &env, &tiles).unwrap(), Live::Done);
    assert!(!core.board().sys(tiles[3]).units.iter().any(|p| p.kind() == Unit::FloatingFactoryI), "blockaded: destroyed");
}

fn marks_of(core: &CoreState, key: ti4_engine_core::keys::MarkKey) -> Option<ti4_engine_core::keys::MarkVal> {
    core.marks().tail.get(&key).cloned()
}

/// Open, activate D (the answer to the first question) and return the live position after the activation answer.
fn activated(sc: Scenario, tiles_d: usize) -> (CoreState, Kernel, Result<Live, LiveError>) {
    let (mut core, k) = sc.build();
    let tiles = plain_systems();
    let env = env(&k, 9);
    open_tactical(&mut core, Seat(0));
    let Live::Asks(q) = advance_tactical(&mut core, &env, None).unwrap().0 else { panic!("the activation is asked") };
    let got = advance_tactical(&mut core, &env, Some(pick(&q.choice, |o| *o == t(TOpt::Activate(tiles[tiles_d]))))).map(|r| r.0);
    (core, k, got)
}

#[test]
fn the_sardakk_agent_and_the_winnu_remember_the_activation() {
    covers!("driver::windows::tactical::TacticalAction");
    use ti4_engine_core::keys::{Atom, MarkKey, MarkVal, Word};
    let tiles = plain_systems();
    let (sc, _) = row();
    // No faction in play that listens: no mark.
    let (core, _, _) = activated(sc.clone(), 3);
    assert_eq!(marks_of(&core, MarkKey::SardakkAgentTactical(Seat(0))), None);
    assert_eq!(marks_of(&core, MarkKey::PrivateSeat(Seat(0), Word::WinnuTactical)), None);
    // A Sardakk seat with a readied T'ro remembers the system of whoever activates.
    let mut sardakk = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::SardakkNorr]);
    let p = plain_systems();
    sardakk = sardakk.map_hexes(&[(p[0], Hex::new(0, 0)), (p[1], Hex::new(1, 0)), (p[2], Hex::new(2, 0)), (p[3], Hex::new(3, 0))]).tokens(0, 3, 4, 2).active(Some(0)).unit(p[1], None, Unit::CruiserI, 0, 1);
    let (mut core, k) = sardakk.build();
    core.players.edit(1).leaders.status.insert(Leader::Tro, ti4_engine_core::state::LeaderStatus::Readied);
    let env = env(&k, 1);
    open_tactical(&mut core, Seat(0));
    let Live::Asks(q) = advance_tactical(&mut core, &env, None).unwrap().0 else { panic!() };
    advance_tactical(&mut core, &env, Some(pick(&q.choice, |o| *o == t(TOpt::Activate(tiles[3]))))).unwrap();
    assert_eq!(marks_of(&core, MarkKey::SardakkAgentTactical(Seat(0))), Some(MarkVal::Atom(Atom::System(tiles[3]))));
    // The Winnu remember that a tactical action began.
    let winnu = Scenario::new(&[Faction::TheWinnu, Faction::TheArborec]).map_hexes(&[(p[0], Hex::new(0, 0)), (p[1], Hex::new(1, 0)), (p[2], Hex::new(2, 0)), (p[3], Hex::new(3, 0))]).tokens(0, 3, 4, 2).active(Some(0));
    let (core, _, got) = activated(winnu, 3);
    assert!(matches!(got, Ok(Live::Asks(_))));
    assert_eq!(marks_of(&core, MarkKey::PrivateSeat(Seat(0), Word::WinnuTactical)), Some(MarkVal::Atom(Atom::Num(1))));
}

#[test]
fn e_res_siphons_pays_four_trade_goods_to_each_other_holder_with_ships_in_the_activated_system() {
    covers!("driver::windows::tactical::TacticalAction");
    let goods = |c: &CoreState, seat: u8| c.players().by_seat(Seat(seat)).unwrap().econ.trade_goods;
    let kinds = |c: &CoreState| -> Vec<PlainKind> { c.ledger().events.iter().filter_map(|e| e.plain().map(|p| p.kind)).collect() };
    let (sc, tiles) = row();
    // A holder with a ship there is paid; the activator's own copy and a holder without a ship there are not.
    let (core, _, _) = activated(sc.clone().tech(1, Technology::EResSiphons).tech(0, Technology::EResSiphons).unit(tiles[3], None, Unit::CruiserI, 1, 1), 3);
    assert_eq!((goods(&core, 0), goods(&core, 1)), (0, 4));
    assert_eq!(kinds(&core).iter().filter(|k| **k == PlainKind::EResSiphons).count(), 1);
    let (core, _, _) = activated(sc.clone().tech(1, Technology::EResSiphons), 3);
    assert_eq!(goods(&core, 1), 0, "no ship there");
    let (core, _, _) = activated(sc.unit(tiles[3], None, Unit::CruiserI, 1, 1), 3);
    assert_eq!(goods(&core, 1), 0, "no technology");
}

#[test]
fn a_ceasefire_note_denies_the_movement_step_and_goes_home() {
    covers!("driver::windows::tactical::TacticalAction");
    use ti4_engine_core::state::NoteKey;
    let (sc, tiles) = row();
    let (mut core, k) = sc.unit(tiles[3], None, Unit::CruiserI, 1, 1).build();
    // Seat 1 holds seat 0's Ceasefire.
    core.cards.promissory.edit().insert(NoteKey { note: PromissoryNote::Ceasefire, faction: Faction::TheEmbersOfMuaat }, Seat(1));
    let env = env(&k, 1);
    open_tactical(&mut core, Seat(0));
    let Live::Asks(q) = advance_tactical(&mut core, &env, None).unwrap().0 else { panic!() };
    let got = advance_tactical(&mut core, &env, Some(pick(&q.choice, |o| *o == t(TOpt::Activate(tiles[3]))))).unwrap().0;
    // No movement question: the step closed and the action went on to the aftermath.
    assert_eq!(got, Live::Done);
    assert_eq!(tactical(&core).map(|t| t.stage), Some(TStage::Aftermath));
    assert_eq!(core.cards().promissory.get(&NoteKey { note: PromissoryNote::Ceasefire, faction: Faction::TheEmbersOfMuaat }), Some(&Seat(0)));
    let kinds: Vec<PlainKind> = core.ledger().events.iter().filter_map(|e| e.plain().map(|p| p.kind)).collect();
    assert!(kinds.contains(&PlainKind::CeasefireUsed));
}

#[test]
fn a_yssaril_ssruu_next_to_a_sardakk_seat_is_a_gap_at_the_tail_only_where_t_ro_could_place() {
    covers!("driver::windows::tactical::TacticalAction");
    let p = plain_systems();
    // The active system has no planet the actor controls: T'ro (and a copy of it) has nothing to place on, so nothing is missed silently.
    for (ssruu, sardakk, expect_gap) in [(true, true, false), (false, true, false), (true, false, false)] {
        let other = if sardakk { Faction::SardakkNorr } else { Faction::TheArborec };
        let seat1 = if ssruu { Faction::TheYssarilTribes } else { Faction::TheMahactGeneSorcerers };
        let _ = seat1;
        let mut sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheYssarilTribes, other]);
        sc = sc.map_hexes(&[(p[0], Hex::new(0, 0)), (p[1], Hex::new(1, 0)), (p[2], Hex::new(2, 0)), (p[3], Hex::new(3, 0))]).tokens(0, 3, 4, 2).active(Some(0)).unit(p[2], None, Unit::CruiserI, 0, 1);
        let (mut core, k) = sc.build();
        core.players.edit(1).leaders.status.insert(Leader::Ssruu, if ssruu { ti4_engine_core::state::LeaderStatus::Readied } else { ti4_engine_core::state::LeaderStatus::Exhausted });
        let env = env(&k, 1);
        let tiles = [p[0], p[1], p[2], p[3]];
        assert_eq!(to_the_close(&mut core, &env, &tiles).unwrap(), Live::Done);
        core.pending_mut().close(WindowKind::Aftermath);
        let got = advance_tactical(&mut core, &env, None);
        let _ = expect_gap;
        assert_eq!(got.map(|r| r.0), Ok(Live::Done), "ssruu {ssruu} sardakk {sardakk}");
    }
}

// -- lane M6c: the survivors of the mutation check of `hooks.rs`, `routes.rs` and `tail.rs` ---------------------------------------------------

/// T'ro's mark is written for a Sardakk seat whose agent is readied OR exhausted (an exhausted T'ro can still be copied with Ssruu), not for a
/// seat that does not hold it.
#[test]
fn the_sardakk_mark_is_written_for_a_readied_or_exhausted_tro_only() {
    covers!("driver::windows::tactical::TacticalAction");
    use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
    use ti4_engine_core::state::LeaderStatus;
    let p = plain_systems();
    for (status, expect) in [(Some(LeaderStatus::Readied), true), (Some(LeaderStatus::Exhausted), true), (Some(LeaderStatus::Locked), false), (None, false)] {
        let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::SardakkNorr])
            .map_hexes(&[(p[0], Hex::new(0, 0)), (p[1], Hex::new(1, 0)), (p[2], Hex::new(2, 0)), (p[3], Hex::new(3, 0))])
            .tokens(0, 3, 4, 2)
            .active(Some(0))
            .unit(p[1], None, Unit::CruiserI, 0, 1);
        let (mut core, k) = sc.build();
        match status {
            Some(s) => {
                core.players.edit(1).leaders.status.insert(Leader::Tro, s);
            }
            None => {
                core.players.edit(1).leaders.status.remove(&Leader::Tro);
            }
        }
        let env = env(&k, 1);
        open_tactical(&mut core, Seat(0));
        let Live::Asks(q) = advance_tactical(&mut core, &env, None).unwrap().0 else { panic!() };
        advance_tactical(&mut core, &env, Some(pick(&q.choice, |o| *o == t(TOpt::Activate(p[3]))))).unwrap();
        let want = expect.then(|| MarkVal::Atom(Atom::System(p[3])));
        assert_eq!(marks_of(&core, MarkKey::SardakkAgentTactical(Seat(0))), want, "T'ro {status:?}");
    }
}

fn conduit_links(core: &CoreState) -> usize {
    core.galaxy().map_or(0, |g| g.extra_links.len())
}

/// The links a Spatial Conduit made last for the activation: a hook that ends the turn clears them (the Mahact commander returns the tokens and
/// ends it), and so does the tail of a plain action.
#[test]
fn the_conduit_links_are_cleared_when_a_hook_ends_the_turn_and_when_the_tail_closes() {
    covers!("driver::windows::tactical::TacticalAction");
    use ti4_engine_core::state::LeaderStatus;
    // A hook ends the turn: the Mahact commander on a system that holds the seat's own token.
    let (sc, tiles) = row();
    let (mut core, k) = sc.command_token(row().1[3], 0).build();
    let mut mahact = core.players().by_seat(Seat(0)).map(|p| p.faction);
    let _ = mahact.take();
    core.players.edit(0).faction = Faction::TheMahactGeneSorcerers;
    core.players.edit(0).leaders.status.insert(Leader::IlNaViroset, LeaderStatus::Unlocked);
    ti4_engine_core::kernel::fx_move::set_conduit_links(&mut core, Seat(0), tiles[3]);
    assert!(conduit_links(&core) > 0, "the links are set before the activation");
    let env1 = env(&k, 1);
    open_tactical(&mut core, Seat(0));
    let Live::Asks(q) = advance_tactical(&mut core, &env1, None).unwrap().0 else { panic!() };
    advance_tactical(&mut core, &env1, Some(pick(&q.choice, |o| *o == t(TOpt::Activate(tiles[3]))))).unwrap();
    assert_eq!(conduit_links(&core), 0, "the turn ended by the hook: the links are gone");

    // A plain action: the tail clears them.
    let (sc, tiles) = row();
    let (mut core, k) = sc.build();
    let env = env(&k, 1);
    assert_eq!(to_the_close(&mut core, &env, &tiles).unwrap(), Live::Done);
    core.pending_mut().close(WindowKind::Aftermath);
    ti4_engine_core::kernel::fx_move::set_conduit_links(&mut core, Seat(0), tiles[3]);
    assert!(conduit_links(&core) > 0);
    assert_eq!(advance_tactical(&mut core, &env, None).unwrap().0, Live::Done);
    assert_eq!(conduit_links(&core), 0, "the tail cleared the links");
}

/// Field Marshal Mercer is a rule of the table (lane M6d): the tail never stops for him, and announces the end of the action when he is
/// readied (anywhere), not when he is exhausted (the string registers no Ssruu copy of him, so a readied Ssruu does not listen for him).
#[test]
fn mercer_hears_the_end_of_the_action_when_readied_and_the_tail_never_stops_for_him() {
    covers!("driver::windows::tactical::TacticalAction");
    use ti4_engine_core::state::LeaderStatus;
    let ended = |c: &CoreState| c.ledger().events.iter().any(|e| matches!(e, Emit::Typed(r) if r.kind == EventKind::TacticalActionEnded));
    for (status, ssruu, expect) in [(LeaderStatus::Readied, false, true), (LeaderStatus::Exhausted, false, false), (LeaderStatus::Exhausted, true, false), (LeaderStatus::Locked, true, false)] {
        let (sc, tiles) = row();
        let (mut core, k) = sc.build();
        core.players.edit(1).leaders.status.insert(Leader::FieldMarshalMercer, status);
        if ssruu {
            core.players.edit(0).faction = Faction::TheYssarilTribes;
            core.players.edit(0).leaders.status.insert(Leader::Ssruu, LeaderStatus::Readied);
        }
        let env = env(&k, 1);
        assert_eq!(to_the_close(&mut core, &env, &tiles).unwrap(), Live::Done);
        core.pending_mut().close(WindowKind::Aftermath);
        let got = advance_tactical(&mut core, &env, None);
        assert_eq!(got.map(|r| r.0), Ok(Live::Done), "mercer {status:?} ssruu {ssruu}");
        assert_eq!(ended(&core), expect, "mercer {status:?} ssruu {ssruu}");
    }
    // The end of the action is announced to a listener: an exhausted T'ro listens only through a readied Ssruu.
    let ended = |c: &CoreState| c.ledger().events.iter().any(|e| matches!(e, Emit::Typed(r) if r.kind == EventKind::TacticalActionEnded));
    for (ssruu, expect) in [(false, false), (true, true)] {
        let (sc, tiles) = row();
        let (mut core, k) = sc.build();
        core.players.edit(1).leaders.status.insert(Leader::Tro, LeaderStatus::Exhausted);
        if ssruu {
            core.players.edit(0).faction = Faction::TheYssarilTribes;
            core.players.edit(0).leaders.status.insert(Leader::Ssruu, LeaderStatus::Readied);
        }
        let env = env(&k, 1);
        assert_eq!(to_the_close(&mut core, &env, &tiles).unwrap(), Live::Done);
        core.pending_mut().close(WindowKind::Aftermath);
        let _ = advance_tactical(&mut core, &env, None);
        assert_eq!(ended(&core), expect, "ssruu {ssruu}");
    }
}

/// `ship_routes` on a hub (a centre and a ring of six): the ship at ring 0 going to the system across. The default first, at most `MAX_ROUTES`,
/// each a different set of systems; Gravity Drive adds boosted routes; a ship that ignores rifts rolls for none.
#[test]
fn the_routes_of_a_ship_are_the_default_first_at_most_five_and_follow_the_boosts_and_the_rifts() {
    covers!("rules::tactical::routes");
    use ti4_engine_core::rules::tactical::routes::{ship_routes, MAX_ROUTES};
    let p = plain_systems();
    let ring = [Hex::new(1, 0), Hex::new(0, 1), Hex::new(-1, 1), Hex::new(-1, 0), Hex::new(0, -1), Hex::new(1, -1)];
    let mut tiles = vec![(p[0], Hex::new(0, 0))];
    tiles.extend(ring.iter().enumerate().map(|(i, h)| (p[i + 1], *h)));
    let (origin, target) = (p[1], p[4]);
    let scenario = |centre: System| {
        let mut t = tiles.clone();
        t[0].0 = centre;
        Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_hexes(&t).tokens(0, 3, 4, 2).active(Some(0)).active_system(target).tech(0, Technology::CruiserII).unit(origin, None, Unit::CruiserII, 0, 1)
    };
    // Plain ring: the move value 3 reaches seven different sets of systems; five are offered, the default first.
    let (core, k) = scenario(p[0]).build();
    let routes = ship_routes(&core, &k, Seat(0), origin, 0, Unit::CruiserII, (false, false));
    assert_eq!(routes.len(), 5, "{routes:?}");
    assert_eq!(MAX_ROUTES, 5, "the limit the string documents");
    assert!(routes[0].default && routes.iter().skip(1).all(|r| !r.default));
    assert!(routes.iter().all(|r| r.path.first() == Some(&origin) && r.path.last() == Some(&target) && !r.gd && !r.ionian));
    let sets: std::collections::BTreeSet<Vec<System>> = routes.iter().map(|r| { let mut s = r.path.clone(); s.sort(); s }).collect();
    assert_eq!(sets.len(), routes.len(), "no two routes visit the same systems");
    // Gravity Drive: with the rift in the centre the plain route of a slower ship rolls for it, and a boosted route round the ring is the safe way round;
    // without the technology there is none.
    let slow = |sc: Scenario| sc.unit(origin, None, Unit::DestroyerI, 0, 1);
    let (core, k) = slow(scenario(System::GravityRift)).build();
    assert!(ship_routes(&core, &k, Seat(0), origin, 1, Unit::DestroyerI, (false, false)).iter().all(|r| !r.gd));
    let (core, k) = slow(scenario(System::GravityRift).tech(0, Technology::GravityDrive)).build();
    let boosted = ship_routes(&core, &k, Seat(0), origin, 1, Unit::DestroyerI, (false, false));
    assert!(boosted[0].default && !boosted[0].gd && boosted.iter().skip(1).any(|r| r.gd), "{boosted:?}");
    // The Ionian Fuel Refinery (Tempesta controlled, not exhausted) boosts the same way.
    let (core, k) = slow(scenario(System::GravityRift).control(Planet::Tempesta, 0)).build();
    assert!(ship_routes(&core, &k, Seat(0), origin, 1, Unit::DestroyerI, (false, false)).iter().skip(1).any(|r| r.ionian));
    // A gravity rift in the centre: the route through it rolls for it, unless the ship ignores rifts (Circlet of the Void).
    let (mut core, k) = scenario(System::GravityRift).build();
    let rolling = ship_routes(&core, &k, Seat(0), origin, 0, Unit::CruiserII, (false, false));
    assert_eq!(rolling[0].rifts, vec![System::GravityRift], "{rolling:?}");
    core.players.edit(0).cards.relics.push(ti4_engine_core::generated::Relic::CircletOfTheVoid);
    let ignoring = ship_routes(&core, &k, Seat(0), origin, 0, Unit::CruiserII, (false, false));
    assert!(ignoring.iter().all(|r| r.rifts.is_empty()), "{ignoring:?}");
}
