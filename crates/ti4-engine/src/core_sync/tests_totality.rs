//! Package D2-1: the fixtures the typed core accepts, the census of refusals, strict mode, and one test per
//! reachable refusal class (the audit table of `error.rs`).

#![cfg(test)]

use ti4_engine_core::state::{Slice, SliceSet};
use ti4_model::id::{FactionId, PlanetId, PlayerId, StrategyCardId, TechnologyId, UnitTypeId};
use ti4_model::state::GameState;

use super::unmapped::{self, ADAPTER};
use super::*;

/// Every slice but the map (which needs the string galaxy).
fn no_map() -> SliceSet {
    SliceSet(SliceSet::ALL.0 & !(1 << Slice::Map as u32))
}

fn json(s: &GameState) -> serde_json::Value {
    serde_json::to_value(s).unwrap()
}

fn names(n: usize) -> Vec<&'static str> {
    ["a", "b", "c", "d", "e", "f", "g", "h"][..n].to_vec()
}

// --- fixtures ---------------------------------------------------------------------------------------------

#[test]
fn a_dealt_table_of_every_lobby_faction_loads_and_round_trips_without_a_refusal() {
    // 24 lobby factions as three tables of eight seats. The fixtures are built with the areas of `TI4_AREAS` on (setup and the deal
    // lend to the core on a table whose factions are not seated yet: those refusals are the fixture's, not the codecs'): only the
    // refusals of the codec calls below are counted.
    let mut refused = 0;
    let _ = unmapped::take_thread();
    for (t, chunk) in crate::seating::LOBBY_FACTIONS.chunks(8).enumerate() {
        let seats: Vec<(&str, &str)> = names(chunk.len()).into_iter().zip(chunk.iter().copied()).collect();
        let gs = crate::fixtures::seated_dealt(&seats, ti4_model::content_types::POK);
        let _ = unmapped::take_thread();
        let mut cx = Ctx::of(&gs).unwrap();
        let core = Codec::load(&mut cx, &gs, Sources::default(), no_map())
            .unwrap_or_else(|e| panic!("table {t} {chunk:?}: {e}"));
        // store into the same table with the seats' factions rotated, so every field a codec writes shows
        let rotated: Vec<(&str, &str)> =
            seats.iter().enumerate().map(|(i, (n, _))| (*n, chunk[(i + 1) % chunk.len()])).collect();
        refused += unmapped::take_thread().total;
        let mut back = crate::fixtures::seated_dealt(&rotated, ti4_model::content_types::POK);
        let _ = unmapped::take_thread();
        Codec::store(&cx, &core, &mut back, no_map()).unwrap();
        let (a, b) = (json(&back), json(&gs));
        for (k, v) in a.as_object().unwrap() {
            assert_eq!(Some(v), b.get(k), "table {t}: field {k}");
        }
        assert_eq!(Codec::load(&mut cx, &back, Sources::default(), no_map()).unwrap(), core, "table {t}");
        refused += unmapped::take_thread().total;
    }
    assert_eq!(refused, 0, "a dealt table must not be refused anywhere");
}

#[test]
fn seated_dealt_names_the_seated_faction_in_every_note_and_a_plain_seated_game_does_not() {
    let seats = [("a", "sol"), ("b", "hacan")];
    let dealt = crate::fixtures::seated_dealt(&seats, ti4_model::content_types::POK);
    assert!(!dealt.promissory_notes.is_empty());
    assert!(dealt.promissory_notes.keys().all(|k| !k.ends_with(":generic")), "{:?}", dealt.promissory_notes);
    let plain = crate::fixtures::seated_game(&seats, ti4_model::content_types::POK);
    assert!(plain.promissory_notes.keys().all(|k| k.ends_with(":generic")), "setup deals before the factions");
    // the bare notes of the plain table are what the Cards codec refuses (trap T1)
    let mut cx = Ctx::of(&plain).unwrap();
    let _ = unmapped::take_thread();
    let err = unmapped::allow_unmapped(|| Codec::load(&mut cx, &plain, Sources::default(), no_map())).unwrap_err();
    assert_eq!((err.slice, err.what), (Slice::Cards, "faction"), "{err}");
}

