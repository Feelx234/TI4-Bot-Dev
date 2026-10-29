//! Lockstep tests of the cards ported in package B6d (the remaining 33 action cards, docs/B6_CARDS.md): each card through the
//! announce/resolve window (`reactions::announce`) on `Area::ActionCards` = `Dual` under three deciders. A bail is a failure.
#![cfg(all(test, feature = "typed-core"))]

use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use crate::action_cards::tests::closing_position;
use crate::action_cards_closing_tests::announce_all;

/// Play `alias` as seat `a`'s component action under the decline-slots decider with the area on `Dual`; the position after it.
fn played(alias: &str, start: &GameState) -> GameState {
    played_by(alias, start, Box::new(crate::action_cards_announce_tests::DeclineSlots))
}

/// [`played`] with a decider of the test's choosing (it must decline the reaction slots itself).
fn played_by(alias: &str, start: &GameState, decider: Box<dyn crate::choice::Decider>) -> GameState {
    use crate::action_cards_announce_tests::announce;
    let a = PlayerId::new("a");
    let mut state = start.clone();
    state.player_mut(&a).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new(alias)];
    crate::action_cards::discard(&mut state, &a, 0);
    crate::dual::with_mask(crate::action_cards_closing_tests::dual(), || announce(&mut state, alias, &a, decider));
    state
}

fn units_of(state: &GameState, kind: &str, owner: &str) -> usize {
    state.board.values().map(|b| b.units.iter().filter(|u| u.type_id.as_str() == kind && u.owner.as_str() == owner).count()).sum()
}

fn unit(kind: &str, owner: &str) -> ti4_model::units::Unit {
    ti4_model::units::Unit::new(ti4_model::id::UnitTypeId::new(kind), PlayerId::new(owner))
}

/// Pirate Contract: a neutral destroyer in a touched non-home system that holds no seated player's ship. The neutral
/// owner is not on the board yet, so the owner table is extended for the window.
#[test]
fn pirate_contract_places_a_neutral_destroyer_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let mut state = closing_position();
    // A system with a neutral ship already in it (neutral ships do not block) and two bare ones.
    for id in ["27", "28", "29"] {
        state.system_mut(&ti4_model::id::SystemId::new(id));
    }
    // A homeworld system without a ship is never a pirate system.
    let home = ti4_engine_core::generated::Planet::ALL
        .iter()
        .find_map(|p| p.data().homeworld_of.and(p.data().system))
        .expect("a homeworld");
    state.system_mut(&ti4_model::id::SystemId::new(crate::generated::IdStr::as_str(home)));
    state.system_mut(&ti4_model::id::SystemId::new("27")).units.push(unit("neutral_cruiser", "neutral"));
    let with_neutral = state.clone();
    announce_all(&[("piratecontract1", closing_position()), ("piratecontract3", with_neutral)]);
}

/// Pirate Fleet: three resources by the question-driven payment (the planets are the seat's to name), then a system.
#[test]
fn pirate_fleet_pays_three_resources_then_places_the_neutral_fleet_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let mut state = closing_position();
    for id in ["27", "28"] {
        state.system_mut(&ti4_model::id::SystemId::new(id));
    }
    // Poor in trade goods so the planets pay (a real choice between faces), and a second case with plenty.
    let a = PlayerId::new("a");
    let mut poor = state.clone();
    poor.player_mut(&a).unwrap().trade_goods = 0;
    let mut rich = state.clone();
    rich.player_mut(&a).unwrap().trade_goods = 6;
    // Too poor to pay: nothing is placed (the card is played and the payment is refused without a question).
    let mut broke = state;
    broke.player_mut(&a).unwrap().trade_goods = 0;
    for board in broke.board.values_mut() {
        board.planet_control.retain(|_, owner| *owner != a);
    }
    announce_all(&[("piratefleet", poor.clone()), ("piratefleet", rich.clone()), ("piratefleet", broke.clone())]);
    // The fleet came for the two that paid and not for the one that could not.
    let _ = crate::dual::take_report();
    for (state, fleet) in [(&poor, true), (&rich, true), (&broke, false)] {
        let after = played("piratefleet", state);
        assert_eq!(units_of(&after, "neutral_carrier", "neutral"), usize::from(fleet), "{fleet}");
        assert_eq!(units_of(&after, "neutral_fighter", "neutral"), if fleet { 2 } else { 0 });
    }
    let rich_after = played("piratefleet", &rich);
    assert_eq!(rich_after.player(&a).unwrap().trade_goods, 3, "three resources paid from six trade goods");
    let _ = crate::dual::take_report();
}

