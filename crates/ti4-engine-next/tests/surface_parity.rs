//! Lane B (docs/todo/D2-8-wrapper.md, 8d): every native item of the product surface answers as its legacy original does, on the same inputs.
//! Constants are compared one by one; queries over positions played forward on real maps (`surface_common`), for every seat and system.

mod surface_common;

use surface_common::{content, positions};
use ti4_engine as thin;
use ti4_engine_legacy as old;
use ti4_model::id::{PlayerId, SystemId};

#[test]
fn the_kinds_are_the_legacy_constants() {
    use thin::kinds as k;
    assert_eq!(k::action_cards::DISCARD_KIND, old::action_cards::DISCARD_KIND);
    assert_eq!(k::action_cards::TAKE_REVEALED_KIND, old::action_cards::TAKE_REVEALED_KIND);
    assert_eq!(k::action_cards::OWN_HAND_KIND, old::action_cards::OWN_HAND_KIND);
    assert_eq!(k::action_cards::ACTION_CARD_KIND, old::reactions::ACTION_CARD_KIND);
    assert_eq!(k::combat::SUSTAIN_KIND, old::combat::SUSTAIN_KIND);
    assert_eq!(k::combat::CASUALTY_KIND, old::combat::CASUALTY_KIND);
    assert_eq!(k::combat::RETREAT_KIND, old::combat::RETREAT_KIND);
    assert_eq!(k::combat::RETREAT_TO_KIND, old::combat::RETREAT_TO_KIND);
    assert_eq!(k::diplomacy::OPEN_KIND, old::diplomacy::candidates::OPEN_KIND);
    assert_eq!(k::draft::STRATEGY_CARD_KIND, old::draft::STRATEGY_CARD_KIND);
    assert_eq!(k::game::CONTINUE_ACTION_ID, old::game::CONTINUE_ACTION_ID);
    assert_eq!(k::game::CONTINUE_ACTION_KIND, old::game::CONTINUE_ACTION_KIND);
    assert_eq!(k::game::SECONDARY_PREVIEW_COMPLETE, old::game::SECONDARY_PREVIEW_COMPLETE);
    assert_eq!(k::game::END_TURN_ID, old::game::END_TURN_ID);
    assert_eq!(k::game::END_TURN_KIND, old::game::END_TURN_KIND);
    assert_eq!(k::game::TACTICAL_ACTION_ID, old::game::TACTICAL_ACTION_ID);
    assert_eq!(k::invasion::COMMIT_KIND, old::invasion::COMMIT_KIND);
    assert_eq!(k::invasion::GROUND_CASUALTY_KIND, old::invasion::GROUND_CASUALTY_KIND);
    assert_eq!(k::neutral_units::NEUTRAL, old::neutral_units::NEUTRAL);
    assert_eq!(k::objectives::VICTORY_TARGET, old::objectives::VICTORY_TARGET);
    assert_eq!(k::objectives::SCORE_KIND, old::objectives::SCORE_KIND);
    assert_eq!(k::production::PRODUCE_KIND, old::production::PRODUCE_KIND);
    assert_eq!(k::production::PAY_KIND, old::production::PAY_KIND);
    assert_eq!(k::production::PLACE_KIND, old::production::PLACE_KIND);
    assert_eq!(k::production::SPACE, old::production::SPACE);
    assert_eq!(k::production::REMOTE_SEPARATOR, old::production::REMOTE_SEPARATOR);
    assert_eq!(k::production::AGENT_OPTION_ID, old::production::AGENT_OPTION_ID);
    assert_eq!(k::production::AGENT_OPTION_KIND, old::production::AGENT_OPTION_KIND);
    assert_eq!(k::production::BUILDABLE, old::production::BUILDABLE);
    assert_eq!(k::production::UNLOCKED_BY, old::production::UNLOCKED_BY);
    assert_eq!(k::promissory::SUPPORT_PREFIX, old::promissory::SUPPORT_PREFIX);
    assert_eq!(k::relics::SHARD, old::relics::SHARD);
    assert_eq!(k::secondary_preview::MAX_ANSWERS, old::secondary_preview::MAX_ANSWERS);
    assert_eq!(k::secrets::HAND_LIMIT, old::secrets::HAND_LIMIT);
    assert_eq!(k::seating::MECATOL, old::seating::MECATOL);
    assert_eq!(k::seating::MECATOL_TE, old::seating::MECATOL_TE);
    assert_eq!(k::starting_tech::KIND, old::starting_tech::KIND);
    assert_eq!(k::strategy::WAIVED_SECONDARY_PREFIX, old::strategy::WAIVED_SECONDARY_PREFIX);
    assert_eq!(k::strategy::STRATEGY_KIND, old::strategy::STRATEGY_KIND);
    assert_eq!(k::strategy::ACTION_KIND, old::strategy::ACTION_KIND);
    assert_eq!(k::strategy::STRATEGIC_ACTION_ID, old::strategy::STRATEGIC_ACTION_ID);
    assert_eq!(k::strategy_cards::INFLUENCE_PER_TOKEN, old::strategy_cards::INFLUENCE_PER_TOKEN);
    assert_eq!(k::strategy_cards::RESEARCH_KIND, old::strategy_cards::RESEARCH_KIND);
    assert_eq!(k::tactical::ROUTES_DETAIL, old::tactical::ROUTES_DETAIL);
    assert_eq!(k::tactical::ACTIVATE_KIND, old::tactical::ACTIVATE_KIND);
    assert_eq!(k::tactical::MOVE_KIND, old::tactical::MOVE_KIND);
    assert_eq!(k::tactical::MAX_ROUTES, old::tactical::MAX_ROUTES);
    assert_eq!(k::timing::ACTIVE_DETAIL, old::timing::ACTIVE_DETAIL);
    assert_eq!(k::transactions::ANSWER_KIND, old::transactions::ANSWER_KIND);
    assert_eq!(k::transactions::FREE_FORM_PREFIX, old::transactions::FREE_FORM_PREFIX);
    assert_eq!(k::transactions::OFFER_KIND, old::transactions::OFFER_KIND);
    assert_eq!(k::transactions::OPEN_KIND, old::transactions::OPEN_KIND);
    assert_eq!(k::transit::LOAD_KIND, old::transit::LOAD_KIND);
    assert_eq!(k::transit::RIFT_DESTROYS_ON, old::transit::RIFT_DESTROYS_ON);
}