#[test]
fn real_game_is_a_table_of_distinct_lobby_factions_the_typed_core_holds() {
    let gs = crate::fixtures::real_game(&["a", "b", "c"]);
    let _ = unmapped::take_thread(); // what the fixture's own setup lent under `TI4_AREAS` is not the codec's refusal
    let mut seen = std::collections::BTreeSet::new();
    for p in &gs.players {
        assert!(crate::seating::LOBBY_FACTIONS.contains(&p.faction.as_str()), "{}", p.faction);
        assert!(seen.insert(p.faction.as_str().to_owned()));
    }
    let mut cx = Ctx::of(&gs).unwrap();
    Codec::load(&mut cx, &gs, Sources::default(), no_map()).unwrap();
    assert_eq!(unmapped::take_thread().total, 0);
}

// --- the census ---------------------------------------------------------------------------------------------

#[test]
fn a_refused_state_is_counted_by_slice_and_reason() {
    // the scaffolding faction `generic` of `fixtures::game` is not a corpus faction
    let gs = crate::fixtures::game(&["a", "b"]);
    let mut cx = Ctx::of(&gs).unwrap();
    let _ = unmapped::take_thread();
    let r = unmapped::allow_unmapped(|| Codec::load(&mut cx, &gs, Sources::default(), no_map()));
    assert!(r.is_err());
    let counts = unmapped::take_thread();
    assert_eq!(counts.total, 1);
    assert_eq!(counts.by_reason.get(&(Slice::Players as u8, "faction")), Some(&1), "{counts:?}");
    assert_eq!(counts.by_name().get("Players: faction"), Some(&1));
    // drained: a second take is empty
    assert_eq!(unmapped::take_thread().total, 0);
}

#[test]
fn the_adapter_refusals_are_counted_under_their_own_name() {
    let _ = unmapped::take_thread();
    unmapped::note_adapter("adapter: system", "zz");
    let counts = unmapped::take_thread();
    assert_eq!(counts.by_reason.get(&(ADAPTER, "adapter: system")), Some(&1));
    assert_eq!(counts.by_name().get("adapter: adapter: system"), Some(&1));
}

#[test]
fn the_process_wide_counter_sees_what_a_thread_counted_and_is_drained_by_take() {
    // every other test that drains the process-wide counter (`dual::take_report`) holds this lock
    let _serial = crate::dual::tests::serial();
    let key = (Slice::Meta as u8, "a unit test of the census");
    let _ = unmapped::take_thread();
    let _ = unmapped::take();
    let _ = bad(Slice::Meta, "a unit test of the census", "x");
    let _ = bad(Slice::Meta, "a unit test of the census", "y");
    assert_eq!(unmapped::peek().by_reason.get(&key), Some(&2));
    assert_eq!(unmapped::take_thread().by_reason.get(&key), Some(&2));
    let drained = unmapped::take();
    assert_eq!(drained.by_reason.get(&key), Some(&2));
    assert_eq!(unmapped::peek().by_reason.get(&key), None, "take drains");
}

#[test]
fn allow_unmapped_nests_and_strict_mode_comes_back_after_it() {
    let refusal = || {
        let _ = bad(Slice::Meta, "a nesting test", "x");
    };
    unmapped::strict_on_this_thread(|| {
        unmapped::allow_unmapped(|| {
            unmapped::allow_unmapped(refusal);
            refusal(); // the outer guard is still on
        });
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let r = std::panic::catch_unwind(refusal);
        std::panic::set_hook(prev);
        assert!(r.is_err(), "outside every allow_unmapped the refusal panics again");
    });
    let _ = unmapped::take_thread();
}

#[test]
fn the_reports_of_two_runs_sum_their_refusals() {
    let mut a = crate::dual::DualReport::default();
    a.unmapped.insert("Players: faction".to_owned(), 2);
    let mut b = crate::dual::DualReport::default();
    b.unmapped.insert("Players: faction".to_owned(), 3);
    b.unmapped.insert("Board: planet".to_owned(), 1);
    a.add(&b);
    assert_eq!(a.unmapped.get("Players: faction"), Some(&5));
    assert_eq!(a.unmapped.get("Board: planet"), Some(&1));
    let text = a.to_string();
    assert!(text.contains("unmapped: 6 refusals") && text.contains("Players: faction"), "{text}");
    // an empty census prints nothing, so the text of a clean run is what it always was
    assert!(!crate::dual::DualReport::default().to_string().contains("unmapped"));
}

