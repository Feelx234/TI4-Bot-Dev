//! Lane F2b (package KM2): the options of the action menu whose bodies the core now runs (`ti4_engine_core::driver::turn::{action, component}`)
//! chosen from the menu question and run through `live::live_step` against the string's own `Game::step`: the questions as rendered, the events,
//! the position and the dice streams. The string step is the turn prelude, the menu asked, and the body of the answer.

use ti4_engine_core::driver::session::Arm;
use ti4_model::state::GameState;

use super::turn_live_tests::{action_table, assert_same, native_marked, pid, string_run_first, Run, THREE};
use crate::generated::IdStr;

/// The string's step of a position, the menu answered by `ids` (then the first option of what the body asks), against the native prelude and
/// menu steps answered the same.
fn both(state: &GameState, ids: &[&str]) -> (Run, Run) {
    let (string, taken) = string_run_first(state, ids, |g| {
        let _ = g.step();
    });
    let native = native_marked(state, &[(true, Arm::TurnPrelude, None), (false, Arm::Ask, None)], &taken.iter().map(String::as_str).collect::<Vec<_>>());
    (string, native)
}

fn has(run: &Run, event: &str) -> bool {
    run.events.iter().any(|e| e == event)
}

#[test]
fn a_strategic_action_chosen_from_the_menu_begins_like_the_string() {
    for card in ["pok5trade", "pok3politics", "pok1leadership", "pok8imperial"] {
        let mut state = action_table(&THREE);
        state.player_mut(&pid("a")).unwrap().strategy_cards.push(ti4_model::id::StrategyCardId::new(card));
        for p in ["b", "c"] {
            let seat = state.player_mut(&pid(p)).unwrap();
            seat.strategic_tokens = 2;
            seat.trade_goods = 6;
        }
        let state = super::turn_live_tests::based(state);
        let (string, native) = both(&state, &["strategic"]);
        assert_same(&string, &native);
        assert!(has(&string, "STRATEGIC_ACTION_BEGAN"), "{card}: {:?}", string.events);
    }
}

#[test]
fn an_action_card_played_as_the_action_resolves_and_ends_the_action_like_the_string() {
    use ti4_engine_core::generated::ActionCard;
    let mut state = action_table(&THREE);
    state.player_mut(&pid("a")).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new(ActionCard::EconomicInitiative.as_str())];
    let state = super::turn_live_tests::based(state);
    let (string, native) = both(&state, &["action_card|0"]);
    assert_same(&string, &native);
    assert!(has(&string, "ACTION_CARD_PLAYED") && has(&string, "COMPONENT_ACTION_RESOLVED") && has(&string, "ACTION_COMPLETED"), "{:?}", string.events);
}

#[test]
fn extreme_duress_on_the_acting_seat_punishes_a_card_action_like_the_string() {
    use ti4_engine_core::generated::ActionCard;
    let mut state = action_table(&THREE);
    let seat = state.player_mut(&pid("a")).unwrap();
    seat.action_cards = vec![ti4_model::id::ActionCardId::new(ActionCard::EconomicInitiative.as_str()), ti4_model::id::ActionCardId::new(ActionCard::ALL[0].as_str())];
    seat.trade_goods = 3;
    seat.duress_by = Some(pid("b"));
    let state = super::turn_live_tests::based(state);
    let (string, native) = both(&state, &["action_card|0"]);
    assert_same(&string, &native);
    assert!(string.events.iter().any(|e| e.starts_with("EXTREME_DURESS:")), "{:?}", string.events);
}

/// A Winnu seat that holds Mecatol Rex and a ready Lazax Gate Folding.
fn lazax_table() -> GameState {
    let mut state = action_table(&[("a", "winnu"), ("b", "sol"), ("c", "hacan")]);
    state.player_mut(&pid("a")).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("lgf"));
    let mecatol = ti4_model::id::SystemId::new(crate::seating::mecatol_on(&state));
    let planet = ti4_model::id::PlanetId::new(ti4_engine_core::generated::Planet::MecatolRex.as_str());
    state.board.entry(mecatol).or_default().set_control(planet, pid("a"));
    super::turn_live_tests::based(state)
}

