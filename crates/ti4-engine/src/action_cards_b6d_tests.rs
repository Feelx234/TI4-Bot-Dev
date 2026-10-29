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
    assert_eq!(crate::action_cards_closing_tests::windows("action_cards::announce"), (0, 1), "the discard question is the string's");
    let _ = crate::dual::take_report();
    // Yssaril draws an extra card.
    let mut scheming = start.clone();
    scheming.player_mut(&PlayerId::new("b")).unwrap().faction = ti4_model::id::FactionId::new("yssaril");
    let (_, counts) = announced_once("probe", &scheming, Some(&galaxy), Box::new(crate::action_cards_announce_tests::DeclineSlots));
    assert_eq!(counts, (0, 1));
}

/// The effect of `alias` played through `play_effect` itself (the window outside the announcement) under the area on `Dual`: the
/// position after it and the `(ran on the core, bailed)` count of the thread.
fn played_directly(alias: &str, start: &GameState, galaxy: Option<&ti4_content::galaxy::Galaxy>) -> (GameState, (u32, u32)) {
    let a = PlayerId::new("a");
    let id = ti4_model::id::ActionCardId::new(alias);
    let effect = crate::action_cards::effect_for(&id).expect("a registered effect");
    let mut state = start.clone();
    let _ = crate::dual::take_report();
    crate::action_cards::reset_core_plays();
    crate::dual::with_mask(crate::action_cards_closing_tests::dual(), || {
        let mut table = crate::choice::Table::with_default(Box::new(crate::action_cards_announce_tests::DeclineSlots));
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
