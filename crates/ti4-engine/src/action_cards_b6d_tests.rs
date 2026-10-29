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
    // Structures are not ground forces.
    units.push(unit("spacedock", "a"));
    units.push(unit("pds", "a"));
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

/// A position where seat `a` owns the base technologies of every colour (so a good many are researchable) and is ready to pay.
fn research_position() -> GameState {
    let a = PlayerId::new("a");
    let mut state = closing_position();
    state.exhausted_planets.clear();
    state.rules_version = ti4_model::state::RULES_VERSION;
    let seat = state.player_mut(&a).unwrap();
    seat.trade_goods = 6;
    seat.technologies = ["amd", "gd", "fl", "lwd", "nm", "dxa", "hm", "x89_base", "ps", "md_base", "da", "asc", "st", "gls", "td", "ie", "ws"]
        .iter()
        .map(|t| ti4_model::TechnologyId::new(*t))
        .collect();
    state
}

/// Focused Research: four trade goods, one technology researched (a lone technology is taken without a question).
#[test]
fn focused_research_spends_four_trade_goods_and_researches_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let a = PlayerId::new("a");
    let state = research_position();
    let mut poor = state.clone();
    poor.player_mut(&a).unwrap().trade_goods = 3;
    announce_all(&[("f_researched", state.clone())]);
    announce_all(&[("f_researched", poor.clone())]);
    let _ = crate::dual::take_report();
    let after = played("f_researched", &state);
    assert_eq!(after.player(&a).unwrap().trade_goods, 2);
    assert_eq!(after.player(&a).unwrap().technologies.len(), state.player(&a).unwrap().technologies.len() + 1);
    let after = played("f_researched", &poor);
    assert_eq!(after.player(&a).unwrap().trade_goods, 3, "three trade goods buy nothing");
    let _ = crate::dual::take_report();
}

/// Divert Funding: a non-upgrade, non-faction technology goes back, then one is researched.
#[test]
fn divert_funding_returns_a_technology_and_researches_another_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let a = PlayerId::new("a");
    let state = research_position();
    let mut none = state.clone();
    none.player_mut(&a).unwrap().technologies.clear();
    announce_all(&[("divert_funding", state.clone())]);
    announce_all(&[("divert_funding", none)]);
    let _ = crate::dual::take_report();
    let after = played("divert_funding", &state);
    assert_eq!(after.player(&a).unwrap().technologies.len(), state.player(&a).unwrap().technologies.len(), "one back, one researched");
    let _ = crate::dual::take_report();
}

/// Reveal Prototype: a unit upgrade of the line of a unit in the combat's system is researched for four resources.
#[test]
fn reveal_prototype_researches_a_unit_upgrade_of_a_unit_in_the_combat_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let a = PlayerId::new("a");
    let mut state = research_position();
    state.player_mut(&a).unwrap().trade_goods = 0;
    let active = state.active_system.clone().expect("an active system");
    state.system_mut(&active).units.push(unit("cruiser", "a"));
    state.system_mut(&active).units.push(unit("carrier", "a"));
    let mut bare = state.clone();
    bare.system_mut(&active).units.retain(|u| u.owner.as_str() != "a");
    announce_all(&[("reveal_prototype", state.clone())]);
    announce_all(&[("reveal_prototype", bare)]);
    let _ = crate::dual::take_report();
    let after = played("reveal_prototype", &state);
    assert!(after.player(&a).unwrap().technologies.len() > state.player(&a).unwrap().technologies.len(), "a prototype was researched");
    let _ = crate::dual::take_report();
}

/// Lie in Wait: one action card from each of the first two neighbours (by the map) who traded this round; a neighbour without a
/// card is skipped.
#[test]
fn lie_in_wait_takes_a_card_from_each_neighbour_who_traded_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let galaxy = ring();
    let (a, b, c) = (PlayerId::new("a"), PlayerId::new("b"), PlayerId::new("c"));
    let mut state = closing_position();
    // Ships side by side on the ring make the three neighbours of one another.
    state.board.clear();
    for (system, owner) in [("25", "a"), ("26", "b"), ("51", "c")] {
        state.system_mut(&ti4_model::id::SystemId::new(system)).units.push(unit("cruiser", owner));
    }
    state.transactions_this_round = vec![(b.clone(), c.clone()), (c.clone(), b.clone())];
    let hand = |aliases: &[&str]| aliases.iter().map(|x| ti4_model::id::ActionCardId::new(*x)).collect::<Vec<_>>();
    state.player_mut(&b).unwrap().action_cards = hand(&["sabo1", "insub", "uprising"]);
    state.player_mut(&c).unwrap().action_cards = hand(&["rout", "waylay"]);
    let mut empty_hand = state.clone();
    empty_hand.player_mut(&b).unwrap().action_cards.clear();
    let mut none = state.clone();
    none.transactions_this_round.clear();
    crate::action_cards_closing_tests::announce_all_on(&[("lieinwait", state.clone()), ("lieinwait", empty_hand.clone()), ("lieinwait", none.clone())], Some(&galaxy));
    let _ = crate::dual::take_report();
    let mut game = state.clone();
    game.player_mut(&a).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new("lieinwait")];
    crate::action_cards::discard(&mut game, &a, 0);
    crate::dual::with_mask(crate::action_cards_closing_tests::dual(), || {
        crate::action_cards_announce_tests::announce_on(&mut game, "lieinwait", &a, Box::new(crate::action_cards_announce_tests::DeclineSlots), Some(&galaxy))
    });
    assert_eq!(game.player(&a).unwrap().action_cards.len(), 2, "one card from each of two neighbours");
    assert_eq!(game.player(&b).unwrap().action_cards.len(), 2);
    let _ = crate::dual::take_report();
}

