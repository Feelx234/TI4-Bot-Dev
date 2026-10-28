//! The offline dual of the typed movement hooks and rules builder (package A2): for one position, the
//! rules `core_adapter::SeatReach` builds from the typed state and the hook table next to the
//! translation of the string `MovementRules` (`with_laws` + `apply_movement_effects`), every seat,
//! destination and card setting; and the per-ship hooks (move bonus, free cargo, activation bars)
//! next to the string dispatch. Used by `core_equivalence` and the faction tests.

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine_core::state::Piece;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlayerId, SystemId};
use ti4_model::state::GameState;

use crate::core_adapter::{self as adapter, Position, SeatReach, rules_from_legacy};
use crate::factions::hooks_movement as string_hooks;
use crate::movement::{Board, MovementRules};

/// What was compared.
#[derive(Debug, Default, Clone)]
pub struct ReachCheck {
    pub positions: u64,
    pub unmapped: u64,
    pub rules: u64,
    pub ship_values: u64,
    pub unit_hooks: u64,
    pub mismatches: Vec<String>,
}

impl ReachCheck {
    pub fn add(&mut self, o: &Self) {
        self.positions += o.positions;
        self.unmapped += o.unmapped;
        self.rules += o.rules;
        self.ship_values += o.ship_values;
        self.unit_hooks += o.unit_hooks;
        for m in &o.mismatches {
            self.miss(m.clone());
        }
    }

    fn miss(&mut self, m: String) {
        if self.mismatches.len() < 60 {
            self.mismatches.push(m);
        }
    }
}

/// Compare everything on one position.
pub fn compare(label: &str, state: &GameState, galaxy: &Galaxy, sources: SourceSet, out: &mut ReachCheck) {
    let content = ContentStore::embedded();
    out.positions += 1;
    let Some(pos) = Position::new(state, sources, galaxy) else {
        out.unmapped += 1;
        return;
    };
    for player in &state.players {
        rules(label, state, galaxy, sources, &pos, &player.id, out);
        ships(label, state, content, sources, &pos, &player.id, out);
    }
    activation_bars(label, state, content, sources, galaxy, &pos, out);
}

fn rules(
    label: &str,
    state: &GameState,
    galaxy: &Galaxy,
    sources: SourceSet,
    pos: &std::rc::Rc<Position>,
    player: &PlayerId,
    out: &mut ReachCheck,
) {
    let content = ContentStore::embedded();
    for effects in [false, true] {
        let Some(reach) = SeatReach::new(pos, player, effects) else {
            out.unmapped += 1;
            continue;
        };
        for dest in galaxy.system_ids_holding_things() {
            let Some(to) = adapter::system_ix(dest) else { continue };
            let board = Board::for_player(state, content, sources, player);
            let mut legacy = MovementRules::with_laws(galaxy, content, sources, dest, board, Some(state));
            if effects {
                crate::action_cards::apply_movement_effects(&mut legacy, state, player);
            }
            let Some(adjacency) = adapter::adjacency_for(legacy.parts().galaxy) else {
                out.unmapped += 1;
                continue;
            };
            let Ok(want) = rules_from_legacy(&legacy, sources, &adjacency) else {
                out.unmapped += 1;
                continue;
            };
            out.rules += 1;
            let got = reach.rules_for(to);
            if want != *got {
                out.miss(format!("{label}: rules of {player} to {dest} (effects {effects}) differ: {}", first_difference(&want, &got)));
            }
        }
    }
}

/// Which field of two rule sets differs first (for the report).
fn first_difference(a: &ti4_engine_core::movement::CoreRules, b: &ti4_engine_core::movement::CoreRules) -> String {
    macro_rules! fields {
        ($($f:ident),*) => {$(
            if a.$f != b.$f {
                return format!("{} string {:?} vs typed {:?}", stringify!($f), a.$f, b.$f);
            }
        )*};
    }
    fields!(
        scope, active, enemy_ships, own_command_tokens, nebulae_open, asteroid_fields_open, supernovae_open,
        supernovae_pass_through, command_tokens_ignored, anomalies_ignored, ignore_enemy_ships_from,
        ignore_enemy_ships, also_adjacent, extra_adjacency, token_wormhole_systems, barred_transit,
        gravity_rift_systems, rift_extra_steps, rifts_ignored, rift_roll_exempt, passing_ship_types,
        own_passage_types, own_unit_systems, ship_adjacent, token_free_types, nebulae_ignored, blocked_edges,
        unladen_pass
    );
    if a.adjacency != b.adjacency {
        return "adjacency".to_owned();
    }
    "unknown".to_owned()
}

