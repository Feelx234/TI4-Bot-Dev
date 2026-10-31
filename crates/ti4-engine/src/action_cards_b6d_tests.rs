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
pub(crate) fn research_position() -> GameState {
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
    let list = thin.system_mut(&ti4_model::id::SystemId::new("33")).planet_units.get_mut("corneeq").unwrap();
    list.extend((0..3).map(|_| unit("pds", "a")));
    // The holder's own PDS stands before the others': it is not replaced and uses up none of the box.
    list.insert(0, unit("pds", "a"));
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
    assert_eq!(on(&thin_after, "pds", "a"), 6, "one PDS was left in the box: one swap (five standing before)");
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
    state.system_mut(&sys).units = vec![unit("infantry", "a"), unit("mech", "a"), unit("fighter", "a"), unit("infantry", "a")];
    state.last_ship_destroyed = Some((sys.clone(), a.clone(), ti4_model::id::UnitTypeId::new("cruiser")));
    let mut one_kind = state.clone();
    one_kind.system_mut(&sys).units = vec![unit("infantry", "a"), unit("infantry", "a")];
    let mut theirs = state.clone();
    theirs.last_ship_destroyed = Some((sys.clone(), b.clone(), ti4_model::id::UnitTypeId::new("cruiser")));
    let mut nothing = state.clone();
    nothing.system_mut(&sys).units.clear();
    // Mecatol Rex is the only planet of its system: nowhere to land.
    let mut mecatol = state.clone();
    mecatol.system_mut(&ti4_model::id::SystemId::new("18")).units = vec![unit("infantry", "a")];
    mecatol.last_ship_destroyed = Some((ti4_model::id::SystemId::new("18"), a.clone(), ti4_model::id::UnitTypeId::new("cruiser")));
    for case in [&state, &one_kind, &theirs, &nothing, &mecatol] {
        announce_all(&[("crashlanding", case.clone())]);
    }
    let _ = crate::dual::take_report();
    let after = played("crashlanding", &state);
    let landed: usize = after.board[&sys].planet_units.values().flatten().filter(|u| u.owner == a).count();
    assert_eq!(landed, 1, "one ground force landed");
    assert_eq!(after.board[&sys].units.len(), 3);
    let _ = crate::dual::take_report();
}

// -- Exploration Probe -----------------------------------------------------------------------------------------------

/// Seat `a` holds a cruiser in "25" of the one-ring map; frontier tokens lie in that system, in a system next to it and in one
/// out of its reach; the frontier deck holds `deck` (the top card first).
fn probe_position(deck: &[&str]) -> GameState {
    let galaxy = ring();
    let mut state = closing_position();
    let (near, far) = probe_sites(&galaxy);
    for id in ["25", near.as_str(), far.as_str()] {
        state.system_mut(&ti4_model::id::SystemId::new(id));
    }
    state.system_mut(&ti4_model::id::SystemId::new("25")).units.push(unit("cruiser", "a"));
    state.frontier_tokens = ["25", near.as_str(), far.as_str()].iter().map(|id| ti4_model::id::SystemId::new(*id)).collect();
    state.exploration_decks.insert("FRONTIER".to_owned(), deck.iter().map(|card| (*card).to_owned()).collect());
    state
}

/// A system of the ring next to "25" (not the centre) and one out of reach of it.
fn probe_sites(galaxy: &ti4_content::galaxy::Galaxy) -> (String, String) {
    let next: Vec<String> = galaxy.adjacent("25").into_iter().map(str::to_owned).collect();
    let near = ["26", "39", "40", "67", "51"].iter().find(|id| next.iter().any(|n| n == **id)).expect("a ring system next to 25").to_string();
    let far = ["26", "39", "40", "67", "51"].iter().find(|id| !next.iter().any(|n| n == **id)).expect("a ring system out of reach").to_string();
    (near, far)
}

/// Exploration Probe: the frontier tokens in or next to a system with a ship of the holder; the top card of the frontier deck is
/// drawn and resolved (here a fragment: it needs no question).
#[test]
fn exploration_probe_explores_a_frontier_token_near_the_holders_ships_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let galaxy = ring();
    let state = probe_position(&["urf1", "urf2", "urf3"]);
    crate::action_cards_closing_tests::announce_all_on(&[("probe", state.clone())], Some(&galaxy));
    let _ = crate::dual::take_report();
    let a = PlayerId::new("a");
    let (_, far) = probe_sites(&galaxy);
    let after = played_on("probe", &state, &galaxy);
    assert_eq!(after.frontier_tokens.len(), 2, "one token was explored");
    assert!(after.frontier_tokens.contains(&ti4_model::id::SystemId::new(far.as_str())), "the token out of reach stays");
    assert_eq!(after.player(&a).unwrap().relic_fragments.get("FRONTIER"), Some(&1));
    assert_eq!(after.exploration_decks["FRONTIER"].len(), 2);
    let row = after.exploration_log.last().expect("a row");
    assert!(row.card == "urf1" && row.outcome == "fragment" && row.placed == ["fragment:frontier"], "{:?}", (&row.card, &row.outcome, &row.placed));
    let _ = crate::dual::take_report();
}

fn played_on(alias: &str, start: &GameState, galaxy: &ti4_content::galaxy::Galaxy) -> GameState {
    let a = PlayerId::new("a");
    let mut state = start.clone();
    state.player_mut(&a).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new(alias)];
    crate::action_cards::discard(&mut state, &a, 0);
    crate::dual::with_mask(crate::action_cards_closing_tests::dual(), || {
        crate::action_cards_announce_tests::announce_on(&mut state, alias, &a, Box::new(crate::action_cards_announce_tests::DeclineSlots), Some(galaxy))
    });
    state
}

/// Every card of the frontier deck, drawn by Exploration Probe: what it does to the holder (a fragment, command tokens into pools of
/// its choice and trade goods, a relic, a secret objective, two action cards, a Merchant Station's choice, the Enigmatic Device
/// kept in the play area, a gamma token in the active system) and the cards that have nowhere to act without a planet (the ion
/// storm and the Mirage), each with the words its log row is closed with.
#[test]
fn exploration_probe_resolves_every_card_of_the_frontier_deck_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let galaxy = ring();
    // The last two are cards of other decks that the string resolves the same way when they are in the frontier deck: the
    // Gamma Wormhole and a cultural relic fragment (the fragment of the card's own deck).
    let cards = ["urf1", "minent", "ent", "majent", "kel1", "kel2", "dw", "dv1", "dv2", "ed1", "ed2", "gamma", "ion", "lc1", "lc2", "ms1", "ms2", "mirage", "gw", "crf1"];
    let mut gamma_less = probe_position(&["gamma"]);
    gamma_less.active_system = None;
    let mut cases: Vec<(&str, GameState)> = cards.iter().map(|card| ("probe", probe_position(&[card, "urf2"]))).collect();
    cases.push(("probe", gamma_less));
    crate::action_cards_closing_tests::announce_all_on(&cases, Some(&galaxy));
    let _ = crate::dual::take_report();
}

/// Without a map only the systems with a ship of the holder are offered (the string asks nothing about a neighbour then); with one,
/// the systems next to them too. A rival's ship, or a unit that is no ship, makes no system the holder's.
#[test]
fn exploration_probe_without_a_map_reaches_only_the_systems_with_a_ship() {
    let _serial = crate::dual::tests::serial();
    let galaxy = ring();
    let (near, far) = probe_sites(&galaxy);
    let a = PlayerId::new("a");
    // Ships in "25" and in `near`; the token of `far` is out of reach of both on the map (`near`'s own neighbours aside: the far
    // system is the one that is no neighbour of "25"), so only a ship there would reach it.
    let mut two = probe_position(&["urf1", "urf2"]);
    two.system_mut(&ti4_model::id::SystemId::new(near.as_str())).units.push(unit("destroyer", "a"));
    // A token beside a ship only: without a map it cannot be reached.
    let mut beside = probe_position(&["urf1", "urf2"]);
    beside.frontier_tokens.remove(&ti4_model::id::SystemId::new("25"));
    // A rival's ship and the holder's ground force in space do not count.
    let mut rivals = probe_position(&["urf1", "urf2"]);
    rivals.system_mut(&ti4_model::id::SystemId::new("25")).units.clear();
    rivals.system_mut(&ti4_model::id::SystemId::new("25")).units.push(unit("cruiser", "b"));
    rivals.system_mut(&ti4_model::id::SystemId::new("25")).units.push(unit("infantry", "a"));
    // A ship in a system the map does not hold (the string asks the map for its neighbours all the same).
    let mut off_map = probe_position(&["urf1", "urf2"]);
    off_map.system_mut(&ti4_model::id::SystemId::new("27")).units.push(unit("carrier", "a"));
    off_map.frontier_tokens.insert(ti4_model::id::SystemId::new("27"));
    for galaxy in [None, Some(&galaxy)] {
        crate::action_cards_closing_tests::announce_all_on(&[("probe", two.clone()), ("probe", beside.clone()), ("probe", rivals.clone()), ("probe", off_map.clone())], galaxy);
    }
    let _ = crate::dual::take_report();
    // Which tokens go: the last option is taken. Without a map: the ship systems, `near` after "25" (the order of the ids); with one
    // the neighbours join, and a lone option is taken without a question.
    let gone = |state: &GameState, galaxy: Option<&ti4_content::galaxy::Galaxy>| {
        let after = {
            let mut game = state.clone();
            game.player_mut(&a).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new("probe")];
            crate::action_cards::discard(&mut game, &a, 0);
            crate::dual::with_mask(crate::action_cards_closing_tests::dual(), || {
                crate::action_cards_announce_tests::announce_on(&mut game, "probe", &a, Box::new(crate::action_cards_announce_tests::DeclineSlots), galaxy)
            });
            game
        };
        let mut lost: Vec<String> = state.frontier_tokens.iter().filter(|s| !after.frontier_tokens.contains(*s)).map(ToString::to_string).collect();
        lost.sort();
        lost
    };
    assert_eq!(gone(&two, None).len(), 1);
    assert!(gone(&beside, None).is_empty(), "no ship in a system with a token, no map: nothing to explore");
    assert_eq!(gone(&beside, Some(&galaxy)).len(), 1, "with the map the neighbour of the ship's system is explored");
    assert!(gone(&rivals, Some(&galaxy)).is_empty(), "a rival's ship and a ground force are not the holder's ships");
    let _ = crate::dual::take_report();
}