/// Bribery: any number of trade goods are spent (a count question), one extra vote each on this agenda.
#[test]
fn bribery_spends_trade_goods_for_votes_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let a = PlayerId::new("a");
    let mut state = closing_position();
    state.agenda_seq = 3;
    state.player_mut(&a).unwrap().trade_goods = 4;
    let mut running = state.clone();
    running.player_mut(&a).unwrap().extra_votes_agenda = Some((3, 2));
    let mut stale = state.clone();
    stale.player_mut(&a).unwrap().extra_votes_agenda = Some((2, 9));
    let mut broke = state.clone();
    broke.player_mut(&a).unwrap().trade_goods = 0;
    for case in [&state, &running, &stale, &broke] {
        announce_all(&[("bribery", case.clone())]);
    }
    let _ = crate::dual::take_report();
    // The last option is the largest count; the first spends nothing.
    assert_eq!(played("bribery", &state).player(&a).unwrap().extra_votes_agenda, Some((3, 4)));
    assert_eq!(played("bribery", &running).player(&a).unwrap().extra_votes_agenda, Some((3, 6)));
    assert_eq!(played("bribery", &stale).player(&a).unwrap().extra_votes_agenda, Some((3, 4)));
    assert_eq!(played("bribery", &state).player(&a).unwrap().trade_goods, 0);
    let first = played_by("bribery", &state, Box::new(crate::action_cards_closing_tests::FirstOption));
    assert_eq!(first.player(&a).unwrap().trade_goods, 4, "zero spent");
    assert_eq!(played("bribery", &broke).player(&a).unwrap().extra_votes_agenda, None);
    let _ = crate::dual::take_report();
}

/// Infiltrate: each PDS and space dock of another seat on the planet just taken is replaced by the holder's own, while the box
/// holds that kind.
#[test]
fn infiltrate_replaces_the_structures_of_the_planet_just_taken_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
    let mut state = closing_position();
    state.rules_version = ti4_model::state::RULES_VERSION;
    let planet = ti4_model::id::PlanetId::new("corneeq");
    state.system_mut(&ti4_model::id::SystemId::new("33")).planet_units.insert(
        planet.clone(),
        vec![unit("pds", "c"), unit("spacedock", "b"), unit("infantry", "b"), unit("pds", "b"), unit("spacedock", "c"), unit("pds", "a")],
    );
    state.last_control_gained = Some((ti4_model::id::SystemId::new("33"), planet, a.clone(), Some(b.clone())));
    // The box holds only one more PDS: the second swap is not made.
    let mut thin = state.clone();
    thin.system_mut(&ti4_model::id::SystemId::new("33")).planet_units.get_mut("corneeq").unwrap().extend((0..4).map(|_| unit("pds", "a")));
    let mut older = state.clone();
    older.rules_version = 0;
    announce_all(&[("infiltrate", state.clone())]);
    announce_all(&[("infiltrate", thin.clone())]);
    let _ = crate::dual::take_report();
    // Below rules version 1 the string plays the card as it did then: the core declines (a counted bail).
    let mut legacy = older.clone();
    legacy.player_mut(&a).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new("infiltrate")];
    crate::action_cards::discard(&mut legacy, &a, 0);
    crate::dual::with_mask(crate::action_cards_closing_tests::dual(), || {
        crate::action_cards_announce_tests::announce(&mut legacy, "infiltrate", &a, Box::new(crate::action_cards_announce_tests::DeclineSlots))
    });
    assert_eq!(crate::action_cards_closing_tests::windows("action_cards::announce"), (0, 1));
    let _ = crate::dual::take_report();
    let after = played("infiltrate", &state);
    let on = |s: &GameState, kind: &str, owner: &str| {
        s.board.get(&ti4_model::id::SystemId::new("33")).unwrap().planet_units["corneeq"].iter().filter(|u| u.type_id.as_str() == kind && u.owner.as_str() == owner).count()
    };
    assert_eq!(on(&after, "pds", "a"), 3, "two PDS replaced beside the holder's own");
    assert_eq!(on(&after, "spacedock", "a"), 2);
    assert_eq!(on(&after, "infantry", "b"), 1, "the infantry stays");
    let thin_after = played("infiltrate", &thin);
    assert_eq!(on(&thin_after, "pds", "a"), 6, "one PDS was left in the box: one swap");
    // Below rules version 1 the string plays another card: the core declines.
    let _ = crate::dual::take_report();
}

