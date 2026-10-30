//! Package W17 (docs/todo/lent-bodies.md): the typed core can hold every table the product creates. The positions the core
//! refuses (the fixture faction, more than 16 owners, an owner id that is a prefix of another, an id with no typed form) and the
//! factions the lobby never seats are assertions of the product (`ti4_engine::product`), so a table the product builds must
//! load into the core in strict mode without one of them firing.
//!
//! * Every map template that builds, with lobby factions seated on it (rotated, so each lobby faction sits at some table).
//! * Every `init.json` of the alpha games under `/var/lib/ti4-public` (or `$TI4_ALPHA_DIR`), when the directory exists.

use std::collections::BTreeMap;

use ti4_content::ContentStore;
use ti4_engine::core_sync::{Codec, Ctx, Slice, SliceSet, Sources};
use ti4_engine::product::with_strict;
use ti4_engine::seating::LOBBY_FACTIONS;
use ti4_model::id::{FactionId, PlayerId};
use ti4_model::state::GameState;

/// Every slice the string state carries, except the dice streams (they need the stream positions of a running game) and, when no
/// galaxy is given, the map.
fn slices(with_map: bool) -> SliceSet {
    let without_rng = SliceSet::ALL.0 & !(1 << Slice::Rng as u32);
    SliceSet(if with_map { without_rng } else { without_rng & !(1 << Slice::Map as u32) })
}

fn load_all(gs: &GameState, galaxy: Option<&ti4_content::galaxy::Galaxy>, what: &str) {
    with_strict(true, || {
        let mut cx = Ctx::of(gs).unwrap_or_else(|e| panic!("{what}: {e}"));
        Codec::load(&mut cx, gs, Sources { galaxy, rng: None }, slices(galaxy.is_some())).unwrap_or_else(|e| panic!("{what}: {e}"));
    });
}

#[test]
fn every_map_template_that_builds_loads_into_the_core_with_lobby_factions() {
    let content = ContentStore::embedded();
    let catalog = ti4_host::maps::catalog::catalog(None).expect("the template catalogue");
    let mut built = 0;
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for (index, template) in catalog.iter().filter(|t| t.buildable).enumerate() {
        let n = template.player_count;
        if !(2..=8).contains(&n) {
            continue;
        }
        let ids: Vec<PlayerId> = (1..=n).map(|i| PlayerId::new(format!("seat_{i:02}"))).collect();
        let assignments: BTreeMap<PlayerId, FactionId> =
            ids.iter().enumerate().map(|(i, id)| (id.clone(), FactionId::new(LOBBY_FACTIONS[(index * 5 + i) % LOBBY_FACTIONS.len()]))).collect();
        for faction in assignments.values() {
            *seen.entry(LOBBY_FACTIONS.iter().find(|f| **f == faction.as_str()).expect("a lobby faction")).or_default() += 1;
        }
        let (gs, galaxy) = ti4_host::map::create_game_with_template_seated(content, &ids, 7 + index as u64, Some(&template.alias), &assignments, None)
            .unwrap_or_else(|e| panic!("{}: {e}", template.alias));
        load_all(&gs, Some(&galaxy), &template.alias);
        built += 1;
    }
    assert!(built >= 10, "only {built} templates were built");
    assert!(seen.len() >= 20, "the rotation seats {} lobby factions only", seen.len());
}

#[test]
fn every_alpha_game_loads_into_the_core() {
    let root = std::env::var("TI4_ALPHA_DIR").unwrap_or_else(|_| "/var/lib/ti4-public".to_owned());
    let Ok(dir) = std::fs::read_dir(&root) else {
        eprintln!("no alpha games under {root}: skipped");
        return;
    };
    let mut loaded = 0;
    for entry in dir.flatten() {
        let path = entry.path().join("init.json");
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let value: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let Some(initial) = value.pointer("/payload/initial_state") else { continue };
        let gs: GameState = serde_json::from_value(initial.clone()).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        load_all(&gs, None, &path.display().to_string());
        loaded += 1;
    }
    eprintln!("{loaded} alpha games loaded");
}
