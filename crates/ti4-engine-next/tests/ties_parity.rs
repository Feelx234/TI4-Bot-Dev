//! The ties (docs/todo/D2-8-wrapper.md, Appendix A) are COPIES of constants and pure functions of the legacy crate that the glue names.
//! This file compares each copy with its original: every constant, and every function over the whole id space of its argument (all cards,
//! all technologies, all factions, all strategy cards, a corpus of option ids), so a copy cannot drift while the legacy crate is the oracle.
//!
//! Not compared (private in the legacy crate, which is frozen and not edited for this): `MUNITIONS_COST`, `crimson_cards::CARD_SYSTEM`,
//! `draft::strategy_card_label`, `faction_abilities::ability_card`, `strategy_cards::{agenda_details, leader_card, commander_card}`,
//! `invasion::breakthrough_card`, `transactions::planet_value`, `timing::ability_label` (display text, verbatim copies; the native_play
//! fingerprint and the decision corpus replay see them).

use ti4_content::ContentStore;
use ti4_engine as thin;
use ti4_engine_core::generated::{ActionCard, Faction, StrategyCard, Technology};
use ti4_engine_legacy as old;
use ti4_engine_legacy::generated::IdStr;
use ti4_model::content_types::{DEFAULT, POK};
use ti4_model::id::{ActionCardId, PlayerId, TechnologyId};

#[test]
fn the_constants_are_the_legacy_constants() {
    use thin::ties as t;
    assert_eq!(t::action_cards::DISCARD_KIND, old::action_cards::DISCARD_KIND);
    assert_eq!(t::action_cards::TAKE_REVEALED_KIND, old::action_cards::TAKE_REVEALED_KIND);
    assert_eq!(t::action_cards::OWN_HAND_KIND, old::action_cards::OWN_HAND_KIND);
    assert_eq!(t::action_cards::ACTION_CARD_KIND, old::reactions::ACTION_CARD_KIND);
    assert_eq!(t::combat::RETREAT_KIND, old::combat::RETREAT_KIND);
    assert_eq!(t::combat::RETREAT_TO_KIND, old::combat::RETREAT_TO_KIND);
    assert_eq!(t::draft::STRATEGY_CARD_KIND, old::draft::STRATEGY_CARD_KIND);
    assert_eq!(t::strategy::WAIVED_SECONDARY_PREFIX, old::strategy::WAIVED_SECONDARY_PREFIX);
    assert_eq!(t::strategy::STRATEGY_KIND, old::strategy::STRATEGY_KIND);
    assert_eq!(t::strategy::ACTION_KIND, old::strategy::ACTION_KIND);
    assert_eq!(t::strategy::STRATEGIC_ACTION_ID, old::strategy::STRATEGIC_ACTION_ID);
    assert_eq!(t::production::PRODUCE_KIND, old::production::PRODUCE_KIND);
    assert_eq!(t::production::PAY_KIND, old::production::PAY_KIND);
    assert_eq!(t::production::PLACE_KIND, old::production::PLACE_KIND);
    assert_eq!(t::production::SPACE, old::production::SPACE);
    assert_eq!(t::production::REMOTE_SEPARATOR, old::production::REMOTE_SEPARATOR);
    assert_eq!(t::production::AGENT_OPTION_ID, old::production::AGENT_OPTION_ID);
    assert_eq!(t::production::AGENT_OPTION_KIND, old::production::AGENT_OPTION_KIND);
    assert_eq!(t::invasion::GROUND_CASUALTY_KIND, old::invasion::GROUND_CASUALTY_KIND);
    assert_eq!(t::game::END_TURN_ID, old::game::END_TURN_ID);
    assert_eq!(t::game::END_TURN_KIND, old::game::END_TURN_KIND);
    assert_eq!(t::game::TACTICAL_ACTION_ID, old::game::TACTICAL_ACTION_ID);
    assert_eq!(t::transactions::ANSWER_KIND, old::transactions::ANSWER_KIND);
    assert_eq!(t::transactions::FREE_FORM_PREFIX, old::transactions::FREE_FORM_PREFIX);
    assert_eq!(t::transactions::OFFER_KIND, old::transactions::OFFER_KIND);
    assert_eq!(t::transactions::OPEN_KIND, old::transactions::OPEN_KIND);
    assert_eq!(t::starting_tech::KIND, old::starting_tech::KIND);
    assert_eq!(t::promissory::SUPPORT_PREFIX, old::promissory::SUPPORT_PREFIX);
    assert_eq!(t::tactical::ACTIVATE_KIND, old::tactical::ACTIVATE_KIND);
    assert_eq!(t::tactical::ROUTES_DETAIL, old::tactical::ROUTES_DETAIL);
    assert_eq!(t::seating::LOBBY_FACTIONS, old::seating::LOBBY_FACTIONS);
    assert_eq!(t::neutral_units::NEUTRAL, old::neutral_units::NEUTRAL);
    assert_eq!(t::strategy_cards::RESEARCH_KIND, old::strategy_cards::RESEARCH_KIND);
    assert_eq!(t::timing::ACTIVE_DETAIL, old::timing::ACTIVE_DETAIL);
    assert_eq!(t::technology::COLOURS, old::technology::COLOURS);
    assert_eq!(t::factions::CRIMSON_MECH, old::factions::crimson::MECH);
    assert_eq!(t::factions::PRIMARY_INSTEAD_ID, old::factions::firmament_plots::PRIMARY_INSTEAD_ID);
    assert_eq!(t::breakthroughs::VARIANTS, old::factions::keleres::VARIANTS);
}

