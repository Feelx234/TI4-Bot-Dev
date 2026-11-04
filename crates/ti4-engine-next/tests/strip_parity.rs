//! D2-9b (docs/D29B_RESUME.md): the few items production took from the legacy crate before the facade glob went are native now; each one answers as
//! its legacy original does. Regression tests: each fails on the code before the strip (the item did not exist on the thin crate).

mod surface_common;

use surface_common::{content, positions};
use ti4_engine as thin;
use ti4_engine_legacy as old;
use ti4_model::content_types::POK;
use ti4_model::id::{ObjectiveId, PlayerId};

#[test]
fn staging_is_enabled_for_exactly_the_seats_the_legacy_modules_cover() {
    // Every faction of the corpus, alone at a table with a faction that has no module (the fixture faction).
    let mut seen_true = 0;
    let mut seen_false = 0;
    for faction in content().records(ti4_model::content_types::ContentType::Factions).iter().filter_map(|r| r.text("alias")).filter(|a| *a != "neutral").map(str::to_owned).chain(["generic".to_owned(), "obsidian".to_owned(), "keleresm".to_owned()]) {
        let mut state = old::fixtures::seated_dealt(&[("a", "sol"), ("b", "hacan")], POK);
        state.player_mut(&PlayerId::new("a")).expect("a").faction = ti4_model::id::FactionId::new(&faction);
        state.player_mut(&PlayerId::new("b")).expect("b").faction = ti4_model::id::FactionId::new("generic");
        state.seating_order = vec![PlayerId::new("a"), PlayerId::new("b")];
        let (a, b) = (old::supply::staging_enabled(&state), thin::supply::staging_enabled(&state));
        assert_eq!(a, b, "staging_enabled with {faction}");
        if a { seen_true += 1 } else { seen_false += 1 }
    }
    assert!(seen_true > 20 && seen_false >= 1, "{seen_true} true, {seen_false} false");
}

#[test]
fn the_objective_registry_is_the_set_the_legacy_requirement_table_answers() {
    for alias in old::objectives::registered_aliases() {
        assert!(thin::queries::objectives::has_requirement(&ObjectiveId::new(alias)), "{alias}");
    }
    let mut checked = 0;
    for record in content().records(ti4_model::content_types::ContentType::PublicObjectives) {
        let Some(alias) = record.text("alias") else { continue };
        let id = ObjectiveId::new(alias);
        assert_eq!(old::objectives::requirement_for(&id).is_some(), thin::queries::objectives::has_requirement(&id), "{alias}");
        checked += 1;
    }
    assert!(checked > 30, "{checked}");
}

#[test]
fn the_over_supply_and_over_capacity_reads_are_the_legacy_ones() {
    let content = content();
    let (mut over, mut over_supply, mut checked) = (0_usize, 0_usize, 0_usize);
    for pos in positions() {
        let mut stacked = pos.state.clone();
        for p in &pos.state.players {
            let Some(system) = stacked.board.iter().find(|(_, b)| b.units.iter().any(|u| u.owner == p.id)).map(|(id, _)| id.clone()) else { continue };
            let board = stacked.board.get_mut(&system).expect("a system");
            for (kind, n) in [("cruiser", 6), ("carrier", 4), ("infantry", 5), ("fighter", 6)] {
                for _ in 0..n {
                    board.units.push(ti4_model::units::Unit::new(ti4_model::id::UnitTypeId::new(kind), p.id.clone()));
                }
            }
        }
        for state in [&pos.state, &stacked] {
            for p in &state.players {
                for system in state.board.keys() {
                    let (a, b) = (old::fleet::over_supply(state, content, pos.sources, &p.id, system), thin::fleet::over_supply(state, content, pos.sources, &p.id, system));
                    assert_eq!(a, b, "{}: over_supply {} {system}", pos.label, p.id);
                    over_supply += usize::from(a > 0);
                    let (a, b) = (old::fleet::over_capacity(state, content, pos.sources, &p.id, system), thin::fleet::over_capacity(state, content, pos.sources, &p.id, system));
                    assert_eq!(a, b, "{}: over_capacity {} {system}", pos.label, p.id);
                    over += usize::from(a > 0);
                    checked += 1;
                }
            }
        }
    }
    assert!(over_supply > 10 && checked > 1000, "over supply {over_supply}, over capacity {over}, checked {checked}");
}