#[test]
fn lazax_gate_folding_chosen_from_the_menu_places_the_infantry_like_the_string() {
    let state = lazax_table();
    let (string, native) = both(&state, &["faction|winnu|lgf"]);
    assert_same(&string, &native);
    assert!(has(&string, "COMPONENT_ACTION_RESOLVED"), "{:?}", string.events);
}

fn fragments_table(deck: &[&str]) -> GameState {
    let mut state = action_table(&THREE);
    state.player_mut(&pid("a")).unwrap().relic_fragments.insert("CULTURAL".to_owned(), 3);
    state.relic_deck = deck.iter().map(|r| ti4_model::id::RelicId::new(*r)).collect();
    super::turn_live_tests::based(state)
}

#[test]
fn purging_three_fragments_for_a_relic_chosen_from_the_menu_gains_it_like_the_string() {
    let state = fragments_table(&["codex", "shard"]);
    let (string, native) = both(&state, &["purge|CULTURAL"]);
    assert_same(&string, &native);
    assert!(has(&string, "COMPONENT_ACTION_RESOLVED"), "{:?}", string.events);
}

#[test]
fn the_shard_of_the_throne_scores_at_once_when_purged_for_like_the_string() {
    let state = fragments_table(&["shard", "codex"]);
    let (string, native) = both(&state, &["purge|CULTURAL"]);
    assert_same(&string, &native);
    assert!(has(&string, "COMPONENT_ACTION_RESOLVED"), "{:?}", string.events);
}

#[test]
fn a_purge_that_cannot_resolve_fails_and_the_menu_withholds_it_this_turn_like_the_string() {
    let state = fragments_table(&[]);
    let (string, taken) = string_run_first(&state, &["purge|CULTURAL"], |g| {
        let _ = g.step();
        let _ = g.step();
    });
    let native = native_marked(&state, &[(true, Arm::TurnPrelude, None), (false, Arm::Ask, None), (true, Arm::Ask, None)], &taken.iter().map(String::as_str).collect::<Vec<_>>());
    assert_same(&string, &native);
    assert!(has(&string, "COMPONENT_ACTION_FAILED"), "{:?}", string.events);
    let second = string.questions.last().expect("the menu again");
    assert!(!second.options.iter().any(|o| o.id.starts_with("purge|")), "the failed purge is withheld: {:?}", second.ids());
}

#[test]
fn the_trade_primary_at_a_cabal_table_stages_the_goods_before_the_replenish_like_the_string() {
    let mut state = action_table(&[("a", "sol"), ("b", "hacan"), ("c", "cabal")]);
    state.player_mut(&pid("a")).unwrap().strategy_cards.push(ti4_model::id::StrategyCardId::new("pok5trade"));
    state.player_mut(&pid("a")).unwrap().commodities = 0;
    let state = super::turn_live_tests::based(state);
    let (string, native) = both(&state, &["strategic", "done"]);
    assert_same(&string, &native);
    assert_eq!(string.staged, ["TRADE_GOODS_GAINED", "COMMODITIES_REPLENISHED"], "the string's order");
}

fn orbital_table() -> GameState {
    let mut state = action_table(&THREE);
    state.player_mut(&pid("a")).unwrap().strategic_tokens = 2;
    super::turn_live_tests::based(state)
}

#[test]
fn orbital_drop_chosen_from_the_menu_drops_the_infantry_like_the_string() {
    let state = orbital_table();
    // The planet (when the seat has more than one), then the offered mech is declined.
    let (string, taken) = string_run_first(&state, &["faction|orbital_drop"], |g| {
        let _ = g.step();
    });
    let native = native_marked(&state, &[(true, Arm::TurnPrelude, None), (false, Arm::Ask, None)], &taken.iter().map(String::as_str).collect::<Vec<_>>());
    assert_same(&string, &native);
    assert!(has(&string, "COMPONENT_ACTION_RESOLVED"), "{:?} / {:?}", string.events, taken);
}

