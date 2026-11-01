//! Lane M2: Freelancers inside an exploration. The unit is `ProduceOne`, its plain state (`OneRun`) lives in the run (`ExploreRun::one`) so a host that
//! keeps the run in a payload (the invasion's first claim) resumes the unit at its question.

use ti4_engine_core::choice::production::{Ask as PAsk, Opt as POpt};
use ti4_engine_core::driver::{Driver, Status};
use ti4_engine_core::kernel::stage::Window as _;
use ti4_engine_core::rules::explore_windows::Exploring;
use ti4_engine_core::rules::production::one::ProduceOne;
use ti4_engine_core::state::{EDeck, ETarget, EStage, ExploreRun, OStage};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;
use ti4_engine_core_tests::windows::explore_table;

fn stack(state: &mut CoreState, cards: &[Explore]) {
    state.decks_mut().explore[ExploreType::Cultural as usize].set(cards.to_vec());
    state.decks_mut().explore_present |= 1 << ExploreType::Cultural as u8;
}

fn table() -> ti4_engine_core::kernel::stage::AbilityTable {
    let mut t = explore_table();
    ti4_engine_core::driver::children::register(&mut t);
    t.facts.roster = Some(Box::leak(Box::default()));
    t.facts.card_groups = (0..ActionCard::COUNT as u8).collect();
    t
}

/// Seat 0 (an Arborec) holds a space dock on the first planet of a plain system and `influence` of planets to pay with.
fn position() -> (CoreState, Kernel, System, Planet) {
    let system = plain_systems()[0];
    let planet = system.data().planets[0];
    let (mut state, kernel) = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat])
        .map_from(&[system])
        .control(planet, 0)
        .unit(system, Some(planet), Unit::SpaceDockI, 0, 1)
        .tokens(0, 2, 2, 2)
        .build();
    stack(&mut state, &[Explore::Freelancers1]);
    (state, kernel, system, planet)
}

fn resolver() -> ti4_engine_core::kernel::Resolver {
    ti4_engine_core::kernel::Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0)))
}

#[test]
fn freelancers_asks_which_unit_and_the_card_closes_when_the_unit_stands_or_is_declined() {
    covers!("rules::explore_windows::Exploring");
    for decline in [true, false] {
        let (mut state, kernel, _, planet) = position();
        let window = Exploring::planet(Seat(0), planet, EDeck::Fixed(ExploreType::Cultural), false);
        let mut driver = Driver::new(window, resolver(), table(), 1);
        let Status::Asking(c) = driver.run(&mut state, &kernel).unwrap() else { panic!("the unit is asked") };
        assert!(matches!(c.ask, Ask::Production(PAsk::One(_))), "{:?}", c.ask);
        assert!(state.meta().influence_pays_for_units.contains(Seat(0)), "influence pays for this unit");
        let pick = if decline {
            Pick::Opt(Opt::Decline)
        } else {
            Pick::Opt(c.opts.iter().copied().find(|o| matches!(o, Opt::Production(POpt::One { .. }))).expect("a unit is offered"))
        };
        let mut status = driver.resume(&mut state, &kernel, pick).unwrap();
        for _ in 0..8 {
            let Status::Asking(c) = status else { break };
            let pick = Pick::Opt(c.opts.as_slice()[0]);
            status = driver.resume(&mut state, &kernel, pick).unwrap();
        }
        assert!(matches!(status, Status::Done), "decline {decline}");
        assert!(driver.window.explored() && driver.window.unsupported().is_none(), "decline {decline}");
        let how = driver.window.run().how;
        assert_eq!(how, if decline { ti4_engine_core::state::EHow::Unresolved } else { ti4_engine_core::state::EHow::Resolved }, "the card resolved when a unit stands (decline {decline})");
        assert!(!state.meta().influence_pays_for_units.contains(Seat(0)), "the permission goes with the unit (decline {decline})");
    }
}

