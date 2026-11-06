//! Wire parity (docs/todo/D2-8-wrapper.md 1.4, 8b1): the format types of `ti4_engine::wire` are COPIES of the legacy crate's, so this
//! file is the contract that keeps them equal. Four checks:
//!
//! 1. the recorded decision corpus (`tools/engine_core/decision_corpus`, every source): each recorded `Choice` JSON reads into both
//!    types and writes back to the same JSON, and the V1/V2 `decision_hash` of the record built from it agrees with the legacy value
//!    and with the recorded one;
//! 2. values the corpus does not carry (events with every payload shape, dice rolls of a seeded stream, stream positions, a decision
//!    log of a played legacy game): JSON both ways and the `event_hash`;
//! 3. the table: the choices a legacy game asks, put to a thin `Table` with the same seeded deciders, give the same answers and the
//!    same log as the legacy table (the `Decider` protocol and the journal are copies too);
//! 4. with the feature `ts`: the TypeScript declaration and the export path of every `ts` type agree.
//!
//! `TI4_CORPUS_SOURCES=golden20,fixtures` limits the corpus sources; `TI4_DECISION_CORPUS` moves the directory.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use ti4_engine::wire as thin;
use ti4_engine_legacy as old;

fn json<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("serialises")
}

/// `value` read into `T` and written back: the same JSON.
fn round_trip<T: Serialize + DeserializeOwned>(value: &Value, what: &str) -> Value {
    let back: T = serde_json::from_value(value.clone()).unwrap_or_else(|e| panic!("{what}: {e}"));
    json(&back)
}

/// The thin type and the legacy type read the same JSON and write the same JSON, also across (written by one, read by the other).
fn same_wire<L: Serialize + DeserializeOwned, T: Serialize + DeserializeOwned>(value: &Value, what: &str) {
    let legacy = round_trip::<L>(value, what);
    let thin = round_trip::<T>(value, what);
    assert_eq!(legacy, thin, "{what}: the legacy and the thin type write different JSON");
    assert_eq!(round_trip::<T>(&legacy, what), legacy, "{what}: thin does not read what legacy wrote");
    assert_eq!(round_trip::<L>(&thin, what), thin, "{what}: legacy does not read what thin wrote");
}

fn corpus_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("TI4_DECISION_CORPUS")
        .map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/engine_core/decision_corpus"), PathBuf::from);
    dir.join("manifest.json").exists().then_some(dir)
}

#[test]
fn every_recorded_decision_has_one_wire_shape_and_one_hash() {
    let Some(dir) = corpus_dir() else {
        assert!(std::env::var_os("TI4_REQUIRE_CORPUS").is_none(), "the decision corpus is required and absent");
        eprintln!("wire_parity: no decision corpus; skipped");
        return;
    };
    let manifest: Value = serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).expect("manifest")).expect("manifest json");
    let only: Option<Vec<String>> = std::env::var("TI4_CORPUS_SOURCES").ok().map(|s| s.split(',').map(str::to_owned).collect());
    let mut seen = 0usize;
    for (name, entry) in manifest["sources"].as_object().expect("sources") {
        if only.as_ref().is_some_and(|only| !only.contains(name)) {
            continue;
        }
        let file = dir.join(entry["file"].as_str().expect("file"));
        let reader = zstd::stream::read::Decoder::new(std::fs::File::open(&file).expect("corpus file")).expect("zstd");
        for (line, text) in BufReader::new(reader).split(b'\n').enumerate() {
            let value: Value = serde_json::from_slice(&text.expect("read")).expect("json line");
            let at = format!("{name} line {}", line + 1);
            let choice = &value["choice"];
            same_wire::<old::choice::Choice, thin::choice::Choice>(choice, &at);
            if let Some(context) = choice.get("context").filter(|c| !c.is_null()) {
                same_wire::<old::decision_context::DecisionContext, thin::decision_context::DecisionContext>(context, &at);
            }
            // The record the table would journal for this answer, hashed by both.
            let chosen = value["chosen"].as_str().expect("chosen");
            let legacy_choice: old::choice::Choice = serde_json::from_value(choice.clone()).expect("legacy choice");
            let thin_choice: thin::choice::Choice = serde_json::from_value(choice.clone()).expect("thin choice");
            let mut legacy_log = old::choice::DecisionLog::default();
            legacy_log.record(&legacy_choice, &old::choice::ChoiceOption::new(chosen, ""));
            let mut thin_log = thin::choice::DecisionLog::default();
            thin_log.record(&thin_choice, &thin::choice::ChoiceOption::new(chosen, ""));
            assert_eq!(json(&legacy_log), json(&thin_log), "{at}: the journaled record");
            for (label, v_old, v_new) in [
                ("V1", old::fingerprint::CanonicalHashVersion::V1, thin::fingerprint::CanonicalHashVersion::V1),
                ("V2", old::fingerprint::CanonicalHashVersion::V2, thin::fingerprint::CanonicalHashVersion::V2),
            ] {
                let a = old::fingerprint::decision_hash(v_old, &legacy_log.records[0]);
                let b = thin::fingerprint::decision_hash(v_new, &thin_log.records[0]);
                assert_eq!(a.digest, b.digest, "{at}: decision_hash {label}");
                assert_eq!(a.version.get(), b.version.get(), "{at}: hash version {label}");
                let recorded = value[label.to_lowercase()].as_str().expect("recorded hash");
                assert_eq!(&b.digest[..16], recorded, "{at}: recorded {label}");
            }
            seen += 1;
        }
    }
    assert!(seen > 0, "no decision was compared");
    eprintln!("wire_parity: {seen} recorded decisions agree");
}

