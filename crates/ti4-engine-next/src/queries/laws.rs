//! The standing-law reads of the surface (copy of the text half of `laws.rs`; the typed half is the same read of the core's agenda slice,
//! equal on every position by the dual runs).

use ti4_model::state::GameState;

const BLANKED_TURN: &str = "keleres|laws_blank";

/// Whether all laws are blank right now: Law's Order was used this turn and the turn has not ended.
#[must_use]
pub fn blanked(state: &GameState) -> bool {
    state.phase == ti4_model::state::Phase::Action && state.faction_marks.get(BLANKED_TURN).is_some_and(|turn| *turn == state.turn_seq.to_string())
}

/// Whether a law is in play. Never while [`blanked`].
#[must_use]
pub fn active(state: &GameState, alias: &str) -> bool {
    !blanked(state) && state.laws.contains_key(alias)
}

/// Regulated Conscription: a fighter or infantry costs its price for one unit, not two.
#[must_use]
pub fn single_unit_production(state: &GameState) -> bool {
    active(state, "conscription")
}

/// Fleet Regulations caps the fleet pool at four.
#[must_use]
pub fn fleet_pool_cap(state: &GameState, base: i32) -> i32 {
    if active(state, "regulations") { base.min(4) } else { base }
}

/// What this law was elected onto (8.9 to 8.11). `None` while [`blanked`].
#[must_use]
pub fn elected<'a>(state: &'a GameState, alias: &str) -> Option<&'a String> {
    if blanked(state) { None } else { state.laws.get(alias) }
}

/// Enforced Travel Ban: alpha and beta wormholes have no effect during movement.
#[must_use]
pub fn wormholes_suppressed(state: &GameState) -> bool {
    active(state, "travel_ban")
}

/// Wormhole Reconstruction: every system with an alpha or beta wormhole is adjacent to every other.
#[must_use]
pub fn wormholes_all_connected(state: &GameState) -> bool {
    active(state, "wormhole_recon")
}

/// Nexus Sovereignty (For): the alpha and beta wormholes in the wormhole nexus have no effect during movement.
#[must_use]
pub fn nexus_wormholes_suppressed(state: &GameState) -> bool {
    active(state, "nexus") && elected(state, "nexus").map(String::as_str) == Some("for")
}

/// Spatial Conduit Cylinder: while the active player's activation carries the technology's marker, the active system and every system holding
/// their units are adjacent.
#[must_use]
pub fn spatial_conduit_links(state: &GameState) -> std::collections::BTreeMap<String, std::collections::BTreeSet<String>> {
    let mut links: std::collections::BTreeMap<String, std::collections::BTreeSet<String>> = std::collections::BTreeMap::new();
    let (Some(active), Some(system)) = (state.active.as_ref(), state.active_system.as_ref()) else { return links };
    if state.player(active).is_none_or(|seat| seat.spatial_conduit != Some(state.activation_seq)) {
        return links;
    }
    for (other, here) in &state.board {
        if other == system || !here.has_units_of(active) {
            continue;
        }
        links.entry(system.to_string()).or_default().insert(other.to_string());
        links.entry(other.to_string()).or_default().insert(system.to_string());
    }
    links
}

/// The wormholes faction pieces carry: the Ghosts of Creuss flagship's delta, in the system it stands in (the Nekro Virus lends it with Z-Rhapsody;
/// that lend is the string's pure reading of the state, `factions::flagship_has_text`).
#[must_use]
pub fn extra_wormholes(state: &GameState) -> std::collections::BTreeMap<String, std::collections::BTreeSet<String>> {
    let mut found: std::collections::BTreeMap<String, std::collections::BTreeSet<String>> = std::collections::BTreeMap::new();
    for (system, board) in &state.board {
        if board.units.iter().any(|unit| crate::factions::flagship_has_text(state, &unit.owner, unit.type_id.as_str(), "ghost_flagship")) {
            found.entry(system.to_string()).or_default().insert("DELTA".to_owned());
        }
    }
    found
}

/// The laws, the printed lanes and the token wormholes onto the owned map: every route query reads the galaxy, so setting it here means no
/// movement path can consult the wrong one. Rebuilt from the state every time (a token can flip).
pub fn apply_to_galaxy(state: &GameState, galaxy: &mut ti4_content::galaxy::Galaxy) {
    galaxy.hyperlane_lanes = state.rules_version >= ti4_model::state::RULES_HYPERLANE_LANES;
    galaxy.wormholes_off = wormholes_suppressed(state);
    galaxy.wormholes_all_linked = wormholes_all_connected(state);
    galaxy.wormhole_star_links = state.active.as_ref().and_then(|acting| state.player(acting)).is_some_and(|seat| seat.lost_star.contains(&state.activation_seq));
    galaxy.extra_links = spatial_conduit_links(state);
    galaxy.token_wormholes.clear();
    galaxy.nexus_wormholes_off = nexus_wormholes_suppressed(state);
    for (kind, system) in &state.wormhole_tokens {
        galaxy.token_wormholes.entry(system.to_string()).or_default().insert(kind.clone());
    }
    // The Wormhole Nexus, once flipped: alpha and beta on the locked face (expressed as tokens, as a face-changing wormhole already is).
    if state.nexus_unlocked {
        let open = galaxy.token_wormholes.entry(crate::seating::LOCKED_NEXUS.to_owned()).or_default();
        open.insert("ALPHA".to_owned());
        open.insert("BETA".to_owned());
    }
    if let Some((system, face)) = state.ion_storm.as_ref() {
        galaxy.token_wormholes.entry(system.to_string()).or_default().insert(face.clone());
    }
    for (system, kinds) in extra_wormholes(state) {
        let held = galaxy.token_wormholes.entry(system).or_default();
        held.extend(kinds);
    }
}
