//! Lane M2: Terragenesis (`cd_registry::titans::offer::TERRAGENESIS`), the Titans' faction ability, as a rule of the core table: "After you explore a
//! planet that does not have a sleeper token: You may place or move 1 sleeper token onto that planet."

use ti4_engine_core::choice::reaction::{Ask as RAsk, Opt as ROpt};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::state::{EventKind, Key, Payload, Val};
use ti4_engine_core_factions::cd_registry::titans::offer::TERRAGENESIS;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::windows::EmitEvent;

fn table() -> AbilityTable {
    let mut t = AbilityTable::default();
    ti4_engine_core_factions::cd_registry::register_abilities(&mut t);
    t
}

fn planets(n: usize) -> Vec<Planet> {
    Planet::ALL.iter().copied().filter(|p| p.data().system.is_some()).take(n).collect()
}

fn sleeper(s: &mut CoreState, owner: u8, planet: Planet) {
    let system = planet.data().system.unwrap();
    s.marks_mut().tail.edit().insert(MarkKey::TitansSleeper(planet), MarkVal::Rec(vec![MarkVal::Atom(Atom::Seat(Seat(owner))), MarkVal::Atom(Atom::System(system))]));
}

fn has(s: &CoreState, planet: Planet) -> bool {
    s.marks().tail.contains_key(&MarkKey::TitansSleeper(planet))
}

/// Seat 0 (`faction`) has explored `target`; the sleeper tokens in `board` stand on the other planets.
fn explored(faction: Faction, target: Planet, board: &[Planet]) -> Rig<EmitEvent> {
    let other = if faction == Faction::TheEmiratesOfHacan { Faction::TheFederationOfSol } else { Faction::TheEmiratesOfHacan };
    let (mut state, kernel) = Scenario::new(&[faction, other]).kernel(kernel_all()).build();
    for p in board {
        sleeper(&mut state, 0, *p);
    }
    let payload = Payload::new().with(Key::Player, Val::Seat(Seat(0))).with(Key::Planet, Val::Planet(target)).with(Key::Deck, Val::Int(0));
    Rig::from_parts(state, kernel, EmitEvent::new(fx::event(EventKind::PlanetExplored, payload))).with_abilities(table())
}

#[test]
fn the_ability_is_offered_after_an_exploration_and_a_lone_option_is_taken_unasked() {
    covers!("cd_registry::titans::offer::TERRAGENESIS");
    let p = planets(8);
    let mut rig = explored(Faction::TheTitansOfUl, p[0], &[]);
    rig.run();
    assert_eq!(TERRAGENESIS.0, 17001);
    assert_eq!(rig.asked().actor, Seat(0), "{}", rig.transcript());
    assert!(matches!(rig.asked().ask, Ask::Reaction(RAsk::Window { .. })), "{}", rig.transcript());
    let use_it = rig.opts().iter().position(|o| *o != Opt::Decline).expect("the ability");
    rig.pick(use_it);
    assert!(rig.is_done(), "with no token on the board the only option is to place one: {}", rig.transcript());
    assert!(has(&rig.state, p[0]), "the token is on the explored planet");
}

#[test]
fn a_token_on_the_board_gives_a_choice_between_placing_a_new_one_and_moving_it() {
    covers!("cd_registry::titans::offer::TERRAGENESIS");
    let p = planets(8);
    let mut rig = explored(Faction::TheTitansOfUl, p[0], &[p[3]]);
    rig.run();
    let use_it = rig.opts().iter().position(|o| *o != Opt::Decline).expect("the ability");
    rig.pick(use_it);
    assert!(matches!(rig.asked().ask, Ask::Reaction(RAsk::Terragenesis { planet }) if planet == p[0]), "{}", rig.transcript());
    assert!(!rig.asked().ctx.optional && !rig.asked().ctx.space_battle, "a plain question of the faction ability");
    assert_eq!(rig.opts(), vec![Opt::Reaction(ROpt::Sleeper { from: None }), Opt::Reaction(ROpt::Sleeper { from: Some(p[3]) }), Opt::Decline].into_iter().filter(|o| *o != Opt::Decline).collect::<Vec<_>>());
    rig.pick_where(|o| matches!(o, Opt::Reaction(ROpt::Sleeper { from: Some(_) })));
    assert!(rig.is_done());
    assert!(has(&rig.state, p[0]) && !has(&rig.state, p[3]), "moved, not placed");
}

#[test]
fn placing_a_new_token_leaves_the_others_and_five_tokens_can_only_be_moved() {
    covers!("cd_registry::titans::offer::TERRAGENESIS");
    let p = planets(8);
    let mut rig = explored(Faction::TheTitansOfUl, p[0], &[p[3]]);
    rig.run();
    let use_it = rig.opts().iter().position(|o| *o != Opt::Decline).unwrap();
    rig.pick(use_it);
    rig.pick_where(|o| matches!(o, Opt::Reaction(ROpt::Sleeper { from: None })));
    assert!(has(&rig.state, p[0]) && has(&rig.state, p[3]));
    // Five on the board: none to place.
    let mut full = explored(Faction::TheTitansOfUl, p[0], &p[1..6]);
    full.run();
    let use_it = full.opts().iter().position(|o| *o != Opt::Decline).unwrap();
    full.pick(use_it);
    assert!(full.opts().iter().all(|o| matches!(o, Opt::Reaction(ROpt::Sleeper { from: Some(_) }))), "{}", full.transcript());
    assert_eq!(full.opts().len(), 5);
}

#[test]
fn a_declined_ability_changes_nothing_and_the_rule_needs_a_titans_seat_and_a_planet_without_a_token() {
    covers!("cd_registry::titans::offer::TERRAGENESIS");
    let p = planets(8);
    let mut rig = explored(Faction::TheTitansOfUl, p[0], &[p[3]]);
    rig.run();
    rig.decline();
    assert!(rig.is_done() && !has(&rig.state, p[0]) && has(&rig.state, p[3]));
    // Another faction explores: nothing is offered.
    let mut other = explored(Faction::TheEmiratesOfHacan, p[0], &[]);
    other.run();
    assert!(other.is_done(), "{}", other.transcript());
    assert!(!has(&other.state, p[0]));
    // The planet already has a token (even one of the Titans' own): nothing is offered.
    let mut taken = explored(Faction::TheTitansOfUl, p[0], &[p[0]]);
    taken.run();
    assert!(taken.is_done(), "{}", taken.transcript());
}