#[test]
fn the_module_table_facts_are_the_legacy_ones() {
    let aliases: Vec<&str> = old::factions::MODULES.iter().map(|m| m.alias).collect();
    assert_eq!(thin::ties::factions::MODULE_ALIASES.to_vec(), aliases, "factions::MODULES, in order");
    let waivers: Vec<&str> = old::factions::MODULES.iter().filter(|m| m.hooks.strategy.secondary_waivers.is_some()).map(|m| m.alias).collect();
    assert_eq!(thin::ties::factions::WAIVER_MODULES.to_vec(), waivers, "the modules with the secondary_waivers hook");
    for faction in Faction::ALL {
        let alias = faction.as_str();
        assert_eq!(thin::ties::factions::has_module(alias), aliases.contains(&alias), "{alias}");
        assert_eq!(thin::ties::factions::waiver_module_index(alias), waivers.iter().position(|w| *w == alias), "{alias}");
    }
}

#[test]
fn every_action_card_has_a_string_effect_as_the_live_play_facts_assume() {
    for card in ActionCard::ALL {
        assert!(old::action_cards::effect_for(&ActionCardId::new(card.as_str())).is_some(), "{} has no string effect: `ties::reactions_core::card_tables` must say so", card.as_str());
    }
}

#[test]
fn the_pure_functions_agree_over_the_whole_id_space() {
    use thin::ties as t;
    let content = ContentStore::embedded();
    for card in ActionCard::ALL {
        let id = ActionCardId::new(card.as_str());
        assert_eq!(t::action_cards::name_of(content, &id), old::action_cards::name_of(content, &id), "{id}");
    }
    for card in StrategyCard::ALL {
        let id = card.as_str();
        assert_eq!(t::strategy_cards::card_name(content, id), old::strategy_cards::card_name(content, id), "{id}");
    }
    for tech in Technology::ALL {
        let id = TechnologyId::new(tech.as_str());
        assert_eq!(t::technology::name(content, &id), old::technology::name(content, &id), "{id}");
        assert_eq!(t::technology::colour_type(content, &id), old::technology::colour_type(content, &id), "{id}");
        assert_eq!(t::technology::prerequisites(content, &id), old::technology::prerequisites(content, &id), "{id}");
        assert_eq!(t::technology::is_unit_upgrade(content, &id), old::technology::is_unit_upgrade(content, &id), "{id}");
    }
    let unknown = TechnologyId::new("no_such_technology");
    assert_eq!(t::technology::colour_type(content, &unknown), old::technology::colour_type(content, &unknown));
    assert_eq!(t::technology::prerequisites(content, &unknown), old::technology::prerequisites(content, &unknown));
    for faction in Faction::ALL {
        let alias = faction.as_str();
        assert_eq!(t::breakthroughs::for_faction(content, alias), old::breakthroughs::for_faction(content, alias), "{alias}");
    }
    assert_eq!(t::promissory::support("sol"), old::promissory::support("sol"));
    for fragment in ["cultural", "UNKNOWN", "Hazardous", "frontier"] {
        assert_eq!(t::transactions::fragment_key(fragment), old::transactions::fragment_key(fragment), "{fragment}");
    }
    for id in ["move|206|0", "move|206|0|r2", "move|206|0|r0", "move|206|0|r02", "move|x|r", "plain", "a|r2|r3", "|r5"] {
        assert_eq!(t::tactical::route_suffix(id), old::tactical::route_suffix(id), "{id}");
    }
}