#[test]
fn orbital_drop_at_a_table_without_a_module_announces_nothing_like_the_string() {
    // sol, letnev, xxcha, jolnar: no seated faction has a module, so the string's token event is never made.
    let mut state = action_table(&[("a", "sol"), ("b", "letnev"), ("c", "xxcha"), ("d", "jolnar")]);
    state.player_mut(&pid("a")).unwrap().strategic_tokens = 2;
    let state = super::turn_live_tests::based(state);
    let (string, taken) = string_run_first(&state, &["faction|orbital_drop"], |g| {
        let _ = g.step();
    });
    let native = native_marked(&state, &[(true, Arm::TurnPrelude, None), (false, Arm::Ask, None)], &taken.iter().map(String::as_str).collect::<Vec<_>>());
    assert_same(&string, &native);
    assert!(!has(&string, "STRATEGY_TOKEN_SPENT"), "{:?}", string.events);
}

// ---- lane F2d: the leaders, relics and faction components the core now runs, against the string ----

use ti4_model::state::LeaderStatus;

/// A table in the action phase whose seat `a` holds `leader` in `status`.
fn leader_table(seats: &[(&str, &str)], leader: &str, status: LeaderStatus) -> GameState {
    let mut state = action_table(seats);
    state.player_mut(&pid("a")).unwrap().leaders.insert(ti4_model::id::LeaderId::new(leader), status);
    super::turn_live_tests::based(state)
}

fn lockstep(state: &GameState, ids: &[&str], resolved: &str) -> Run {
    let (string, native) = both(state, ids);
    assert_same(&string, &native);
    assert!(has(&string, resolved), "{resolved} expected: {:?}", string.events);
    string
}

#[test]
fn jace_x_chosen_from_the_menu_returns_the_command_tokens_and_is_purged_like_the_string() {
    let mut state = leader_table(&THREE, "solhero", LeaderStatus::Unlocked);
    let system = ti4_model::id::SystemId::new(crate::seating::mecatol_on(&state));
    state.board.entry(system).or_default().command_tokens.insert(pid("a"));
    let state = super::turn_live_tests::based(state);
    lockstep(&state, &["component|leader|solhero"], "COMPONENT_ACTION_RESOLVED");
}

#[test]
fn darktalon_treilla_chosen_from_the_menu_lifts_the_fleet_supply_like_the_string() {
    let state = leader_table(&[("a", "letnev"), ("b", "sol"), ("c", "hacan")], "letnevhero", LeaderStatus::Unlocked);
    lockstep(&state, &["component|leader|letnevhero"], "COMPONENT_ACTION_RESOLVED");
}

/// Only seat `b` can pay (the others have no card and no token), so the payer is not asked: the payment is (first option: the discard), then the card.
#[test]
fn nekro_malleon_chosen_from_the_menu_takes_a_discard_for_two_goods_like_the_string() {
    let mut state = action_table(&[("a", "nekro"), ("b", "sol"), ("c", "hacan")]);
    for who in ["a", "c"] {
        let seat = state.player_mut(&pid(who)).unwrap();
        seat.action_cards.clear();
        (seat.tactic_tokens, seat.fleet_tokens, seat.strategic_tokens) = (0, 0, 0);
    }
    let b = state.player_mut(&pid("b")).unwrap();
    b.action_cards = vec![ti4_model::id::ActionCardId::new(ti4_engine_core::generated::ActionCard::ALL[0].as_str()), ti4_model::id::ActionCardId::new(ti4_engine_core::generated::ActionCard::ALL[3].as_str())];
    state.player_mut(&pid("a")).unwrap().leaders.insert(ti4_model::id::LeaderId::new("nekroagent"), LeaderStatus::Readied);
    let state = super::turn_live_tests::based(state);
    lockstep(&state, &["component|leader|nekroagent"], "LEADER_ABILITY_RESOLVED");
}

fn relic_table(relic: &str) -> GameState {
    let mut state = action_table(&THREE);
    state.player_mut(&pid("a")).unwrap().relics = vec![ti4_model::id::RelicId::new(relic)];
    super::turn_live_tests::based(state)
}

#[test]
fn the_dynamis_core_and_the_book_of_latvinia_chosen_from_the_menu_resolve_like_the_string() {
    for relic in ["dynamiscore", "bookoflatvinia"] {
        let state = relic_table(relic);
        lockstep(&state, &[&format!("relic|{relic}")], "COMPONENT_ACTION_RESOLVED");
    }
}