/// Crash Landing: after the holder's last ship in a system is destroyed, a ground force from the space area lands on a planet
/// there (not Mecatol Rex); on a planet with other players' units it coexists.
#[test]
fn crash_landing_lands_a_ground_force_on_a_planet_of_the_system_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
    let mut state = closing_position();
    // A system with two planets (b's units on one), the holder's infantry and mech waiting in space.
    let (system, found) = ti4_engine_core::generated::Planet::ALL
        .iter()
        .filter(|p| p.data().source == ti4_engine_core::generated::Source::Base || p.data().source == ti4_engine_core::generated::Source::Pok)
        .filter_map(|p| p.data().system.map(|s| (s, *p)))
        .fold(std::collections::BTreeMap::<_, Vec<_>>::new(), |mut m, (s, p)| {
            m.entry(s).or_default().push(p);
            m
        })
        .into_iter()
        .find(|(s, ps)| ps.len() >= 2 && !ps.iter().any(|p| p.data().homeworld_of.is_some() || matches!(p, ti4_engine_core::generated::Planet::MecatolRex)) && state.board.get(&ti4_model::id::SystemId::new(crate::generated::IdStr::as_str(*s))).is_none())
        .expect("a system with two planets");
    let sys = ti4_model::id::SystemId::new(crate::generated::IdStr::as_str(system));
    let planets: Vec<String> = found.iter().map(|p| crate::generated::IdStr::as_str(*p).to_owned()).collect();
    state.system_mut(&sys).planet_units.insert(ti4_model::id::PlanetId::new(planets[0].clone()), vec![unit("infantry", "b")]);
    state.system_mut(&sys).planet_units.insert(ti4_model::id::PlanetId::new(planets[1].clone()), vec![]);
    state.system_mut(&sys).units = vec![unit("infantry", "a"), unit("mech", "a"), unit("infantry", "a")];
    state.last_ship_destroyed = Some((sys.clone(), a.clone(), ti4_model::id::UnitTypeId::new("cruiser")));
    let mut one_kind = state.clone();
    one_kind.system_mut(&sys).units = vec![unit("infantry", "a"), unit("infantry", "a")];
    let mut theirs = state.clone();
    theirs.last_ship_destroyed = Some((sys.clone(), b.clone(), ti4_model::id::UnitTypeId::new("cruiser")));
    let mut nothing = state.clone();
    nothing.system_mut(&sys).units.clear();
    for case in [&state, &one_kind, &theirs, &nothing] {
        announce_all(&[("crashlanding", case.clone())]);
    }
    let _ = crate::dual::take_report();
    let after = played("crashlanding", &state);
    let landed: usize = after.board[&sys].planet_units.values().flatten().filter(|u| u.owner == a).count();
    assert_eq!(landed, 1, "one ground force landed");
    assert_eq!(after.board[&sys].units.len(), 2);
    let _ = crate::dual::take_report();
}

// -- Strategize and Overrule ---------------------------------------------------------------------------------------------------

/// The table of [`closing_position`] with the strategy cards laid out as the test wants: `b` and `c` hold `b_held` and `c_held`
/// (each card readied, `exhausted` names the ones that are not) and `mat` is the cards nobody has picked.
fn strategy_position(b_held: &[&str], c_held: &[&str], exhausted: &[&str], mat: &[&str]) -> GameState {
    strategy_on(closing_position(), b_held, c_held, exhausted, mat)
}

fn strategy_on(mut state: GameState, b_held: &[&str], c_held: &[&str], exhausted: &[&str], mat: &[&str]) -> GameState {
    let card = |id: &&str| ti4_model::StrategyCardId::new(*id);
    for (seat, held) in [("b", b_held), ("c", c_held)] {
        let seat = state.player_mut(&PlayerId::new(seat)).unwrap();
        seat.strategy_cards = held.iter().map(card).collect();
        seat.exhausted_strategy_cards = held.iter().filter(|id| exhausted.contains(id)).map(card).collect();
    }
    state.player_mut(&PlayerId::new("a")).unwrap().strategy_cards.clear();
    state.unclaimed_strategy_cards = mat.iter().map(card).collect();
    state
}

