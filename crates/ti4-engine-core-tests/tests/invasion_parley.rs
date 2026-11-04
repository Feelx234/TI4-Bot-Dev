//! Parley (rules version 36, alpha report fb_1792786598, string-engine bug S7 in `docs/STRING_ENGINE_BUGS.md`): "return the committed units to the space
//! area" cancels the whole landing. The commit step lands one unit per decision; the string card returned the last unit only and the invader landed
//! again. Here the Parley effect of the core (`rules::invasion::commit::parley`, the function the card registers) is the reaction of the landing's AFTER
//! window, played by the planet's controller, and the invasion machine runs the commit step around it.

use std::rc::Rc;

use ti4_engine_core::choice::invasion::{Ask as IAsk, Opt as IOpt};
use ti4_engine_core::choice::reaction::{AbilityId, AbilityKind};
use ti4_engine_core::containers::Cow;
use ti4_engine_core::driver::live::{Env, Live, LiveError};
use ti4_engine_core::driver::windows::invasion::{advance_invasion, open_invasion};
use ti4_engine_core::kernel::ability::{CondCx, Eff, InvasionCodes, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::kernel::Registry;
use ti4_engine_core::state::{Code, EventKind, Key, NoteHold, Piece, WindowKind};
use ti4_engine_core::choice::reaction::Relation;
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

/// Parley, as the reaction of the landing: the controller of the planet may play it once `AT` units of the invader stand on it (the window is
/// asked; `answer` plays or declines it).
const PARLEY: Slot = Slot(60_001);

fn parley_held(c: &CondCx<'_>) -> bool {
    let Some(planet) = c.event.payload.planet(Key::Planet) else { return false };
    let landed = c.state.board().planet(planet).units.iter().filter(|p| p.owner() == Seat(0)).count();
    c.event.payload.seat(Key::Controller) == Some(c.def.owner) && landed >= AT.with(std::cell::Cell::get)
}

thread_local! {
    /// The number of invader units on the planet at which the holder plays the card.
    static AT: std::cell::Cell<usize> = const { std::cell::Cell::new(3) };
}

fn play_parley(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let (state, kernel) = eff.state_and_kernel();
    let scope = ti4_engine_core::tables::Scope(u32::from(kernel.sources.0));
    ti4_engine_core::rules::invasion::commit::parley(state, scope);
    Ok(())
}

/// The environment of a run with Parley in the hand of the defender (seat 1).
fn env<'k>(k: &'k Kernel, core: &CoreState, seed: u64) -> Env<'k> {
    let mut table = AbilityTable::default();
    table.add(Spec::new(PARLEY, AbilityKind::Other, EventKind::UnitsCommitted, Relation::After, play_parley).optional().when(parley_held));
    let mut registry = Registry::default();
    registry.register(table.registry(core, &[Seat(0), Seat(1)], &mut |spec, _, _| AbilityId { kind: AbilityKind::Other, key: Code(spec.slot.0) }));
    ti4_engine_core::driver::children::register(&mut table);
    table.codes.invasion = codes();
    table.codes.ground_combat = Code(1);
    table.codes.harrow = Code(2);
    Env { kernel: k, registry: Cow::new(registry), table, seeds: Rc::new(KitSeeds(seed)) }
}

/// The invader (seat 0) in the space area with a carrier and `n` infantry; the defender (seat 1) holds both planets with `d` infantry on the first.
fn conquest(n: usize, d: usize) -> (CoreState, Kernel) {
    let (p1, p2) = planets();
    let (mut s, k) = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec])
        .map_from(&[sys()])
        .unit(sys(), None, Unit::CarrierI, 0, 1)
        .unit(sys(), Some(p1), Unit::InfantryI, 1, d)
        .control(p1, 1)
        .control(p2, 1)
        .build();
    for _ in 0..n {
        s.board_mut().sys_mut(sys()).units.push(Piece::new(Unit::InfantryI, Seat(0)));
    }
    (s, k)
}

