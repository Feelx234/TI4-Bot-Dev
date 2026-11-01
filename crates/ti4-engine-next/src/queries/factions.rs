//! The faction readers of the surface (`factions::mahact`, `factions::firmament`, `factions::crimson`, `factions::bastion_units`,
//! `fleet::limit`, `faction_abilities::fleet_supply`): the hooks of the core kernel, or a plain read of the marks the factions write.

use ti4_content::ContentStore;
use ti4_engine_core::query::fleet as core_fleet;
use ti4_engine_core::rules::abilities as core_abilities;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlayerId, SystemId};
use ti4_model::state::GameState;

use super::view::Full;

/// The owners of the other players' command tokens in `mahact`'s fleet pool, one entry per token, sorted (Edict). Public information.
#[must_use]
pub fn mahact_fleet_pool_owners(state: &GameState, mahact: &PlayerId) -> Vec<PlayerId> {
    Full::of(state)
        .and_then(|v| {
            let owners = v.kernel().hooks.direct.mahact_fleet_pool_owners(v.core(), v.seat(mahact)?);
            v.players(owners.iter().copied())
        })
        .unwrap_or_default()
}

/// How many foreign command tokens `player`'s fleet pool holds (0 unless they are the Mahact).
#[must_use]
pub fn mahact_foreign_tokens(state: &GameState, player: &PlayerId) -> i32 {
    Full::of(state).and_then(|v| Some(v.kernel().hooks.direct.mahact_foreign_tokens(v.core(), v.seat(player)?))).unwrap_or(0)
}

/// The players whose control tokens are on any of `owner`'s plot cards (the Firmament's puppets).
#[must_use]
pub fn firmament_puppeted(state: &GameState, owner: &PlayerId) -> std::collections::BTreeSet<PlayerId> {
    Full::of(state)
        .and_then(|v| {
            let seats = v.kernel().hooks.direct.firmament_puppeted(v.core(), v.seat(owner)?);
            v.player_set(seats.iter().copied())
        })
        .unwrap_or_default()
}

/// Every system holding an active breach (the Crimson Rebellion), in id order.
#[must_use]
pub fn crimson_active_breaches(state: &GameState) -> Vec<SystemId> {
    state.breach_tokens.iter().filter(|system| state.faction_marks.contains_key(&format!("crimson:breach:active:{system}"))).cloned().collect()
}

/// Non-home systems that contain a planet `player` controls. A space station is not a planet.
#[must_use]
pub fn non_home_systems_with_planets(state: &GameState, content: &ContentStore, sources: SourceSet, player: &PlayerId) -> usize {
    let homes: std::collections::BTreeSet<&SystemId> = state.players.iter().filter_map(|seat| seat.home_system.as_ref()).collect();
    state
        .board
        .iter()
        .filter(|(system, board)| {
            !homes.contains(system)
                && board.planet_control.iter().any(|(planet, owner)| owner == player && !ti4_content::galaxy::is_space_station(content, planet.as_str(), sources))
        })
        .count()
}

/// How many non-fighter ships this player may keep in one system (`fleet::limit`): the fleet pool (its tokens, the foreign ones of the
/// Mahact, capped by Fleet Regulations), then lifted by the abilities (Letnev's Armada, +2) and the faction modules.
#[must_use]
pub fn fleet_limit(state: &GameState, content: &ContentStore, player: &PlayerId) -> i32 {
    let Some(v) = Full::of(state) else {
        // A position the core cannot hold (a table with no faction seated, an id the tables do not know) has no faction module to ask: the
        // pool of the seat, capped by the law, lifted by the printed ability.
        let tokens = state.player(player).map_or(0, |seat| seat.fleet_tokens);
        let capped = super::laws::fleet_pool_cap(state, tokens);
        let armada = state
            .player(player)
            .and_then(|seat| ti4_content::factions::get(content, seat.faction.as_str()))
            .is_some_and(|faction| faction.abilities().contains(&"armada"));
        return capped + if armada { 2 } else { 0 };
    };
    let Some(seat) = v.seat(player) else { return 0 };
    let foreign = v.kernel().hooks.direct.mahact_foreign_tokens(v.core(), seat);
    let capped = super::laws::fleet_pool_cap(state, core_fleet::pool_base(v.core(), seat, foreign));
    let env = ti4_engine_core::driver::kernel_env::KernelEnv::new(v.core(), v.kernel());
    core_abilities::fleet_supply(&core_abilities::Cx { core: v.core(), env: &env }, seat, capped)
}