/// Strategize asks which card's secondary ability to perform: the other seats' readied cards in seating order, then the cards on
/// the mat (`holder's name` and `name`, the holders as a display fact); an exhausted card is not on offer. The Trade secondary
/// replenishes the holder's commodities and the Imperial secondary draws a secret objective (a hand over the limit hands one back).
#[test]
fn strategize_asks_which_card_to_perform_and_performs_the_trade_and_imperial_secondaries_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let a = PlayerId::new("a");
    let mut base = closing_position();
    base.player_mut(&a).unwrap().commodities = 0;
    // The pick among held and unclaimed cards: the three deciders take a different one of the four.
    let pick = strategy_on(base.clone(), &["pok5trade"], &["pok8imperial", "pok6warfare"], &["pok6warfare"], &[]);
    // Only the mat: the names carry no holder.
    let mat = strategy_on(base.clone(), &[], &[], &[], &["pok5trade", "pok8imperial"]);
    // A held card and a card on the mat (the label of the second has no holder).
    let both = strategy_on(base.clone(), &["pok5trade"], &[], &[], &["pok8imperial"]);
    // One card on offer is taken without a question.
    let lone_trade = strategy_on(base.clone(), &["pok5trade"], &[], &[], &[]);
    let lone_imperial = strategy_on(base.clone(), &[], &[], &[], &["pok8imperial"]);
    // The hand is over the limit after the draw: one secret goes back.
    let mut full = lone_imperial.clone();
    let held: Vec<_> = full.secret_deck.drain(..3).collect();
    full.player_mut(&a).unwrap().secret_objectives = held;
    // The deck has run out: nothing is drawn and the hand, over its limit already, is not asked about.
    let mut dry = full.clone();
    dry.secret_deck.clear();
    for case in [&pick, &mat, &both, &lone_trade, &lone_imperial, &full, &dry] {
        announce_all(&[("strategize1", case.clone())]);
    }
    announce_all(&[("strategize3", pick.clone())]);
    let _ = crate::dual::take_report();
    let after = played("strategize2", &lone_trade);
    assert_eq!(after.player(&a).unwrap().commodities, 3, "the Trade secondary replenishes (Arborec: 3)");
    let after = played("strategize4", &lone_imperial);
    assert_eq!(after.player(&a).unwrap().secret_objectives.len(), lone_imperial.player(&a).unwrap().secret_objectives.len() + 1, "the Imperial secondary draws a secret");
    let after = played("strategize1", &full);
    assert_eq!(after.player(&a).unwrap().secret_objectives.len(), 3, "over the limit: drawn, then handed back");
    let _ = crate::dual::take_report();
}

/// The Politics secondary draws two action cards; the Technology secondary pays four resources (the question-driven payment) and
/// researches (the research window of the strategy area, as a child of the card).
#[test]
fn strategize_performs_the_politics_and_technology_secondaries_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let a = PlayerId::new("a");
    let draw = strategy_position(&[], &[], &[], &["pok3politics"]);
    let research = strategy_on(research_position(), &[], &[], &[], &["pok7technology"]);
    // The pick among the two: the first and the last card are different abilities.
    let both = strategy_on(research_position(), &["pok3politics"], &[], &[], &["pok7technology"]);
    // Four resources are not there: the research is not asked.
    let mut poor = research.clone();
    poor.player_mut(&a).unwrap().trade_goods = 0;
    for board in poor.board.values_mut() {
        board.planet_control.retain(|_, owner| *owner != a);
    }
    for case in [&draw, &research, &both, &poor] {
        announce_all(&[("strategize1", case.clone())]);
    }
    let _ = crate::dual::take_report();
    let after = played("strategize2", &draw);
    assert_eq!(after.player(&a).unwrap().action_cards.len(), 2, "the played card is gone and two action cards were drawn");
    let after = played_by("strategize3", &research, Box::new(crate::action_cards_closing_tests::FirstOption));
    assert_eq!(after.player(&a).unwrap().technologies.len(), research.player(&a).unwrap().technologies.len() + 1, "one technology researched");
    let after = played("strategize4", &poor);
    assert_eq!(after.player(&a).unwrap().technologies.len(), poor.player(&a).unwrap().technologies.len(), "four resources are not there");
    let _ = crate::dual::take_report();
}