/// Play the invasion to its end; `pick` chooses among the options of each question. The questions asked, in order.
fn play(core: &mut CoreState, env: &Env<'_>, pick: &mut dyn FnMut(&Choice) -> usize) -> (Vec<Choice>, Result<(), LiveError>) {
    let mut asked = Vec::new();
    let mut answer: Option<Pick> = None;
    for _ in 0..600 {
        match advance_invasion(core, env, answer.take()) {
            Err(e) => return (asked, Err(e)),
            Ok((Live::Asks(a), _)) => {
                let i = pick(&a.choice).min(a.choice.opts.len() - 1);
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

fn commits(asked: &[Choice]) -> usize {
    asked.iter().filter(|c| matches!(c.ask, Ask::Invasion(IAsk::CommitGround { .. }))).count()
}

fn fights(asked: &[Choice]) -> usize {
    asked.iter().filter(|c| matches!(c.ask, Ask::Invasion(IAsk::FightRound { .. }))).count()
}

fn on(core: &CoreState, planet: Planet, seat: u8) -> usize {
    core.board().planet(planet).units.iter().filter(|p| p.owner() == Seat(seat)).count()
}

fn in_space(core: &CoreState, seat: u8) -> usize {
    core.board().sys(sys()).units.iter().filter(|p| p.owner() == Seat(seat) && p.kind() == Unit::InfantryI).count()
}

/// The option that lands on `planet` (the first troop); a reaction window is answered by playing the card (its first option).
fn onto(planet: Planet) -> impl FnMut(&Choice) -> usize {
    move |c| c.opts.as_slice().iter().position(|o| matches!(o, Opt::Invasion(IOpt::Commit { planet: p, .. }) if *p == planet)).unwrap_or(0)
}

fn is_window(c: &Choice) -> bool {
    matches!(c.ask, Ask::Reaction(_))
}

#[test]
fn parley_after_the_third_landing_returns_all_three_and_the_invader_cannot_land_again() {
    covers!("driver::windows::invasion::InvasionMachine");
    let (p1, _) = planets();
    AT.with(|a| a.set(3));
    let (mut core, k) = conquest(6, 1);
    let env = env(&k, &core, 3);
    open_invasion(&mut core, Seat(0), sys(), NoteHold::new());
    let (asked, end) = play(&mut core, &env, &mut onto(p1));
    assert_eq!(end, Ok(()), "the invasion ends");
    assert_eq!(asked.iter().filter(|c| is_window(c)).count(), 1, "the holder was asked once: the window of the third landing");
    assert_eq!(commits(&asked), 3, "three landings, then the card ends the commit step: the invader is not asked again");
    assert_eq!(fights(&asked), 0, "nothing stands on the planet to fight");
    assert_eq!(on(&core, p1, 0), 0, "all three units left the planet, not only the last");
    assert_eq!(in_space(&core, 0), 6, "and stand in the space area again");
    assert_eq!(core.board().planet(p1).control, Some(Seat(1)), "the defender keeps the planet");
    assert_eq!(on(&core, p1, 1), 1, "and its infantry");
}

#[test]
fn parley_after_the_first_landing_cancels_the_whole_invasion() {
    covers!("driver::windows::invasion::InvasionMachine");
    let (p1, _) = planets();
    AT.with(|a| a.set(1));
    let (mut core, k) = conquest(6, 1);
    let env = env(&k, &core, 4);
    open_invasion(&mut core, Seat(0), sys(), NoteHold::new());
    let (asked, end) = play(&mut core, &env, &mut onto(p1));
    assert_eq!(end, Ok(()));
    assert_eq!((commits(&asked), fights(&asked)), (1, 0));
    assert_eq!((on(&core, p1, 0), in_space(&core, 0)), (0, 6));
    assert_eq!(core.board().planet(p1).control, Some(Seat(1)));
}

#[test]
fn parley_returns_the_units_of_its_planet_only() {
    covers!("driver::windows::invasion::InvasionMachine");
    let (p1, p2) = planets();
    // The landings go to the second planet, the first, the second: the card is played when two units stand on a planet, which the third landing
    // makes true on the second planet. The unit on the first planet was committed too, to a planet the card does not name: it stays and fights.
    AT.with(|a| a.set(2));
    let (mut core, k) = conquest(6, 1);
    let env = env(&k, &core, 5);
    open_invasion(&mut core, Seat(0), sys(), NoteHold::new());
    let mut landings = 0;
    let mut choose = |c: &Choice| {
        if !matches!(c.ask, Ask::Invasion(IAsk::CommitGround { .. })) {
            return 0;
        }
        landings += 1;
        onto(if landings == 2 { p1 } else { p2 })(c)
    };
    let (asked, end) = play(&mut core, &env, &mut choose);
    assert_eq!(end, Ok(()));
    assert_eq!(commits(&asked), 3, "the third landing is answered; the step is over");
    assert_eq!(on(&core, p2, 0), 0, "both units of the second planet came back");
    assert!(fights(&asked) > 0, "the unit that landed on the first planet stays and fights the defender there");
    assert_eq!(in_space(&core, 0), 5, "five infantry in space: the two that came back and the three never committed");
}

#[test]
fn a_declined_window_leaves_the_next_landing_open_to_the_card() {
    covers!("driver::windows::invasion::InvasionMachine");
    let (p1, _) = planets();
    // The card could be played from the first landing on; the holder declines twice and plays it at the third.
    AT.with(|a| a.set(1));
    let (mut core, k) = conquest(6, 1);
    let env = env(&k, &core, 7);
    open_invasion(&mut core, Seat(0), sys(), NoteHold::new());
    let (mut windows, mut land) = (0, onto(p1));
    let mut choose = |c: &Choice| {
        if is_window(c) {
            windows += 1;
            // (Index 0 plays the card, the last option declines it.)
            return if windows < 3 { c.opts.len() - 1 } else { 0 };
        }
        land(c)
    };
    let (asked, end) = play(&mut core, &env, &mut choose);
    assert_eq!(end, Ok(()));
    assert_eq!(commits(&asked), 3, "two landings stood, the third was answered");
    assert_eq!((on(&core, p1, 0), in_space(&core, 0)), (0, 6), "all three came back");
    assert_eq!(fights(&asked), 0);
}

#[test]
fn a_landing_nobody_answers_goes_on_as_before() {
    covers!("driver::windows::invasion::InvasionMachine");
    let (p1, _) = planets();
    AT.with(|a| a.set(99));
    let (mut core, k) = conquest(6, 1);
    let env = env(&k, &core, 6);
    open_invasion(&mut core, Seat(0), sys(), NoteHold::new());
    let (asked, end) = play(&mut core, &env, &mut onto(p1));
    assert_eq!(end, Ok(()));
    assert!(commits(&asked) >= 6, "the invader lands every infantry it has");
    assert!(fights(&asked) > 0);
    assert_eq!(core.board().planet(p1).control, Some(Seat(0)), "six infantry against one take the planet");
}