// --- strict mode --------------------------------------------------------------------------------------------

#[test]
#[should_panic(expected = "unmapped (strict mode): Players: faction")]
fn strict_mode_turns_the_first_refusal_into_a_panic_naming_slice_and_reason() {
    let gs = crate::fixtures::game(&["a", "b"]);
    let mut cx = Ctx::of(&gs).unwrap();
    unmapped::strict_on_this_thread(|| {
        let _ = Codec::load(&mut cx, &gs, Sources::default(), no_map());
    });
}

#[test]
fn a_test_that_refuses_on_purpose_suspends_strict_mode_and_is_still_counted() {
    let gs = crate::fixtures::game(&["a", "b"]);
    let mut cx = Ctx::of(&gs).unwrap();
    let _ = unmapped::take_thread();
    unmapped::strict_on_this_thread(|| {
        let r = unmapped::allow_unmapped(|| Codec::load(&mut cx, &gs, Sources::default(), no_map()));
        assert!(r.is_err());
    });
    assert_eq!(unmapped::take_thread().total, 1);
    // outside the closures strict is off again on this thread
    assert!(!unmapped::strict() || std::env::var("TI4_UNMAPPED").is_ok());
}

// --- the refusal audit: one test per reachable class (see the table in `error.rs`) ----------------------------

/// An id that is in no corpus category.
const BOGUS: &str = "zzz";

fn refuses(label: &str, slice: Slice, what: &str, mutate: impl FnOnce(&mut GameState)) {
    let mut gs = crate::fixtures::real_game(&["a", "b", "c"]);
    mutate(&mut gs);
    let mut cx = Ctx::of(&gs).unwrap_or_else(|e| panic!("{label}: owners: {e}"));
    let _ = unmapped::take_thread();
    let err = unmapped::allow_unmapped(|| Codec::load(&mut cx, &gs, Sources::default(), no_map()))
        .expect_err(label);
    assert_eq!((err.slice, err.what), (slice, what), "{label}: {err}");
    let counts = unmapped::take_thread();
    assert_eq!(counts.by_reason.get(&(slice as u8, err.what)), Some(&1), "{label}: {counts:?}");
}

#[test]
fn a_faction_outside_the_corpus_is_refused() {
    refuses("generic", Slice::Players, "faction", |gs| gs.players[0].faction = FactionId::new("generic"));
}

#[test]
fn a_content_id_outside_the_corpus_is_refused_by_the_slice_that_holds_it() {
    refuses("unit type", Slice::Board, "unit type", |gs| {
        let sys = gs.players[0].home_system.clone().expect("a deployed home system");
        gs.system_mut(&sys).units.push(ti4_model::units::Unit::new(UnitTypeId::new("zzz"), PlayerId::new("a")));
    });
    refuses("planet", Slice::Board, "planet", |gs| {
        gs.exhausted_planets.insert(PlanetId::new(BOGUS));
    });
    refuses("technology", Slice::Players, "technology", |gs| {
        gs.players[0].technologies.insert(TechnologyId::new("zzz"));
    });
    refuses("strategy card", Slice::Strategy, "strategy card", |gs| {
        gs.strategy_card_goods.insert(StrategyCardId::new("zzz"), 1);
    });
}

#[test]
fn a_counter_out_of_the_typed_range_is_refused_instead_of_wrapped() {
    refuses("trade goods", Slice::Players, "trade_goods", |gs| gs.players[0].trade_goods = i32::MAX);
    refuses("u32::MAX stamp", Slice::Meta, "combat_draw_round", |gs| gs.combat_draw_round = Some(u32::MAX));
}

#[test]
fn a_table_the_seats_codec_cannot_hold_is_refused() {
    refuses("nine seats", Slice::Seats, "more than 8 players", |gs| {
        for i in 0..6 {
            let mut p = gs.players[0].clone();
            p.id = PlayerId::new(format!("z{i}"));
            gs.players.push(p);
        }
    });
    refuses("seating order", Slice::Seats, "seating_order is not one entry per player", |gs| {
        gs.seating_order.pop();
    });
}