/// The Leadership secondary buys command tokens for influence (the purchase loop, the first purchase accepted), Diplomacy readies
/// up to two exhausted planets and Construction places a structure.
#[test]
fn strategize_performs_the_leadership_diplomacy_and_construction_secondaries_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let a = PlayerId::new("a");
    // The seat is rich (trade goods and ready planets pay), so the purchase asks what to pay with.
    let mut rich = closing_position();
    rich.exhausted_planets.clear();
    rich.player_mut(&a).unwrap().trade_goods = 4;
    let leadership = strategy_on(rich.clone(), &[], &[], &[], &["pok1leadership"]);
    // Trade goods alone pay (a lone option is taken without a question), then the offer of one more.
    let mut goods = closing_position();
    goods.player_mut(&a).unwrap().trade_goods = 6;
    let goods = strategy_on(goods, &[], &[], &[], &["pok1leadership"]);
    // Nothing to pay with: nothing is asked.
    let mut broke = closing_position();
    broke.player_mut(&a).unwrap().trade_goods = 0;
    let broke = strategy_on(broke, &[], &[], &[], &["pok1leadership"]);
    // Diplomacy: the position exhausts the planets of the seat (two or more to choose from), both printings.
    let diplomacy = strategy_position(&[], &[], &[], &["pok2diplomacy"]);
    let base_diplomacy = strategy_position(&[], &[], &[], &["base2"]);
    let mut ready = closing_position();
    ready.exhausted_planets.clear();
    let ready = strategy_on(ready, &[], &[], &[], &["pok2diplomacy"]);
    let construction = strategy_on(rich.clone(), &[], &[], &[], &["pok4construction"]);
    let base_construction = strategy_on(rich.clone(), &[], &[], &[], &["base4"]);
    for case in [&leadership, &goods, &broke, &diplomacy, &base_diplomacy, &ready, &construction, &base_construction] {
        announce_all(&[("strategize1", case.clone())]);
    }
    let _ = crate::dual::take_report();
    let tokens = |s: &GameState| {
        let seat = s.player(&a).unwrap();
        seat.tactic_tokens + seat.fleet_tokens + seat.strategic_tokens
    };
    let after = played_by("strategize2", &leadership, Box::new(crate::action_cards_closing_tests::FirstOption));
    assert!(tokens(&after) > tokens(&leadership), "a command token was bought");
    let after = played("strategize3", &diplomacy);
    assert!(after.exhausted_planets.len() < diplomacy.exhausted_planets.len(), "a planet was readied");
    let after = played_by("strategize4", &construction, Box::new(crate::action_cards_closing_tests::FirstOption));
    let structures = |s: &GameState| s.board.values().flat_map(|b| b.planet_units.values().flatten()).filter(|u| u.owner == a).count();
    assert_eq!(structures(&after), structures(&construction) + 1, "a structure was placed");
    let _ = crate::dual::take_report();
}

/// Overrule performs a primary ability: Trade (three trade goods and a replenishment, then the seats below their commodities the
/// holder lets replenish), Technology (a free research and a paid one), Leadership (three free tokens and the purchase loop),
/// Construction (two structures) and Warfare (recall a token, gain one, redistribute). The options of the Trade primary name the
/// seats by faction, so the table is named.
#[test]
fn overrule_performs_the_trade_technology_leadership_construction_and_warfare_primaries_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let a = PlayerId::new("a");
    let mut trade = closing_position();
    trade.player_mut(&a).unwrap().commodities = 0;
    trade.player_mut(&PlayerId::new("b")).unwrap().commodities = 0;
    trade.player_mut(&PlayerId::new("c")).unwrap().commodities = 1;
    let trade = strategy_on(trade, &[], &[], &[], &["pok5trade"]);
    // Nobody needs a replenishment: no question.
    let mut full = closing_position();
    for seat in ["a", "b", "c"] {
        full.player_mut(&PlayerId::new(seat)).unwrap().commodities = 9;
    }
    let full = strategy_on(full, &[], &[], &[], &["pok5trade"]);
    let research = strategy_on(research_position(), &[], &[], &[], &["pok7technology"]);
    let mut rich = closing_position();
    rich.exhausted_planets.clear();
    rich.player_mut(&a).unwrap().trade_goods = 4;
    let leadership = strategy_on(rich.clone(), &[], &[], &[], &["pok1leadership"]);
    let construction = strategy_on(rich.clone(), &[], &[], &[], &["pok4construction"]);
    let mut warfare = rich.clone();
    warfare.system_mut(&ti4_model::id::SystemId::new("33")).command_tokens.insert(a.clone());
    let warfare = strategy_on(warfare, &[], &[], &[], &["pok6warfare"]);
    for case in [&trade, &full, &research, &leadership, &construction, &warfare] {
        announce_all(&[("overrule", case.clone())]);
    }
    let _ = crate::dual::take_report();
    let after = played("overrule", &trade);
    assert!(after.player(&a).unwrap().trade_goods >= trade.player(&a).unwrap().trade_goods + 3, "the Trade primary pays three trade goods");
    let _ = crate::dual::take_report();
}

