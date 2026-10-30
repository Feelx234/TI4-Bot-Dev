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

/// `combat:sustain_target` is a typed mark: the JSON the string writes loads as six fields and stores back byte for byte
/// (the planet of a planet-standing unit, none for a ship); a text of another shape stays a blob.
#[test]
fn the_sustain_target_mark_is_typed_and_round_trips() {
    let mut gs = game();
    let ship = r#"{"planet":null,"system":"18","unit":{"galvanized":false,"owner":"b","sustained_damage":true,"type_id":"dreadnought"}}"#;
    let ground = r#"{"planet":"mr","system":"18","unit":{"galvanized":true,"owner":"a","sustained_damage":true,"type_id":"naaz_voltron"}}"#;
    for (i, text) in [ship, ground].into_iter().enumerate() {
        gs.faction_marks.clear();
        gs.faction_marks.insert("combat:sustain_target".into(), text.into());
        let mut cx = Ctx::of(&gs).unwrap();
        let core = Codec::load(&mut cx, &gs, Sources::default(), SliceSet::of(&[Slice::Marks])).unwrap();
        let typed = core.marks().tail.iter().any(|(k, v)| matches!(k, ti4_engine_core::keys::MarkKey::CombatSustainTarget()) && matches!(v, ti4_engine_core::keys::MarkVal::Rec(f) if f.len() == 6));
        assert!(typed, "case {i} loads as six typed fields");
        let mut back = gs.clone();
        back.faction_marks.clear();
        Codec::store(&cx, &core, &mut back, SliceSet::of(&[Slice::Marks])).unwrap();
        assert_eq!(back.faction_marks, gs.faction_marks, "case {i}");
    }
}