/// Mercenary Contract: two trade goods, two neutral infantry on a bare non-home planet (a planet with a units entry and no
/// units; a planet with no entry is not a candidate).
#[test]
fn mercenary_contract_lands_neutral_infantry_on_a_bare_planet_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    use ti4_engine_core::generated::{Planet, Source};
    let a = PlayerId::new("a");
    let mut state = closing_position();
    // Three bare planets of the base game (not homes), one more without a units entry.
    let bare: Vec<Planet> = Planet::ALL
        .iter()
        .copied()
        .filter(|p| {
            let d = p.data();
            d.system.is_some() && d.homeworld_of.is_none() && d.source == Source::Base
        })
        .filter(|p| {
            let sys = ti4_model::id::SystemId::new(crate::generated::IdStr::as_str(p.data().system.unwrap()));
            state.board.get(&sys).is_none_or(|b| !b.planet_units.contains_key(ti4_model::id::text(crate::generated::IdStr::as_str(*p))))
        })
        .take(4)
        .collect();
    assert_eq!(bare.len(), 4);
    for p in &bare[..3] {
        let sys = ti4_model::id::SystemId::new(crate::generated::IdStr::as_str(p.data().system.unwrap()));
        state.system_mut(&sys).planet_units.insert(ti4_model::id::PlanetId::new(crate::generated::IdStr::as_str(*p)), vec![]);
    }
    state.player_mut(&a).unwrap().trade_goods = 5;
    let mut poor = state.clone();
    poor.player_mut(&a).unwrap().trade_goods = 1;
    announce_all(&[("mercenarycontract", state.clone()), ("mercenarycontract", poor.clone())]);
    let _ = crate::dual::take_report();
    let after = played("mercenarycontract", &state);
    let on_planets: usize = after.board.values().flat_map(|b| b.planet_units.values()).flatten().filter(|u| u.type_id.as_str() == "neutral_infantry").count();
    assert_eq!(on_planets, 2);
    assert_eq!(after.player(&a).unwrap().trade_goods, 3);
    let after = played("mercenarycontract", &poor);
    assert_eq!(after.player(&a).unwrap().trade_goods, 1, "one trade good buys nothing");
    let _ = crate::dual::take_report();
}

/// A map of one ring: Mecatol Rex and six systems, four of which carry a wormhole (alpha, alpha, beta, alpha) and one is a
/// homeworld that carries one too (the Creuss home, "51").
fn ring() -> ti4_content::galaxy::Galaxy {
    ti4_content::galaxy::Galaxy::build(ti4_content::ContentStore::embedded(), &["18", "25", "26", "39", "40", "67", "51"], ti4_model::content_types::POK, 1)
        .expect("one ring holds the tiles")
}

/// Ghost Ship: the wormhole systems of the map without another player's ship, in the order of the map's hexes.
#[test]
fn ghost_ship_names_the_wormhole_systems_of_the_map_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let galaxy = ring();
    let a = PlayerId::new("a");
    let mut state = closing_position();
    // A rival's cruiser in one wormhole system closes it; the holder's own ship does not.
    state.system_mut(&ti4_model::id::SystemId::new("26")).units.push(unit("cruiser", "b"));
    state.system_mut(&ti4_model::id::SystemId::new("39")).units.push(unit("cruiser", "a"));
    state.system_mut(&ti4_model::id::SystemId::new("40")).units.push(unit("neutral_cruiser", "neutral"));
    crate::action_cards_closing_tests::announce_all_on(&[("ghost_ship", state.clone())], Some(&galaxy));
    let _ = crate::dual::take_report();
    let mut game = state.clone();
    game.player_mut(&a).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new("ghost_ship")];
    crate::action_cards::discard(&mut game, &a, 0);
    crate::dual::with_mask(crate::action_cards_closing_tests::dual(), || {
        crate::action_cards_announce_tests::announce_on(&mut game, "ghost_ship", &a, Box::new(crate::action_cards_announce_tests::DeclineSlots), Some(&galaxy))
    });
    assert_eq!(units_of(&game, "destroyer", "a") - units_of(&state, "destroyer", "a"), 1);
    let _ = crate::dual::take_report();
}