/// Overrule on Politics: the holder elects a new speaker (the options are named by faction), draws two action cards and puts the
/// top two agendas back (an old rules version draws from the deck with the unofficial cards in it).
#[test]
fn overrule_performs_the_politics_primary_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let politics = strategy_position(&["pok3politics"], &[], &[], &[]);
    let mut dry = politics.clone();
    dry.agenda_deck.clear();
    let mut old = politics.clone();
    old.rules_version = ti4_model::state::RULES_OFFICIAL_ACTION_DECK - 1;
    let mut edge = politics.clone();
    edge.rules_version = ti4_model::state::RULES_OFFICIAL_ACTION_DECK;
    for case in [&politics, &dry, &old, &edge] {
        announce_all(&[("overrule", case.clone())]);
    }
    let _ = crate::dual::take_report();
    let after = played("overrule", &politics);
    assert_eq!(after.player(&PlayerId::new("a")).unwrap().action_cards.len(), 2, "two action cards drawn");
    assert_eq!(after.agenda_deck.len(), politics.agenda_deck.len(), "the agendas are back on the deck");
    let _ = crate::dual::take_report();
}

/// The positions the abilities' guards and orders turn on: the seating order (the options of the pick are the other seats' cards
/// in seating order, not in the order of the players), a Letnev holder (the fleet pool may be emptied, the pool questions show
/// the bonus), a Naalu holder (the Trade primary).
#[test]
fn strategize_and_overrule_follow_the_seating_order_the_faction_and_the_rules_version_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let (a, b, c) = (PlayerId::new("a"), PlayerId::new("b"), PlayerId::new("c"));
    // c sits before b: its card is the first on offer.
    let mut turned = strategy_position(&["pok5trade"], &["pok8imperial"], &[], &[]);
    turned.seating_order = vec![c.clone(), b.clone(), a.clone()];
    // The same with a seat that sits between the two.
    let mut between = strategy_position(&["pok5trade"], &["pok8imperial"], &[], &["pok3politics"]);
    between.seating_order = vec![b.clone(), a.clone(), c.clone()];
    let with_faction = |mut state: GameState, faction: &str| {
        state.player_mut(&a).unwrap().faction = ti4_model::id::FactionId::new(faction);
        state
    };
    let mut warfare = closing_position();
    warfare.system_mut(&ti4_model::id::SystemId::new("33")).command_tokens.insert(a.clone());
    warfare.player_mut(&a).unwrap().fleet_tokens = 1;
    let letnev_warfare = with_faction(strategy_on(warfare, &[], &[], &[], &["pok6warfare"]), "letnev");
    let mut rich = closing_position();
    rich.exhausted_planets.clear();
    rich.player_mut(&a).unwrap().trade_goods = 4;
    let letnev_leadership = with_faction(strategy_on(rich, &[], &[], &[], &["pok1leadership"]), "letnev");
    let mut hacan = closing_position();
    hacan.player_mut(&a).unwrap().commodities = 0;
    hacan.player_mut(&b).unwrap().commodities = 0;
    let naalu_trade = with_faction(strategy_on(hacan, &[], &[], &[], &["pok5trade"]), "naalu");
    // The holder's own card is never on offer (it holds Leadership, which would be the last option otherwise).
    let mut own = strategy_position(&["pok5trade"], &["pok8imperial"], &[], &[]);
    own.player_mut(&a).unwrap().strategy_cards = vec![ti4_model::StrategyCardId::new("pok1leadership")];
    // The Thunder's Edge Construction is Construction for the secondary.
    let mut open = closing_position();
    open.exhausted_planets.clear();
    let te_construction = strategy_on(open, &[], &[], &[], &["te4construction"]);
    for case in [&turned, &between, &own, &te_construction] {
        announce_all(&[("strategize1", case.clone())]);
    }
    announce_all(&[("overrule", letnev_warfare.clone())]);
    for version in [ti4_model::state::RULES_LETNEV_FLEET_POOL - 1, ti4_model::state::RULES_LETNEV_FLEET_POOL] {
        let mut older = letnev_warfare.clone();
        older.rules_version = version;
        announce_all(&[("overrule", older)]);
    }
    announce_all(&[("overrule", letnev_leadership.clone())]);
    announce_all(&[("overrule", naalu_trade.clone())]);
    announce_all(&[("strategize1", naalu_trade)]);
    let _ = crate::dual::take_report();
}