#[test]
fn events_rolls_and_stream_positions_have_one_wire_shape() {
    use old::rng::GameRng as OldRng;
    use thin::rng::GameRng as NewRng;

    // Events: every payload shape, cancelled or not, and the hashes of both versions.
    let payloads: Vec<BTreeMap<String, Value>> = vec![
        BTreeMap::new(),
        [("b".to_owned(), Value::from(2)), ("a".to_owned(), Value::from("x"))].into_iter().collect(),
        [("nested".to_owned(), serde_json::json!({"k": [1, 2, {"z": null}], "f": 1.5, "t": true}))].into_iter().collect(),
        [("big".to_owned(), Value::from(u64::MAX)), ("neg".to_owned(), Value::from(-7))].into_iter().collect(),
    ];
    for (i, payload) in payloads.iter().enumerate() {
        for cancelled in [false, true] {
            let mut a = old::event::Event::new(i as u64 + 1, "COMBAT_DICE_ROLLED", payload.clone());
            let mut b = thin::event::Event::new(i as u64 + 1, "COMBAT_DICE_ROLLED", payload.clone());
            if cancelled {
                a.cancel();
                b.cancel();
            }
            assert_eq!(json(&a), json(&b), "event {i} {cancelled}");
            same_wire::<old::event::Event, thin::event::Event>(&json(&a), "event");
            for (v_old, v_new) in [
                (old::fingerprint::CanonicalHashVersion::V1, thin::fingerprint::CanonicalHashVersion::V1),
                (old::fingerprint::CanonicalHashVersion::V2, thin::fingerprint::CanonicalHashVersion::V2),
            ] {
                assert_eq!(old::fingerprint::event_hash(v_old, &a).digest, thin::fingerprint::event_hash(v_new, &b).digest, "event hash {i}");
            }
        }
    }

    // Dice and stream positions: the same seed draws the same faces, writes the same roll records and the same positions.
    for seed in [0_u64, 1, 7, 12345] {
        let (mut old_rng, mut new_rng) = (OldRng::new(seed), NewRng::new(seed));
        let (mut old_dice, mut new_dice) = (old::dice::Dice::new(), thin::dice::Dice::new());
        let by = ti4_model::id::PlayerId::new("a");
        for round in 0..6_usize {
            let a = old_dice.roll_by(&mut old_rng, round + 1, "combat", Some(5), &by);
            let b = new_dice.roll_by(&mut new_rng, round + 1, "combat", Some(5), &by);
            assert_eq!(json(&a), json(&b), "roll seed {seed} round {round}");
            same_wire::<old::dice::Roll, thin::dice::Roll>(&json(&a), "roll");
        }
        assert_eq!(json(&old_rng.positions()), json(&new_rng.positions()), "positions, seed {seed}");
        same_wire::<old::rng::RngPositions, thin::rng::RngPositions>(&json(&old_rng.positions()), "positions");
        assert_eq!(json(&old_dice.history()), json(&new_dice.history()), "history, seed {seed}");
    }
}