#[test]
fn more_than_sixteen_owners_are_refused_when_the_owner_table_is_built() {
    let gs = crate::fixtures::real_game(&["a", "b", "c"]);
    let extra: Vec<String> = (0..14).map(|i| format!("x{i:02}")).collect();
    let extra: Vec<&str> = extra.iter().map(String::as_str).collect();
    let _ = unmapped::take_thread();
    let err = unmapped::allow_unmapped(|| Ctx::with_extra(&gs, &extra)).unwrap_err();
    assert_eq!((err.slice, err.what), (Slice::Seats, "more than 16 owners"));
    assert_eq!(unmapped::take_thread().total, 1);
    // up to sixteen is fine
    assert!(Ctx::with_extra(&gs, &extra[..13]).is_ok());
}

/// Found by the soak (`old_100000_6_wide`, the first game: `unmapped (strict mode): Transient: owner not in the table "neutral"`, no backtrace, no window label): the
/// record of the last fight still names the neutral force when the board no longer holds one of its units (the last of them fell), and the table was built from the
/// board only. Every owner the transient part of the position names is in the table, so the `Transient` slice loads and goes back whatever the board holds.
#[test]
fn the_owner_table_holds_the_neutral_force_the_record_of_the_last_fight_names_with_no_unit_left_on_the_board() {
    let mut gs = crate::fixtures::real_game(&["a", "b"]);
    let neutral = PlayerId::new(crate::neutral_units::NEUTRAL);
    let system = ti4_model::id::SystemId::new("18");
    assert!(Ctx::of(&gs).unwrap().seat(Slice::Transient, neutral.as_str()).is_err(), "no unit of the neutral force on the board: not in the table");
    gs.last_combat_sides = Some((system.clone(), vec![PlayerId::new("a"), neutral.clone()]));
    gs.last_ship_destroyed = Some((system.clone(), neutral.clone(), UnitTypeId::new("neutral_cruiser")));
    let mut cx = Ctx::of(&gs).unwrap();
    assert!(cx.seat(Slice::Transient, neutral.as_str()).is_ok(), "named by the record of the last fight: in the table");
    let _ = unmapped::take_thread();
    let mut core = ti4_engine_core::state::CoreState::with_loaded(SliceSet::EMPTY);
    cx.attach(&mut core);
    Codec::load_slice(&mut cx, &gs, Sources::default(), Slice::Transient, &mut core).expect("the Transient slice loads");
    assert_eq!(unmapped::take_thread().total, 0);
    // ... and back.
    let mut back = gs.clone();
    back.last_combat_sides = None;
    back.last_ship_destroyed = None;
    Codec::store_slice(&cx, &core, &mut back, Slice::Transient).expect("the Transient slice goes back");
    assert_eq!(back.last_combat_sides, gs.last_combat_sides);
    assert_eq!(back.last_ship_destroyed, gs.last_ship_destroyed);
}

#[test]
fn an_owner_or_seat_the_table_does_not_know_is_refused() {
    let gs = crate::fixtures::real_game(&["a", "b"]);
    let cx = Ctx::of(&gs).unwrap();
    let _ = unmapped::take_thread();
    let e = unmapped::allow_unmapped(|| cx.seat(Slice::Seats, "nobody")).unwrap_err();
    assert_eq!(e.what, "owner not in the table");
    let e = unmapped::allow_unmapped(|| cx.owner(Slice::Seats, ti4_engine_core::Seat(9))).unwrap_err();
    assert_eq!(e.what, "seat not in the table");
}

#[test]
fn a_mark_nobody_typed_and_a_pending_tag_nobody_knows_are_refused() {
    refuses("mark", Slice::Marks, "mark not typed", |gs| {
        gs.faction_marks.insert("zz".to_owned(), "1".to_owned());
    });
    refuses("pending", Slice::Pending, "pending tag", |gs| gs.pending = Some("zz".to_owned()));
}

#[test]
fn a_promissory_note_that_is_not_alias_and_faction_is_refused() {
    refuses("no colon", Slice::Cards, "promissory note id", |gs| {
        gs.promissory_notes.insert("zzz".to_owned(), PlayerId::new("a"));
    });
    refuses("unknown alias", Slice::Cards, "promissory note", |gs| {
        gs.promissory_notes.insert("zz:sol".to_owned(), PlayerId::new("a"));
    });
}