// -- Strategize and Overrule: what the core leaves to the string -----------------------------------------------------------------

/// Takes the card `pick` at the pick question of Strategize or Overrule, declines the reaction slots and takes the last option of
/// every other question.
struct Performs(&'static str);

impl crate::choice::Decider for Performs {
    fn choose(&mut self, choice: &crate::choice::Choice) -> Result<crate::choice::ChoiceOption, crate::choice::IllegalChoice> {
        if choice.options.iter().any(|o| o.id.starts_with("reaction:")) {
            if let Some(decline) = choice.options.iter().find(|o| o.is_decline()) {
                return Ok(decline.clone());
            }
        }
        choice
            .options
            .iter()
            .find(|o| o.id == self.0)
            .or(choice.options.last())
            .cloned()
            .ok_or_else(|| crate::choice::IllegalChoice::NoOptions { player: choice.player.clone(), prompt: choice.prompt.clone() })
    }
}

/// `alias` played as the component action of seat `a` (with `filler` more action cards in the hand) choosing the card `pick`: the
/// `(compared, bailed)` counts of the announce window. The string run goes on alone; no difference may be reported.
fn bails_picking(alias: &str, start: &GameState, pick: &'static str, filler: usize) -> (u64, u64) {
    let a = PlayerId::new("a");
    let _ = crate::dual::take_report();
    let mut state = start.clone();
    let hand: Vec<_> = std::iter::once(alias).chain(std::iter::repeat_n("spy", filler)).map(ti4_model::id::ActionCardId::new).collect();
    state.player_mut(&a).unwrap().action_cards = hand;
    crate::action_cards::discard(&mut state, &a, 0);
    crate::dual::with_mask(crate::action_cards_closing_tests::dual(), || {
        crate::action_cards_announce_tests::announce(&mut state, alias, &a, Box::new(Performs(pick)))
    });
    assert_eq!(state.discarded_action_cards.last(), Some(&ti4_model::id::ActionCardId::new(alias)), "the card is on the pile");
    let report = crate::dual::report();
    assert!(report.differences.is_empty(), "{} differences", report.differences.len());
    let counts = crate::action_cards_closing_tests::windows("action_cards::announce");
    let _ = crate::dual::take_report();
    counts
}

/// Bail: the Warfare secondary (a production window at the home system), the Imperial primary (the scoreable objectives, the award
/// tail), the Diplomacy primary (both printings) and the Thunder's Edge primaries (a free tactical action; the choice of a
/// production) are the string's.
#[test]
fn bail_the_strategy_abilities_the_core_window_does_not_carry_are_the_strings() {
    let _serial = crate::dual::tests::serial();
    let on_offer = |held: &str| strategy_position(&[held], &["pok5trade"], &[], &[]);
    for (alias, held) in [
        ("strategize1", "pok6warfare"),
        ("strategize2", "te6warfare"),
        ("overrule", "pok8imperial"),
        ("overrule", "pok2diplomacy"),
        ("overrule", "base2"),
        ("overrule", "te6warfare"),
        ("overrule", "te4construction"),
    ] {
        assert_eq!(bails_picking(alias, &on_offer(held), held_static(held), 0), (0, 1), "{alias} performing {held}");
    }
}

/// The picked card's id as a `'static` text (the decider holds it).
fn held_static(id: &str) -> &'static str {
    ["pok6warfare", "te6warfare", "pok8imperial", "pok2diplomacy", "base2", "te4construction", "pok5trade", "pok3politics", "pok7technology", "pok1leadership", "pok4construction"]
        .into_iter()
        .find(|known| *known == id)
        .expect("a known card")
}

/// Bail: a replenishment that causes events (a Cabal at the table) is the string's, for both abilities of Trade.
#[test]
fn bail_the_trade_abilities_at_a_table_where_a_replenishment_causes_events() {
    let _serial = crate::dual::tests::serial();
    let mut cabal = strategy_position(&["pok5trade"], &[], &[], &[]);
    cabal.player_mut(&PlayerId::new("c")).unwrap().faction = ti4_model::id::FactionId::new("cabal");
    let plain = strategy_position(&["pok5trade"], &[], &[], &[]);
    for alias in ["strategize3", "overrule"] {
        assert_eq!(bails_picking(alias, &cabal, "pok5trade", 0), (0, 1), "{alias}");
        assert_eq!(bails_picking(alias, &plain, "pok5trade", 0), (1, 0), "{alias} at a plain table is carried");
    }
}