fn players(state: &ti4_model::state::GameState) -> Vec<PlayerId> {
    state.players.iter().map(|p| p.id.clone()).collect()
}

/// `Movable`/`MoveRoute` do not share a type with the legacy ones: compared through their fields.
fn movable_key(m: &thin::queries::tactical::Movable) -> String {
    format!("{} {} {:?} {} {} {}", m.origin, m.index, m.unit, m.capacity, m.gravity_drive, m.ionian)
}
fn movable_key_old(m: &old::tactical::Movable) -> String {
    format!("{} {} {:?} {} {} {}", m.origin, m.index, m.unit, m.capacity, m.gravity_drive, m.ionian)
}

#[test]
fn the_movement_queries_answer_as_the_legacy_ones_do() {
    let content = content();
    let (mut ships, mut routes) = (0_usize, 0_usize);
    for pos in positions() {
        let systems: Vec<SystemId> = pos.galaxy.system_ids().into_iter().map(SystemId::new).collect();
        for player in players(&pos.state) {
            for dest in &systems {
                let o: Vec<String> = old::tactical::movable_into(&pos.state, content, pos.sources, &pos.galaxy, &player, dest).iter().map(movable_key_old).collect();
                let t: Vec<String> = thin::queries::tactical::movable_into(&pos.state, content, pos.sources, &pos.galaxy, &player, dest).iter().map(movable_key).collect();
                assert_eq!(o, t, "{}: movable_into {player} -> {dest}", pos.label);
                ships += o.len();
                for (origin, index) in pos.state.systems_with_units_of(&player).into_iter().flat_map(|s| (0..pos.state.ships_of(&player, s).len()).map(move |i| (s.clone(), i))).take(6) {
                    for boosts in [(false, false), (true, false), (false, true), (true, true)] {
                        let o = old::tactical::routes_into(&pos.state, content, pos.sources, &pos.galaxy, &player, dest, &origin, index, boosts);
                        let t = thin::queries::tactical::routes_into(&pos.state, content, pos.sources, &pos.galaxy, &player, dest, &origin, index, boosts);
                        assert_eq!(serde_json::to_string(&o).unwrap(), serde_json::to_string(&t).unwrap(), "{}: routes_into {player} {origin}#{index} {boosts:?} -> {dest}", pos.label);
                        routes += o.len();
                    }
                }
            }
        }
    }
    assert!(ships > 50, "only {ships} movable ships compared");
    assert!(routes > 50, "only {routes} routes compared");
}

#[test]
fn the_production_queries_answer_as_the_legacy_ones_do() {
    use thin::queries::production as t;
    let content = content();
    let types = ti4_content::units::catalogue(content, ti4_model::content_types::POK);
    let mut compared = 0_usize;
    for pos in positions() {
        let systems: Vec<SystemId> = pos.galaxy.system_ids().into_iter().map(SystemId::new).collect();
        for player in players(&pos.state) {
            for (o, n) in [(old::production::Spend::Resources, t::Spend::Resources), (old::production::Spend::Influence, t::Spend::Influence)] {
                assert_eq!(
                    old::production::available(&pos.state, content, pos.sources, &player, o),
                    t::available(&pos.state, content, pos.sources, &player, n),
                    "{}: available {player} {o:?}",
                    pos.label
                );
            }
            assert_eq!(old::production::trade_good_worth(&pos.state, &player), t::trade_good_worth(&pos.state, &player), "{}: trade_good_worth {player}", pos.label);
            for system in &systems {
                assert_eq!(
                    old::production::capacity(&pos.state, content, pos.sources, &player, system),
                    t::capacity(&pos.state, content, pos.sources, &player, system),
                    "{}: capacity {player} {system}",
                    pos.label
                );
                for unit in old::production::BUILDABLE {
                    let kind = types.get(unit).expect("a buildable unit");
                    assert_eq!(
                        old::production::placements(&pos.state, content, pos.sources, &player, system, kind),
                        t::placements(&pos.state, content, pos.sources, &player, system, kind),
                        "{}: placements {player} {system} {unit}",
                        pos.label
                    );
                    compared += 1;
                }
            }
        }
        let mut with_law = pos.state.clone();
        with_law.laws.insert("conscription".to_owned(), "for".to_owned());
        for state in [Some(&pos.state), Some(&with_law), None] {
            for kind in types.values() {
                assert_eq!(old::production::price_of_under(state, kind), t::price_of_under(state, kind), "{}: price_of_under {}", pos.label, kind.id());
            }
        }
    }
    assert!(compared > 1000, "only {compared} placement questions compared");
}