/// The move value of every ship with every boost, and the fleet's answers, against the string
/// `effective_move_value_for_ship`.
fn ships(
    label: &str,
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    pos: &std::rc::Rc<Position>,
    player: &PlayerId,
    out: &mut ReachCheck,
) {
    let Some(seat) = pos.seat(player) else { return };
    let fleet = ti4_engine_core::query::reach::SeatFleet::new(pos.core(), pos.kernel(), seat);
    let types = ti4_content::units::catalogue(content, sources);
    for origin in state.systems_with_units_of(player) {
        let Some(from) = adapter::system_ix(origin.as_str()) else { continue };
        for (index, hull) in state.ships_of(player, origin).into_iter().enumerate() {
            let Some(kind) = types.get(hull.type_id.as_str()) else { continue };
            if !kind.moves_as_ship() {
                continue;
            }
            for boosts in [(false, false), (true, false), (false, true), (true, true)] {
                let want = crate::tactical::effective_move_value_for_ship(
                    state, kind, player, origin, Some(index), boosts.0, boosts.1,
                );
                let got = fleet.move_value(from, index as u16, boosts);
                out.ship_values += 1;
                if got != Some(want) {
                    out.miss(format!(
                        "{label}: move value of {player}'s {} #{index} in {origin} {boosts:?}: string {want} vs typed {got:?}",
                        hull.type_id
                    ));
                }
                // The same ship without its index (Winnu's Imperator and the Saar agent need it).
                let want = crate::tactical::effective_move_value_with_boosts(state, kind, player, origin, boosts.0, boosts.1);
                if let Some(unit) = adapter::unit_ix(hull.type_id.as_str()) {
                    let got = ti4_engine_core::query::reach::effective_move_value(
                        pos.core(), pos.kernel(), seat, from, unit, boosts,
                    );
                    out.ship_values += 1;
                    if got != Some(want) {
                        out.miss(format!(
                            "{label}: move value (no index) of {player}'s {} in {origin} {boosts:?}: string {want} vs typed {got:?}",
                            hull.type_id
                        ));
                    }
                }
            }
        }
    }
    // Free cargo, unit by unit.
    for system in state.board.values() {
        for unit in system.units.iter().chain(system.planet_units.values().flatten()) {
            let (Some(kind), Some(owner)) = (adapter::unit_ix(unit.type_id.as_str()), pos.seat(&unit.owner)) else {
                continue;
            };
            let want = string_hooks::free_cargo(state, content, sources, unit);
            let got = pos.kernel().hooks.movement.free_cargo(pos.core(), pos.kernel(), Piece::new(kind, owner));
            out.unit_hooks += 1;
            if want != got {
                out.miss(format!("{label}: free_cargo {} of {}: string {want} vs typed {got}", unit.type_id, unit.owner));
            }
        }
    }
}