#[test]
fn the_codex_chosen_from_the_menu_takes_the_discarded_cards_like_the_string() {
    let mut state = relic_table("codex");
    state.discarded_action_cards = vec![ti4_model::id::ActionCardId::new(ti4_engine_core::generated::ActionCard::ALL[0].as_str()), ti4_model::id::ActionCardId::new(ti4_engine_core::generated::ActionCard::ALL[2].as_str())];
    lockstep(&state, &["relic|codex"], "COMPONENT_ACTION_RESOLVED");
}

#[test]
fn an_action_note_chosen_from_the_menu_goes_faceup_like_the_string() {
    for (note, id, table) in [
        ("convoys:hacan", "faction|trade_convoys", [("a", "sol"), ("b", "hacan"), ("c", "arborec")]),
        ("blood_pact:empyrean", "faction|empyrean|blood_pact", [("a", "sol"), ("b", "empyrean"), ("c", "arborec")]),
        ("dark_pact:empyrean", "faction|empyrean|dark_pact", [("a", "sol"), ("b", "empyrean"), ("c", "arborec")]),
        ("pop:mentak", "faction|mentak|pop", [("a", "sol"), ("b", "mentak"), ("c", "arborec")]),
    ] {
        let mut state = action_table(&table);
        state.promissory_notes.insert(note.to_owned(), pid("a"));
        let state = super::turn_live_tests::based(state);
        lockstep(&state, &[id], "COMPONENT_ACTION_RESOLVED");
    }
}

#[test]
fn production_biomes_chosen_from_the_menu_pays_four_and_two_like_the_string() {
    let mut state = action_table(&[("a", "hacan"), ("b", "sol"), ("c", "arborec")]);
    let seat = state.player_mut(&pid("a")).unwrap();
    seat.technologies.insert(ti4_model::id::TechnologyId::new(ti4_engine_core::generated::Technology::ProductionBiomes.as_str()));
    seat.strategic_tokens = 2;
    let state = super::turn_live_tests::based(state);
    lockstep(&state, &["faction|production_biomes"], "COMPONENT_ACTION_RESOLVED");
}

#[test]
fn stall_tactics_chosen_from_the_menu_discards_a_card_like_the_string() {
    let mut state = action_table(&[("a", "yssaril"), ("b", "sol"), ("c", "hacan")]);
    state.player_mut(&pid("a")).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new(ti4_engine_core::generated::ActionCard::ALL[0].as_str()), ti4_model::id::ActionCardId::new(ti4_engine_core::generated::ActionCard::ALL[3].as_str())];
    let state = super::turn_live_tests::based(state);
    lockstep(&state, &["faction|yssaril|stall_tactics"], "COMPONENT_ACTION_RESOLVED");
}

/// KM2 section 8: an action card played as the action and cancelled by a Sabotage fails without withholding (22.4) and leaves the turn open; the
/// card is gone either way (22.3).
#[test]
fn an_action_card_cancelled_by_sabotage_fails_and_leaves_the_turn_open_like_the_string() {
    let mut state = action_table(&THREE);
    state.player_mut(&pid("a")).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new("industrial_initiative")];
    state.player_mut(&pid("b")).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new("sabo1")];
    let state = super::turn_live_tests::based(state);
    let (string, native) = both(&state, &["action_card|0"]);
    assert_same(&string, &native);
    assert!(has(&string, "COMPONENT_ACTION_FAILED") && !has(&string, "ACTION_COMPLETED"), "the play was cancelled: {:?}", string.events);
}