/// The frontier deck has run out (or was never dealt): the token is spent for nothing and no row is logged — the string removes the
/// token before it draws, whatever its comment says.
#[test]
fn exploration_probe_spends_the_token_even_when_the_frontier_deck_has_run_out() {
    let _serial = crate::dual::tests::serial();
    let galaxy = ring();
    let empty = probe_position(&[]);
    let mut absent = probe_position(&[]);
    absent.exploration_decks.remove("FRONTIER");
    crate::action_cards_closing_tests::announce_all_on(&[("probe", empty.clone()), ("probe", absent.clone())], Some(&galaxy));
    let _ = crate::dual::take_report();
    for start in [&empty, &absent] {
        let after = played_on("probe", start, &galaxy);
        assert_eq!(after.frontier_tokens.len(), start.frontier_tokens.len() - 1, "the token is gone");
        assert_eq!(after.exploration_log.len(), start.exploration_log.len(), "no card was drawn, no row");
    }
    let _ = crate::dual::take_report();
}

/// Command tokens: Entropic Fields and the Keleres Ship hand them out one by one into pools of the holder's choice (rules version
/// 12); before that into the strategy pool without a question. The trade goods of the field follow the tokens.
#[test]
fn exploration_probe_gains_command_tokens_into_pools_and_trade_goods_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let galaxy = ring();
    let a = PlayerId::new("a");
    // The pool question begins at version 12.
    let mut twelve = probe_position(&["majent"]);
    twelve.rules_version = 12;
    let mut old = probe_position(&["majent"]);
    old.rules_version = 11;
    let mut old_ship = probe_position(&["kel1"]);
    old_ship.rules_version = 11;
    let cases = [("probe", probe_position(&["majent"])), ("probe", probe_position(&["kel2"])), ("probe", twelve.clone()), ("probe", old.clone()), ("probe", old_ship.clone())];
    crate::action_cards_closing_tests::announce_all_on(&cases, Some(&galaxy));
    let _ = crate::dual::take_report();
    let held = |s: &GameState| {
        let p = s.player(&a).unwrap();
        (p.tactic_tokens + p.fleet_tokens + p.strategic_tokens, p.strategic_tokens, p.trade_goods)
    };
    let start = probe_position(&["majent"]);
    let after = played_on("probe", &start, &galaxy);
    assert_eq!(held(&after).2 - held(&start).2, 3, "Major Entropic Field: three trade goods");
    assert_eq!(held(&after).0 - held(&start).0, 1, "and one command token");
    let (asked, _) = announced_once("probe", &twelve, Some(&galaxy), Box::new(crate::action_cards_closing_tests::FirstOption));
    assert_eq!(held(&asked).1 - held(&twelve).1, 0, "version 12 asks: the first pool is the tactic pool");
    assert_eq!(asked.player(&a).unwrap().tactic_tokens - twelve.player(&a).unwrap().tactic_tokens, 1);
    let after = played_on("probe", &old, &galaxy);
    assert_eq!(held(&after).1 - held(&old).1, 1, "before version 12 the token goes into the strategy pool");
    let after = played_on("probe", &old_ship, &galaxy);
    assert_eq!(held(&after).1 - held(&old_ship).1, 2);
    let _ = crate::dual::take_report();
}

/// Play `alias` once from `start` with `decider` (on a map when one is given): the position after it and the `(compared, bailed)`
/// counts of the announce window.
fn announced_once(
    alias: &str,
    start: &GameState,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
    decider: Box<dyn crate::choice::Decider>,
) -> (GameState, (u64, u64)) {
    let a = PlayerId::new("a");
    let _ = crate::dual::take_report();
    let mut state = start.clone();
    state.player_mut(&a).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new(alias)];
    crate::action_cards::discard(&mut state, &a, 0);
    crate::dual::with_mask(crate::action_cards_closing_tests::dual(), || crate::action_cards_announce_tests::announce_on(&mut state, alias, &a, decider, galaxy));
    let report = crate::dual::report();
    assert!(report.differences.is_empty(), "differences: {:?}", format!("{:?}", report.differences).chars().take(600).collect::<String>());
    let counts = crate::action_cards_closing_tests::windows("action_cards::announce");
    let _ = crate::dual::take_report();
    (state, counts)
}

/// A secret objective drawn by a Derelict Vessel over the hand limit asks which unscored secret goes back to the deck.
#[test]
fn exploration_probe_draws_a_secret_objective_and_returns_one_over_the_limit_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let galaxy = ring();
    let a = PlayerId::new("a");
    let start = probe_position(&["dv1", "dv2"]);
    let mut full = start.clone();
    let hand = full.secret_deck.drain(..3).collect::<Vec<_>>();
    full.player_mut(&a).unwrap().secret_objectives = hand;
    assert_eq!(full.player(&a).unwrap().secret_objectives.len(), 3);
    let mut empty_deck = start.clone();
    empty_deck.secret_deck.clear();
    crate::action_cards_closing_tests::announce_all_on(&[("probe", start.clone()), ("probe", full.clone()), ("probe", empty_deck.clone())], Some(&galaxy));
    let _ = crate::dual::take_report();
    let after = played_on("probe", &start, &galaxy);
    assert_eq!(after.player(&a).unwrap().secret_objectives.len(), start.player(&a).unwrap().secret_objectives.len() + 1);
    let after = played_on("probe", &full, &galaxy);
    assert_eq!(after.player(&a).unwrap().secret_objectives.len(), 3, "four secrets, one went back");
    assert_eq!(after.secret_deck.len(), full.secret_deck.len(), "the drawn one and the returned one are both in the deck count");
    let after = played_on("probe", &empty_deck, &galaxy);
    assert_eq!(after.player(&a).unwrap().secret_objectives, empty_deck.player(&a).unwrap().secret_objectives, "an empty deck draws nothing");
    let _ = crate::dual::take_report();
}

/// Dead World draws a relic (the Shard of the Throne scores at once; an empty relic deck gives nothing); the Enigmatic Device stays in
/// the play area; the gamma relay places its token in the active system (nowhere without one, nothing new where it already lies).
#[test]
fn exploration_probe_gains_relics_and_places_the_device_and_the_gamma_token_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let galaxy = ring();
    let a = PlayerId::new("a");
    let mut shard = probe_position(&["dw"]);
    shard.relic_deck = vec![ti4_model::id::RelicId::new("shard"), ti4_model::id::RelicId::new("thetriad")];
    let mut no_relics = probe_position(&["dw"]);
    no_relics.relic_deck.clear();
    let mut lying = probe_position(&["gamma"]);
    let here = lying.active_system.clone().expect("an active system");
    lying.wormhole_tokens.insert("GAMMA".to_owned(), here.clone());
    let mut elsewhere = probe_position(&["gamma"]);
    elsewhere.wormhole_tokens.insert("GAMMA".to_owned(), ti4_model::id::SystemId::new("27"));
    crate::action_cards_closing_tests::announce_all_on(
        &[("probe", shard.clone()), ("probe", no_relics.clone()), ("probe", lying.clone()), ("probe", elsewhere.clone()), ("probe", probe_position(&["ed2"]))],
        Some(&galaxy),
    );
    let _ = crate::dual::take_report();
    let after = played_on("probe", &shard, &galaxy);
    assert_eq!(after.player(&a).unwrap().relics, [ti4_model::id::RelicId::new("shard")]);
    assert_eq!(after.player(&a).unwrap().victory_points, shard.player(&a).unwrap().victory_points + 1, "the Shard scores");
    let after = played_on("probe", &no_relics, &galaxy);
    assert!(after.player(&a).unwrap().relics.is_empty());
    assert_eq!(after.exploration_log.last().unwrap().outcome, "resolved");
    let after = played_on("probe", &lying, &galaxy);
    assert!(after.exploration_log.last().unwrap().placed.is_empty(), "the token lay there already");
    let after = played_on("probe", &elsewhere, &galaxy);
    assert_eq!(after.wormhole_tokens.get("GAMMA"), Some(&here), "the token moves to the active system");
    assert_eq!(after.exploration_log.last().unwrap().placed, [format!("wormhole:GAMMA:{here}")]);
    let after = played_on("probe", &probe_position(&["ed2"]), &galaxy);
    assert_eq!(after.player(&a).unwrap().exploration_cards, ["ed2"]);
    assert_eq!(after.exploration_log.last().unwrap().placed, ["play_area:ed2"]);
    let _ = crate::dual::take_report();
}

/// Merchant Station: "replenish, or convert to trade goods" is asked (from rules version 19 only to a seat that has commodities to
/// convert); the replenish fills up to the faction's value (a seat above it is set down to it), the conversion turns all of them
/// into trade goods.
#[test]
fn exploration_probe_replenishes_or_converts_commodities_at_a_merchant_station_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let galaxy = ring();
    let a = PlayerId::new("a");
    let with = |commodities: i32, version: u32| {
        let mut state = probe_position(&["ms1", "ms2"]);
        state.player_mut(&a).unwrap().commodities = commodities;
        state.rules_version = version;
        state
    };
    let current = ti4_model::state::RULES_VERSION;
    let cases = [with(1, current), with(0, current), with(0, 18), with(3, current), with(5, current), with(0, 19)];
    let calls: Vec<(&str, GameState)> = cases.iter().map(|state| ("probe", state.clone())).collect();
    crate::action_cards_closing_tests::announce_all_on(&calls, Some(&galaxy));
    let _ = crate::dual::take_report();
    let goods = |s: &GameState| s.player(&a).unwrap().trade_goods;
    let limit = 3;
    // The last option is "convert", the first "replenish".
    let (converted, _) = announced_once("probe", &cases[0], Some(&galaxy), Box::new(crate::action_cards_announce_tests::DeclineSlots));
    assert_eq!((converted.player(&a).unwrap().commodities, goods(&converted) - goods(&cases[0])), (0, 1), "one commodity became a trade good");
    let (filled, _) = announced_once("probe", &cases[0], Some(&galaxy), Box::new(crate::action_cards_closing_tests::FirstOption));
    assert_eq!(filled.player(&a).unwrap().commodities, limit, "replenished to the faction's value");
    let (set_down, _) = announced_once("probe", &cases[4], Some(&galaxy), Box::new(crate::action_cards_closing_tests::FirstOption));
    assert_eq!(set_down.player(&a).unwrap().commodities, limit, "a seat above its value is set down to it");
    // Nothing to convert: the replenish is taken without a question (version 19), the conversion is still offered before.
    let (lone, _) = announced_once("probe", &cases[1], Some(&galaxy), Box::new(crate::action_cards_announce_tests::DeclineSlots));
    assert_eq!(lone.player(&a).unwrap().commodities, limit);
    let (offered, _) = announced_once("probe", &cases[2], Some(&galaxy), Box::new(crate::action_cards_announce_tests::DeclineSlots));
    assert_eq!((offered.player(&a).unwrap().commodities, goods(&offered)), (0, goods(&cases[2])), "version 18 offered the conversion of nothing");
    let (lone, _) = announced_once("probe", &cases[5], Some(&galaxy), Box::new(crate::action_cards_announce_tests::DeclineSlots));
    assert_eq!(lone.player(&a).unwrap().commodities, limit, "version 19 asks nothing of a seat without commodities");
    let _ = crate::dual::take_report();
}