/// `cannot_activate` for every seat and every system on the map.
fn activation_bars(
    label: &str,
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: &Galaxy,
    pos: &std::rc::Rc<Position>,
    out: &mut ReachCheck,
) {
    for player in &state.players {
        let Some(seat) = pos.seat(&player.id) else { continue };
        for system in galaxy.system_ids_holding_things() {
            let Some(ix) = adapter::system_ix(system) else { continue };
            let want = string_hooks::cannot_activate(state, content, sources, &player.id, &SystemId::new(system));
            let got = pos.kernel().hooks.movement.cannot_activate(pos.core(), pos.kernel(), seat, ix);
            out.unit_hooks += 1;
            if want != got {
                out.miss(format!("{label}: cannot_activate {} {system}: string {want} vs typed {got}", player.id));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Each faction's movement hooks on a constructed state: the typed table and the string hooks must
    //! build the same rules, move values and bars, and the hook must visibly fire (a test that compares
    //! two empty answers proves nothing).

    use ti4_model::content_types::DEFAULT;
    use ti4_model::id::{BreakthroughId, PlanetId, PlayerId, SystemId, TechnologyId};

    use super::*;
    use crate::fixtures::{Hub, a_system_where, hub_with_centre, plain_hub, put, put_on_planet, seated_game};

    fn pid(id: &str) -> PlayerId {
        PlayerId::new(id)
    }
    fn sid(id: &str) -> SystemId {
        SystemId::new(id)
    }

    /// `a`, `b`, `c` seated with these factions, an empty board.
    fn table(factions: [&str; 3]) -> GameState {
        let mut state = seated_game(&[("a", factions[0]), ("b", factions[1]), ("c", factions[2])], DEFAULT);
        state.board.clear();
        state
    }

    fn check(state: &GameState, hub: &Hub) -> ReachCheck {
        let mut out = ReachCheck::default();
        compare("test", state, &hub.galaxy, DEFAULT, &mut out);
        assert!(out.mismatches.is_empty(), "{:#?}", out.mismatches);
        assert_eq!(out.unmapped, 0, "the typed state held everything");
        assert!(out.rules > 0);
        out
    }

    /// The typed rules of `player` into `dest`, with the cards in play.
    fn typed(state: &GameState, hub: &Hub, player: &str, dest: &str) -> ti4_engine_core::movement::CoreRules {
        let pos = Position::new(state, DEFAULT, &hub.galaxy).expect("mapped");
        let reach = SeatReach::new(&pos, &pid(player), true).expect("a map");
        (*reach.rules_for(adapter::system_ix(dest).expect("a system"))).clone()
    }

    fn holds(state: &mut GameState, player: &str, tech: &str) {
        state.player_mut(&pid(player)).expect("seat").technologies.insert(TechnologyId::new(tech));
    }

    #[test]
    fn the_empty_table_builds_plain_rules() {
        let mut state = table(["sol", "hacan", "jolnar"]);
        let hub = plain_hub();
        put(&mut state, &sid(&hub.outer[0]), "cruiser", &pid("a"), 1);
        put(&mut state, &sid(&hub.centre), "cruiser", &pid("b"), 1);
        check(&state, &hub);
    }

    #[test]
    fn mentak_corsair_passes_only_when_the_active_system_holds_foreign_non_fighters() {
        let mut state = table(["mentak", "sol", "hacan"]);
        let hub = plain_hub();
        let (origin, target) = (hub.outer[0].clone(), hub.across(&hub.outer[0]));
        put(&mut state, &sid(&origin), "mentak_cruiser3", &pid("a"), 1);
        put(&mut state, &sid(&hub.centre), "cruiser", &pid("b"), 1);
        put(&mut state, &sid(&target), "fighter", &pid("b"), 1);
        check(&state, &hub);
        assert!(typed(&state, &hub, "a", &target).passing_ship_types.is_empty(), "fighters do not count");
        put(&mut state, &sid(&target), "destroyer", &pid("b"), 1);
        check(&state, &hub);
        assert!(!typed(&state, &hub, "a", &target).passing_ship_types.is_empty());
    }

    #[test]
    fn yssaril_flagship_and_a_nekro_copy_pass_everywhere() {
        let mut state = table(["yssaril", "sol", "hacan"]);
        let hub = plain_hub();
        put(&mut state, &sid(&hub.outer[0]), "yssaril_flagship", &pid("a"), 1);
        put(&mut state, &sid(&hub.centre), "cruiser", &pid("b"), 1);
        check(&state, &hub);
        assert!(!typed(&state, &hub, "a", &hub.across(&hub.outer[0])).passing_ship_types.is_empty());
        let mut nekro = crate::fixtures::nekro_with_z(&[("a", "nekro"), ("b", "sol"), ("c", "hacan")], &["yssaril"]);
        nekro.board.clear();
        put(&mut nekro, &sid(&hub.outer[0]), "nekro_flagship", &pid("a"), 1);
        put(&mut nekro, &sid(&hub.centre), "cruiser", &pid("b"), 1);
        check(&nekro, &hub);
        assert!(!typed(&nekro, &hub, "a", &hub.across(&hub.outer[0])).passing_ship_types.is_empty());
    }

    #[test]
    fn deepwrought_flagship_earns_steps_through_its_own_systems() {
        let mut state = table(["deepwrought", "sol", "hacan"]);
        let hub = plain_hub();
        put(&mut state, &sid(&hub.outer[0]), "deepwrought_flagship", &pid("a"), 1);
        put(&mut state, &sid(&hub.centre), "cruiser", &pid("a"), 1);
        put(&mut state, &sid(&hub.centre), "cruiser", &pid("b"), 1);
        check(&state, &hub);
        let rules = typed(&state, &hub, "a", &hub.across(&hub.outer[0]));
        assert!(!rules.own_passage_types.is_empty() && !rules.own_unit_systems.is_empty());
    }

    #[test]
    fn muaat_ships_pass_and_with_the_reactor_enter_supernovae() {
        let mut state = table(["muaat", "sol", "hacan"]);
        let hub = hub_with_centre(&a_system_where("supernova"));
        put(&mut state, &sid(&hub.outer[0]), "cruiser", &pid("a"), 1);
        check(&state, &hub);
        let rules = typed(&state, &hub, "a", &hub.centre);
        assert!(rules.supernovae_pass_through && !rules.supernovae_open);
        holds(&mut state, "a", "mr");
        check(&state, &hub);
        assert!(typed(&state, &hub, "a", &hub.centre).supernovae_open);
    }

    #[test]
    fn argent_hololattice_bars_passage_and_the_sentinel_rides_free() {
        let mut state = table(["argent", "sol", "hacan"]);
        let hub = plain_hub();
        holds(&mut state, "a", "ah");
        put(&mut state, &sid(&hub.centre), "pds", &pid("a"), 1);
        put(&mut state, &sid(&hub.outer[0]), "argent_mech", &pid("a"), 1);
        put(&mut state, &sid(&hub.outer[1]), "cruiser", &pid("b"), 1);
        check(&state, &hub);
        let rules = typed(&state, &hub, "b", &hub.outer[0]);
        assert!(rules.barred_transit.contains(adapter::system_ix(&hub.centre).expect("system").index()));
    }

    #[test]
    fn ralnel_structures_ride_free_and_saar_bars_asteroid_activation() {
        let mut state = table(["ralnel", "saar", "hacan"]);
        let field = a_system_where("asteroid field");
        let hub = hub_with_centre(&field);
        put(&mut state, &sid(&hub.outer[0]), "spacedock", &pid("a"), 1);
        holds(&mut state, "b", "cm");
        put(&mut state, &sid(&field), "cruiser", &pid("b"), 1);
        check(&state, &hub);
    }

    #[test]
    fn nomad_memoria_reaches_mech_systems_and_the_hero_frees_the_flagship() {
        let mut state = table(["nomad", "sol", "hacan"]);
        let hub = plain_hub();
        put(&mut state, &sid(&hub.outer[0]), "nomad_flagship", &pid("a"), 1);
        let (planet_system, planet) = crate::fixtures::a_placed_planet();
        put_on_planet(&mut state, &planet_system, &planet, "nomad_mech", &pid("a"), 1);
        check(&state, &hub);
        state.faction_marks.insert("nomad:hero:a".into(), state.round.to_string());
        state.system_mut(&sid(&hub.outer[0])).command_tokens.insert(pid("a"));
        check(&state, &hub);
        let rules = typed(&state, &hub, "a", &hub.outer[3]);
        assert!(!rules.ship_adjacent.is_empty() && !rules.token_free_types.is_empty());
    }

    #[test]
    fn cabal_tears_make_rifts_and_the_crucible_adds_a_step() {
        let mut state = table(["cabal", "sol", "hacan"]);
        let hub = plain_hub();
        put(&mut state, &sid(&hub.outer[2]), "cabal_spacedock", &pid("a"), 1);
        put(&mut state, &sid(&hub.outer[0]), "carrier", &pid("a"), 1);
        check(&state, &hub);
        let rules = typed(&state, &hub, "a", &hub.across(&hub.outer[0]));
        assert!(rules.gravity_rift_systems.contains(adapter::system_ix(&hub.outer[2]).expect("system").index()));
        assert!(rules.rift_roll_exempt.contains(adapter::system_ix(&hub.outer[2]).expect("system").index()));
        state.faction_marks.insert("cabal:crucible:a".into(), state.activation_seq.to_string());
        check(&state, &hub);
        assert_eq!(typed(&state, &hub, "a", &hub.outer[1]).rift_extra_steps, 1);
    }

    #[test]
    fn crimson_sunders_wormholes_severs_systems_and_links_breaches() {
        let mut state = table(["crimson", "sol", "hacan"]);
        let hub = plain_hub();
        put(&mut state, &sid(&hub.outer[0]), "cruiser", &pid("a"), 1);
        let (b1, b2) = (hub.outer[1].clone(), hub.outer[3].clone());
        state.breach_tokens.insert(sid(&b1));
        state.breach_tokens.insert(sid(&b2));
        state.faction_marks.insert(format!("crimson:breach:active:{b1}"), "1".into());
        state.faction_marks.insert(format!("crimson:breach:active:{b2}"), "1".into());
        state.faction_marks.insert("crimson:sever:sever:crimson".into(), format!("a|{}", hub.outer[4]));
        check(&state, &hub);
        let rules = typed(&state, &hub, "a", &hub.centre);
        assert!(!rules.extra_adjacency.is_empty());
    }

    #[test]
    fn empyrean_ignores_nebulae_and_the_marks_open_passage_and_block_borders() {
        let mut state = table(["empyrean", "sol", "hacan"]);
        let hub = hub_with_centre(&a_system_where("nebula"));
        put(&mut state, &sid(&hub.outer[0]), "cruiser", &pid("a"), 1);
        put(&mut state, &sid(&hub.outer[0]), "cruiser", &pid("b"), 1);
        put(&mut state, &sid(&hub.outer[2]), "cruiser", &pid("b"), 1);
        let seq = state.activation_seq;
        state.faction_marks.insert("empyrean|passage".into(), format!("{seq}|c|b"));
        state.faction_marks.insert("empyrean|aetherstream".into(), format!("{seq}|b"));
        state.player_mut(&pid("a")).expect("seat").breakthrough = Some(BreakthroughId::new("empyreanbt"));
        state.faction_marks.insert("empyrean|tether|a".into(), format!("{}|{}", hub.outer[0], hub.outer[1]));
        check(&state, &hub);
        assert!(typed(&state, &hub, "a", &hub.outer[3]).nebulae_ignored);
        assert!(!typed(&state, &hub, "b", &hub.outer[3]).blocked_edges.is_empty());
        state.faction_marks.insert("empyrean|tether_allow".into(), format!("{seq}|b|a"));
        check(&state, &hub);
    }

    #[test]
    fn ghost_flagship_carries_a_delta_wormhole_and_the_agent_links_wormholes() {
        let mut state = table(["ghost", "sol", "hacan"]);
        let hub = plain_hub();
        put(&mut state, &sid(&hub.outer[0]), "ghost_flagship", &pid("a"), 1);
        put(&mut state, &sid(&hub.outer[3]), "carrier", &pid("b"), 1);
        check(&state, &hub);
        state.active_system = Some(sid(&hub.outer[2]));
        state.faction_marks.insert("ghost:agent_link".into(), format!("{}|{}", state.activation_seq, hub.outer[2]));
        check(&state, &hub);
        assert!(!typed(&state, &hub, "b", &hub.outer[3]).extra_adjacency.is_empty());
    }

    #[test]
    fn winnu_gate_folding_links_mecatol_to_wormholes() {
        let mut state = table(["winnu", "sol", "hacan"]);
        let wormhole = ["25", "26", "39", "40"].map(str::to_owned);
        let mut ids = vec!["18".to_owned()];
        ids.extend(wormhole.iter().cloned());
        ids.extend(crate::fixtures::plain_systems(2));
        let hub = crate::fixtures::hub_from(&ids);
        holds(&mut state, "a", "lgf");
        state.active = Some(pid("a"));
        state.active_system = Some(sid(&hub.outer[0]));
        put(&mut state, &sid(&hub.outer[4]), "cruiser", &pid("a"), 1);
        check(&state, &hub);
        assert!(!typed(&state, &hub, "a", &hub.outer[1]).extra_adjacency.is_empty());
    }

    #[test]
    fn firmament_agent_and_the_saar_agent_adjust_moves_and_passing() {
        let mut state = table(["firmament", "saar", "hacan"]);
        let hub = plain_hub();
        put(&mut state, &sid(&hub.outer[0]), "firmament_flagship", &pid("a"), 1);
        put(&mut state, &sid(&hub.outer[0]), "carrier", &pid("b"), 2);
        let seq = state.activation_seq;
        state.faction_marks.insert("firmament:agent".into(), format!("{seq}|a"));
        state.faction_marks.insert("saar:agent:boost:b".into(), format!("{seq}|{}|carrier|0|2|1", hub.outer[0]));
        check(&state, &hub);
        assert!(typed(&state, &hub, "a", &hub.outer[1]).unladen_pass);
        let pos = Position::new(&state, DEFAULT, &hub.galaxy).expect("mapped");
        let seat = pos.seat(&pid("b")).expect("seat");
        let fleet = ti4_engine_core::query::reach::SeatFleet::new(pos.core(), pos.kernel(), seat);
        let origin = adapter::system_ix(&hub.outer[0]).expect("system");
        let printed = ti4_engine_core::query::reach::effective_move_value(pos.core(), pos.kernel(), seat, origin, adapter::unit_ix("carrier").expect("unit"), (false, false));
        assert_eq!(fleet.move_value(origin, 0, (false, false)), printed.map(|v| v + 1), "the Saar agent's boost lands on the first carrier");
    }

    #[test]
    fn a_planet_with_units_is_found_by_its_system() {
        let mut state = table(["cabal", "sol", "hacan"]);
        let hub = plain_hub();
        let (system, planet) = crate::fixtures::a_placed_planet();
        put_on_planet(&mut state, &system, &planet, "cabal_spacedock", &pid("a"), 1);
        let _ = (PlanetId::new("x"), &hub);
        check(&state, &hub);
    }
}