/// Bail: Ghost Ship played without a map (the string names no system; the core cannot read the galaxy it was not lent).
#[test]
fn bail_ghost_ship_without_a_map_is_counted() {
    let _serial = crate::dual::tests::serial();
    let a = PlayerId::new("a");
    let mut state = closing_position();
    state.player_mut(&a).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new("ghost_ship")];
    crate::action_cards::discard(&mut state, &a, 0);
    let _ = crate::dual::take_report();
    crate::dual::with_mask(crate::action_cards_closing_tests::dual(), || {
        crate::action_cards_announce_tests::announce(&mut state, "ghost_ship", &a, Box::new(crate::action_cards_announce_tests::DeclineSlots))
    });
    assert_eq!(crate::action_cards_closing_tests::windows("action_cards::announce"), (0, 1));
    let _ = crate::dual::take_report();
}

/// Signal Jamming: a non-home system on the map that holds one of the holder's ships or touches one, then the rival (named by
/// faction) whose command token goes there.
#[test]
fn signal_jamming_closes_a_system_near_the_holders_ships_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let galaxy = ring();
    let a = PlayerId::new("a");
    let mut state = closing_position();
    state.system_mut(&ti4_model::id::SystemId::new("25")).units.push(unit("cruiser", "a"));
    state.system_mut(&ti4_model::id::SystemId::new("40")).units.push(unit("carrier", "a"));
    // A ship at home counts for the adjacency, the home system itself is never offered.
    state.system_mut(&ti4_model::id::SystemId::new("51")).units.push(unit("cruiser", "a"));
    // Off the map and the holder's: never offered.
    let mut two = state.clone();
    two.seating_order.retain(|p| p.as_str() != "c");
    two.players.retain(|p| p.id.as_str() != "c");
    crate::action_cards_closing_tests::announce_all_on(&[("jamming", state.clone()), ("jamming", two)], Some(&galaxy));
    let _ = crate::dual::take_report();
    let mut game = state.clone();
    game.player_mut(&a).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new("jamming")];
    crate::action_cards::discard(&mut game, &a, 0);
    crate::dual::with_mask(crate::action_cards_closing_tests::dual(), || {
        crate::action_cards_announce_tests::announce_on(&mut game, "jamming", &a, Box::new(crate::action_cards_announce_tests::DeclineSlots), Some(&galaxy))
    });
    let tokens = |s: &GameState| s.board.values().map(|b| b.command_tokens.len()).sum::<usize>();
    assert_eq!(tokens(&game), tokens(&state) + 1, "one rival token went into a system");
    let _ = crate::dual::take_report();
}

/// Upgrade: one of the holder's cruisers in the active system becomes a dreadnought (a question when there are several; the
/// dreadnoughts left in the box limit it).
#[test]
fn upgrade_replaces_a_cruiser_of_the_active_system_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let a = PlayerId::new("a");
    let state = closing_position();
    let active = state.active_system.clone().expect("an active system");
    let mut two = state.clone();
    two.system_mut(&active).units.push(unit("cruiser", "a"));
    two.system_mut(&active).units.push(unit("cruiser", "a"));
    let mut one = state.clone();
    one.system_mut(&active).units.retain(|u| u.type_id.as_str() != "cruiser" || u.owner.as_str() != "a");
    one.system_mut(&active).units.push(unit("cruiser", "a"));
    // No dreadnought left in the box: nothing happens.
    let mut empty_box = two.clone();
    for _ in 0..5 {
        empty_box.system_mut(&ti4_model::id::SystemId::new("27")).units.push(unit("dreadnought", "a"));
    }
    crate::action_cards_closing_tests::announce_all(&[("upgrade", two.clone()), ("upgrade", one.clone()), ("upgrade", empty_box.clone())]);
    let _ = crate::dual::take_report();
    let after = played("upgrade", &two);
    assert_eq!(units_of(&after, "dreadnought", "a"), units_of(&two, "dreadnought", "a") + 1);
    assert_eq!(units_of(&after, "cruiser", "a"), units_of(&two, "cruiser", "a") - 1);
    let after = played("upgrade", &empty_box);
    assert_eq!(units_of(&after, "dreadnought", "a"), units_of(&empty_box, "dreadnought", "a"), "the box holds no more");
    let _ = crate::dual::take_report();
}