/// Lost Crew: two action cards, drawn plainly; a draw that would put the hand over its limit (a question to discard) or a faction
/// that changes the draw (Yssaril) is the string's.
#[test]
fn exploration_probe_draws_two_action_cards_for_a_lost_crew_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let galaxy = ring();
    let a = PlayerId::new("a");
    let start = probe_position(&["lc1", "lc2"]);
    crate::action_cards_closing_tests::announce_all_on(&[("probe", start.clone())], Some(&galaxy));
    let _ = crate::dual::take_report();
    // The unofficial cards leave the deck before the draw from version 30.
    let (mut thirty, mut before_thirty) = (start.clone(), start.clone());
    thirty.rules_version = 30;
    before_thirty.rules_version = 29;
    crate::action_cards_closing_tests::announce_all_on(&[("probe", thirty.clone()), ("probe", before_thirty.clone())], Some(&galaxy));
    let _ = crate::dual::take_report();
    let (after, _) = announced_once("probe", &before_thirty, Some(&galaxy), Box::new(crate::action_cards_announce_tests::DeclineSlots));
    assert_eq!(after.action_card_deck.len() + 2, before_thirty.action_card_deck.len(), "version 29 keeps the unofficial cards");
    let (after, counts) = announced_once("probe", &start, Some(&galaxy), Box::new(crate::action_cards_announce_tests::DeclineSlots));
    assert_eq!(counts, (1, 0));
    assert_eq!(after.player(&a).unwrap().action_cards.len(), 2);
    assert_eq!(after.action_card_deck.len() + 2, crate::deck::action_deck_len(&start), "two cards left the deck (the unofficial ones left it before)");
    assert_eq!(after.exploration_log.last().unwrap().gains.get("action_cards"), Some(&2));
    // Six cards in hand: the draw ends in a discard question.
    let mut crowded = start.clone();
    let extra: Vec<_> = crowded.action_card_deck.drain(..6).collect();
    crowded.player_mut(&a).unwrap().action_cards = extra;
    let crowded_hand = crowded.player(&a).unwrap().action_cards.clone();
    let mut played = crowded.clone();
    played.player_mut(&a).unwrap().action_cards.insert(0, ti4_model::id::ActionCardId::new("probe"));
    let _ = crate::dual::take_report();
    crate::action_cards::discard(&mut played, &a, 0);
    assert_eq!(played.player(&a).unwrap().action_cards, crowded_hand);
    crate::dual::with_mask(crate::action_cards_closing_tests::dual(), || {
        crate::action_cards_announce_tests::announce_on(&mut played, "probe", &a, Box::new(crate::action_cards_announce_tests::DeclineSlots), Some(&galaxy))
    });
    assert_eq!(crate::action_cards_closing_tests::windows("action_cards::announce"), (0, 1), "the discard question is the string's (Probe draws with fx::draw_action_cards)");
    let _ = crate::dual::take_report();
    // Yssaril draws an extra card.
    let mut scheming = start.clone();
    scheming.player_mut(&a).unwrap().faction = ti4_model::id::FactionId::new("yssaril");
    let (_, counts) = announced_once("probe", &scheming, Some(&galaxy), Box::new(crate::action_cards_announce_tests::DeclineSlots));
    assert_eq!(counts, (0, 1));
}

/// The effect of `alias` played through `play_effect` itself (the window outside the announcement) under the area on `Dual`: the
/// position after it and the `(ran on the core, bailed)` count of the thread.
fn played_directly(alias: &str, start: &GameState, galaxy: Option<&ti4_content::galaxy::Galaxy>) -> (GameState, (u32, u32)) {
    played_directly_by(alias, start, galaxy, Box::new(crate::action_cards_announce_tests::DeclineSlots))
}

/// [`played_directly`] with a decider of the test's choosing (it must decline the reaction slots itself).
fn played_directly_by(alias: &str, start: &GameState, galaxy: Option<&ti4_content::galaxy::Galaxy>, decider: Box<dyn crate::choice::Decider>) -> (GameState, (u32, u32)) {
    let a = PlayerId::new("a");
    let id = ti4_model::id::ActionCardId::new(alias);
    let effect = crate::action_cards::effect_for(&id).expect("a registered effect");
    let mut state = start.clone();
    let _ = crate::dual::take_report();
    crate::action_cards::reset_core_plays();
    crate::dual::with_mask(crate::action_cards_closing_tests::dual(), || {
        let mut table = crate::choice::Table::with_default(decider);
        let mut dice = crate::dice::Dice::new();
        let mut rng = crate::rng::GameRng::new(0);
        let mut sequence = crate::event::EventSequence::new();
        let mut context = crate::timing::TimingContext {
            state: &mut state,
            content: ti4_content::ContentStore::embedded(),
            sources: ti4_model::content_types::POK,
            table: &mut table,
            dice: &mut dice,
            rng: &mut rng,
            event_sequence: &mut sequence,
            galaxy,
        };
        crate::action_cards::play_effect(&mut context, &id, &a, effect);
    });
    let report = crate::dual::report();
    assert!(report.differences.is_empty(), "differences: {}", format!("{:?}", report.differences).chars().take(600).collect::<String>());
    let counts = crate::action_cards::core_plays();
    let _ = crate::dual::take_report();
    (state, counts)
}

/// The effect played on its own (the play window, not the announcement) agrees with the core, on a map and without one, for a card
/// that asks nothing, a card that asks for command tokens and a card that opens a child window.
#[test]
fn exploration_probe_agrees_with_the_core_play_window_outside_the_announcement() {
    let _serial = crate::dual::tests::serial();
    let galaxy = ring();
    for card in ["urf1", "majent", "ms1", "dv1", "gamma", "lc1", "ed1"] {
        let start = probe_position(&[card, "urf2"]);
        for map in [Some(&galaxy), None] {
            let (after, counts) = played_directly("probe", &start, map);
            assert_eq!(counts, (1, 0), "{card} ran on the core");
            assert_eq!(after.frontier_tokens.len(), start.frontier_tokens.len() - 1, "{card}: a token was explored");
            assert_eq!(after.exploration_log.last().unwrap().card, card);
        }
    }
}

/// Bail: command tokens while a Mahact fleet holds tokens of the holder (the reinforcements the pool question previews and the cap
/// count them): the core declines after the pick, the string plays alone. A card without tokens is not affected.
#[test]
fn bail_exploration_probe_command_tokens_with_a_mahact_fleet_holding_the_holders_tokens_are_counted() {
    let _serial = crate::dual::tests::serial();
    let galaxy = ring();
    let mut held = probe_position(&["ent", "urf1"]);
    held.faction_marks.insert("mahact:fleet:b".to_owned(), "a".to_owned());
    let (_, counts) = announced_once("probe", &held, Some(&galaxy), Box::new(crate::action_cards_announce_tests::DeclineSlots));
    assert_eq!(counts, (0, 1));
    let mut quiet = probe_position(&["urf1", "urf2"]);
    quiet.faction_marks.insert("mahact:fleet:b".to_owned(), "a".to_owned());
    let (_, counts) = announced_once("probe", &quiet, Some(&galaxy), Box::new(crate::action_cards_announce_tests::DeclineSlots));
    assert_eq!(counts, (1, 0));
}

/// Bail: a Merchant Station's replenish while a Trade Agreement is in play (the replenishment is announced to the note and to the
/// Cabal): the core declines the replenish, carries the conversion.
#[test]
fn bail_exploration_probe_a_replenish_with_a_trade_agreement_in_play_is_counted() {
    let _serial = crate::dual::tests::serial();
    let galaxy = ring();
    let a = PlayerId::new("a");
    let mut state = probe_position(&["ms1", "ms2"]);
    state.player_mut(&a).unwrap().commodities = 1;
    state.promissory_notes.insert("ta:arborec".to_owned(), PlayerId::new("b"));
    // The first option is the replenish, the last the conversion.
    let (_, counts) = announced_once("probe", &state, Some(&galaxy), Box::new(crate::action_cards_closing_tests::FirstOption));
    assert_eq!(counts, (0, 1));
    let (after, counts) = announced_once("probe", &state, Some(&galaxy), Box::new(crate::action_cards_announce_tests::DeclineSlots));
    assert_eq!(counts, (1, 0));
    assert_eq!(after.player(&a).unwrap().commodities, 0);
}

/// Bail: a card of another deck in the frontier deck (an attachment, a planet's reward): the string resolves it, the core does not carry it.
#[test]
fn bail_exploration_probe_a_card_of_another_deck_is_counted() {
    let _serial = crate::dual::tests::serial();
    let galaxy = ring();
    for card in ["ds", "dmz", "aw1", "lf1", "frln1", "mo1", "cm1", "exp1", "vfs1"] {
        let state = probe_position(&[card, "urf1"]);
        let (_, counts) = announced_once("probe", &state, Some(&galaxy), Box::new(crate::action_cards_announce_tests::DeclineSlots));
        assert_eq!(counts, (0, 1), "{card}");
    }
}

