//! The corpus replay harness of the typed decision model (docs/ENGINE_CORE_STATE.md 2.3; package F4).
//!
//! Loads the recorded decision corpus (package X2, `tools/engine_core/decision_corpus/`, one
//! `<source>.jsonl.zst` per source plus `manifest.json`) and checks, for every recorded decision:
//!
//! 1. the file matches the manifest (raw SHA-256, decision count) and the recorded `Choice` JSON hashes to
//!    its recorded V1/V2 fingerprints (the corpus is what it claims to be);
//! 2. generic families: every option that is `decline`, `done` or a count parses back
//!    (`parse(option(o).id) == o`) and renders to the recorded id and kind; the recorded text wording gives
//!    the recorded label and payload; options with the generic default wording render exactly;
//! 3. context: everything but the subtype text decodes into the typed `Ctx` and encodes to the recorded
//!    JSON (source, target, optional, outstanding, space_battle, invasion_seq, actor, phase, round);
//! 4. area lifters (none yet): a ported area registers a function that lifts a recorded `Choice` to a typed
//!    one; the harness then requires `render_choice(lifted) == recorded` (ids, kinds, labels, payload JSON,
//!    prompt, context).
//!
//! Where the corpus lives: `$TI4_DECISION_CORPUS`, else `tools/engine_core/decision_corpus` in the repo.
//! Absent corpus: the test is skipped with a message, unless `TI4_REQUIRE_CORPUS=1`.
//! `TI4_CORPUS_SOURCES=golden20,fixtures` limits the sources.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use serde_json::Value;
use sha2::{Digest, Sha256};
use ti4_engine::choice::{Choice, ChoiceOption, DecisionLog};
use ti4_engine::fingerprint::{CanonicalHashVersion, decision_hash};
use ti4_engine::render::{self, Render, generic};
use ti4_engine_core::choice::generic::Ask as GAsk;
use ti4_engine_core::choice::{self as core, Ask, Opt};

fn corpus_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("TI4_DECISION_CORPUS").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/engine_core/decision_corpus"),
        PathBuf::from,
    );
    dir.join("manifest.json").exists().then_some(dir)
}

/// A lifter turns a recorded string choice into the typed one of a ported area (None: not mine).
type Lifter = fn(&Render, &Choice) -> Option<core::Choice>;
/// Registered by the area packages (B1 ... B9) when their family is ported.
const LIFTERS: &[Lifter] = &[];

#[derive(Default, Debug)]
struct Stats {
    decisions: usize,
    options: usize,
    hashes_ok: usize,
    decline: usize,
    done: usize,
    number: usize,
    default_wording_exact: usize,
    ctx_decoded: usize,
    ctx_undecoded: BTreeMap<String, usize>,
    lifted: usize,
}

fn seat_table(c: &Choice) -> Render {
    let mut seats = vec![c.player.as_str().to_owned()];
    if let Some(ctx) = &c.context {
        seats.push(ctx.actor.as_str().to_owned());
        if let Some(ti4_engine::decision_context::DecisionTarget::Player(p)) = &ctx.target {
            seats.push(p.as_str().to_owned());
        }
    }
    seats.sort();
    seats.dedup();
    Render::new(seats)
}

fn same_option(a: &ChoiceOption, b: &ChoiceOption) -> bool {
    a.id == b.id && a.kind == b.kind && a.label == b.label && a.payload == b.payload
}