/// Refit Troops: one or two of the holder's infantry become mechs (the picks come in two questions; the second offers to
/// stop), the removals of one system from the highest place down.
#[test]
fn refit_troops_replaces_infantry_with_mechs_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let a = PlayerId::new("a");
    let state = closing_position();
    assert!(units_on_planets(&state, "infantry", "a") >= 3);
    // Exactly one infantry of the holder's.
    let mut single = state.clone();
    let mut kept = false;
    for board in single.board.values_mut() {
        for units in board.planet_units.values_mut() {
            units.retain(|u| {
                if u.owner != a || u.type_id.as_str() != "infantry" {
                    return true;
                }
                let keep = !kept;
                kept = true;
                keep
            });
        }
    }
    // No mech left in the box.
    let mut empty_box = state.clone();
    for _ in 0..4 {
        empty_box.system_mut(&ti4_model::id::SystemId::new("27")).units.push(unit("mech", "a"));
    }
    crate::action_cards_closing_tests::announce_all(&[("refit", state.clone()), ("refit", single.clone()), ("refit", empty_box.clone())]);
    let _ = crate::dual::take_report();
    // The last option of the second question is the stop: one infantry; the first options take two.
    let after = played("refit", &state);
    assert_eq!(units_on_planets(&after, "mech", "a"), units_on_planets(&state, "mech", "a") + 1);
    let after = played_by("refit", &state, Box::new(crate::action_cards_closing_tests::FirstOption));
    assert_eq!(units_on_planets(&after, "mech", "a"), units_on_planets(&state, "mech", "a") + 2);
    assert_eq!(units_on_planets(&after, "infantry", "a"), units_on_planets(&state, "infantry", "a") - 2);
    let after = played("refit", &single);
    assert_eq!(units_on_planets(&after, "mech", "a"), 1);
    let after = played("refit", &empty_box);
    assert_eq!(units_on_planets(&after, "mech", "a"), 0, "the box holds no more mechs");
    let _ = crate::dual::take_report();
}

fn units_on_planets(state: &GameState, kind: &str, owner: &str) -> usize {
    state.board.values().flat_map(|b| b.planet_units.values()).flatten().filter(|u| u.type_id.as_str() == kind && u.owner.as_str() == owner).count()
}