#[test]
fn the_map_slice_without_the_string_galaxy_is_refused() {
    let gs = crate::fixtures::real_game(&["a", "b"]);
    let mut cx = Ctx::of(&gs).unwrap();
    let mut core = ti4_engine_core::state::CoreState::with_loaded(SliceSet::EMPTY);
    let _ = unmapped::take_thread();
    let e = unmapped::allow_unmapped(|| Codec::load_slice(&mut cx, &gs, Sources::default(), Slice::Map, &mut core))
        .unwrap_err();
    assert_eq!((e.slice, e.what), (Slice::Map, "the map slice needs the string galaxy"));
}

#[test]
fn the_text_table_refuses_the_65536th_text_and_a_code_it_never_issued() {
    let mut t = Texts::default();
    let _ = unmapped::take_thread();
    // Code(0) is the diplomacy rules tag; 65 534 more fill the table to 65 535 texts
    for i in 1..usize::from(u16::MAX) {
        t.intern(Slice::Marks, &format!("t{i}")).unwrap();
    }
    assert_eq!(t.len(), usize::from(u16::MAX));
    let again = t.intern(Slice::Marks, "t1").unwrap();
    assert_eq!(t.text(Slice::Marks, again).unwrap(), "t1", "a known text still resolves when the table is full");
    let e = unmapped::allow_unmapped(|| t.intern(Slice::Marks, "one more")).unwrap_err();
    assert_eq!(e.what, "text table full");
    let fresh = Texts::default();
    let e = unmapped::allow_unmapped(|| fresh.text(Slice::Marks, ti4_engine_core::state::Code(5))).unwrap_err();
    assert_eq!(e.what, "unknown text code");
}

// --- creation-time guards ---------------------------------------------------------------------------------------

fn ids(list: &[&str]) -> Vec<PlayerId> {
    list.iter().map(|s| PlayerId::new(*s)).collect()
}

#[test]
fn an_id_that_is_a_proper_prefix_of_another_is_refused_at_creation_and_would_leave_the_keys_unmapped() {
    let e = guard::ids_supported(&ids(&["p1", "p10"])).unwrap_err();
    assert!(e.contains("\"p1\" is a prefix of \"p10\""), "{e}");
    let e = guard::ids_supported(&ids(&["p10", "p1"])).unwrap_err();
    assert!(e.contains("\"p1\" is a prefix of \"p10\""), "the shorter id is named first whatever the order: {e}");
    assert!(guard::ids_supported(&ids(&["ab", "abc", "x"])).is_err());
    // what the guard protects: the key codec has no owner table for such a game
    let gs = crate::fixtures::real_game(&["p1", "p10"]);
    let cx = Ctx::of(&gs).unwrap();
    assert!(cx.keys.owners.is_none(), "the key order cannot represent a prefix owner");
    // and the shapes the products generate are all fine
    let hex = "0123456789abcdef".repeat(4);
    assert!(guard::ids_supported(&ids(&[&format!("player_{hex}"), &format!("player_{}", hex.replace('0', "1"))])).is_ok());
    let seat_ids: Vec<String> = (1..=8).map(|i| format!("seat_{i:02}")).collect();
    assert!(guard::ids_supported(&seat_ids.iter().map(|s| PlayerId::new(s.as_str())).collect::<Vec<_>>()).is_ok());
    assert!(guard::ids_supported(&ids(&["p0", "p1", "p2", "p3", "p4", "p5", "p6", "p7"])).is_ok());
    assert!(guard::ids_supported(&ids(&["a", "b", "c"])).is_ok());
}

#[test]
fn more_than_eight_non_ascii_empty_or_repeated_ids_are_refused_at_creation() {
    assert!(guard::ids_supported(&ids(&["a", "b", "c", "d", "e", "f", "g", "h", "i"])).unwrap_err().contains("at most 8"));
    assert!(guard::ids_supported(&ids(&["a", "\u{e4}"])).is_err());
    assert!(guard::ids_supported(&ids(&["a", ""])).is_err());
    assert!(guard::ids_supported(&ids(&["a", "a"])).unwrap_err().contains("twice"));
}

#[test]
fn only_corpus_factions_pass_the_creation_guard() {
    assert!(guard::factions_known(crate::seating::LOBBY_FACTIONS).is_ok());
    assert!(guard::factions_known(["sol", "generic"]).unwrap_err().contains("generic"));
}