fn check_options(rec: &Choice, r: &Render, st: &mut Stats) {
    for o in &rec.options {
        st.options += 1;
        // Which generic family (if any) claims the recorded option.
        let (opt, ask) = if o.id == "decline" && o.kind == "decline" {
            st.decline += 1;
            (Opt::Decline, GAsk::Confirm)
        } else if o.id == "done" && o.kind == "decline" {
            st.done += 1;
            (Opt::Done, GAsk::Confirm)
        } else if o.kind == "count" && o.id.parse::<i32>().is_ok_and(|n| n.to_string() == o.id) {
            st.number += 1;
            let n: i32 = o.id.parse().unwrap();
            (Opt::Number(n), GAsk::Count { lo: n.min(0), hi: n.max(0) })
        } else {
            continue;
        };
        let ask = Ask::Generic(ask);
        // parse(option(o).id) == o
        let got = render::option(r, &ask, &opt);
        assert_eq!(render::parse(r, &ask, &got.id), Some(opt), "round trip of {opt:?}");
        // the recorded id parses to the option, and the option renders to the recorded id and kind
        assert_eq!(render::parse(r, &ask, &o.id), Some(opt), "recorded id {:?}", o.id);
        assert_eq!((got.id.as_str(), got.kind.as_str()), (o.id.as_str(), o.kind.as_str()));
        // with the recorded wording the whole option is the recorded one
        let w = render::Wording { kind: None, label: Some(o.label.clone()), payload: o.payload.clone(), preview: None };
        assert!(same_option(&generic::option(&opt, &w), o), "wording of {o:?}");
        if got.label == o.label && o.payload.is_empty() {
            st.default_wording_exact += 1;
        }
    }
}

fn check_context(rec: &Choice, r: &Render, st: &mut Stats) {
    let Some(ctx) = &rec.context else { return };
    let Some((typed, actor, phase, round)) = generic::decode_context(r, ctx) else {
        let kind = format!("{:?}", std::mem::discriminant(&ctx.source));
        *st.ctx_undecoded.entry(kind).or_default() += 1;
        return;
    };
    let mut again = generic::encode_context(r, &typed, actor, phase, round, ctx.subtype.clone());
    // display-only parts are the area's to render; they are not typed here
    again.trigger = ctx.trigger.clone();
    again.hit = ctx.hit.clone();
    again.previews = ctx.previews.clone();
    assert_eq!(
        serde_json::to_value(&again).unwrap(),
        serde_json::to_value(ctx).unwrap(),
        "context round trip"
    );
    st.ctx_decoded += 1;
}

fn check_source(dir: &std::path::Path, name: &str, entry: &Value) -> Stats {
    let file = dir.join(entry["file"].as_str().expect("manifest file"));
    let reader = zstd::stream::read::Decoder::new(std::fs::File::open(&file).expect("corpus file")).expect("zstd");
    let mut st = Stats::default();
    let mut sha = Sha256::new();
    let mut raw = 0usize;
    for line in BufReader::new(reader).split(b'\n') {
        let line = line.expect("read");
        sha.update(&line);
        sha.update(b"\n");
        raw += line.len() + 1;
        let v: Value = serde_json::from_slice(&line).expect("json line");
        let rec: Choice = serde_json::from_value(v["choice"].clone()).expect("Choice json");
        st.decisions += 1;

        // 1. the recorded hashes
        let chosen = v["chosen"].as_str().expect("chosen");
        let mut log = DecisionLog::default();
        log.record(&rec, &ChoiceOption::new(chosen, ""));
        let (h1, h2) = (
            decision_hash(CanonicalHashVersion::V1, &log.records[0]).digest,
            decision_hash(CanonicalHashVersion::V2, &log.records[0]).digest,
        );
        assert_eq!(&h1[..16], v["v1"].as_str().unwrap(), "{name}: V1 of line {}", st.decisions);
        assert_eq!(&h2[..16], v["v2"].as_str().unwrap(), "{name}: V2 of line {}", st.decisions);
        st.hashes_ok += 1;

        let r = seat_table(&rec);
        check_options(&rec, &r, &mut st);
        check_context(&rec, &r, &mut st);
        for lift in LIFTERS {
            if let Some(typed) = lift(&r, &rec) {
                let ctx = rec.context.as_ref().expect("context");
                let back = render::render_choice(&r, &typed, generic::core_phase(ctx.phase), ctx.round);
                assert_eq!(serde_json::to_value(&back).unwrap(), serde_json::to_value(&rec).unwrap());
                st.lifted += 1;
            }
        }
    }
    assert_eq!(st.decisions as u64, entry["decisions"].as_u64().unwrap(), "{name}: decision count");
    assert_eq!(raw as u64, entry["raw_bytes"].as_u64().unwrap(), "{name}: raw bytes");
    let digest: String = sha.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(digest, entry["sha256"].as_str().unwrap(), "{name}: sha256");
    st
}