#[test]
fn text_codes_are_stable_between_lends() {
    let mut gs = game();
    gs.faction_marks.insert("allow_pass".into(), "yes".into());
    gs.faction_marks.insert("combat:sustain_target".into(), r#"{"planet":"mr"}"#.into());
    let mut cx = Ctx::of(&gs).unwrap();
    let a = Codec::load(&mut cx, &gs, Sources::default(), SliceSet::of(&[Slice::Marks])).unwrap();
    let b = Codec::load(&mut cx, &gs, Sources::default(), SliceSet::of(&[Slice::Marks])).unwrap();
    assert_eq!(a, b);
    let mut back = gs.clone();
    back.faction_marks.clear();
    Codec::store(&cx, &a, &mut back, SliceSet::of(&[Slice::Marks])).unwrap();
    assert_eq!(back.faction_marks, gs.faction_marks);
    let _ = CoreState::default();
}

#[test]
fn a_mark_the_families_do_not_type_is_unmapped() {
    let mut gs = game();
    gs.faction_marks.insert("x:a:b".into(), "1".into());
    let mut cx = Ctx::of(&gs).unwrap();
    let err = Codec::load(&mut cx, &gs, Sources::default(), SliceSet::of(&[Slice::Marks])).unwrap_err();
    assert_eq!(err.slice, Slice::Marks);
}

#[test]
fn state_sizes_stay_inside_the_f1_targets() {
    use std::mem::size_of;
    assert!(size_of::<CoreState>() <= 4096, "CoreState {}", size_of::<CoreState>());
    assert!(size_of::<ti4_engine_core::state::Player>() <= 600, "Player {}", size_of::<ti4_engine_core::state::Player>());
}

// ---- D2-3: the text tables travel with the state --------------------------------------------------

mod text_tables {
    use ti4_engine_core::state::{Code, CoreState, RngDomain, SeedSource};
    use ti4_model::id::PlayerId;

    use super::*;

    const TRADE: [Slice; 2] = [Slice::Seats, Slice::Trade];

    fn with_promises(texts: &[&str]) -> GameState {
        let mut gs = game();
        for t in texts {
            gs.promises.insert((PlayerId::new("a"), PlayerId::new("b"), (*t).to_owned()), Some(true));
        }
        gs
    }

    fn code_of(core: &CoreState, text: &str) -> Code {
        Code(core.texts.codes.find(text.as_bytes()).unwrap_or_else(|| panic!("{text:?} is not in the table")) as u16)
    }

    #[test]
    fn a_code_means_the_same_text_in_every_later_lend_of_the_game() {
        let gs = with_promises(&["give a planet", "share a tech"]);
        let slices = SliceSet::of(&TRADE);
        let mut cx = Ctx::of(&gs).unwrap();
        let first = Codec::load(&mut cx, &gs, Sources::default(), slices).unwrap();
        let share = code_of(&first, "share a tech");
        // a window fact is minted between two lends ...
        let staged = cx.texts.intern(Slice::Transient, "pre_roll").unwrap();
        let second = Codec::load(&mut cx, &gs, Sources::default(), slices).unwrap();
        // ... the old codes keep their texts and the staged one travels into the next core
        assert_eq!(code_of(&second, "share a tech"), share);
        assert_eq!(code_of(&second, "pre_roll"), staged);
        assert_eq!(second.texts.code_text(Slice::Trade, share).unwrap(), "share a tech");
        // and the lineage adopted what the core minted
        let minted = Codec::load(&mut cx, &with_promises(&["a brand new promise"]), Sources::default(), slices).unwrap();
        assert_eq!(cx.texts.text(Slice::Trade, code_of(&minted, "a brand new promise")).unwrap(), "a brand new promise");
    }

    #[test]
    fn a_lend_hands_its_minted_texts_back_to_the_context() {
        let mut gs = with_promises(&["only in the core"]);
        let mut cx = Ctx::of(&gs).unwrap();
        assert_eq!(cx.texts.len(), 1);
        let fp = Footprint::new(&[Slice::Seats, Slice::Trade], &[Slice::Trade]);
        let code = with_core(&mut gs, &mut cx, Sources::default(), fp, |core, _| code_of(core, "only in the core")).unwrap();
        assert_eq!(cx.texts.text(Slice::Trade, code).unwrap(), "only in the core");
        // the next lend of the same game finds it under the same code
        let again = with_core(&mut gs, &mut cx, Sources::default(), fp, |core, _| code_of(core, "only in the core")).unwrap();
        assert_eq!(code, again);
    }

    #[test]
    fn the_store_resolves_codes_from_the_state_not_from_the_context() {
        let gs = with_promises(&["give a planet"]);
        let mut cx = Ctx::of(&gs).unwrap();
        let core = Codec::load(&mut cx, &gs, Sources::default(), SliceSet::of(&TRADE)).unwrap();
        // a state that carries its tables stores in any context of the game, even a fresh one
        let fresh = Ctx::of(&gs).unwrap();
        let mut back = game();
        Codec::store(&fresh, &core, &mut back, SliceSet::of(&[Slice::Trade])).unwrap();
        assert_eq!(back.promises, gs.promises);
        // a state without its tables cannot say what a code means
        let mut lost = core.clone();
        lost.texts = Default::default();
        let e = Codec::store(&fresh, &lost, &mut back, SliceSet::of(&[Slice::Trade])).unwrap_err();
        assert_eq!((e.slice, e.what), (Slice::Trade, "unknown text code"));
    }

    #[test]
    fn a_blob_is_resolved_from_the_state_too() {
        let mut gs = game();
        gs.faction_marks.insert("combat:sustain_target".into(), r#"{"planet":"mr"}"#.into());
        let mut cx = Ctx::of(&gs).unwrap();
        let core = Codec::load(&mut cx, &gs, Sources::default(), SliceSet::of(&[Slice::Marks])).unwrap();
        assert_eq!(core.texts.blobs.len(), 1);
        assert_eq!(core.texts.blobs.get(0), Some(r#"{"planet":"mr"}"#.as_bytes()));
        let mut back = gs.clone();
        back.faction_marks.clear();
        Codec::store(&Ctx::of(&gs).unwrap(), &core, &mut back, SliceSet::of(&[Slice::Marks])).unwrap();
        assert_eq!(back.faction_marks, gs.faction_marks, "any context of the game stores it");
        let mut lost = core.clone();
        lost.texts.blobs = Default::default();
        let e = Codec::store(&cx, &lost, &mut back, SliceSet::of(&[Slice::Marks])).unwrap_err();
        assert_eq!(e.what, "mark without a text");
    }

    #[test]
    fn clones_of_a_state_resolve_their_own_texts_and_the_shared_base() {
        let gs = with_promises(&["shared"]);
        let mut cx = Ctx::of(&gs).unwrap();
        let mut a = Codec::load(&mut cx, &gs, Sources::default(), SliceSet::of(&TRADE)).unwrap();
        a.texts.compact();
        let mut b = a.clone();
        assert!(a.texts.codes.shares_base_with(&b.texts.codes));
        let left = a.texts.intern_code(Slice::Trade, "left").unwrap();
        let right = b.texts.intern_code(Slice::Trade, "right").unwrap();
        assert_eq!(left, right, "the next free code in each lineage");
        assert_eq!(a.texts.code_text(Slice::Trade, left).unwrap(), "left");
        assert_eq!(b.texts.code_text(Slice::Trade, right).unwrap(), "right");
        assert_eq!(a.texts.code_text(Slice::Trade, code_of(&a, "shared")).unwrap(), "shared");
        assert_eq!(b.texts.code_text(Slice::Trade, code_of(&b, "shared")).unwrap(), "shared");
        assert_ne!(a, b);
    }

    #[test]
    fn a_full_code_table_is_unmapped_not_a_panic() {
        let mut texts = CoreState::default().texts;
        for i in 1..usize::from(u16::MAX) {
            texts.intern_code(Slice::Transient, &i.to_string()).unwrap();
        }
        assert_eq!(texts.codes.len(), usize::from(u16::MAX));
        let e = texts.intern_code(Slice::Transient, "one more").unwrap_err();
        assert_eq!((e.slice, e.what), (Slice::Transient, "text table full"));
        assert_eq!(texts.intern_code(Slice::Transient, "7").unwrap(), Code(7), "a known text still resolves");
        assert_eq!(texts.codes.len(), usize::from(u16::MAX), "a refused text is not added");
    }

    #[test]
    fn a_loaded_state_carries_the_owner_table_and_clones_share_it() {
        let gs = game();
        let mut cx = Ctx::of(&gs).unwrap();
        let a = Codec::load(&mut cx, &gs, Sources::default(), SliceSet::of(&[Slice::Seats])).unwrap();
        let b = Codec::load(&mut cx, &gs, Sources::default(), SliceSet::of(&[Slice::Players, Slice::Seats])).unwrap();
        assert_eq!(a.owners.len(), 4);
        let ids: Vec<&[u8]> = a.owners.iter().collect();
        assert_eq!(ids, cx.owners.ids().iter().map(String::as_bytes).collect::<Vec<_>>());
        assert!(a.owners.shares_with(&b.owners), "one table per game, not per lend");
        assert!(a.clone().owners.shares_with(&a.owners));
    }

    #[test]
    fn an_owner_that_joins_after_the_table_was_made_is_unmapped() {
        let mut gs = game();
        let mut cx = Ctx::of(&gs).unwrap();
        assert!(cx.refresh(&gs).is_ok());
        gs.styx_holder = Some(PlayerId::new("stranger"));
        let e = cx.refresh(&gs).unwrap_err();
        assert_eq!((e.slice, e.what, e.text.as_str()), (Slice::Seats, "owner not in the game's table", "stranger"));
        let fp = Footprint::reads(&[Slice::Seats]);
        assert!(with_core(&mut gs, &mut cx, Sources::default(), fp, |_, _| ()).is_err());
        // a table made after the owner joined has him
        let mut named = Ctx::with_extra(&gs, &[]).unwrap();
        assert!(named.refresh(&gs).is_ok());
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "has no text in the code table")]
    fn a_dyn_rng_domain_without_a_text_fails_in_debug_instead_of_seeding_wrong() {
        let seeds = super::codec::rng::TextSeeds(CoreState::default().texts);
        let _ = seeds.seed_for(1, RngDomain::Dyn(9, 0));
    }

    #[test]
    fn a_dyn_rng_domain_minted_before_the_clone_seeds_like_the_string_engine() {
        let mut cx = Ctx::of(&game()).unwrap();
        let d = super::codec::rng::domain(&mut cx, "a stream nobody named before").unwrap();
        assert!(matches!(d, RngDomain::Dyn(..)));
        let seeds = super::codec::rng::TextSeeds::of(&cx);
        assert_eq!(seeds.seed_for(5, d), crate::rng::GameRng::derive_seed(5, "a stream nobody named before"));
    }
}
