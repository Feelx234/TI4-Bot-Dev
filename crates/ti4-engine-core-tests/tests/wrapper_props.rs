//! D4-3: properties of the wrapper (the string engine's side of the codec) on real positions: the template openings of every map and
//! seat count, `fixtures::game` tables with real factions, and positions of golden games. The string engine is used as a **position
//! factory** here, not as an oracle (`legacy-positions`): what is checked is the codec and the typed state, never the string rules.
//!
//! Run with `cargo test -p ti4-engine-core-tests --features wrapper wrapper_props` (builds `ti4-engine`; minutes).

use ti4_engine::core_sync::{Codec, Ctx, Slice, SliceSet, Sources};
use ti4_engine_core::state::CoreState;
use ti4_engine_core_bridge::workload::{Position, golden_positions, template_openings};
use ti4_model::state::GameState;

const GOLDEN_CONFIG: &str = "../../tools/engine_profile/profile-20.json";

fn json(state: &GameState) -> serde_json::Value {
    serde_json::to_value(state).expect("a state serialises")
}

/// The fixture tables seat the placeholder faction `generic`, which the corpus does not have: seat real ones.
fn real_fixture(names: &[&str]) -> GameState {
    let mut gs = ti4_engine::fixtures::game(names);
    for (i, p) in gs.players.iter_mut().enumerate() {
        p.faction = ti4_model::id::FactionId::new(ti4_engine::generated::IdStr::as_str(ti4_engine_core::generated::Faction::ALL[i]));
    }
    gs.promissory_notes.clear();
    gs.promissory_faceup.clear();
    gs
}

/// P7 on one position: `store(load(g)) == g` byte for byte (stored into the PREVIOUS position, so a field no codec writes would keep the
/// other position's value) and `load(store(x)) == x`. `Err` carries the reason a position was not mapped (a documented fallback, counted).
fn round_trip(p: &Position, previous: Option<&GameState>) -> Result<Vec<String>, String> {
    let mut cx = Ctx::of(&p.state).map_err(|e| format!("{e}"))?;
    let src = Sources { galaxy: Some(&p.galaxy), rng: None };
    let mut core = CoreState::with_loaded(SliceSet::EMPTY);
    for slice in Slice::ALL {
        Codec::load_slice(&mut cx, &p.state, src, slice, &mut core).map_err(|e| format!("{:?}: {}", e.slice, e.what))?;
    }
    let mut back = previous.cloned().unwrap_or_else(|| p.state.clone());
    let mut bad = Vec::new();
    for slice in Slice::ALL {
        if let Err(e) = Codec::store_slice(&cx, &core, &mut back, slice) {
            bad.push(format!("{}: store {e}", p.label));
        }
    }
    if json(&back) != json(&p.state) {
        bad.push(format!("{}: store(load(g)) differs from g", p.label));
    } else if serde_json::to_vec(&back).ok() != serde_json::to_vec(&p.state).ok() {
        bad.push(format!("{}: serialised bytes differ", p.label));
    }
    match Codec::load(&mut cx, &back, src, SliceSet::ALL) {
        Ok(again) if again == core => {}
        Ok(_) => bad.push(format!("{}: load(store(x)) differs from x", p.label)),
        Err(e) => bad.push(format!("{}: reload {e}", p.label)),
    }
    Ok(bad)
}

fn check(positions: &[Position]) -> (usize, usize, Vec<String>) {
    let (mut mapped, mut unmapped) = (0, 0);
    let mut bad = Vec::new();
    let mut previous: Option<GameState> = None;
    for p in positions {
        match round_trip(p, previous.as_ref()) {
            Ok(b) => {
                mapped += 1;
                bad.extend(b);
            }
            Err(_) => unmapped += 1,
        }
        previous = Some(p.state.clone());
    }
    (mapped, unmapped, bad)
}

#[test]
fn p7_the_codec_round_trips_every_template_opening() {
    let (positions, refused) = template_openings();
    assert!(refused.is_empty(), "templates the loader refused: {refused:?}");
    assert!(positions.len() >= 20, "{} openings", positions.len());
    let (mapped, unmapped, bad) = check(&positions);
    assert_eq!(bad, Vec::<String>::new());
    assert_eq!(unmapped, 0, "every template opening of real factions maps completely ({mapped} mapped)");
}

#[test]
fn p7_the_codec_round_trips_fixture_tables_with_real_factions() {
    let (templates, _) = template_openings();
    let galaxy = templates.first().expect("a template").galaxy.clone();
    let positions: Vec<Position> = [&["a", "b", "c"][..], &["a", "b", "c", "d"], &["a", "b", "c", "d", "e", "f"]]
        .iter()
        .map(|names| Position { label: format!("fixture {}", names.len()), state: real_fixture(names), galaxy: galaxy.clone(), sources: ti4_model::content_types::POK })
        .collect();
    let (mapped, unmapped, bad) = check(&positions);
    assert_eq!(bad, Vec::<String>::new());
    assert_eq!((mapped, unmapped), (3, 0));
}

#[test]
fn p7_the_codec_round_trips_positions_of_golden_games() {
    let (positions, hashes) = golden_positions(GOLDEN_CONFIG, 2, 40).expect("golden games");
    assert!(!positions.is_empty() && hashes.len() == 2);
    let (mapped, unmapped, bad) = check(&positions);
    assert_eq!(bad, Vec::<String>::new());
    assert!(mapped > 0, "{unmapped} unmapped positions and none mapped");
}

#[test]
fn p3_playing_the_same_games_twice_gives_the_same_positions_and_hashes() {
    let (a, ha) = golden_positions(GOLDEN_CONFIG, 1, 25).expect("golden game");
    let (b, hb) = golden_positions(GOLDEN_CONFIG, 1, 25).expect("golden game");
    assert_eq!(ha, hb, "the decision hashes of one game differ between two runs");
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(&b) {
        assert_eq!(json(&x.state), json(&y.state), "{}", x.label);
    }
}

#[test]
fn a_generic_fixture_is_refused_not_silently_accepted() {
    // The fixture factory seats the placeholder faction `generic`. The codec must refuse such a table (the core half then falls back to the
    // string engine as a counted bail); it must never map it to some real faction.
    let gs = ti4_engine::fixtures::game(&["a", "b", "c"]);
    let (templates, _) = template_openings();
    let galaxy = templates.first().expect("a template").galaxy.clone();
    let p = Position { label: "generic fixture".into(), state: gs, galaxy, sources: ti4_model::content_types::POK };
    assert!(round_trip(&p, None).is_err(), "a table with the faction `generic` must not map");
}