/// Carth at a table with a Cabal: the replenish is announced (`COMMODITIES_REPLENISHED`) before `LEADER_ABILITY_RESOLVED`, as the string says, and the
/// string's Cabal agent reaction to that event is offered by the native step too (integration 13: the reaction is a rule of the table since lane G/H1, so
/// this is a full lockstep now; it was "the native step does not open it yet" in KM4 section 3).
#[test]
fn carth_replenishing_a_player_at_a_cabal_table_announces_it_before_the_ability_resolves_like_the_string() {
    let mut state = action_table(&[("a", "hacan"), ("b", "sol"), ("c", "cabal")]);
    state.player_mut(&pid("b")).unwrap().commodities = 0;
    state.player_mut(&pid("a")).unwrap().leaders.insert(ti4_model::id::LeaderId::new("hacanagent"), LeaderStatus::Readied);
    let state = super::turn_live_tests::based(state);
    let (string, native) = both(&state, &["component|leader|hacanagent", "b", "decline"]);
    let order = |run: &Run| run.events.iter().filter(|e| matches!(e.as_str(), "COMMODITIES_REPLENISHED" | "LEADER_ABILITY_RESOLVED")).cloned().collect::<Vec<_>>();
    assert_eq!(order(&native), ["COMMODITIES_REPLENISHED", "LEADER_ABILITY_RESOLVED"], "{:?}", native.events);
    assert_eq!(order(&native), order(&string), "the string's order");
    assert_same(&string, &native);
}

#[test]
fn mageon_implants_chosen_from_the_menu_takes_a_card_like_the_string() {
    let mut state = action_table(&[("a", "yssaril"), ("b", "sol"), ("c", "hacan")]);
    state.player_mut(&pid("a")).unwrap().technologies.insert(ti4_model::id::TechnologyId::new(ti4_engine_core::generated::Technology::MageonImplants.as_str()));
    state.player_mut(&pid("b")).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new(ti4_engine_core::generated::ActionCard::ALL[0].as_str()), ti4_model::id::ActionCardId::new(ti4_engine_core::generated::ActionCard::ALL[3].as_str())];
    let state = super::turn_live_tests::based(state);
    lockstep(&state, &["faction|yssaril|mi|b"], "COMPONENT_ACTION_RESOLVED");
}

// (Ggrocuto Rinn reads the map's adjacency: this file's harness loads no map. The route runs in the shadow of `native_play`, and `tests/action_dispatch.rs`
// of the core tests runs it on a map.)

// ---- lane F2e: the production leaders against the string ----

fn home_of(state: &GameState, who: &str) -> ti4_model::id::SystemId {
    state.player(&pid(who)).unwrap().home_system.clone().expect("a seated home system")
}

/// Umbat: the holder names a player (the seat itself, the only one with a war sun), who produces in its only system.
#[test]
fn umbat_chosen_from_the_menu_lets_the_named_player_produce_like_the_string() {
    let mut state = leader_table(&[("a", "muaat"), ("b", "sol"), ("c", "hacan")], "muaatagent", LeaderStatus::Readied);
    let home = home_of(&state, "a");
    crate::fixtures::put(&mut state, &home, "warsun", &pid("a"), 1);
    let state = super::turn_live_tests::based(state);
    lockstep(&state, &["component|leader|muaatagent", "a"], "COMPONENT_ACTION_RESOLVED");
}

/// Umbat for another player: the production is that player's, the announcement names them.
#[test]
fn umbat_for_another_player_announces_the_use_of_that_player_like_the_string() {
    let mut state = leader_table(&[("a", "muaat"), ("b", "sol"), ("c", "hacan")], "muaatagent", LeaderStatus::Readied);
    let home = home_of(&state, "b");
    crate::fixtures::put(&mut state, &home, "warsun", &pid("b"), 1);
    let state = super::turn_live_tests::based(state);
    lockstep(&state, &["component|leader|muaatagent", "b"], "COMPONENT_ACTION_RESOLVED");
}

/// Letani Miasmiala: a use of PRODUCTION in every system holding a ground force of the seat, and the hero is purged.
#[test]
fn letani_miasmiala_chosen_from_the_menu_produces_where_the_ground_forces_stand_like_the_string() {
    let state = leader_table(&[("a", "arborec"), ("b", "sol"), ("c", "hacan")], "arborechero", LeaderStatus::Unlocked);
    lockstep(&state, &["component|leader|arborechero"], "COMPONENT_ACTION_RESOLVED");
}

// ---- lane F2e: the relics with a question of their own, and Rin, against the string ----

