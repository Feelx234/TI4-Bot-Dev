//! Lane B (8d items 2 and 3, docs/D28B2_RESUME.md): the production tail and the state queries answer as the legacy originals do, on the
//! positions of `surface_common` and on variants of them (technologies researched, planets exhausted, laws and attachments on planets).

mod surface_common;

use surface_common::{content, positions};
use ti4_engine as thin;
use ti4_engine_legacy as old;
use ti4_model::id::{LeaderId, PlanetId, PlayerId, TechnologyId};
use ti4_model::state::{GameState, LeaderStatus};

fn players(state: &GameState) -> Vec<PlayerId> {
    state.players.iter().map(|p| p.id.clone()).collect()
}

fn unit_upgrades(sources: ti4_model::content_types::SourceSet) -> Vec<TechnologyId> {
    old::technology::active_aliases(content(), sources).into_iter().filter(|t| old::technology::is_unit_upgrade(content(), t)).collect()
}

#[test]
fn buildable_for_answers_as_the_legacy_one_does() {
    let content = content();
    let mut compared = 0_usize;
    let mut distinct = std::collections::BTreeSet::new();
    for pos in positions() {
        let upgrades = unit_upgrades(pos.sources);
        assert!(upgrades.len() > 5, "the corpus has unit upgrades");
        for player in players(&pos.state) {
            // The position as it stands; then with the war sun technology; then with each unit upgrade; then with all of them.
            let mut variants: Vec<(String, Vec<TechnologyId>)> = vec![("as is".to_owned(), Vec::new()), ("ws".to_owned(), vec![TechnologyId::new("ws")])];
            variants.extend(upgrades.iter().map(|t| (t.as_str().to_owned(), vec![t.clone()])));
            variants.push(("all".to_owned(), upgrades.clone()));
            for (label, techs) in variants {
                let mut state = pos.state.clone();
                state.player_mut(&player).expect("a seat").technologies.extend(techs);
                let (a, b) = (old::production::buildable_for(&state, content, pos.sources, &player), thin::queries::production::buildable_for(&state, content, pos.sources, &player));
                assert_eq!(a, b, "{}: buildable_for {player} with {label}", pos.label);
                distinct.insert(a);
                compared += 1;
            }
        }
    }
    assert!(compared > 500, "only {compared} compared");
    assert!(distinct.len() > 8, "only {} distinct answers: the corpus does not vary what is buildable", distinct.len());
}

#[test]
fn the_payment_planets_answer_as_the_legacy_ones_do() {
    use thin::queries::production::Spend as T;
    let content = content();
    let mut listed = 0_usize;
    for pos in positions() {
        for player in players(&pos.state) {
            // As is; half the planets exhausted; Xxekir Grom unlocked (a planet pays its two values together).
            let mut halved = pos.state.clone();
            let owned: Vec<PlanetId> = halved.controlled_planets(&player).into_iter().map(|(_, p)| p.clone()).collect();
            for planet in owned.iter().step_by(2) {
                halved.exhaust_planet(planet.clone());
            }
            let mut combined = pos.state.clone();
            combined.player_mut(&player).expect("a seat").leaders.insert(LeaderId::new("xxchahero"), LeaderStatus::Unlocked);
            for (label, state) in [("as is", &pos.state), ("half exhausted", &halved), ("xxekir grom", &combined)] {
                for (o, n) in [(old::production::Spend::Resources, T::Resources), (old::production::Spend::Influence, T::Influence)] {
                    let a = old::production::native_payment_planets(state, content, pos.sources, &player, o);
                    let b = thin::queries::production::native_payment_planets(state, content, pos.sources, &player, n);
                    assert_eq!(a, b, "{}: native_payment_planets {player} {o:?} ({label})", pos.label);
                    listed += a.len();
                }
            }
        }
    }
    assert!(listed > 200, "only {listed} planets listed");
}

#[test]
fn the_value_boosts_answer_as_the_legacy_ones_do() {
    let content = content();
    let attachments: Vec<String> = content
        .records(ti4_model::content_types::ContentType::Attachments)
        .iter()
        .filter(|r| r.int("resourcesModifier").unwrap_or(0) != 0 || r.int("influenceModifier").unwrap_or(0) != 0)
        .filter_map(|r| r.text("id").map(ToOwned::to_owned))
        .collect();
    assert!(attachments.len() > 3, "the corpus has attachments that move a value");
    let (mut boosts, mut with_law, mut with_attachment, mut helios) = (0_usize, 0_usize, 0_usize, 0_usize);
    for pos in positions() {
        let planets: Vec<PlanetId> = pos.state.board.values().flat_map(|s| s.planet_control.keys().cloned()).collect();
        for planet in &planets {
            let mut variants: Vec<(String, GameState)> = vec![("as is".to_owned(), pos.state.clone())];
            for law in ["core_mining", "senate_sanctuary", "terraforming_initiative"] {
                let mut s = pos.state.clone();
                s.laws.insert(law.to_owned(), planet.as_str().to_owned());
                variants.push((law.to_owned(), s));
            }
            let mut both = pos.state.clone();
            for law in ["core_mining", "senate_sanctuary", "terraforming_initiative"] {
                both.laws.insert(law.to_owned(), planet.as_str().to_owned());
            }
            both.planet_attachments.insert(planet.clone(), attachments.iter().take(2).cloned().collect());
            variants.push(("three laws and two attachments".to_owned(), both));
            let mut attached = pos.state.clone();
            attached.planet_attachments.insert(planet.clone(), attachments.clone());
            variants.push(("every attachment".to_owned(), attached));
            // A Last Bastion seat with a Helios dock (V1, then V2) on the planet: +1 / +2 resources.
            for (dock, label) in [("bastion_spacedock", "helios 1"), ("bastion_spacedock2", "helios 2")] {
                let mut s = pos.state.clone();
                s.players[0].faction = ti4_model::id::FactionId::new("bastion");
                let owner = s.players[0].id.clone();
                let system = s.board.iter().find(|(_, b)| b.planet_control.contains_key(planet)).map(|(id, _)| id.clone()).expect("a system");
                s.board.get_mut(&system).expect("a system").planet_units.entry(planet.clone()).or_default().push(ti4_model::units::Unit::new(ti4_model::id::UnitTypeId::new(dock), owner));
                variants.push((label.to_owned(), s));
            }
            for (label, state) in variants {
                let (a, b) = (old::production::planet_value_boosts(&state, content, planet), thin::queries::production::planet_value_boosts(&state, content, planet));
                assert_eq!(a, b, "{}: planet_value_boosts {planet} ({label})", pos.label);
                boosts += a.len();
                with_law += usize::from(a.iter().any(|(name, _, _)| matches!(name.as_str(), "Core Mining" | "Senate Sanctuary" | "Terraforming Initiative")));
                with_attachment += usize::from(a.len() > 3);
                helios += usize::from(a.iter().any(|(name, _, _)| name == "Helios dock"));
            }
        }
    }
    assert!(boosts > 300 && with_law > 100 && with_attachment > 20, "boosts {boosts}, with a law {with_law}, with attachments {with_attachment}");
    assert!(helios > 20, "only {helios} Helios docks listed");
}