#[test]
fn the_unit_in_production_is_kept_in_the_run_and_a_rebuilt_window_asks_the_same_question() {
    covers!("rules::explore_windows::Exploring");
    let (mut state, kernel, _, planet) = position();
    let mut driver = Driver::new(Exploring::hosted(ExploreRun::new(Seat(0), ETarget::Planet(planet), EDeck::Fixed(ExploreType::Cultural), false)), resolver(), table(), 1);
    let Status::Asking(first) = driver.run(&mut state, &kernel).unwrap() else { panic!("the unit is asked") };
    let run = driver.window.run().clone();
    assert_eq!(run.stage, EStage::Producing);
    let one = run.one.expect("the unit's state is in the run");
    assert_eq!(one.stage, OStage::Choosing);
    // The host drops the window and builds another from the stored run: the question and the answer's effect are the same.
    // (A host supplies the link to the production area: it rebuilds the window before it is stepped.)
    assert!(Exploring::hosted(run.clone()).ask_of(&state, &kernel).is_none(), "without the link the unit's question cannot be read");
    let mut resumed = Driver::new(Exploring::hosted(run).with_one(ti4_engine_core::driver::children::one_link()), resolver(), table(), 1);
    assert!(resumed.restore_stage(&state, &kernel));
    let mut straight = state.clone();
    let pick = Pick::Opt(first.opts.iter().copied().find(|o| matches!(o, Opt::Production(POpt::One { .. }))).unwrap());
    let a = resumed.resume(&mut state, &kernel, pick).unwrap();
    let b = driver.resume(&mut straight, &kernel, pick).unwrap();
    assert_eq!(a, b);
    assert_eq!(state, straight, "a rebuilt window continues identically");
    // ProduceOne::resume(run()) is the identity.
    let w = ProduceOne::resume(one);
    assert_eq!(w.run(), one);
}

/// Lane L: a unit that stands over the capacity of its system (Freelancers makes two fighters for one cost; a lone cruiser carries one) no longer ends the
/// exploration in the counted bail `EWhy::Freelancers`: the enforcement of the system (`Spawn::Seeing`, the string's `fleet::enforce_seeing`) is a child of
/// the card, which takes the units that do not fit away. (A ship over the fleet supply is never offered: `Why::FleetFull`.)
#[test]
fn a_unit_over_the_capacity_of_its_system_is_followed_by_the_enforcement_of_the_system() {
    covers!("rules::explore_windows::Exploring");
    let (mut state, kernel, system, planet) = position();
    state.board_mut().sys_mut(system).units.push(ti4_engine_core::state::Piece::new(Unit::CruiserI, Seat(0)));
    let window = Exploring::planet(Seat(0), planet, EDeck::Fixed(ExploreType::Cultural), false);
    let mut driver = Driver::new(window, resolver(), table(), 1);
    let Status::Asking(c) = driver.run(&mut state, &kernel).unwrap() else { panic!("the unit is asked") };
    let fighter = c.opts.iter().copied().find(|o| matches!(o, Opt::Production(POpt::One { unit: Unit::FighterI, .. }))).expect("fighters are offered");
    let mut status = driver.resume(&mut state, &kernel, Pick::Opt(fighter)).unwrap();
    for _ in 0..16 {
        let Status::Asking(c) = status else { break };
        let pick = Pick::Opt(c.opts.as_slice()[0]);
        status = driver.resume(&mut state, &kernel, pick).unwrap();
    }
    assert!(matches!(status, Status::Done), "the card ends: {status:?}");
    assert!(driver.window.unsupported().is_none(), "no bail: {:?}", driver.window.unsupported());
    let fighters = state.board().sys(system).units.as_slice().iter().filter(|p| p.owner() == Seat(0) && p.kind() == Unit::FighterI).count();
    assert!(fighters <= 1, "the cruiser carries one: {fighters}");
    assert_eq!(driver.window.run().how, ti4_engine_core::state::EHow::Resolved, "the card resolved: a unit stood");
}