/// Bail: a replay reserve is installed (a turn redo draws its cards through it): the string draws.
#[test]
fn bail_exploration_probe_with_a_replay_reserve_is_counted() {
    let _serial = crate::dual::tests::serial();
    let galaxy = ring();
    let mut state = probe_position(&["urf1", "urf2"]);
    state.deck_reserve = Some(ti4_model::deck_reserve::DeckReserve::new(
        std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(ti4_model::deck_reserve::IDLE)),
        Vec::new(),
    ));
    let (_, counts) = announced_once("probe", &state, Some(&galaxy), Box::new(crate::action_cards_announce_tests::DeclineSlots));
    assert_eq!(counts, (0, 1));
}

/// The ion storm and the Mirage say "in this system": the probe's draw names no system (`explore_with` passes none), so the string
/// cannot resolve them: the token is spent, the row says so, nothing is placed. A shortcoming of the string, copied on purpose
/// (docs/ENGINE_KNOWN_ISSUES.md, B6-copy-07).
#[test]
fn exploration_probe_cannot_resolve_the_ion_storm_and_the_mirage_without_a_system() {
    let _serial = crate::dual::tests::serial();
    let galaxy = ring();
    for card in ["ion", "mirage"] {
        let start = probe_position(&[card, "urf1"]);
        let (after, counts) = announced_once("probe", &start, Some(&galaxy), Box::new(crate::action_cards_announce_tests::DeclineSlots));
        assert_eq!(counts, (1, 0), "{card}");
        let row = after.exploration_log.last().expect("a row");
        assert!(row.card == card && row.outcome == "unresolved" && row.result == "could not be resolved", "{card}: {:?}", (&row.outcome, &row.result));
        assert!(after.wormhole_tokens == start.wormhole_tokens && after.ion_storm == start.ion_storm, "{card}: no token placed");
        assert!(serde_json::to_string(&after.board).unwrap() == serde_json::to_string(&start.board).unwrap(), "{card}: nothing on the board");
        assert_eq!(after.frontier_tokens.len(), start.frontier_tokens.len() - 1, "{card}: the token is spent");
    }
}
// -- Strategize and Overrule ---------------------------------------------------------------------------------------------------

/// The table of [`closing_position`] with the strategy cards laid out as the test wants: `b` and `c` hold `b_held` and `c_held`
/// (each card readied, `exhausted` names the ones that are not) and `mat` is the cards nobody has picked.
pub(crate) fn strategy_position(b_held: &[&str], c_held: &[&str], exhausted: &[&str], mat: &[&str]) -> GameState {
    strategy_on(closing_position(), b_held, c_held, exhausted, mat)
}