/// Decoy Operation: up to two ground forces from anywhere leave the board for a planet of the holder in the active system
/// (two questions for the forces, then the planet; nothing is asked of two forces or fewer, and nothing is removed without a
/// planet to land on).
#[test]
fn decoy_operation_pulls_ground_forces_to_a_planet_of_the_active_system_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let a = PlayerId::new("a");
    let mut state = closing_position();
    let active = state.active_system.clone().expect("an active system");
    // The holder controls every planet of the active system; its first planet also holds some of the holder's forces.
    let planets: Vec<_> = state.system_mut(&active).planet_control.keys().cloned().collect();
    assert!(!planets.is_empty());
    for p in &planets {
        state.system_mut(&active).planet_control.insert(p.clone(), a.clone());
    }
    state.system_mut(&active).planet_units.entry(planets[0].clone()).or_default().push(unit("infantry", "a"));
    state.system_mut(&active).planet_units.entry(planets[0].clone()).or_default().push(unit("mech", "a"));
    let nowhere = closing_position();
    let mut two = state.clone();
    for board in two.board.values_mut() {
        for units in board.planet_units.values_mut() {
            units.retain(|u| u.owner != a);
        }
    }
    two.system_mut(&active).planet_units.entry(planets[0].clone()).or_default().push(unit("infantry", "a"));
    let elsewhere = ti4_engine_core::generated::Planet::ALL
        .iter()
        .find(|p| p.data().system.is_some_and(|s| crate::generated::IdStr::as_str(s) != active.as_str()) && p.data().homeworld_of.is_none())
        .expect("a planet elsewhere");
    let sys = ti4_model::id::SystemId::new(crate::generated::IdStr::as_str(elsewhere.data().system.unwrap()));
    two.system_mut(&sys).planet_units.entry(ti4_model::id::PlanetId::new(crate::generated::IdStr::as_str(*elsewhere))).or_default().push(unit("infantry", "a"));
    // A list with units behind the pulled one (the freed place is taken by the last unit), and a space station as the only
    // planet of the active system (it takes no ground forces).
    let mut crowded = state.clone();
    for board in crowded.board.values_mut() {
        for units in board.planet_units.values_mut() {
            units.retain(|u| u.owner != a);
        }
    }
    let sys = ti4_model::id::SystemId::new(crate::generated::IdStr::as_str(elsewhere.data().system.unwrap()));
    let list = crowded.system_mut(&sys).planet_units.entry(ti4_model::id::PlanetId::new(crate::generated::IdStr::as_str(*elsewhere))).or_default();
    list.extend([unit("infantry", "a"), unit("infantry", "b"), unit("infantry", "c"), unit("infantry", "b"), unit("infantry", "a")]);
    crowded.system_mut(&active).planet_units.entry(planets[0].clone()).or_default().push(unit("infantry", "a"));
    let station = ti4_engine_core::generated::Planet::ALL
        .iter()
        .find(|p| {
            p.data().traits.contains(&ti4_engine_core::generated::PlanetTrait::Spacestation)
                && p.data().system.is_some()
        })
        .expect("a space station");
    let station_sys = ti4_model::id::SystemId::new(crate::generated::IdStr::as_str(station.data().system.unwrap()));
    let mut at_station = state.clone();
    at_station.system_mut(&station_sys).planet_control.insert(ti4_model::id::PlanetId::new(crate::generated::IdStr::as_str(*station)), a.clone());
    at_station.active_system = Some(station_sys);
    for case in [&state, &two, &nowhere, &crowded] {
        announce_all(&[("decoy", case.clone())]);
    }
    // The stations are Thunder's Edge's: only with that source in play is the station one (a planet outside the sources is not
    // looked up at all).
    crate::action_cards_announce_tests::with_sources(ti4_model::content_types::FULL, || announce_all(&[("decoy", at_station.clone())]));
    announce_all(&[("decoy", at_station.clone())]);
    let _ = crate::dual::take_report();
    let before = units_on_planets(&state, "infantry", "a") + units_on_planets(&state, "mech", "a");
    let after = played("decoy", &state);
    assert_eq!(units_on_planets(&after, "infantry", "a") + units_on_planets(&after, "mech", "a"), before, "the forces moved, none was lost");
    assert_ne!(
        serde_json::to_string(&after.board).unwrap(),
        serde_json::to_string(&state.board).unwrap(),
        "the forces moved"
    );
    let untouched = played("decoy", &nowhere);
    assert_eq!(serde_json::to_string(&untouched.board).unwrap(), serde_json::to_string(&nowhere.board).unwrap(), "nowhere to land: nothing is removed");
    let _ = crate::dual::take_report();
}

/// A position where seat `a` holds two planets of the active system and ground forces on one of them (infantry and mechs).
fn squad_position() -> GameState {
    use ti4_engine_core::generated::Planet;
    let a = PlayerId::new("a");
    let mut state = closing_position();
    let (system, planets): (ti4_engine_core::generated::System, Vec<Planet>) = Planet::ALL
        .iter()
        .filter_map(|p| p.data().system.map(|s| (s, *p)))
        .fold(std::collections::BTreeMap::<_, Vec<Planet>>::new(), |mut m, (s, p)| {
            m.entry(s).or_default().push(p);
            m
        })
        .into_iter()
        .find(|(s, ps)| ps.len() >= 2 && !ps.iter().any(|p| p.data().homeworld_of.is_some()) && state.board.get(&ti4_model::id::SystemId::new(crate::generated::IdStr::as_str(*s))).is_none())
        .expect("a system with two planets");
    let sys = ti4_model::id::SystemId::new(crate::generated::IdStr::as_str(system));
    for p in &planets {
        let pid = ti4_model::id::PlanetId::new(crate::generated::IdStr::as_str(*p));
        state.system_mut(&sys).planet_control.insert(pid.clone(), a.clone());
        state.system_mut(&sys).planet_units.insert(pid, vec![]);
    }
    let first = ti4_model::id::PlanetId::new(crate::generated::IdStr::as_str(planets[0]));
    let units = state.system_mut(&sys).planet_units.entry(first).or_default();
    units.push(unit("infantry", "a"));
    units.push(unit("mech", "a"));
    units.push(unit("infantry", "a"));
    state.active_system = Some(sys);
    state
}