#[test]
fn the_silver_flame_chosen_from_the_menu_rolls_the_same_die_like_the_string() {
    let state = relic_table("thesilverflame");
    lockstep(&state, &["relic|thesilverflame"], "COMPONENT_ACTION_RESOLVED");
}

#[test]
fn the_maw_of_worlds_chosen_from_the_menu_exhausts_the_planets_and_gains_a_technology_like_the_string() {
    let state = relic_table("mawofworlds");
    lockstep(&state, &["relic|mawofworlds"], "COMPONENT_ACTION_RESOLVED");
}

#[test]
fn the_enigmatic_device_relic_chosen_from_the_menu_pays_six_and_researches_like_the_string() {
    let mut state = relic_table("enigmaticdevice");
    state.player_mut(&pid("a")).unwrap().trade_goods = 8;
    lockstep(&state, &["relic|enigmaticdevice"], "COMPONENT_ACTION_RESOLVED");
}

#[test]
fn the_titan_prototype_chosen_from_the_menu_lets_the_named_player_build_like_the_string() {
    let state = relic_table("titanprototype");
    lockstep(&state, &["relic|titanprototype", "b"], "COMPONENT_ACTION_RESOLVED");
}

#[test]
fn rin_chosen_from_the_menu_swaps_a_technology_like_the_string() {
    let mut state = leader_table(&[("a", "jolnar"), ("b", "sol"), ("c", "hacan")], "jolnarhero", LeaderStatus::Unlocked);
    state.player_mut(&pid("a")).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("amd"));
    let state = super::turn_live_tests::based(state);
    lockstep(&state, &["component|leader|jolnarhero"], "COMPONENT_ACTION_RESOLVED");
}

// ---- lane F2e: the heroes that read the position for their facts, against the string ----

/// A planet with a technology specialty, in a system that is on the board, with the units of the other seats on it.
fn specialty_planet(state: &mut GameState) -> (ti4_model::id::SystemId, ti4_model::id::PlanetId) {
    let content = ti4_content::ContentStore::embedded();
    let (system, planet) = ti4_content::galaxy::all_planets(content, ti4_model::content_types::POK)
        .iter()
        .filter(|(_, p)| p.homeworld_of().is_none() && !p.is_placed_during_play() && !p.is_legendary() && !p.tech_specialties().is_empty())
        .find_map(|(id, p)| p.system_id().map(|s| (ti4_model::id::SystemId::new(s), ti4_model::id::PlanetId::new(*id))))
        .expect("a planet with a specialty");
    let _ = state;
    (system, planet)
}

#[test]
fn flayesh_chosen_from_the_menu_devours_a_planet_like_the_string() {
    let mut state = leader_table(&[("a", "nekro"), ("b", "sol"), ("c", "hacan")], "nekrohero", LeaderStatus::Unlocked);
    let (system, planet) = specialty_planet(&mut state);
    crate::fixtures::put(&mut state, &system, "cruiser", &pid("a"), 1);
    crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &pid("b"), 2);
    let state = super::turn_live_tests::based(state);
    lockstep(&state, &["component|leader|nekrohero"], "COMPONENT_ACTION_RESOLVED");
}

#[test]
fn kyver_chosen_from_the_menu_takes_a_card_like_the_string() {
    let mut state = leader_table(&[("a", "yssaril"), ("b", "sol"), ("c", "hacan")], "yssarilhero", LeaderStatus::Unlocked);
    state.player_mut(&pid("b")).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new(ti4_engine_core::generated::ActionCard::ALL[0].as_str()), ti4_model::id::ActionCardId::new(ti4_engine_core::generated::ActionCard::ALL[3].as_str())];
    state.player_mut(&pid("c")).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new(ti4_engine_core::generated::ActionCard::ALL[5].as_str())];
    let state = super::turn_live_tests::based(state);
    lockstep(&state, &["component|leader|yssarilhero"], "COMPONENT_ACTION_RESOLVED");
}

#[test]
fn hesh_and_prit_chosen_from_the_menu_gains_a_relic_like_the_string() {
    let mut state = leader_table(&[("a", "naaz"), ("b", "sol"), ("c", "hacan")], "naazhero", LeaderStatus::Unlocked);
    state.relic_deck = vec![ti4_model::id::RelicId::new("dynamiscore")];
    let state = super::turn_live_tests::based(state);
    lockstep(&state, &["component|leader|naazhero", "decline"], "COMPONENT_ACTION_RESOLVED");
}

