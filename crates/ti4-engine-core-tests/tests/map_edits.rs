//! Lane M6d: the map edits a faction effect records (the Ghosts hero swaps two systems, the Muaat hero replaces one) and the replay of them onto the
//! core's own map (`galaxy::CoreGalaxy::{swap_systems, replace_system}`, `kernel::fx_map`): the twin of the string `Galaxy::{swap_systems,
//! replace_system}` and `movement::replay_map_edits`.

use ti4_engine_core::galaxy::{CoreGalaxy, PlaceError};
use ti4_engine_core::generated::{Faction, System};
use ti4_engine_core::hex::Hex;
use ti4_engine_core::kernel::fx_map::{self, MapEdit};
use ti4_engine_core::state::CoreState;
use ti4_engine_core::tables::{SYSTEM_COUNT, WH_ALPHA};
use ti4_engine_core_tests::scenario::{plain_systems, Scenario};

static NO_LANES: [Option<&[(u8, u8)]>; SYSTEM_COUNT] = [None; SYSTEM_COUNT];

fn row() -> (CoreGalaxy, [System; 3]) {
    let p = plain_systems();
    let tiles = [p[0], p[1], p[2]];
    let mut g = CoreGalaxy::new(&NO_LANES);
    for (i, t) in tiles.iter().enumerate() {
        g.place(*t, Hex::new(i as i32, 0)).unwrap();
    }
    (g, tiles)
}

#[test]
fn a_swap_exchanges_the_hexes_and_counts_the_edit() {
    let (mut g, [a, b, c]) = row();
    assert_eq!(g.edits_applied(), 0);
    g.swap_systems(a, c).unwrap();
    assert_eq!((g.coord_of(a), g.coord_of(c), g.coord_of(b)), (Some(Hex::new(2, 0)), Some(Hex::new(0, 0)), Some(Hex::new(1, 0))));
    assert_eq!((g.system_at(Hex::new(0, 0)), g.system_at(Hex::new(2, 0))), (Some(c), Some(a)));
    assert_eq!(g.edits_applied(), 1);
    assert_eq!(g.swap_systems(b, b), Err(PlaceError::Duplicate(b)), "a system with itself");
    let off = System::ALL.iter().copied().find(|s| !tile_in(&g, *s)).unwrap();
    assert_eq!(g.swap_systems(a, off), Err(PlaceError::OffGrid(off)), "a system that is not on the grid");
    assert_eq!(g.edits_applied(), 1, "a refused swap counts nothing");
}

fn tile_in(g: &CoreGalaxy, s: System) -> bool {
    g.coord_of(s).is_some()
}

#[test]
fn a_replace_puts_the_new_tile_on_the_hex_and_takes_the_old_one_off_the_board() {
    let (mut g, [a, b, c]) = row();
    let new = System::ALL.iter().copied().find(|s| !tile_in(&g, *s) && !s.data().hyperlane && s.data().wormholes.is_empty()).unwrap();
    g.set_wormholes(b, WH_ALPHA);
    g.replace_system(b, new).unwrap();
    assert_eq!((g.coord_of(b), g.coord_of(new)), (None, Some(Hex::new(1, 0))));
    assert_eq!(g.system_at(Hex::new(1, 0)), Some(new));
    assert!(!g.in_play().contains(b.index()) && g.in_play().contains(new.index()));
    assert_eq!(g.wormhole_kinds(b), 0, "the old tile's wormholes left with it");
    assert_eq!((g.coord_of(a), g.coord_of(c)), (Some(Hex::new(0, 0)), Some(Hex::new(2, 0))));
    assert_eq!(g.edits_applied(), 1);
    // The old tile is off the grid; the new one is in play already.
    assert_eq!(g.replace_system(b, a), Err(PlaceError::OffGrid(b)));
    assert_eq!(g.replace_system(a, new), Err(PlaceError::Duplicate(new)));
    assert_eq!(g.edits_applied(), 1);
}

fn state_with_row() -> (CoreState, [System; 3]) {
    let p = plain_systems();
    let tiles = [p[0], p[1], p[2]];
    let (core, _) = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_hexes(&[(tiles[0], Hex::new(0, 0)), (tiles[1], Hex::new(1, 0)), (tiles[2], Hex::new(2, 0))]).build();
    (core, tiles)
}

#[test]
fn recorded_edits_are_replayed_once_and_in_order() {
    let (mut core, [a, b, c]) = state_with_row();
    let new = System::ALL.iter().copied().find(|s| core.galaxy().is_some_and(|g| !g.in_play().contains(s.index())) && !s.data().hyperlane).unwrap();
    assert!(fx_map::recorded(&core).is_empty() && fx_map::replay_map_edits(&mut core));
    fx_map::record(&mut core, MapEdit::Swap(a, c));
    fx_map::record(&mut core, MapEdit::Replace(b, new));
    assert_eq!(fx_map::recorded(&core), vec![MapEdit::Swap(a, c), MapEdit::Replace(b, new)], "oldest first");
    assert!(fx_map::replay_map_edits(&mut core));
    let g = core.galaxy().unwrap();
    assert_eq!((g.coord_of(a), g.coord_of(c), g.coord_of(new), g.coord_of(b)), (Some(Hex::new(2, 0)), Some(Hex::new(0, 0)), Some(Hex::new(1, 0)), None));
    assert_eq!(g.edits_applied(), 2);
    // Idempotent: the map counts the edits it carries.
    let before = core.clone();
    assert!(fx_map::replay_map_edits(&mut core));
    assert_eq!(core.galaxy().unwrap().edits_applied(), 2);
    assert_eq!(core, before);
}

#[test]
fn a_replay_stops_at_the_edit_that_no_longer_fits_and_keeps_the_ones_before() {
    let (mut core, [a, b, c]) = state_with_row();
    fx_map::record(&mut core, MapEdit::Swap(a, b));
    fx_map::record(&mut core, MapEdit::Replace(c, a));
    assert!(!fx_map::replay_map_edits(&mut core), "a is in play: the second edit does not fit");
    let g = core.galaxy().unwrap();
    assert_eq!((g.edits_applied(), g.coord_of(a), g.coord_of(b)), (1, Some(Hex::new(1, 0)), Some(Hex::new(0, 0))), "the first edit stays applied");
}

#[test]
fn an_edit_applied_where_it_was_recorded_is_not_replayed_again() {
    let (mut core, [a, _, c]) = state_with_row();
    assert!(fx_map::apply_to_map(&mut core, MapEdit::Swap(a, c)));
    fx_map::record(&mut core, MapEdit::Swap(a, c));
    assert!(fx_map::replay_map_edits(&mut core));
    let g = core.galaxy().unwrap();
    assert_eq!((g.edits_applied(), g.coord_of(a)), (1, Some(Hex::new(2, 0))), "swapped once, not twice");
}