#[test]
fn corpus_replay() {
    let Some(dir) = corpus_dir() else {
        assert!(std::env::var_os("TI4_REQUIRE_CORPUS").is_none(), "decision corpus required but not found");
        eprintln!("decision corpus not found: skipped (set TI4_DECISION_CORPUS)");
        return;
    };
    let manifest: Value = serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).unwrap()).unwrap();
    let only: Option<Vec<String>> =
        std::env::var("TI4_CORPUS_SOURCES").ok().map(|s| s.split(',').map(str::to_owned).collect());
    let mut total = Stats::default();
    for (name, entry) in manifest["sources"].as_object().expect("sources") {
        if only.as_ref().is_some_and(|o| !o.contains(name)) {
            continue;
        }
        let st = check_source(&dir, name, entry);
        eprintln!("{name}: {st:?}");
        total.decisions += st.decisions;
        total.options += st.options;
        total.hashes_ok += st.hashes_ok;
        total.decline += st.decline;
        total.done += st.done;
        total.number += st.number;
        total.default_wording_exact += st.default_wording_exact;
        total.ctx_decoded += st.ctx_decoded;
        total.lifted += st.lifted;
        for (k, n) in st.ctx_undecoded {
            *total.ctx_undecoded.entry(k).or_default() += n;
        }
    }
    eprintln!("TOTAL: {total:?}");
    assert!(total.decisions > 0);
}

/// Every generic value round trips through its id, with and without number asks.
#[test]
fn generic_round_trip_synthetic() {
    let r = Render::new(vec!["a".to_owned()]);
    let ask = Ask::Generic(GAsk::Count { lo: -3, hi: 3000 });
    let confirm = Ask::Generic(GAsk::Confirm);
    for opt in [Opt::Decline, Opt::Done].into_iter().chain((-3..=3000).map(Opt::Number)) {
        let o = render::option(&r, &ask, &opt);
        assert_eq!(render::parse(&r, &ask, &o.id), Some(opt));
    }
    // non-canonical numbers and numbers in an ask that takes none are not answers
    for id in ["+1", "01", "-0", " 1", "1 ", "", "1.0", "99999999999"] {
        assert_eq!(render::parse(&r, &ask, id), None, "{id:?}");
    }
    assert_eq!(render::parse(&r, &confirm, "5"), None);
    assert_eq!(render::parse(&r, &confirm, "decline"), Some(Opt::Decline));
}

/// A deliberately wrong lift is caught by the equality the harness uses.
#[test]
fn render_equality_detects_a_difference() {
    use ti4_engine_core::choice::{Ctx, Origin, Subtype, Opts};
    use ti4_engine_core::choice::generic::{Rule, Subtype as GSub};
    use ti4_engine_core::state::Phase;
    let r = Render::new(vec!["a".to_owned()]);
    let mk = |opts: Vec<Opt>| core::Choice {
        actor: ti4_engine_core::Seat(0),
        ask: Ask::Generic(GAsk::Confirm),
        ctx: Ctx {
            origin: Origin::Rule(Rule::R22),
            subtype: Subtype::Generic(GSub::Unclassified),
            optional: true,
            target: None,
            outstanding: ti4_engine_core::SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        },
        opts: opts.into_iter().collect::<Opts>(),
    };
    let a = serde_json::to_value(render::render_choice(&r, &mk(vec![Opt::Done, Opt::Decline]), Phase::Action, 1)).unwrap();
    let b = serde_json::to_value(render::render_choice(&r, &mk(vec![Opt::Decline, Opt::Done]), Phase::Action, 1)).unwrap();
    assert_ne!(a, b, "option order is part of the rendering");
    assert_eq!(a, serde_json::to_value(render::render_choice(&r, &mk(vec![Opt::Done, Opt::Decline]), Phase::Action, 1)).unwrap());
}