// ---- lane F2e: Nekro's ???_ERROR_ERROR_??? and Terraform against the string ----

fn nekro_c4r_table() -> GameState {
    let mut state = action_table(&[("a", "nekro"), ("b", "sol"), ("c", "hacan")]);
    state.player_mut(&pid("a")).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("nekroc4r"));
    state
}

#[test]
fn nekros_error_error_card_places_a_pds_like_the_string() {
    let state = super::turn_live_tests::based(nekro_c4r_table());
    lockstep(&state, &["faction|nekro|c4r|pds"], "COMPONENT_ACTION_RESOLVED");
}

#[test]
fn nekros_error_error_card_repairs_the_damaged_units_like_the_string() {
    let mut state = nekro_c4r_table();
    let home = home_of(&state, "a");
    crate::fixtures::put(&mut state, &home, "dreadnought", &pid("a"), 1);
    for unit in &mut state.system_mut(&home).units {
        unit.sustained_damage = true;
    }
    let state = super::turn_live_tests::based(state);
    lockstep(&state, &["faction|nekro|c4r|repair"], "COMPONENT_ACTION_RESOLVED");
}

#[test]
fn nekros_error_error_card_discards_and_draws_like_the_string() {
    let mut state = nekro_c4r_table();
    state.player_mut(&pid("a")).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new(ti4_engine_core::generated::ActionCard::ALL[0].as_str()), ti4_model::id::ActionCardId::new(ti4_engine_core::generated::ActionCard::ALL[3].as_str())];
    let state = super::turn_live_tests::based(state);
    lockstep(&state, &["faction|nekro|c4r|draw"], "COMPONENT_ACTION_RESOLVED");
}

#[test]
fn terraform_chosen_from_the_menu_attaches_like_the_string() {
    let mut state = action_table(&[("a", "sol"), ("b", "titans"), ("c", "hacan")]);
    let planet = ti4_model::id::PlanetId::new(crate::fixtures::non_home_planets(1).remove(0));
    let system = ti4_model::id::SystemId::new(ti4_content::galaxy::all_planets(ti4_content::ContentStore::embedded(), ti4_model::content_types::POK).get(planet.as_str()).and_then(|p| p.system_id()).expect("a placed planet"));
    state.board.entry(system).or_default().set_control(planet.clone(), pid("a"));
    state.promissory_notes.insert("terraform:titans".to_owned(), pid("a"));
    let state = super::turn_live_tests::based(state);
    lockstep(&state, &[&format!("faction|titans|terraform|{planet}")], "COMPONENT_ACTION_RESOLVED");
}

#[test]
fn an_enigmatic_device_card_chosen_from_the_menu_researches_like_the_string() {
    let mut state = action_table(&THREE);
    let card = ti4_engine_core::generated::Explore::EnigmaticDevice1.as_str();
    let seat = state.player_mut(&pid("a")).unwrap();
    seat.exploration_cards = vec![card.to_owned()];
    seat.trade_goods = 8;
    let state = super::turn_live_tests::based(state);
    lockstep(&state, &[&format!("play_area:{card}")], "COMPONENT_ACTION_RESOLVED");
}

#[test]
fn mathis_mathinus_chosen_from_the_menu_performs_the_primary_like_the_string() {
    let mut state = leader_table(&[("a", "winnu"), ("b", "sol"), ("c", "hacan")], "winnuhero", LeaderStatus::Unlocked);
    state.unclaimed_strategy_cards = vec![ti4_model::id::StrategyCardId::new("pok5trade")];
    let state = super::turn_live_tests::based(state);
    let (string, taken) = string_run_first(&state, &["component|leader|winnuhero", "pok5trade"], |g| {
        let _ = g.step();
    });
    let native = native_marked(&state, &[(true, Arm::TurnPrelude, None), (false, Arm::Ask, None)], &taken.iter().map(String::as_str).collect::<Vec<_>>());
    assert_same(&string, &native);
}
