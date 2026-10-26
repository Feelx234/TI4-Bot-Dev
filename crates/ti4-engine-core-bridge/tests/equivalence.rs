//! The typed tables and the movement slice answer what the string engine answers.
//!
//! Fast checks for `cargo test`; the full dual run over golden games is the release binary
//! `core_equivalence` (docs/ENGINE_CORE.md, "Measurements").

use std::collections::BTreeSet;

use ti4_content::ContentStore;
use ti4_content::galaxy::{Galaxy, System, all_systems};
use ti4_engine_core::hex::Hex;
use ti4_engine::generated::IdStr;
use ti4_engine::generated::planets::PLANET_IDS as PLANET_NAMES;
use ti4_engine::generated::systems::SYSTEM_IDS as SYSTEM_NAMES;
use ti4_engine::generated::units::UNIT_IDS as UNIT_NAMES;
use ti4_engine_core::SystemIx;
use ti4_engine_core::tables::{Flags, SystemFlags, is_nexus, moves_as_ship};
use ti4_engine_core_bridge::adapter::{galaxy_from_legacy, galaxy_native, scope, system_ix};
use ti4_engine_core_bridge::check::{Report, check_adjacency, check_galaxy_variants, check_position};
use ti4_engine_core_bridge::workload::template_openings;
use ti4_model::content_types::{FULL, POK};

fn sorted_unique(names: &[&str]) -> bool {
    names.windows(2).all(|w| w[0] < w[1])
}

#[test]
fn ids_are_numbered_in_text_order() {
    // Index order == `str` order: what makes a bitset iterate like a `BTreeSet<String>`.
    assert!(sorted_unique(&SYSTEM_NAMES));
    assert!(sorted_unique(&PLANET_NAMES));
    assert!(sorted_unique(&UNIT_NAMES));
    for (i, a) in SYSTEM_NAMES.iter().enumerate() {
        for (j, b) in SYSTEM_NAMES.iter().enumerate() {
            assert_eq!(i.cmp(&j), a.cmp(b), "{a} vs {b}");
        }
    }
}

#[test]
fn system_table_matches_the_corpus() {
    let store = ContentStore::embedded();
    let full = all_systems(store, FULL);
    assert_eq!(full.len(), SYSTEM_NAMES.len());
    for (i, id) in SYSTEM_NAMES.iter().enumerate() {
        let s: &System<'_> = &full[id];
        let t = SystemIx::ALL[i].data();
        assert_eq!(t.has(SystemFlags::NEBULA), s.is_nebula(), "{id}");
        assert_eq!(t.has(SystemFlags::SUPERNOVA), s.is_supernova(), "{id}");
        assert_eq!(t.has(SystemFlags::ASTEROID_FIELD), s.is_asteroid_field(), "{id}");
        assert_eq!(t.has(SystemFlags::GRAVITY_RIFT), s.is_gravity_rift(), "{id}");
        assert_eq!(t.has(SystemFlags::HYPERLANE), s.is_hyperlane(), "{id}");
        assert_eq!(t.has(SystemFlags::SCAR), s.is_scar(), "{id}");
        let planets: Vec<&str> = t.planets.iter().map(|p| PLANET_NAMES[p.index()]).collect();
        assert_eq!(is_nexus(SystemIx::ALL[i]), id.starts_with("82"), "{id}");
        assert_eq!(planets, s.planets(), "{id}");
        if s.is_hyperlane() {
            assert_eq!(
                ti4_engine_core_bridge::adapter::lane_table()[i].map(<[_]>::to_vec),
                ti4_content::hyperlane::lanes_of(id),
                "{id}"
            );
        }
    }
    // Scope: the POK index holds exactly the systems the table puts in the POK mask.
    let pok = all_systems(store, POK);
    let mask = scope(POK);
    for (i, id) in SYSTEM_NAMES.iter().enumerate() {
        assert_eq!(
            mask.system(SystemIx::ALL[i]).is_some(),
            pok.contains_key(id),
            "{id}"
        );
    }
}

#[test]
fn unit_flags_the_movement_slice_reads_match_the_corpus() {
    // The planet and unit data as a whole are tested by the generator (ti4-engine-core/tests/adapter.rs).
    let store = ContentStore::embedded();
    let units = ti4_content::units::catalogue(store, FULL);
    for (i, id) in UNIT_NAMES.iter().enumerate() {
        let data = ti4_engine_core::UnitIx::ALL[i].data();
        if let Some(u) = units.get(id) {
            assert_eq!(data.ship, u.is_ship(), "{id}");
            assert_eq!(moves_as_ship(data), u.moves_as_ship(), "{id}");
        }
    }
    assert_eq!(<SystemIx as IdStr>::from_id("18"), Some(SystemIx::MecatolRex18));
    let _ = PLANET_NAMES;
}

#[test]
fn every_template_opening_matches() {
    let (positions, refused) = template_openings();
    assert!(positions.len() >= 30, "built {} (refused {refused:?})", positions.len());
    let mut report = Report::default();
    for p in &positions {
        check_galaxy_variants(p, &mut report);
        check_position(p, false, &mut report);
    }
    assert!(report.mismatches.is_empty(), "{:#?}", report.mismatches);
    assert_eq!(report.unmapped, 0);
    assert!(report.queries > 10_000, "{report:?}");
}

#[test]
fn a_natively_built_galaxy_equals_the_converted_one() {
    // The typed core can own its galaxy: built from tiles it has the adjacency the string galaxy has.
    let (positions, _) = template_openings();
    for p in positions {
        let tiles: Vec<(&str, Hex)> = p
            .galaxy
            .system_ids()
            .into_iter()
            .map(|id| {
                let h = p.galaxy.coord_of(id).unwrap();
                (id, Hex::new(h.q, h.r))
            })
            .collect();
        let off = p.galaxy.off_map_systems();
        let mut native = galaxy_native(&tiles, &off).unwrap();
        native.hyperlane_lanes = p.galaxy.hyperlane_lanes;
        let mut report = Report::default();
        check_adjacency(&p.label, &p.galaxy, &native.adjacency(), &mut report);
        assert!(report.mismatches.is_empty(), "{:#?}", report.mismatches);
        assert_eq!(galaxy_from_legacy(&p.galaxy).unwrap().adjacency().of(system_ix("18").unwrap()), native.adjacency().of(system_ix("18").unwrap()));
    }
}

#[test]
fn a_small_hand_built_board_matches() {
    let store = ContentStore::embedded();
    let galaxy = Galaxy::build(store, &["18", "19", "20", "21", "22", "23", "39", "26"], FULL, 3).unwrap();
    let raw = galaxy_from_legacy(&galaxy).unwrap();
    let adjacency = raw.adjacency();
    let mut report = Report::default();
    check_adjacency("small", &galaxy, &adjacency, &mut report);
    assert!(report.mismatches.is_empty(), "{:#?}", report.mismatches);
    let partners: BTreeSet<&str> = adjacency
        .of(system_ix("39").unwrap())
        .iter()
        .map(|i| SYSTEM_NAMES[i])
        .collect();
    assert!(partners.contains("26"), "the alpha pair");
}