#[test]
fn a_flagship_has_the_text_of_the_one_a_nekro_z_token_lends_it() {
    use ti4_model::id::BreakthroughId;
    let mut state = old::fixtures::seated_dealt(&[("a", "nekro"), ("b", "hacan")], POK);
    let a = PlayerId::new("a");
    let nekro_flagship = "nekro_flagship";
    for (unit, flagship) in [("hacan_flagship", "hacan_flagship"), (nekro_flagship, "hacan_flagship"), (nekro_flagship, "ghost_flagship"), ("cruiser", "ghost_flagship"), (nekro_flagship, "keleres_flagship")] {
        // Before the breakthrough and the token, only the flagship itself.
        let (x, y) = (old::factions::flagship_has_text(&state, &a, unit, flagship), thin::factions::flagship_has_text(&state, &a, unit, flagship));
        assert_eq!(x, y, "{unit} {flagship} (no token)");
    }
    state.player_mut(&a).expect("a").breakthrough = Some(BreakthroughId::new("nekrobt"));
    assert!(old::factions::nekro::place_z(&mut state, &a, "ghost"));
    assert!(old::factions::nekro::place_z(&mut state, &a, "keleresx"));
    let mut lent = 0;
    for (unit, flagship) in [("hacan_flagship", "hacan_flagship"), (nekro_flagship, "hacan_flagship"), (nekro_flagship, "ghost_flagship"), ("cruiser", "ghost_flagship"), (nekro_flagship, "keleres_flagship"), (nekro_flagship, nekro_flagship)] {
        let (x, y) = (old::factions::flagship_has_text(&state, &a, unit, flagship), thin::factions::flagship_has_text(&state, &a, unit, flagship));
        assert_eq!(x, y, "{unit} {flagship} (tokens)");
        lent += usize::from(x && unit == nekro_flagship && flagship != nekro_flagship);
    }
    assert_eq!(lent, 2, "the Z tokens on the Ghosts and the Keleres lend those two flagships");
}

/// The starting-technology pick of the Winnu ("any technology with no prerequisites") and the other factions that could ask the same question
/// in a position that is being made.
#[test]
fn the_researchable_by_prerequisites_read_at_setup_is_the_legacy_one() {
    let content = content();
    let mut nonempty = 0;
    for faction in ["winnu", "sol", "jolnar", "hacan", "yin", "l1z1x", "nekro", "arborec", "saar", "ghost", "mentak", "naalu", "xxcha", "letnev", "muaat", "sardakk", "yssaril", "argent", "cabal", "empyrean", "mahact", "naaz", "nomad", "titans", "keleresm"] {
        let state = old::fixtures::seated_dealt(&[("a", faction), ("b", "sol")], POK);
        let a = PlayerId::new("a");
        let want = old::technology::researchable_by_prerequisites(&state, content, POK, &a);
        let got = thin::technology::researchable_by_prerequisites(&state, content, POK, &a);
        assert_eq!(want, got, "researchable_by_prerequisites at setup for {faction}");
        nonempty += usize::from(!want.is_empty());
        // With a few technologies owned as well.
        let mut richer = state.clone();
        for t in thin::technology::active_aliases(content, POK).into_iter().filter(|t| thin::technology::faction_of(content, t).is_none()).take(6) {
            richer.player_mut(&a).expect("a").technologies.insert(t);
        }
        let want = old::technology::researchable_by_prerequisites(&richer, content, POK, &a);
        assert_eq!(want, thin::technology::researchable_by_prerequisites(&richer, content, POK, &a), "researchable_by_prerequisites with techs for {faction}");
    }
    assert!(nonempty > 10, "{nonempty}");
}