pub(crate) fn strategy_on(mut state: GameState, b_held: &[&str], c_held: &[&str], exhausted: &[&str], mat: &[&str]) -> GameState {
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
    // The faction effects of a research that the production kernel's hooks carry: Jol-Nar's waiver of a prerequisite (Analytical, the
    // agent spent), Nekro's Propagation (command tokens instead of a technology), the Anti-Intellectual Revolution (a ship is destroyed).
    let mut jolnar = research.clone();
    jolnar.player_mut(&a).unwrap().faction = ti4_model::id::FactionId::new("jolnar");
    jolnar.player_mut(&a).unwrap().leaders.insert(ti4_model::id::LeaderId::new("jolnaragent"), ti4_model::state::LeaderStatus::Exhausted);
    let mut nekro = research.clone();
    nekro.player_mut(&a).unwrap().faction = ti4_model::id::FactionId::new("nekro");
    let mut revolution = research.clone();
    revolution.enact_law("revolution", "for");
    for case in [&draw, &research, &both, &poor, &jolnar, &nekro, &revolution] {
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
    // Diplomacy: a system the holder controls a planet in, the other seats place a token there (announced), then up to two planets are
    // readied; the base printing readies the whole system from rules version 35.
    let diplomacy = strategy_position(&["pok2diplomacy"], &[], &[], &[]);
    let base_diplomacy = strategy_position(&["base2"], &[], &[], &[]);
    let mut older = base_diplomacy.clone();
    older.rules_version = ti4_model::state::RULES_BASE_DIPLOMACY - 1;
    for case in [&trade, &full, &research, &leadership, &construction, &warfare, &diplomacy, &base_diplomacy, &older] {
        announce_all(&[("overrule", case.clone())]);
    }
    let _ = crate::dual::take_report();
    let after = played("overrule", &trade);
    assert!(after.player(&a).unwrap().trade_goods >= trade.player(&a).unwrap().trade_goods + 3, "the Trade primary pays three trade goods");
    let after = played("overrule", &diplomacy);
    let tokens = |s: &GameState| s.board.values().map(|b| b.command_tokens.len()).sum::<usize>();
    assert!(tokens(&after) > tokens(&diplomacy), "the other seats placed a command token");
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
/// tail) and the Thunder's Edge primaries (a free tactical action; the choice of a production) are the string's.
#[test]
fn bail_the_strategy_abilities_the_core_window_does_not_carry_are_the_strings() {
    let _serial = crate::dual::tests::serial();
    let on_offer = |held: &str| strategy_position(&[held], &["pok5trade"], &[], &[]);
    for (alias, held) in [
        ("strategize1", "pok6warfare"),
        ("strategize2", "te6warfare"),
        ("overrule", "pok8imperial"),
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

/// A Politics draw that a faction hook changes (Yssaril's Scheming and Crafty are the drawer's own) is the core's since W14, for both
/// abilities: the child draw window adds the card, asks the discard (`OwnHand { Scheming }`) and lifts the limit. A draw that ends over the hand
/// limit is carried since W6 (the child draw window asks the discard), and a Yssaril beside the drawer changes nothing.
#[test]
fn politics_follows_the_hooks_that_change_the_draw() {
    let _serial = crate::dual::tests::serial();
    let politics = strategy_position(&["pok3politics"], &[], &[], &[]);
    let mut scheming = politics.clone();
    scheming.player_mut(&PlayerId::new("a")).unwrap().faction = ti4_model::id::FactionId::new("yssaril");
    let mut beside = politics.clone();
    beside.player_mut(&PlayerId::new("c")).unwrap().faction = ti4_model::id::FactionId::new("yssaril");
    for alias in ["strategize4", "overrule"] {
        assert_eq!(bails_picking(alias, &scheming, "pok3politics", 0), (1, 0), "{alias} with Scheming");
        assert_eq!(bails_picking(alias, &politics, "pok3politics", 6), (1, 0), "{alias} with a full hand: the discard is asked in the core");
        assert_eq!(bails_picking(alias, &beside, "pok3politics", 0), (1, 0), "{alias} beside a Yssaril");
    }
    // A hand that stays within its limit is carried.
    assert_eq!(bails_picking("strategize4", &politics, "pok3politics", 0), (1, 0));
}

/// Bail: a research at a table the research window does not carry (a Mentak seat with The Table's Grace: the Corsair's form), below the rules version
/// that asked the payment, is the string's, for both abilities of Technology.
#[test]
fn bail_technology_at_a_table_the_research_window_does_not_carry() {
    let _serial = crate::dual::tests::serial();
    let research = strategy_on(research_position(), &["pok7technology"], &[], &[], &[]);
    let mut mentak = research.clone();
    mentak.player_mut(&PlayerId::new("c")).unwrap().faction = ti4_model::id::FactionId::new("mentak");
    let plain_mentak = mentak.clone();
    mentak.player_mut(&PlayerId::new("c")).unwrap().breakthrough = Some(ti4_model::id::BreakthroughId::new("mentakbt"));
    let mut old = research.clone();
    old.rules_version = ti4_model::state::RULES_TECH_PAYMENT_CHOICE - 1;
    // A faction effect the core table does not hold (here a waiver of prerequisites the string offers): the hooks disagree.
    let waiver = crate::factions::hooks_strategy::StrategyHooks {
        research_waiver_offer: Some(|_, _, _, _| Some(crate::factions::hooks_strategy::ResearchWaiver { id: "t".to_owned(), label: "t".to_owned(), payments: Vec::new() })),
        ..crate::factions::hooks_strategy::StrategyHooks::NONE
    };
    for alias in ["strategize1", "overrule"] {
        let declined = crate::factions::hooks_strategy::with_test_hooks(waiver, || bails_picking(alias, &research, "pok7technology", 0));
        assert_eq!(declined, (0, 1), "{alias} where the string modules and the core table disagree on the hooks of a research");
        assert_eq!(bails_picking(alias, &mentak, "pok7technology", 0), (0, 1), "{alias} at a table with a Mentak who holds The Table's Grace");
        assert_eq!(bails_picking(alias, &plain_mentak, "pok7technology", 0), (1, 0), "{alias} at a table with a Mentak without it is carried (W6)");
        assert_eq!(bails_picking(alias, &old, "pok7technology", 0), (0, 1), "{alias} below the rules version of the payment plan");
        assert_eq!(bails_picking(alias, &research, "pok7technology", 0), (1, 0), "{alias} is carried at a plain table");
    }
}

/// Bail: Leadership and Construction at a table whose payments or placements the production rules do not follow (a Yssaril seat:
/// borrowed agents), (Leadership and Warfare with a command token in a Mahact pool are the core's since package W13.)
#[test]
fn bail_leadership_construction_and_warfare_where_the_production_rules_do_not_follow() {
    let _serial = crate::dual::tests::serial();
    let plain = strategy_position(&["pok1leadership"], &["pok4construction"], &[], &[]);
    // Package W5: a Yssaril seat at the table is carried now; the Keleres own seat (dropped by decision) stays string.
    let mut yssaril = plain.clone();
    yssaril.player_mut(&PlayerId::new("c")).unwrap().faction = ti4_model::id::FactionId::new("yssaril");
    let mut keleres = plain.clone();
    keleres.player_mut(&PlayerId::new("a")).unwrap().faction = ti4_model::id::FactionId::new("keleresm");
    for (alias, pick) in [("strategize1", "pok1leadership"), ("overrule", "pok1leadership"), ("strategize2", "pok4construction"), ("overrule", "pok4construction")] {
        assert_eq!(bails_picking(alias, &yssaril, pick, 0), (1, 0), "{alias} performing {pick} at a table with Yssaril");
        assert_eq!(bails_picking(alias, &keleres, pick, 0), (0, 1), "{alias} performing {pick} with a Keleres own seat");
        assert_eq!(bails_picking(alias, &plain, pick, 0), (1, 0), "{alias} performing {pick} at a plain table is carried");
    }
    let mut warfare = strategy_position(&["pok1leadership"], &["pok6warfare"], &[], &[]);
    warfare.system_mut(&ti4_model::id::SystemId::new("33")).command_tokens.insert(PlayerId::new("a"));
    let mut mahact = warfare.clone();
    mahact.faction_marks.insert("mahact:fleet:b".to_owned(), "a".to_owned());
    for (alias, pick) in [("strategize3", "pok1leadership"), ("overrule", "pok1leadership"), ("overrule", "pok6warfare")] {
        // Package W13: the core reads the pool from the marks (`fx::tokens_held_by_mahact`): the performed ability is carried.
        assert_eq!(bails_picking(alias, &mahact, pick, 0), (1, 0), "{alias} performing {pick} with a token in a Mahact pool is carried");
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

/// The tables a play window runs on carry the links to the areas whose windows a card runs as children (the payment, the hand
/// limit, the research, the exploration reward, the abilities of the strategy cards; `ti4_engine_core::driver::children`): the
/// plain table of `play_window` and `announce_window`, and the reactions table a window that follows nested activity runs on.
/// A table without them makes every such card decline (a bail in each lockstep window above, counted `(0, 1)`).
#[test]
fn the_tables_a_play_window_runs_on_carry_the_links_to_the_other_areas() {
    assert!(crate::action_cards_view::play_table().links.complete());
    assert!(crate::reactions_core::table().links.complete());
    assert!(ti4_engine_core::kernel::stage::AbilityTable::default().links.spawn.is_none(), "the empty table has none");
}

// -- Fire Team and Scramble Frequency: the dice a roll staged (docs/B6_CARDS.md, B6d rerolls) --------------------------------

/// Stage a roll for `who` in the string state (`GameState::reroll_staging`): `groups` are (group text, hits on, faces).
fn stage_roll(state: &mut GameState, who: &str, kind: &str, groups: &[(&str, u32, &[u32])]) {
    let system = state.board.keys().next().expect("a system on the board").clone();
    let rolls = groups
        .iter()
        .map(|(unit, hits_on, faces)| ti4_model::state::RerollEntry {
            unit: (*unit).to_owned(),
            planet: None,
            hits_on: Some(*hits_on),
            faces: faces.to_vec(),
            rerolled: std::collections::BTreeSet::new(),
            deltas: std::collections::BTreeMap::new(),
            unit_types: std::collections::BTreeMap::new(),
        })
        .collect();
    state.reroll_staging.insert(PlayerId::new(who), ti4_model::state::RerollSet { kind: kind.to_owned(), system, rolls });
}

/// Rolls every other staged die again (the first question, the third, ...); the reaction slots are declined.
struct EveryOther(usize);

impl crate::choice::Decider for EveryOther {
    fn choose(&mut self, choice: &crate::choice::Choice) -> Result<crate::choice::ChoiceOption, crate::choice::IllegalChoice> {
        let no_options = || crate::choice::IllegalChoice::NoOptions { player: choice.player.clone(), prompt: choice.prompt.clone() };
        if choice.options.iter().any(|o| o.id.starts_with("reaction:")) {
            return choice.options.iter().find(|o| o.is_decline()).cloned().ok_or_else(no_options);
        }
        self.0 += 1;
        let pick = if self.0 % 2 == 1 { choice.options.iter().find(|o| o.id.starts_with("reroll|")) } else { choice.options.iter().find(|o| o.is_decline()) };
        pick.or(choice.options.last()).cloned().ok_or_else(no_options)
    }
}

/// A ground roll of seat `a` as the string stages it: two groups of dice, the first with a Thalnos +1 on one die and a die that was
/// rolled again already; another seat's roll next to it.
fn ground_roll_position() -> GameState {
    let mut state = closing_position();
    stage_roll(&mut state, "a", "ground", &[("combat value 7", 7, &[2, 9, 5]), ("combat value 8", 8, &[4, 10])]);
    let set = state.reroll_staging.get_mut(&PlayerId::new("a")).unwrap();
    set.rolls[0].deltas.insert(2, 1);
    set.rolls[0].rerolled.insert(1);
    stage_roll(&mut state, "b", "ground", &[("combat value 8", 8, &[3, 3])]);
    state.last_reroll_player = Some(PlayerId::new("a"));
    state
}

/// [`ground_roll_position`] as the invasion stages it: the planet the dice were rolled against and the unit types behind each group,
/// and a group of dice with no number to hit on (an attachment's own dice).
fn staged_with_planets_and_a_group_without_a_threshold() -> GameState {
    let mut state = ground_roll_position();
    let planet = state.board.values().find_map(|b| b.planet_units.keys().next().cloned()).expect("a planet with units");
    let set = state.reroll_staging.get_mut(&PlayerId::new("a")).unwrap();
    for (entry, infantry) in set.rolls.iter_mut().zip([3, 2]) {
        entry.planet = Some(planet.clone());
        entry.unit_types.insert("infantry".to_owned(), infantry);
    }
    let mut free = set.rolls[1].clone();
    free.unit = "extra dice".to_owned();
    free.hits_on = None;
    free.faces = vec![6];
    free.rerolled.clear();
    free.unit_types.clear();
    set.rolls.push(free);
    state
}

fn faces_of(state: &GameState, who: &str) -> Vec<Vec<u32>> {
    state.reroll_staging.get(&PlayerId::new(who)).map(|set| set.rolls.iter().map(|r| r.faces.clone()).collect()).unwrap_or_default()
}

/// Fire Team through the announcement: a question per staged die (the dice shown with the question as the string shows them), the
/// dice that are chosen drawn again from the dice stream, the stream's positions and the faces compared, under all three deciders
/// and one that rolls every other die.
#[test]
fn fire_team_rolls_the_chosen_staged_dice_again_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let start = ground_roll_position();
    let realistic = staged_with_planets_and_a_group_without_a_threshold();
    announce_all(&[("fire_team", start.clone()), ("fire_team", realistic.clone())]);
    let _ = crate::dual::take_report();
    let after = played_by("fire_team", &realistic, Box::new(EveryOther(0)));
    let report = crate::dual::report();
    assert!(report.differences.is_empty(), "{:?}", report.differences.first());
    assert_eq!(crate::action_cards_closing_tests::windows("action_cards::announce"), (1, 0));
    assert_eq!(after.reroll_staging[&PlayerId::new("a")].rolls[1].planet, realistic.reroll_staging[&PlayerId::new("a")].rolls[1].planet);
    let rolls = &after.reroll_staging[&PlayerId::new("a")].rolls;
    assert_eq!((rolls[2].faces.clone(), rolls[2].rerolled.len()), (vec![6], 0), "the sixth question (the group with no threshold) was declined");
    assert_eq!(rolls[1].rerolled.iter().copied().collect::<Vec<_>>(), [1], "the fifth question was answered");
    let _ = crate::dual::take_report();
    // The dice 0, 2 and 4 of the five are rolled again, the others stay.
    let after = played_by("fire_team", &start, Box::new(EveryOther(0)));
    let report = crate::dual::report();
    assert!(report.differences.is_empty(), "{:?}", report.differences.first());
    assert_eq!(crate::action_cards_closing_tests::windows("action_cards::announce"), (1, 0));
    let (was, now) = (faces_of(&start, "a"), faces_of(&after, "a"));
    assert_eq!((now[0][1], now[1][0]), (was[0][1], was[1][0]), "the dice that were declined stay");
    let set = &after.reroll_staging[&PlayerId::new("a")];
    assert_eq!(set.rolls[0].rerolled.iter().copied().collect::<Vec<_>>(), [0, 1, 2], "the rerolled dice are marked, the one that was stays marked");
    assert_eq!(set.rolls[1].rerolled.iter().copied().collect::<Vec<_>>(), [1]);
    assert!(set.rolls[0].deltas.is_empty(), "the +1 on die 2 died with the die");
    assert_eq!(faces_of(&after, "b"), faces_of(&start, "b"), "another seat's roll is not touched");
    assert!(now.iter().flatten().all(|f| (1..=10).contains(f)));
    let _ = crate::dual::take_report();
    // Rolling every die: all five are marked.
    let all = played_by("fire_team", &start, Box::new(crate::action_cards_closing_tests::FirstOption));
    let set = &all.reroll_staging[&PlayerId::new("a")];
    assert_eq!(set.rolls[0].rerolled.len() + set.rolls[1].rerolled.len(), 5);
    let _ = crate::dual::take_report();
}

/// The same card played through `play_effect` itself (the play window, outside the announcement).
#[test]
fn fire_team_and_scramble_frequency_agree_with_the_core_play_window_outside_the_announcement() {
    let _serial = crate::dual::tests::serial();
    let start = ground_roll_position();
    let (after, counts) = played_directly_by("fire_team", &start, None, Box::new(EveryOther(0)));
    assert_eq!(counts, (1, 0), "Fire Team ran on the core");
    assert_ne!(faces_of(&after, "a"), faces_of(&start, "a"), "a die was rolled again");
    let mut cannon = closing_position();
    stage_roll(&mut cannon, "b", "space_cannon", &[("pds", 6, &[1, 2, 3])]);
    cannon.last_reroll_player = Some(PlayerId::new("b"));
    let (after, counts) = played_directly_by("scramble", &cannon, None, Box::new(crate::action_cards_announce_tests::DeclineSlots));
    assert_eq!(counts, (1, 0), "Scramble Frequency ran on the core");
    assert_eq!(after.reroll_staging[&PlayerId::new("b")].rolls[0].rerolled.len(), 3);
}

/// Fire Team names the holder's own ground roll; Scramble Frequency a bombardment, a space cannon or an anti-fighter barrage of the
/// seat that rolled last. Everything else is a card that does nothing (and still goes to the pile), in the core as in the string.
#[test]
fn fire_team_and_scramble_frequency_with_no_roll_they_name_do_nothing_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let mut cases: Vec<(&str, GameState)> = vec![("fire_team", closing_position()), ("scramble", closing_position())];
    // Fire Team: another seat's ground roll, the holder's roll of another kind.
    let mut theirs = closing_position();
    stage_roll(&mut theirs, "b", "ground", &[("combat value 8", 8, &[3, 3])]);
    cases.push(("fire_team", theirs));
    for kind in ["fleet", "space_cannon", "bombardment", "anti_fighter_barrage"] {
        let mut mine = closing_position();
        stage_roll(&mut mine, "a", kind, &[("dreadnought", 5, &[3, 3])]);
        mine.last_reroll_player = Some(PlayerId::new("a"));
        cases.push(("fire_team", mine));
    }
    // Scramble Frequency: a ground roll, a fleet roll, no roller named, a roller with nothing staged.
    for kind in ["ground", "fleet"] {
        let mut roll = closing_position();
        stage_roll(&mut roll, "b", kind, &[("combat value 8", 8, &[3, 3])]);
        roll.last_reroll_player = Some(PlayerId::new("b"));
        cases.push(("scramble", roll));
    }
    let mut unnamed = closing_position();
    stage_roll(&mut unnamed, "b", "space_cannon", &[("pds", 6, &[1, 2])]);
    cases.push(("scramble", unnamed));
    let mut hollow = closing_position();
    hollow.last_reroll_player = Some(PlayerId::new("b"));
    cases.push(("scramble", hollow));
    announce_all(&cases);
    let _ = crate::dual::take_report();
    // Nothing was drawn: the staged dice are the ones that were staged.
    for (alias, start) in &cases {
        let after = played(alias, start);
        assert_eq!(faces_of(&after, "a"), faces_of(start, "a"), "{alias}");
        assert_eq!(faces_of(&after, "b"), faces_of(start, "b"), "{alias}");
    }
    let _ = crate::dual::take_report();
}

/// Scramble Frequency through the announcement: every die of the roller's roll is drawn again, with no question, whatever the
/// holder decides; the roll of the holder and of the others stays.
#[test]
fn scramble_frequency_rolls_all_the_dice_of_the_roller_again_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let cases: Vec<(&str, GameState)> = ["bombardment", "space_cannon", "anti_fighter_barrage"]
        .iter()
        .map(|kind| {
            let mut state = closing_position();
            stage_roll(&mut state, "b", kind, &[("dreadnought", 5, &[3, 7]), ("pds", 6, &[10])]);
            state.reroll_staging.get_mut(&PlayerId::new("b")).unwrap().rolls[0].deltas.insert(1, 1);
            stage_roll(&mut state, "c", "space_cannon", &[("pds", 6, &[1])]);
            stage_roll(&mut state, "a", "space_cannon", &[("pds", 6, &[2, 2])]);
            state.last_reroll_player = Some(PlayerId::new("b"));
            ("scramble", state)
        })
        .collect();
    announce_all(&cases);
    let _ = crate::dual::take_report();
    for (alias, start) in &cases {
        let after = played(alias, start);
        let set = &after.reroll_staging[&PlayerId::new("b")];
        assert_eq!(set.rolls[0].rerolled.len() + set.rolls[1].rerolled.len(), 3, "all three dice were drawn again");
        assert!(set.rolls[0].deltas.is_empty());
        assert!(faces_of(&after, "b").iter().flatten().all(|f| (1..=10).contains(f)));
        assert_eq!(faces_of(&after, "c"), faces_of(start, "c"));
        assert_eq!(faces_of(&after, "a"), faces_of(start, "a"), "the holder's own roll is not the roller's");
    }
    let _ = crate::dual::take_report();
}

/// Bail: a ground roll of more groups of dice than the adapter names for the questions is the string's (counted), and the
/// roll of twelve groups is carried.
#[test]
fn bail_fire_team_with_more_groups_of_dice_than_the_adapter_names_is_counted() {
    let _serial = crate::dual::tests::serial();
    let groups = |n: usize| -> Vec<(String, u32, Vec<u32>)> { (0..n).map(|i| (format!("combat value {}", i + 1), u32::try_from(i + 1).unwrap(), vec![5])).collect() };
    let staged = |n: usize| {
        let mut state = closing_position();
        let rolls = groups(n);
        let borrowed: Vec<(&str, u32, &[u32])> = rolls.iter().map(|(t, on, f)| (t.as_str(), *on, f.as_slice())).collect();
        stage_roll(&mut state, "a", "ground", &borrowed);
        state
    };
    let (_, counts) = announced_once("fire_team", &staged(13), None, Box::new(crate::action_cards_closing_tests::FirstOption));
    assert_eq!(counts, (0, 1));
    let (_, counts) = announced_once("fire_team", &staged(12), None, Box::new(crate::action_cards_closing_tests::FirstOption));
    assert_eq!(counts, (1, 0));
}

/// [`announced_once`] on dice and a generator of the test's choosing.
fn announced_on_dice(alias: &str, start: &GameState, dice: crate::dice::Dice, rng: crate::rng::GameRng) -> (GameState, (u64, u64)) {
    let a = PlayerId::new("a");
    let _ = crate::dice::Dice::new();
    let _ = crate::dual::take_report();
    let mut state = start.clone();
    state.player_mut(&a).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new(alias)];
    crate::action_cards::discard(&mut state, &a, 0);
    crate::dual::with_mask(crate::action_cards_closing_tests::dual(), || {
        let (mut dice, mut rng) = (dice, rng);
        let mut table = crate::choice::Table::with_default(Box::new(crate::action_cards_closing_tests::FirstOption));
        let mut sequence = crate::event::EventSequence::new();
        let mut resolver = crate::fixtures::armed_resolver(&state);
        let mut context = crate::timing::TimingContext {
            state: &mut state,
            content: ti4_content::ContentStore::embedded(),
            sources: ti4_model::content_types::POK,
            table: &mut table,
            dice: &mut dice,
            rng: &mut rng,
            event_sequence: &mut sequence,
            galaxy: None,
        };
        let _ = crate::reactions::announce(&mut context, &mut resolver, &a, &ti4_model::id::ActionCardId::new(alias));
    });
    let report = crate::dual::report();
    assert!(report.differences.is_empty(), "differences: {:?}", format!("{:?}", report.differences).chars().take(600).collect::<String>());
    let counts = crate::action_cards_closing_tests::windows("action_cards::announce");
    let _ = crate::dual::take_report();
    (state, counts)
}

/// Bail: the dice of a play that cannot be reproduced (test faces, a generator that is not the game's) are the string's, counted.
/// The same play on the game's own generator is carried.
#[test]
fn bail_fire_team_and_scramble_frequency_with_dice_the_core_cannot_reproduce_are_counted() {
    let _serial = crate::dual::tests::serial();
    let mut scramble = ground_roll_position();
    stage_roll(&mut scramble, "b", "space_cannon", &[("pds", 6, &[1, 2])]);
    scramble.last_reroll_player = Some(PlayerId::new("b"));
    let seed = scramble.rng_seed;
    for (alias, start) in [("fire_team", ground_roll_position()), ("scramble", scramble)] {
        let (_, own) = announced_on_dice(alias, &start, crate::dice::Dice::new(), crate::rng::GameRng::new(seed));
        assert_eq!(own, (1, 0), "{alias} on the game's generator is carried");
        let (_, preloaded) = announced_on_dice(alias, &start, crate::dice::Dice::from_faces([3, 3, 3]), crate::rng::GameRng::new(seed));
        assert_eq!(preloaded, (0, 1), "{alias} with test faces");
        let (_, foreign) = announced_on_dice(alias, &start, crate::dice::Dice::new(), crate::rng::GameRng::new(seed.wrapping_add(1)));
        assert_eq!(foreign, (0, 1), "{alias} on a generator that is not the game's");
    }
}

// -- Courageous To The End and Experimental Battlestation (branch b6d-hits): the hit flows of the combat area as child windows -------

/// The seeds of the dice the two cards are played under: every outcome of the rolls is met.
const SEEDS: [u64; 12] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];

/// [`played_by`] under the dice of `seed` and on a map, the first decider that declines the slots and takes the last option.
fn played_seeded(alias: &str, start: &GameState, seed: u64, galaxy: Option<&ti4_content::galaxy::Galaxy>) -> GameState {
    played_seeded_by(alias, start, seed, galaxy, Box::new(crate::action_cards_announce_tests::DeclineSlots))
}

/// [`played_seeded`] with a decider of the test's choosing (it must decline the reaction slots itself).
fn played_seeded_by(alias: &str, start: &GameState, seed: u64, galaxy: Option<&ti4_content::galaxy::Galaxy>, decider: Box<dyn crate::choice::Decider>) -> GameState {
    let a = PlayerId::new("a");
    let mut state = start.clone();
    state.player_mut(&a).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new(alias)];
    crate::action_cards::discard(&mut state, &a, 0);
    crate::dual::with_mask(crate::action_cards_closing_tests::dual(), || {
        crate::action_cards_announce_tests::announce_seeded(&mut state, alias, &a, decider, galaxy, seed)
    });
    state
}