/// A legacy game played on seeded random deciders: its decision log is wire data, and every question it asked is put to a thin table.
#[test]
fn a_played_game_has_one_log_and_the_thin_table_answers_the_same() {
    use std::cell::RefCell;
    use std::rc::Rc;

    use old::choice::{Capturing, SeededRandom, Table};
    use ti4_model::content_types::POK;
    use ti4_model::id::PlayerId;

    let content = ti4_content::ContentStore::embedded();
    let ids: Vec<PlayerId> = ["a", "b", "c"].iter().map(|n| PlayerId::new(*n)).collect();
    let state = old::setup::start_game_seeded(content, &ids, POK, None, 3).expect("setup");
    let mut table = Table::new();
    let mut asked: Vec<Rc<RefCell<Vec<old::choice::Choice>>>> = Vec::new();
    for (i, id) in ids.iter().enumerate() {
        let (capturing, seen) = Capturing::new(Box::new(SeededRandom::new(100 + i as u64)));
        asked.push(seen);
        table.seat(id.clone(), Box::new(capturing));
    }
    let mut game = old::game::Game::with_table(state, content, table);
    let _ = game.run(1, 400);
    let log = json(&game.table.log);
    let records = log["records"].as_array().expect("records").len();
    assert!(records > 20, "the game asked only {records} questions");
    same_wire::<old::choice::DecisionLog, thin::choice::DecisionLog>(&log, "decision log");

    // Put every captured question to a thin table with the same decider seeds, in the order the legacy game asked them.
    let mut thin_table = thin::choice::Table::new();
    for (i, id) in ids.iter().enumerate() {
        thin_table.seat(id.clone(), Box::new(thin::choice::SeededRandom::new(100 + i as u64)));
    }
    let mut cursor = vec![0usize; ids.len()];
    let legacy_records: Vec<(String, String)> = log["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["player"].as_str().unwrap().to_owned(), r["chosen"].as_str().unwrap().to_owned()))
        .collect();
    // The legacy log is in asking order across seats; the captures are per seat, so walk the log and take each seat's next capture.
    for (player, chosen) in &legacy_records {
        let seat = ids.iter().position(|id| id.as_str() == player).expect("seat");
        let choice = asked[seat].borrow()[cursor[seat]].clone();
        cursor[seat] += 1;
        let thin_choice: thin::choice::Choice = serde_json::from_value(json(&choice)).expect("thin choice");
        let answer = thin_table.ask(&thin_choice).expect("the thin table answers");
        assert_eq!(&answer.id, chosen, "{player}: the thin SeededRandom answers differently");
    }
    assert_eq!(json(&thin_table.log), log, "the thin table journals a different log");
}

#[cfg(feature = "ts")]
mod ts {
    use ts_rs::{Config, TS};

    use super::{old, thin};

    fn config() -> Config {
        Config::new().with_large_int("number").with_import_extension(Some("ts"))
    }

    fn same<L: TS + ?Sized + 'static, T: TS + ?Sized + 'static>() {
        let cfg = config();
        assert_eq!(L::name(&cfg), T::name(&cfg));
        assert_eq!(L::decl(&cfg), T::decl(&cfg), "the declaration of {}", L::name(&cfg));
        assert_eq!(L::output_path(), T::output_path(), "the export path of {}", L::name(&cfg));
        assert_eq!(L::export_to_string(&cfg).ok(), T::export_to_string(&cfg).ok(), "the export text of {}", L::name(&cfg));
    }

    /// The thin `Choice` is the legacy one plus the multi-pick field (`Choice::multi_pick`, the multi-ask conversion; the string
    /// engine never asks a multi-pick): with that field taken out, the declarations agree.
    fn same_choice_but_the_multi_pick() {
        let cfg = config();
        assert_eq!(old::choice::Choice::name(&cfg), thin::choice::Choice::name(&cfg));
        assert_eq!(old::choice::Choice::output_path(), thin::choice::Choice::output_path());
        let thin_decl = thin::choice::Choice::decl(&cfg);
        let field = "multi_pick?: MultiPick | null, ";
        let end = thin_decl.find(field).expect("the thin Choice has the multi-pick field") + field.len();
        let start = thin_decl[..end].rfind("\n/**").expect("the field's doc comment");
        let without = format!("{}{}", &thin_decl[..start], &thin_decl[end..]);
        assert_eq!(old::choice::Choice::decl(&cfg), without, "the declaration of Choice without multi_pick");
    }

    #[test]
    fn every_ts_type_of_the_wire_set_agrees() {
        same::<old::choice::ChoiceOption, thin::choice::ChoiceOption>();
        same_choice_but_the_multi_pick();
        same::<old::decision_context::DecisionSource, thin::decision_context::DecisionSource>();
        same::<old::decision_context::DecisionTarget, thin::decision_context::DecisionTarget>();
        same::<old::decision_context::OutstandingConstraint, thin::decision_context::OutstandingConstraint>();
        same::<old::decision_context::ConstraintKind, thin::decision_context::ConstraintKind>();
        same::<old::decision_context::DecisionContext, thin::decision_context::DecisionContext>();
        same::<old::decision_context::EffectPreview, thin::decision_context::EffectPreview>();
        same::<old::decision_context::HitCause, thin::decision_context::HitCause>();
        same::<old::decision_context::HitRestriction, thin::decision_context::HitRestriction>();
        same::<old::decision_context::HitDetail, thin::decision_context::HitDetail>();
        same::<old::decision_context::TriggerKind, thin::decision_context::TriggerKind>();
        same::<old::decision_context::TriggerUnits, thin::decision_context::TriggerUnits>();
        same::<old::decision_context::DecisionTrigger, thin::decision_context::DecisionTrigger>();
    }
}