/// Ghost Squad: a move per question (every ground force of one type from one planet to another), a decline in each, a planet that
/// received forces does not ship out again.
#[test]
fn ghost_squad_moves_ground_forces_between_planets_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let state = squad_position();
    // Nothing to move: the card does nothing (no question).
    let mut bare = state.clone();
    for board in bare.board.values_mut() {
        for units in board.planet_units.values_mut() {
            units.retain(|u| u.owner.as_str() != "a");
        }
    }
    announce_all(&[("ghost_squad", state.clone()), ("ghost_squad", bare.clone())]);
    let _ = crate::dual::take_report();
    let before = serde_json::to_string(&state.board).unwrap();
    let declined = played("ghost_squad", &state);
    assert!(before == serde_json::to_string(&declined.board).unwrap(), "the last option is the decline: nothing moved");
    let after = played_by("ghost_squad", &state, Box::new(crate::action_cards_closing_tests::LastGo));
    assert!(before != serde_json::to_string(&after.board).unwrap(), "a move was made");
    assert_eq!(
        units_on_planets(&after, "infantry", "a") + units_on_planets(&after, "mech", "a"),
        units_on_planets(&state, "infantry", "a") + units_on_planets(&state, "mech", "a"),
        "ground forces only change planets"
    );
    // Two moves on first options: infantry, then the mech (the first planet shipped out, nothing is left to move back).
    let after = played_by("ghost_squad", &state, Box::new(crate::action_cards_closing_tests::FirstOption));
    assert_eq!(
        units_on_planets(&after, "infantry", "a") + units_on_planets(&after, "mech", "a"),
        units_on_planets(&state, "infantry", "a") + units_on_planets(&state, "mech", "a")
    );
    assert!(serde_json::to_string(&after.board).unwrap() != before);
    let _ = crate::dual::take_report();
}

/// Harness Energy: the holder's commodities go to the faction's printed value, nothing else.
#[test]
fn harness_energy_replenishes_the_holders_commodities_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let a = PlayerId::new("a");
    let mut low = closing_position();
    low.player_mut(&a).unwrap().commodities = 0;
    let mut full = closing_position();
    full.player_mut(&a).unwrap().commodities = 9;
    announce_all(&[("harness", low.clone()), ("harness", full.clone())]);
    let _ = crate::dual::take_report();
    assert_eq!(played("harness", &low).player(&a).unwrap().commodities, 3, "the Arborec print 3");
    assert_eq!(played("harness", &full).player(&a).unwrap().commodities, 3, "a seat above the print is set down to it");
    // A Cabal seat: the replenish note is the string's; the card is played by the string alone.
    let mut cabal = low.clone();
    cabal.player_mut(&PlayerId::new("c")).unwrap().faction = ti4_model::id::FactionId::new("cabal");
    let _ = crate::dual::take_report();
    let _ = played("harness", &cabal);
    assert_eq!(crate::action_cards_closing_tests::windows("action_cards::announce"), (0, 1));
    let _ = crate::dual::take_report();
}

/// Brilliance: a seat without a breakthrough gains its own; the roll for the Fracture is the Fracture stream's (positions
/// compared). The seats' breakthroughs are the Thunder's Edge ones of their factions.
#[test]
fn brilliance_gives_a_seat_its_own_breakthrough_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let mut state = closing_position();
    state.rules_version = ti4_model::state::RULES_VERSION;
    let _ = crate::dual::take_report();
    crate::action_cards_closing_tests::announce_all(&[("brilliance", state.clone())]);
    let _ = crate::dual::take_report();
    let after = played("brilliance", &state);
    let held = |s: &GameState| s.players.iter().filter(|p| p.breakthrough.is_some()).count();
    assert!(held(&after) > held(&state), "a seat gained its breakthrough");
    // Everybody has one already: no question, nothing happens.
    let mut all = state.clone();
    for p in &mut all.players {
        p.breakthrough = Some(ti4_model::id::BreakthroughId::new("cabalbt"));
    }
    announce_all(&[("brilliance", all)]);
    let _ = crate::dual::take_report();
}

