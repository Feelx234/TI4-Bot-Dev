//! Lane M6: the tactical composer (`driver::windows::tactical`) run through the live arm (`advance_tactical`): a whole tactical action from the opening to
//! the hand-off to the aftermath and the tail, held as a payload on the `Pending` stack between the questions. The leaf windows have their own tests
//! (`tactical.rs`, `fleet`); these check what the composer adds: the order of the stages, the payload round trip, a clone in the middle, the hand-off,
//! the enforcement in the head, the tail, the bails.

use std::rc::Rc;

use ti4_engine_core::choice::tactical::{Ask as TAsk, Opt as TOpt};
use ti4_engine_core::containers::Cow;
use ti4_engine_core::driver::live::{ask, Env, Live, LiveError};
use ti4_engine_core::driver::windows::tactical::{advance_tactical, ask_tactical, open_free, open_tactical, TacticalAction};
use ti4_engine_core::hex::Hex;
use ti4_engine_core::kernel::ability::{TacticalCodes, TacticalGap, Unknown};
use ti4_engine_core::kernel::stage::{AbilityTable, Held};
use ti4_engine_core::kernel::Registry;
use ti4_engine_core::state::{AStage, Code, EventKind, PlainKind, TStage, Window as OpenWindow, WindowKind};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::rig::KitSeeds;
use ti4_engine_core_tests::scenario::plain_systems;

fn codes() -> TacticalCodes {
    TacticalCodes { tactic: Code(21), space_cannon: Code(22), space_cannon_cause: Code(23), casualty: Code(24), commander: Code(25), pre_roll: Code(26) }
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