/// The ships (units that are ships) of `owner` in `system`.
fn ships_in(state: &GameState, system: &ti4_model::id::SystemId, owner: &str) -> usize {
    state
        .system_state(system)
        .units
        .iter()
        .filter(|u| u.owner.as_str() == owner && matches!(u.type_id.as_str(), "carrier" | "cruiser" | "destroyer" | "dreadnought" | "fighter" | "flagship" | "warsun"))
        .count()
}

/// The system where the holder's ship was just destroyed: one of the systems where `a`, `b` and `c` all have ships (`closing_position`):
/// `b` has two cruisers and a dreadnought there, `c` a destroyer, `a` a carrier, a fighter and a cruiser.
fn fight_system(state: &GameState) -> ti4_model::id::SystemId {
    state
        .board
        .iter()
        .find(|(_, board)| ["a", "b", "c"].iter().all(|p| board.units.iter().any(|u| u.owner.as_str() == *p)))
        .map(|(system, _)| system.clone())
        .expect("a system with the ships of all three seats")
}

/// The position of a Courageous To The End: `a` has lost a `victim` ship in `system`.
fn courageous_position(victim: &str) -> (GameState, ti4_model::id::SystemId) {
    let mut state = closing_position();
    let system = fight_system(&state);
    state.last_ship_destroyed = Some((system.clone(), PlayerId::new("a"), ti4_model::id::UnitTypeId::new(victim)));
    (state, system)
}

