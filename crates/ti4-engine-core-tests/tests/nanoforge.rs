//! The relic Nano-Forge, rule-derived (the string engine has no counterpart): "ACTION: Attach this card to a non-legendary, non-home planet you
//! control, its resource and influence values are increased by 2 and it is a legendary planet. This action cannot be performed once attached."

use ti4_engine_core::choice::relic::Opt as ROpt;
use ti4_engine_core::generated::{Attachment, Faction, Planet, Relic};
use ti4_engine_core::query::economy::{attachment_bonus, Spend};
use ti4_engine_core::query::{board, relic};
use ti4_engine_core::rules::relic_windows::NanoForge;
use ti4_engine_core::state::PlanetFlags;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn table() -> Scenario {
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec, Faction::TheYinBrotherhood]).map_ring(7)
}

fn r(o: ROpt) -> Opt {
    Opt::Relic(o)
}

fn give(s: &mut CoreState, seat: u8, x: Relic) {
    s.players_mut().by_seat_mut(Seat(seat)).unwrap().cards.relics.push(x);
}

fn legendary() -> Planet {
    Planet::ALL.iter().copied().find(|p| p.data().legendary && p.data().homeworld_of.is_none()).expect("a legendary planet")
}

fn homeworld() -> Planet {
    Planet::ALL.iter().copied().find(|p| p.data().homeworld_of.is_some()).expect("a home planet")
}

#[test]
fn the_nano_forge_attaches_to_the_planet_chosen_and_only_that_one() {
    covers!("rules::relic_windows::NanoForge");
    let s = plain_systems();
    let (p0, p1) = (s[1].data().planets[0], s[2].data().planets[0]);
    let sc = || table().control(p0, 0).control(p1, 0);
    let w = || NanoForge::new(Seat(0), &[(s[1], p0), (s[2], p1)]);
    let mut rig = Rig::new(sc(), w());
    rig.run();
    assert_eq!(rig.asked().opts.len(), 2, "no decline: the action is committed");
    rig.pick(1);
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.attached, Some(p1));
    assert_eq!(rig.state.board().planet(p1).attach.as_slice(), &[Attachment::Nanoforge]);
    assert!(rig.state.board().planet(p1).flags.has(PlanetFlags::ATTACH_ENTRY), "the string's `entry` exists");
    assert!(rig.state.board().planet(p0).attach.is_empty(), "the other planet is untouched");
    let mut bad = Rig::new(sc(), w());
    bad.run();
    bad.expect_illegal(Pick::Opt(r(ROpt::Attach(s[3].data().planets[0], Default::default()))));
    bad.expect_illegal(Pick::Opt(r(ROpt::Destroy(p0, Default::default()))));
    let mut none = Rig::new(sc(), NanoForge::new(Seat(0), &[]));
    none.run();
    assert!(none.is_done() && none.driver.window.attached.is_none());
    smoke("nanoforge/window", || Rig::new(sc(), w()));
}

#[test]
fn the_attachment_adds_two_resources_and_two_influence_and_makes_the_planet_legendary() {
    let p = plain_systems()[1].data().planets[0];
    let mut s = table().control(p, 0).state();
    assert_eq!((attachment_bonus(&s, p, Spend::Resources), attachment_bonus(&s, p, Spend::Influence)), (0, 0));
    assert!(!board::is_legendary(&s, p));
    s.board_mut().planet_mut(p).attach.push(Attachment::Nanoforge);
    assert_eq!((attachment_bonus(&s, p, Spend::Resources), attachment_bonus(&s, p, Spend::Influence)), (2, 2));
    assert!(board::is_legendary(&s, p), "legendary now");
    assert!(board::is_legendary(&s, legendary()), "a printed legendary planet still is");
}

#[test]
fn the_targets_are_the_controlled_non_home_non_legendary_planets() {
    let s = plain_systems();
    let (mine, theirs, home, destroyed, attached) = (s[1].data().planets[0], s[2].data().planets[0], s[3].data().planets[0], s[4].data().planets[0], s[5].data().planets[0]);
    let mut st = table().control(mine, 0).control(theirs, 1).control(home, 0).control(destroyed, 0).control(attached, 0).control(legendary(), 0).control(homeworld(), 0).state();
    st.players_mut().edit(0).home_planets.push(home);
    st.board_mut().planet_mut(destroyed).flags.set(PlanetFlags::PURGED, true);
    st.board_mut().planet_mut(attached).attach.push(Attachment::Nanoforge);
    let got: Vec<Planet> = relic::nano_targets(&st, Seat(0)).into_iter().map(|(_, p)| p).collect();
    assert_eq!(got, vec![mine], "not another seat's, not a seated home, not a printed homeworld, not legendary (printed or made so), not destroyed");
    assert_eq!(relic::nano_targets(&st, Seat(1)).into_iter().map(|(_, p)| p).collect::<Vec<_>>(), vec![theirs]);
    assert!(relic::nano_targets(&st, Seat(2)).is_empty());
}

#[test]
fn mecatol_rex_is_a_target_because_the_card_does_not_exclude_it() {
    let st = table().control(Planet::MecatolRex, 0).state();
    assert_eq!(relic::nano_targets(&st, Seat(0)).into_iter().map(|(_, p)| p).collect::<Vec<_>>(), vec![Planet::MecatolRex]);
}

#[test]
fn the_nano_forge_is_an_action_only_for_a_holder_with_a_target() {
    let p = plain_systems()[1].data().planets[0];
    let mut s = table().control(p, 0).state();
    assert!(relic::has_action(Relic::NanoForge));
    assert!(relic::action_relics(&s, Seat(0), true, true).is_empty(), "not held");
    give(&mut s, 0, Relic::NanoForge);
    assert_eq!(relic::action_relics(&s, Seat(0), true, true).as_slice(), &[Relic::NanoForge]);
    assert_eq!(relic::action_relics(&s, Seat(0), false, false).as_slice(), &[Relic::NanoForge], "the Converter's and Device's facts do not matter");
    assert!(relic::action_relics(&s, Seat(1), true, true).is_empty(), "not offered to the other seats");
    s.board_mut().planet_mut(p).control = None;
    assert!(relic::action_relics(&s, Seat(0), true, true).is_empty(), "no planet to attach to");
}
