//! Writes the board view of a real engine state before and after the Muaat hero's Nova Seed
//! replaces a system tile, for the GK-nova-seed-tile screenshots (`web/e2e/screenshots`).
//!
//! `cargo run -p ti4-host --example nova_seed_board -- <out dir>`
use ti4_content::ContentStore;
use ti4_engine::movement::{MapEdit, NOVA_SEED, apply_map_edit};
use ti4_host::map::{build_board_tiles, create_game_with_template};
use ti4_host::projection::project_board_view_with_map;
use ti4_model::content_types::POK;
use ti4_model::id::{PlayerId, SystemId, UnitTypeId};
use ti4_model::units::Unit;

fn main() {
    let out = std::env::args().nth(1).expect("output directory");
    let content = ContentStore::embedded();
    let players: Vec<PlayerId> = ["gallery_seat", "other_seat", "p3", "p4", "p5", "p6"]
        .into_iter()
        .map(PlayerId::new)
        .collect();
    let (mut state, mut galaxy) =
        create_game_with_template(content, &players, 4, Some("6pStandard")).unwrap();
    let setup = build_board_tiles(content, &galaxy);
    let target = setup
        .iter()
        .filter(|tile| tile.special_area.is_none() && !tile.planets.is_empty())
        .filter(|tile| {
            tile.system_id != "18"
                && !ti4_content::galaxy::is_home_system(content, &tile.system_id, POK)
        })
        .nth(3)
        .expect("a non-home system with planets")
        .system_id
        .clone();
    let system = SystemId::new(target.clone());
    let unit = |kind: &str, owner: &PlayerId| Unit::new(UnitTypeId::new(kind), owner.clone());
    let (muaat, other) = (&players[0], &players[1]);
    state.system_mut(&system).add(&[
        unit("muaat_warsun", muaat),
        unit("cruiser", muaat),
        unit("destroyer", muaat),
        unit("cruiser", other),
    ]);
    let write = |name: &str, state: &ti4_model::state::GameState| {
        let view = project_board_view_with_map(state, &setup);
        std::fs::write(
            format!("{out}/{name}.json"),
            serde_json::to_string_pretty(&view).unwrap(),
        )
        .unwrap();
    };
    write("before", &state);
    // The hero destroys the other players' units, then replaces the tile (the caller of
    // `apply_map_edit` clears the planets first, as the hero does).
    state
        .system_mut(&system)
        .units
        .retain(|unit| &unit.owner == muaat);
    apply_map_edit(
        &mut state,
        &mut galaxy,
        content,
        POK,
        &MapEdit::Replace {
            old: target.clone(),
            new: NOVA_SEED.to_owned(),
        },
    )
    .unwrap();
    state.active_system = Some(SystemId::new(NOVA_SEED));
    write("after", &state);
    println!("replaced {target} with {NOVA_SEED}");
}