/// Impersonation: three influence by the question-driven payment, a secret objective is drawn and, over the hand limit, one is
/// handed back (a question of the objectives area).
#[test]
fn impersonation_pays_draws_a_secret_and_enforces_the_hand_limit_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let a = PlayerId::new("a");
    let mut state = closing_position();
    let mut deck = state.secret_deck.clone();
    assert!(deck.len() >= 8, "the deck holds secrets");
    let held: Vec<_> = deck.drain(..3).collect();
    state.secret_deck = deck;
    state.player_mut(&a).unwrap().secret_objectives = held.clone();
    state.player_mut(&a).unwrap().trade_goods = 0;
    state.exhausted_planets.clear();
    // The hand is within the limit after the draw.
    let mut within = state.clone();
    within.player_mut(&a).unwrap().secret_objectives = held[..1].to_vec();
    within.player_mut(&a).unwrap().trade_goods = 5;
    // Plenty of trade goods pay the influence without a question.
    let mut rich = state.clone();
    rich.player_mut(&a).unwrap().trade_goods = 5;
    // The deck is empty: the payment is made, nothing is drawn.
    let mut dry = state.clone();
    dry.secret_deck.clear();
    // Cannot pay.
    let mut poor = state.clone();
    poor.player_mut(&a).unwrap().trade_goods = 0;
    for board in poor.board.values_mut() {
        board.planet_control.retain(|_, owner| *owner != a);
    }
    for case in [&state, &within, &rich, &dry, &poor] {
        announce_all(&[("impersonation", case.clone())]);
    }
    let _ = crate::dual::take_report();
    let after = played("impersonation", &within);
    assert_eq!(after.player(&a).unwrap().secret_objectives.len(), 2, "one secret drawn");
    let after = played("impersonation", &rich);
    assert_eq!(after.player(&a).unwrap().trade_goods, 2, "three influence paid");
    assert_eq!(after.player(&a).unwrap().secret_objectives.len(), 3, "over the limit: drawn then handed back");
    let after = played("impersonation", &poor);
    assert_eq!(after.player(&a).unwrap().secret_objectives.len(), 3, "not paid: nothing drawn");
    let _ = crate::dual::take_report();
}

/// Plagiarize: five influence, then a non-faction technology of a neighbour (not one the holder owns, below rules version 28
/// any) is taken from it; a unit upgrade reaches the board.
#[test]
fn plagiarize_pays_five_influence_and_takes_a_neighbours_technology_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let (a, b, c) = (PlayerId::new("a"), PlayerId::new("b"), PlayerId::new("c"));
    let mut state = closing_position();
    state.exhausted_planets.clear();
    state.player_mut(&a).unwrap().trade_goods = 6;
    let tech = |s: &str| ti4_model::TechnologyId::new(s);
    state.player_mut(&b).unwrap().technologies = ["nm", "gd", "amd", "cr2", "lw2"].iter().map(|t| tech(t)).collect();
    state.player_mut(&c).unwrap().technologies = ["ps", "dxa", "hm"].iter().map(|t| tech(t)).collect();
    state.player_mut(&a).unwrap().technologies = ["gd"].iter().map(|t| tech(t)).collect();
    // The holder's cruisers become cruiser IIs when the upgrade is taken.
    let mut old = state.clone();
    old.rules_version = 27;
    let mut new = state.clone();
    new.rules_version = ti4_model::state::RULES_VERSION;
    // Nothing to take: the five influence are not spent (rules version 23 and later).
    let mut bare = state.clone();
    bare.player_mut(&b).unwrap().technologies.clear();
    bare.player_mut(&c).unwrap().technologies.clear();
    let mut poor = state.clone();
    poor.player_mut(&a).unwrap().trade_goods = 0;
    for board in poor.board.values_mut() {
        board.planet_control.retain(|_, owner| *owner != a);
    }
    for case in [&old, &new, &bare, &poor] {
        announce_all(&[("plagiarize", case.clone())]);
    }
    let _ = crate::dual::take_report();
    let after = played("plagiarize", &new);
    assert_eq!(after.player(&a).unwrap().technologies.len(), 2, "one technology taken");
    assert_eq!(after.player(&a).unwrap().trade_goods, 1, "five influence paid");
    let after = played("plagiarize", &bare);
    assert_eq!(after.player(&a).unwrap().trade_goods, 6, "nothing to take: nothing spent");
    let _ = crate::dual::take_report();
}