/// Bail: a Politics draw that ends in a discard question (a full hand) or that a faction hook changes (Yssaril's Scheming) is the
/// string's, for both abilities.
#[test]
fn bail_politics_when_the_draw_is_not_plain() {
    let _serial = crate::dual::tests::serial();
    let politics = strategy_position(&["pok3politics"], &[], &[], &[]);
    let mut scheming = politics.clone();
    scheming.player_mut(&PlayerId::new("c")).unwrap().faction = ti4_model::id::FactionId::new("yssaril");
    for alias in ["strategize4", "overrule"] {
        assert_eq!(bails_picking(alias, &politics, "pok3politics", 6), (0, 1), "{alias} with a full hand");
        assert_eq!(bails_picking(alias, &scheming, "pok3politics", 0), (0, 1), "{alias} at a table with Scheming");
    }
    // A hand that stays within its limit is carried.
    assert_eq!(bails_picking("strategize4", &politics, "pok3politics", 0), (1, 0));
}

/// Bail: a research at a table the research window does not carry (a Mentak seat: the Corsair's form), below the rules version
/// that asked the payment, is the string's, for both abilities of Technology.
#[test]
fn bail_technology_at_a_table_the_research_window_does_not_carry() {
    let _serial = crate::dual::tests::serial();
    let research = strategy_on(research_position(), &["pok7technology"], &[], &[], &[]);
    let mut mentak = research.clone();
    mentak.player_mut(&PlayerId::new("c")).unwrap().faction = ti4_model::id::FactionId::new("mentak");
    let mut old = research.clone();
    old.rules_version = ti4_model::state::RULES_TECH_PAYMENT_CHOICE - 1;
    for alias in ["strategize1", "overrule"] {
        assert_eq!(bails_picking(alias, &mentak, "pok7technology", 0), (0, 1), "{alias} at a table with Mentak");
        assert_eq!(bails_picking(alias, &old, "pok7technology", 0), (0, 1), "{alias} below the rules version of the payment plan");
        assert_eq!(bails_picking(alias, &research, "pok7technology", 0), (1, 0), "{alias} is carried at a plain table");
    }
}

/// Bail: Leadership and Construction at a table whose payments or placements the production rules do not follow (a Yssaril seat:
/// borrowed agents), Leadership and Warfare with a command token in a Mahact pool, are the string's.
#[test]
fn bail_leadership_construction_and_warfare_where_the_production_rules_do_not_follow() {
    let _serial = crate::dual::tests::serial();
    let plain = strategy_position(&["pok1leadership"], &["pok4construction"], &[], &[]);
    let mut yssaril = plain.clone();
    yssaril.player_mut(&PlayerId::new("c")).unwrap().faction = ti4_model::id::FactionId::new("yssaril");
    for (alias, pick) in [("strategize1", "pok1leadership"), ("overrule", "pok1leadership"), ("strategize2", "pok4construction"), ("overrule", "pok4construction")] {
        assert_eq!(bails_picking(alias, &yssaril, pick, 0), (0, 1), "{alias} performing {pick} at a table with Yssaril");
        assert_eq!(bails_picking(alias, &plain, pick, 0), (1, 0), "{alias} performing {pick} at a plain table is carried");
    }
    let mut warfare = strategy_position(&["pok1leadership"], &["pok6warfare"], &[], &[]);
    warfare.system_mut(&ti4_model::id::SystemId::new("33")).command_tokens.insert(PlayerId::new("a"));
    let mut mahact = warfare.clone();
    mahact.faction_marks.insert("mahact:fleet:b".to_owned(), "a".to_owned());
    for (alias, pick) in [("strategize3", "pok1leadership"), ("overrule", "pok1leadership"), ("overrule", "pok6warfare")] {
        assert_eq!(bails_picking(alias, &mahact, pick, 0), (0, 1), "{alias} performing {pick} with a token in a Mahact pool");
        assert_eq!(bails_picking(alias, &warfare, pick, 0), (1, 0), "{alias} performing {pick} without one is carried");
    }
}

/// Bail: Overrule names the seats of the Trade primary and the speaker of Politics by faction: at a table where two seats share a
/// faction the options would clash, so the whole card is the string's.
#[test]
fn bail_overrule_at_a_table_where_two_seats_share_a_faction() {
    let _serial = crate::dual::tests::serial();
    let mut twins = strategy_position(&["pok5trade"], &[], &[], &[]);
    twins.player_mut(&PlayerId::new("c")).unwrap().faction = ti4_model::id::FactionId::new("hacan");
    assert_eq!(bails_picking("overrule", &twins, "pok5trade", 0), (0, 1));
    // The secondaries name nobody: the same table is carried.
    assert_eq!(bails_picking("strategize1", &twins, "pok5trade", 0), (1, 0));
}