#[test]
fn the_deal_ids_read_the_same_deal() {
    use thin::ties::transactions as t;
    use old::transactions as o;
    let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
    let names = |p: &PlayerId| if p == &a { "sol".to_owned() } else { "hacan".to_owned() };
    let ids = [
        "g3", "c2:0", "cc3", "np support:sol:3", "x;gt=2;rc=1;", "x;gt=2;rt=1;gn=support:sol;", "x;gf=cultural,hazardous;rt=1;", "x;ga=mb1;rt=1;", "x;gt=1;gt=2;", "x;gz=1;", "x;", "garbage", "",
        "t2:1", "n2", "support", "x;rp=jord>mr;gt=1;",
    ];
    for id in ids {
        let mine = t::offer_from_with(&names, id, &a, &b);
        let theirs = o::offer_from_with(&names, id, &a, &b);
        assert_eq!(mine.is_some(), theirs.is_some(), "{id}");
        if let (Some(m), Some(n)) = (mine, theirs) {
            assert_eq!(m.describe_as("sol"), n.describe_as("sol"), "{id}");
            assert_eq!(m.given.is_empty(), n.given.is_empty(), "{id}");
            assert_eq!(format!("{:?}", m.given), format!("{:?}", n.given), "{id}");
            assert_eq!(format!("{:?}", m.received), format!("{:?}", n.received), "{id}");
            assert_eq!(t::free_form_id(&m.given, &m.received), o::free_form_id(&n.given, &n.received), "{id}");
        }
    }
}

#[test]
fn the_position_queries_agree() {
    let content = ContentStore::embedded();
    let mut state = old::fixtures::seated_dealt(&[("a", "mentak"), ("b", "sol")], POK);
    for planet in ["mr", "jord", "bakal", "no_such_planet", "eylau"] {
        assert_eq!(thin::ties::planets::system_of(&state, content, POK, planet), old::planets::system_of(&state, content, POK, planet), "{planet}");
    }
    assert_eq!(thin::ties::laws::blanked(&state), old::laws::blanked(&state));
    state.phase = ti4_model::state::Phase::Action;
    state.turn_seq = 4;
    state.faction_marks.insert("keleres|laws_blank".to_owned(), "4".to_owned());
    assert!(thin::ties::laws::blanked(&state));
    assert_eq!(thin::ties::laws::blanked(&state), old::laws::blanked(&state));
    state.turn_seq = 5;
    assert!(!thin::ties::laws::blanked(&state));
    assert_eq!(thin::ties::laws::blanked(&state), old::laws::blanked(&state));
    let _ = DEFAULT;
}