/// Courageous To The End: two dice, one casualty for the opponent for every result at or above the combat value of the destroyed
/// ship. The opponent is the first other seat with ships in the system (seating order); a single ship goes without a question, the
/// dice are drawn from the Dice stream (faces and stream positions compared), and each loss is staged and announced as a
/// `SHIP_DESTROYED` after the card's discard (the events are compared with the string's).
#[test]
fn courageous_to_the_end_makes_the_opponent_destroy_ships_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let mut cases: Vec<(&str, GameState)> = Vec::new();
    // The three values (dreadnought 5, cruiser 7, destroyer 9) meet every number of hits under the seeds.
    for victim in ["dreadnought", "cruiser", "destroyer"] {
        cases.push(("courageous", courageous_position(victim).0));
    }
    // `c` sits before `b`: it is the opponent, and its only ship (a hurt destroyer) goes without a question; a second hit finds none.
    let (mut lone, system) = courageous_position("dreadnought");
    lone.seating_order = ["c", "a", "b"].iter().map(|p| PlayerId::new(*p)).collect();
    cases.push(("courageous", lone));
    // The holder has no ship left: the opponent is still the first other seat with ships.
    let (mut bare, system_bare) = courageous_position("cruiser");
    bare.system_mut(&system_bare).units.retain(|u| u.owner.as_str() != "a");
    cases.push(("courageous", bare));
    // `b` loses a damaged and a fresh ship of one type: the options are told apart by the damage.
    let (mut hurt, system_hurt) = courageous_position("dreadnought");
    hurt.system_mut(&system_hurt).units.iter_mut().filter(|u| u.owner.as_str() == "b" && u.type_id.as_str() == "cruiser").take(1).for_each(|u| u.sustained_damage = true);
    cases.push(("courageous", hurt));
    crate::action_cards_closing_tests::announce_all_seeded(&cases, None, &SEEDS);
    let _ = crate::dual::take_report();
    // The outcomes met: how many ships the opponent lost in each case.
    let mut met = std::collections::BTreeSet::new();
    for (i, (_, start)) in cases.iter().enumerate() {
        let system = fight_system(start);
        let opponent = if i == 3 { "c" } else { "b" };
        for seed in SEEDS {
            let after = played_seeded("courageous", start, seed, None);
            met.insert((i, ships_in(start, &system, opponent) - ships_in(&after, &system, opponent)));
        }
    }
    for i in [0, 1, 2] {
        assert!(met.contains(&(i, 0)) || i == 0, "case {i}: some seed rolls no hit: {met:?}");
    }
    assert!(met.iter().any(|(i, lost)| *i == 0 && *lost == 2), "two dreadnought hits cost two ships: {met:?}");
    assert!(met.iter().any(|(i, lost)| *i == 0 && *lost == 1), "one hit costs one ship: {met:?}");
    assert!(met.iter().any(|(i, lost)| *i == 0 && *lost == 0), "no hit costs none: {met:?}");
    assert!(met.iter().all(|(i, lost)| *i != 3 || *lost <= 1), "the lone destroyer of `c` can be lost once: {met:?}");
    assert!(met.contains(&(3, 1)) && met.contains(&(3, 0)), "the lone ship is lost or spared by the dice: {met:?}");
    let _ = system;
}

/// A planet in the system `id` of the map, if the tile has one.
fn planet_of(id: &str) -> Option<ti4_model::id::PlanetId> {
    use crate::generated::IdStr;
    ti4_engine_core::generated::Planet::ALL
        .iter()
        .find(|p| p.data().system.is_some_and(|s| s.as_str() == id))
        .map(|p| ti4_model::id::PlanetId::new(p.as_str()))
}

/// `owner`'s space dock on the first planet of the system `id` (the planet is theirs, too); `false` when the tile has no planet.
fn dock_in(state: &mut GameState, id: &str, owner: &str) -> bool {
    let Some(planet) = planet_of(id) else { return false };
    let system = ti4_model::id::SystemId::new(id);
    state.system_mut(&system).planet_control.insert(planet.clone(), PlayerId::new(owner));
    state.system_mut(&system).planet_units.entry(planet).or_default().push(unit("spacedock", owner));
    true
}

/// The map of one ring, with the sites of Experimental Battlestation: the active system (`25`, where `b` moved a fleet), a system
/// next to it with a planet, and a system with a planet that is not next to it.
fn battlestation_sites(galaxy: &ti4_content::galaxy::Galaxy) -> (&'static str, &'static str, &'static str) {
    let ids = ["18", "25", "26", "39", "40", "67", "51"];
    let active = "25";
    let adjacent = galaxy.adjacent(active);
    let near = ids.iter().copied().find(|id| *id != active && adjacent.contains(*id) && planet_of(id).is_some()).expect("a neighbour with a planet");
    let far = ids.iter().copied().find(|id| *id != active && !adjacent.contains(*id) && planet_of(id).is_some()).expect("a system with a planet that is not next to it");
    assert!(planet_of(active).is_some(), "the active system has a planet for a dock");
    (active, near, far)
}

/// The position of an Experimental Battlestation: `b` is the active seat and has moved `fleet` into the active system.
fn battlestation_position(active: &str, fleet: &[&str]) -> GameState {
    let mut state = closing_position();
    let system = ti4_model::id::SystemId::new(active);
    state.active = Some(PlayerId::new("b"));
    state.active_system = Some(system.clone());
    state.system_mut(&system).units.retain(|u| !matches!(u.owner.as_str(), "b" | "c"));
    for kind in fleet {
        state.system_mut(&system).units.push(unit(kind, "b"));
    }
    state
}

/// Experimental Battlestation: a space dock of the holder in or next to the active system rolls three dice (hits on 5); the hits
/// are absorbed by the active seat's ships there (SUSTAIN DAMAGE offers, then casualty questions: a child hit flow of the combat
/// area). The dock is the holder's pick (none to ask when one stands), the dice come from the Dice stream, and a loss announces
/// nothing (the string applies the hits on a stub resolver).
#[test]
fn experimental_battlestation_rolls_and_the_active_seat_absorbs_the_hits_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let galaxy = ring();
    let (active, near, far) = battlestation_sites(&galaxy);
    let fleet = ["dreadnought", "cruiser", "cruiser", "fighter", "fighter"];
    // One dock in the active system: nothing to ask the holder.
    let mut here = battlestation_position(active, &fleet);
    assert!(dock_in(&mut here, active, "a"));
    // A dock in the active system and one next to it: the holder picks (sorted by the system ids); the far one never counts.
    let mut two = here.clone();
    assert!(dock_in(&mut two, near, "a") && dock_in(&mut two, far, "a"));
    // Only a dock next to the active system and a far one: the neighbour fires without a question.
    let mut next_to = battlestation_position(active, &["dreadnought", "dreadnought", "cruiser"]);
    assert!(dock_in(&mut next_to, near, "a") && dock_in(&mut next_to, far, "a"));
    // Docks of the rivals and a far dock only: no dock may fire, no dice are rolled.
    let mut none = battlestation_position(active, &fleet);
    assert!(dock_in(&mut none, far, "a") && dock_in(&mut none, near, "c"));
    // The active seat has no ship in the system: the dice are rolled and the hits have no effect.
    let mut empty = battlestation_position(active, &[]);
    assert!(dock_in(&mut empty, active, "a"));
    // A damaged dreadnought and fresh ones: the sustain options and the casualty options tell them apart.
    let mut hurt = battlestation_position(active, &["dreadnought", "dreadnought", "cruiser", "carrier"]);
    assert!(dock_in(&mut hurt, active, "a"));
    let system = ti4_model::id::SystemId::new(active);
    hurt.system_mut(&system).units.iter_mut().filter(|u| u.type_id.as_str() == "dreadnought").take(1).for_each(|u| u.sustained_damage = true);
    let cases = [here.clone(), two.clone(), next_to.clone(), none.clone(), empty.clone(), hurt.clone()];
    let played: Vec<(&str, GameState)> = cases.iter().map(|c| ("experimental", c.clone())).collect();
    crate::action_cards_closing_tests::announce_all_seeded(&played, Some(&galaxy), &SEEDS);
    let _ = crate::dual::take_report();
    // The outcomes met: the active seat's ships lost (and the number of dreadnoughts left undamaged) over the seeds.
    let lost = |start: &GameState, seed: u64| {
        // The first option of every question: SUSTAIN DAMAGE is taken whenever it is offered.
        let after = played_seeded_by("experimental", start, seed, Some(&galaxy), Box::new(crate::action_cards_closing_tests::FirstOption));
        (ships_in(start, &system, "b") - ships_in(&after, &system, "b"), after.system_state(&system).units.iter().filter(|u| u.sustained_damage).count())
    };
    let outcomes: std::collections::BTreeSet<(usize, usize)> = SEEDS.iter().map(|seed| lost(&here, *seed)).collect();
    assert!(outcomes.iter().any(|(gone, _)| *gone == 0) && outcomes.iter().any(|(gone, hurt)| *gone >= 1 || *hurt >= 1), "hits are met and missed: {outcomes:?}");
    assert!(outcomes.iter().any(|(_, hurt)| *hurt == 1), "the dreadnought sustains a hit under some seed: {outcomes:?}");
    for seed in SEEDS {
        assert_eq!(lost(&none, seed), (0, 0), "no dock may fire (seed {seed})");
        assert_eq!(lost(&empty, seed), (0, 0), "no ship to hit (seed {seed})");
    }
    // Without the map only a dock in the active system counts, as in the string.
    crate::action_cards_closing_tests::announce_all_seeded(&[("experimental", here.clone()), ("experimental", next_to.clone()), ("experimental", two.clone())], None, &SEEDS);
    let _ = crate::dual::take_report();
    for seed in SEEDS {
        let after = played_seeded("experimental", &next_to, seed, None);
        assert_eq!(ships_in(&after, &system, "b"), ships_in(&next_to, &system, "b"), "a dock next to the system cannot fire without a map (seed {seed})");
    }
}

/// Experimental Battlestation against a seat whose Rear Admiral Farran is unlocked: a sustain is followed by the commander's offer
/// (a trade good), asked by the child sustain window of the core as the string asks it.
#[test]
fn experimental_battlestation_offers_the_letnev_commander_after_a_sustain_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let galaxy = ring();
    let (active, _, _) = battlestation_sites(&galaxy);
    let mut state = battlestation_position(active, &["dreadnought", "dreadnought", "cruiser"]);
    assert!(dock_in(&mut state, active, "a"));
    state.player_mut(&PlayerId::new("b")).unwrap().leaders.insert(ti4_model::id::LeaderId::new("letnevcommander"), ti4_model::state::LeaderStatus::Unlocked);
    crate::action_cards_closing_tests::announce_all_seeded(&[("experimental", state.clone())], Some(&galaxy), &SEEDS);
    let _ = crate::dual::take_report();
    // A seed where a dreadnought sustained under the first-option decider paid the commander's trade good.
    let goods = |s: &GameState| s.player(&PlayerId::new("b")).unwrap().trade_goods;
    let paid = SEEDS.iter().any(|seed| {
        let after = played_seeded_by("experimental", &state, *seed, Some(&galaxy), Box::new(crate::action_cards_closing_tests::FirstOption));
        goods(&after) > goods(&state)
    });
    assert!(paid, "the commander's trade good was offered and taken under some seed");
    let _ = crate::dual::take_report();
}

