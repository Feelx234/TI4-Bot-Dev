//! The Wormhole Nexus reached through a gamma wormhole (a printed one or a gamma token).
//!
//! Rule: every system containing a gamma wormhole is adjacent to every other one, and the locked
//! Nexus (tile 82a) prints a gamma wormhole, so a ship in a system holding a gamma token may
//! activate it and move in.

use crate::fixtures::{plain_hub, put};
use crate::tactical::{activatable_with, activate, movable};
use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_model::content_types::POK;
use ti4_model::id::{PlayerId, SystemId};
use ti4_model::state::GameState;

fn a() -> PlayerId {
    PlayerId::new("a")
}

fn nexus() -> SystemId {
    SystemId::new(crate::seating::LOCKED_NEXUS)
}

/// A hub with the Nexus beside it, and a game where seat `a` has a cruiser in the first ring system.
fn world(token: bool) -> (GameState, Galaxy, String) {
    let hub = plain_hub();
    let mut galaxy = hub.galaxy;
    crate::seating::place_wormhole_nexus(&mut galaxy, ContentStore::embedded(), POK).unwrap();
    let mut state = crate::fixtures::game(&["a", "b"]);
    let from = hub.outer[0].clone();
    put(&mut state, &SystemId::new(&from), "cruiser", &a(), 1);
    if token {
        state
            .wormhole_tokens
            .insert("GAMMA".to_owned(), SystemId::new(&from));
    }
    crate::laws::apply_to_galaxy(&state, &mut galaxy);
    (state, galaxy, from)
}

#[test]
fn a_gamma_token_makes_the_locked_nexus_adjacent_both_ways() {
    let (_, galaxy, from) = world(true);
    assert!(galaxy.are_adjacent(&from, "82a"), "token system -> Nexus");
    assert!(galaxy.are_adjacent("82a", &from), "Nexus -> token system");
}

#[test]
fn without_a_gamma_source_the_nexus_is_unreachable() {
    let (state, galaxy, from) = world(false);
    assert!(!galaxy.are_adjacent(&from, "82a"));
    let mut state = state;
    activate(&mut state, &a(), &nexus()).unwrap();
    assert!(
        movable(&state, ContentStore::embedded(), POK, &galaxy, &a()).is_empty(),
        "no route to the Nexus without gamma"
    );
}

#[test]
fn the_nexus_is_offered_for_activation_and_a_ship_can_move_in() {
    let (mut state, galaxy, from) = world(true);
    let offered = activatable_with(&state, ContentStore::embedded(), POK, &galaxy, &a());
    assert!(
        offered.contains(&nexus()),
        "82a must be an activation target: {offered:?}"
    );
    activate(&mut state, &a(), &nexus()).unwrap();
    let found = movable(&state, ContentStore::embedded(), POK, &galaxy, &a());
    assert_eq!(found.len(), 1, "the cruiser can reach the Nexus: {found:?}");
    assert_eq!(found[0].origin.as_str(), from);
}

#[test]
fn below_rules_version_23_the_nexus_is_still_not_offered() {
    let (mut state, galaxy, _) = world(true);
    state.rules_version = ti4_model::state::RULES_NEXUS_ACTIVATION - 1;
    let offered = activatable_with(&state, ContentStore::embedded(), POK, &galaxy, &a());
    assert!(!offered.contains(&nexus()), "old games behave as before");
}

#[test]
fn a_ship_in_the_nexus_opens_it_and_leaves_through_gamma_and_alpha_beta() {
    let (mut state, mut galaxy, from) = world(true);
    // The cruiser arrives: placed in the Nexus (as the move does), which is what opens it.
    let at = nexus();
    state.system_mut(&SystemId::new(&from)).units.clear();
    put(&mut state, &at, "cruiser", &a(), 1);
    assert!(crate::seating::nexus_is_triggered(&state), "a unit in it opens it");
    state.nexus_unlocked = true;
    crate::laws::apply_to_galaxy(&state, &mut galaxy);
    let kinds = galaxy.wormhole_kinds("82a");
    assert!(kinds.contains("GAMMA") && kinds.contains("ALPHA") && kinds.contains("BETA"));
    // Out again: the token system is adjacent from the Nexus side, and a ship there can activate it.
    assert!(galaxy.are_adjacent("82a", &from));
    let back = SystemId::new(&from);
    activate(&mut state, &a(), &back).unwrap();
    let found = movable(&state, ContentStore::embedded(), POK, &galaxy, &a());
    assert_eq!(found.len(), 1, "the cruiser leaves the Nexus: {found:?}");
    assert_eq!(found[0].origin, at);
}

#[test]
fn a_gamma_token_that_moves_updates_adjacency() {
    let (mut state, mut galaxy, from) = world(true);
    let other = plain_hub().outer[2].clone();
    state
        .wormhole_tokens
        .insert("GAMMA".to_owned(), SystemId::new(&other));
    crate::laws::apply_to_galaxy(&state, &mut galaxy);
    assert!(!galaxy.are_adjacent(&from, "82a"), "the old holder is cut off");
    assert!(galaxy.are_adjacent(&other, "82a"));
}
