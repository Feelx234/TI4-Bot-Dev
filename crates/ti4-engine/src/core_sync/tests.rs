use ti4_engine_core::state::{CoreState, PlanetFlags, Slice, SliceSet};
use ti4_model::state::GameState;

use super::*;

fn json(s: &GameState) -> serde_json::Value {
    serde_json::to_value(s).unwrap()
}

/// The fixture tables seat the placeholder faction `generic`, which the corpus does not have: give
/// every seat a real one.
fn seated(names: &[&str]) -> GameState {
    let mut gs = crate::fixtures::game(names);
    for (i, p) in gs.players.iter_mut().enumerate() {
        p.faction = ti4_model::id::FactionId::new(crate::generated::IdStr::as_str(ti4_engine_core::generated::Faction::ALL[i]));
    }
    gs.promissory_notes.clear();
    gs.promissory_faceup.clear();
    gs
}

fn game() -> GameState {
    seated(&["a", "b", "c", "d"])
}

#[test]
fn every_slice_round_trips_on_a_fresh_game() {
    let gs = game();
    let mut cx = Ctx::of(&gs).unwrap();
    // The map slice needs the string galaxy (not in `GameState`); the bridge gate covers it.
    let all = SliceSet(SliceSet::ALL.0 & !(1 << Slice::Map as u32));
    let src = Sources::default();
    let core = Codec::load(&mut cx, &gs, src, all).unwrap();
    // store into a different game so a field no codec writes would show
    let mut back = seated(&["w", "x", "y"]);
    Codec::store(&cx, &core, &mut back, all).unwrap();
    let (a, b) = (json(&back), json(&gs));
    for (k, v) in a.as_object().unwrap() {
        assert_eq!(Some(v), b.get(k), "field {k}");
    }
    assert_eq!(Codec::load(&mut cx, &back, src, all).unwrap(), core);
}

#[test]
fn a_lend_stores_only_the_written_slices() {
    let mut gs = game();
    let mut cx = Ctx::of(&gs).unwrap();
    let before = gs.clone();
    let fp = Footprint::new(&[Slice::Players, Slice::Seats], &[Slice::Players]);
    let mut lend = Lend::new(&mut gs, &mut cx, Sources::default());
    lend.with_core(fp, |core, _| {
        core.players_mut().edit(1).econ.trade_goods = 7;
        // a slice outside the footprint is not stored even if the call touches the unloaded default
        core.meta.round = 99;
    })
    .unwrap();
    assert_eq!(gs.players[1].trade_goods, 7);
    assert_eq!(gs.round, before.round);
    let mut expect = before;
    expect.players[1].trade_goods = 7;
    assert_eq!(json(&gs), json(&expect));
}

#[test]
fn a_region_loads_once_and_runs_several_calls() {
    let mut gs = game();
    let mut cx = Ctx::of(&gs).unwrap();
    let fp = Footprint::new(&[Slice::Players, Slice::Seats], &[Slice::Players]);
    let mut lend = Lend::new(&mut gs, &mut cx, Sources::default());
    lend.region(fp, |r| {
        r.call(Footprint::new(&[Slice::Players], &[Slice::Players]), |c, _| c.players_mut().edit(0).econ.trade_goods = 3);
        r.call(Footprint::new(&[Slice::Players], &[Slice::Players]), |c, _| c.players_mut().edit(0).econ.trade_goods += 4);
    })
    .unwrap();
    assert_eq!(lend.stats.cost(Slice::Players).loads, 1);
    assert_eq!(gs.players[0].trade_goods, 7);
}

#[test]
fn a_view_loads_only_what_it_reads() {
    let gs = game();
    let mut cx = Ctx::of(&gs).unwrap();
    let view = CoreView::of(&gs, &mut cx, Sources::default(), SliceSet::of(&[Slice::Meta, Slice::Seats])).unwrap();
    assert_eq!(view.meta().round, gs.round);
    assert_eq!(view.seats().n_players(), 4);
    assert!(!view.loaded.has(Slice::Board));
}

#[test]
fn an_out_of_range_counter_is_unmapped_not_wrapped() {
    let mut gs = game();
    gs.players[0].trade_goods = 40_000;
    let mut cx = Ctx::of(&gs).unwrap();
    let e = Codec::load(&mut cx, &gs, Sources::default(), SliceSet::of(&[Slice::Seats, Slice::Players])).unwrap_err();
    assert_eq!(e.slice, Slice::Players);
}

#[test]
fn an_unknown_id_is_unmapped() {
    let mut gs = game();
    gs.unclaimed_strategy_cards.push(ti4_model::id::StrategyCardId::new("not_a_card"));
    let mut cx = Ctx::of(&gs).unwrap();
    assert!(Codec::load(&mut cx, &gs, Sources::default(), SliceSet::of(&[Slice::Strategy])).is_err());
}

#[test]
fn empty_and_absent_entries_stay_distinct() {
    let mut gs = game();
    let (sys, planet) = crate::fixtures::a_placed_planet();
    gs.board.entry(sys.clone()).or_default().planet_units.insert(planet.clone(), Vec::new());
    let mut cx = Ctx::of(&gs).unwrap();
    let core = Codec::load(&mut cx, &gs, Sources::default(), SliceSet::of(&[Slice::Board])).unwrap();
    let p = ti4_engine_core::generated::Planet::ALL
        .iter()
        .find(|p| crate::generated::IdStr::as_str(**p) == planet.as_str())
        .unwrap();
    assert!(core.board().planet(*p).flags.has(PlanetFlags::UNITS_ENTRY));
    let mut back = gs.clone();
    back.board.clear();
    Codec::store(&cx, &core, &mut back, SliceSet::of(&[Slice::Board])).unwrap();
    assert_eq!(json(&back), json(&gs));
}

#[test]
fn text_codes_are_stable_between_lends() {
    let mut gs = game();
    gs.faction_marks.insert("x:a:b".into(), "1".into());
    let mut cx = Ctx::of(&gs).unwrap();
    let a = Codec::load(&mut cx, &gs, Sources::default(), SliceSet::of(&[Slice::Marks])).unwrap();
    let b = Codec::load(&mut cx, &gs, Sources::default(), SliceSet::of(&[Slice::Marks])).unwrap();
    assert_eq!(a, b);
    let _ = CoreState::default();
}

#[test]
fn state_sizes_stay_inside_the_f1_targets() {
    use std::mem::size_of;
    assert!(size_of::<CoreState>() <= 4096, "CoreState {}", size_of::<CoreState>());
    assert!(size_of::<ti4_engine_core::state::Player>() <= 600, "Player {}", size_of::<ti4_engine_core::state::Player>());
}