/// Bail: Experimental Battlestation in a game below the faction effects (the string's sustain offer is not the core window's
/// there) and against a seat that pays on a sustain without an unlocked commander (held another way) is the string's.
#[test]
fn bail_experimental_battlestation_where_the_sustain_offer_is_not_the_core_windows() {
    let _serial = crate::dual::tests::serial();
    let galaxy = ring();
    let (active, _, _) = battlestation_sites(&galaxy);
    let mut plain = battlestation_position(active, &["dreadnought", "cruiser"]);
    assert!(dock_in(&mut plain, active, "a"));
    let mut old = plain.clone();
    old.rules_version = ti4_model::state::RULES_FACTION_EFFECTS - 1;
    let mut held = plain.clone();
    held.faction_marks.insert("commander_ability:b:letnevcommander".to_owned(), "1".to_owned());
    let bailed = |state: &GameState| {
        let _ = crate::dual::take_report();
        let _ = played_seeded("experimental", state, 1, Some(&galaxy));
        let counted = crate::action_cards_closing_tests::windows("action_cards::announce");
        let _ = crate::dual::take_report();
        counted
    };
    assert_eq!(bailed(&plain), (1, 0), "the plain position is carried");
    assert_eq!(bailed(&old), (0, 1), "below the faction effects");
    assert_eq!(bailed(&held), (0, 1), "a commander held another way");
}

/// Courageous To The End against a neutral ship (nobody chooses its loss: the unit lowest on the reference card goes; the string names
/// the neutral side the opponent after the seated ones) is the core's since package W11, in lockstep. At a table with a Naaz or Nekro
/// seat (the units that fight from a planet) it is the string's, counted.
#[test]
fn courageous_to_the_end_against_a_neutral_opponent_is_the_cores_and_a_naaz_seat_is_counted() {
    let _serial = crate::dual::tests::serial();
    let (plain, system) = courageous_position("dreadnought");
    // Only the holder and a neutral cruiser fight in the system.
    let mut neutral = plain.clone();
    neutral.system_mut(&system).units.retain(|u| u.owner.as_str() == "a");
    neutral.system_mut(&system).units.push(unit("neutral_cruiser", "neutral"));
    let mut naaz = plain.clone();
    naaz.player_mut(&PlayerId::new("c")).unwrap().faction = ti4_model::id::FactionId::new("naaz");
    let bailed = |state: &GameState| {
        let _ = crate::dual::take_report();
        // The neutral units are Thunder's Edge's: only with that source in play are they ships.
        let _ = crate::action_cards_announce_tests::with_sources(ti4_model::content_types::FULL, || played_seeded("courageous", state, 1, None));
        let counted = crate::action_cards_closing_tests::windows("action_cards::announce");
        let _ = crate::dual::take_report();
        counted
    };
    assert_eq!(bailed(&plain), (1, 0), "the plain position is carried");
    assert_eq!(bailed(&neutral), (1, 0), "a neutral opponent is carried");
    assert_eq!(bailed(&naaz), (0, 1), "a Naaz seat at the table");
    // The seated opponent comes first: a neutral ship beside it changes nothing.
    let mut beside = plain.clone();
    beside.system_mut(&system).units.push(unit("neutral_cruiser", "neutral"));
    assert_eq!(bailed(&beside), (1, 0), "a seated opponent is chosen before the neutral one");
}

/// The play window (`action_cards::play`, the effect alone without the announcement) agrees with the core for both cards: the
/// destructions Courageous To The End stages stay in the state with the texts of the state, and the dock's hits are absorbed.
#[test]
fn courageous_to_the_end_and_experimental_battlestation_agree_with_the_core_play_window_in_lockstep() {
    let _serial = crate::dual::tests::serial();
    let galaxy = ring();
    let (active, _, _) = battlestation_sites(&galaxy);
    let mut dock = battlestation_position(active, &["dreadnought", "dreadnought", "cruiser", "cruiser", "fighter"]);
    assert!(dock_in(&mut dock, active, "a"));
    // Non-Euclidean Shielding is a row of the combat kernel the window lends: one sustain cancels two hits.
    dock.player_mut(&PlayerId::new("b")).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("nes"));
    let (fight, _) = courageous_position("dreadnought");
    let a = PlayerId::new("a");
    let mut ran = 0;
    for seed in SEEDS {
        for (alias, start, map) in [("courageous", &fight, None), ("experimental", &dock, Some(&galaxy))] {
            let mut state = start.clone();
            state.rng_seed = seed;
            let id = ti4_model::id::ActionCardId::new(alias);
            let effect = crate::action_cards::effect_for(&id).expect("a registered effect");
            let before = state.clone();
            let mut table = crate::choice::Table::with_default(Box::new(crate::action_cards_closing_tests::FirstOption));
            let mut dice = crate::dice::Dice::new();
            let mut rng = crate::rng::GameRng::new(seed);
            let mut sequence = crate::event::EventSequence::new();
            let mut context = crate::timing::TimingContext {
                state: &mut state,
                content: ti4_content::ContentStore::embedded(),
                sources: ti4_model::content_types::POK,
                table: &mut table,
                dice: &mut dice,
                rng: &mut rng,
                event_sequence: &mut sequence,
                galaxy: map,
            };
            let _ = crate::dual::take_report();
            crate::dual::with_mask(crate::action_cards_closing_tests::dual(), || crate::action_cards::play_effect(&mut context, &id, &a, effect));
            let report = crate::dual::report();
            assert!(report.differences.is_empty(), "{alias} seed {seed}: {:?}", report.differences.iter().take(3).collect::<Vec<_>>());
            assert_eq!(crate::action_cards_closing_tests::windows("action_cards::play"), (1, 0), "{alias} seed {seed}: {:?}", report.windows);
            ran += 1;
            let _ = (&before, &state);
        }
    }
    assert_eq!(ran, 2 * SEEDS.len());
    let _ = crate::dual::take_report();
}

/// Courageous To The End where no seat holds a card that could answer the announcement: the string's run has no nested activity, so
/// the core window runs on the plain table of the play window (not the reactions table a nested run is followed on), which must
/// carry the announcement of the staged destructions itself (`action_cards_view::play_table`).
#[test]
fn courageous_to_the_end_is_announced_on_the_plain_table_of_the_play_window() {
    let _serial = crate::dual::tests::serial();
    let mut cases: Vec<(&str, GameState)> = Vec::new();
    for victim in ["dreadnought", "cruiser"] {
        let (mut state, _) = courageous_position(victim);
        for player in state.players.iter_mut() {
            player.action_cards.clear();
        }
        cases.push(("courageous", state));
    }
    crate::action_cards_closing_tests::announce_all_seeded(&cases, None, &SEEDS);
    let _ = crate::dual::take_report();
    // Some seed destroys a ship on that table (the lockstep compared its `SHIP_DESTROYED` with the string's).
    let (system, lost) = {
        let system = fight_system(&cases[0].1);
        let lost = SEEDS.iter().any(|seed| ships_in(&played_seeded("courageous", &cases[0].1, *seed, None), &system, "b") < ships_in(&cases[0].1, &system, "b"));
        (system, lost)
    };
    let _ = system;
    assert!(lost, "a ship was destroyed under some seed");
    let _ = crate::dual::take_report();
}

/// Experimental Battlestation against a seat with Non-Euclidean Shielding ("cancel 2 hits" for each use of SUSTAIN DAMAGE): the
/// sustain offer of the child window reads the combat kernel the play window lends (the faction modules' rows), not a plain one.
#[test]
fn experimental_battlestation_follows_non_euclidean_shielding_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let galaxy = ring();
    let (active, _, _) = battlestation_sites(&galaxy);
    let mut state = battlestation_position(active, &["dreadnought", "dreadnought", "cruiser", "cruiser"]);
    assert!(dock_in(&mut state, active, "a"));
    state.player_mut(&PlayerId::new("b")).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("nes"));
    crate::action_cards_closing_tests::announce_all_seeded(&[("experimental", state.clone())], Some(&galaxy), &SEEDS);
    let _ = crate::dual::take_report();
    // With the shielding a single sustain absorbs two hits: the fleet loses fewer ships than without it, under some seed.
    let mut bare = state.clone();
    bare.player_mut(&PlayerId::new("b")).unwrap().technologies.clear();
    let system = ti4_model::id::SystemId::new(active);
    let fewer = SEEDS.iter().any(|seed| {
        let shielded = played_seeded_by("experimental", &state, *seed, Some(&galaxy), Box::new(crate::action_cards_closing_tests::FirstOption));
        let open = played_seeded_by("experimental", &bare, *seed, Some(&galaxy), Box::new(crate::action_cards_closing_tests::FirstOption));
        ships_in(&shielded, &system, "b") > ships_in(&open, &system, "b")
    });
    assert!(fewer, "two hits cancelled by one sustain saved a ship under some seed");
    let _ = crate::dual::take_report();
}

/// Reveal Prototype played at the start of a space combat: the four resources are paid by questions the string asks of the table
/// directly, which carry neither the battle's system nor `space_battle` (found by the golden dual of the gate: the core question
/// carried the battle's system, `context.target` string null / core System).
#[test]
fn reveal_prototype_paid_in_a_space_combat_asks_payment_questions_without_the_battle() {
    let _serial = crate::dual::tests::serial();
    let a = PlayerId::new("a");
    let mut state = research_position();
    state.player_mut(&a).unwrap().trade_goods = 0;
    let active = state.active_system.clone().expect("an active system");
    state.system_mut(&active).units.push(unit("cruiser", "a"));
    state.system_mut(&active).units.push(unit("cruiser", "b"));
    state.active_space_combat = Some((active, a.clone(), PlayerId::new("b")));
    announce_all(&[("reveal_prototype", state.clone())]);
    let _ = crate::dual::take_report();
}
