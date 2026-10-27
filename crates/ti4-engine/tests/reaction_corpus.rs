//! The reaction renderer (package A5) against the recorded decision corpus: every reaction window of the
//! corpus (prompt `when|after EVENT`, ability options) renders to the recorded prompt, subtype and option
//! ids, and every id parses back. Labels and payloads are compared for the options the typed ask can
//! express (all but the `reaction:` card slots, whose card names are display facts of the state).

use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use serde_json::Value;
use ti4_engine::choice::Choice;
use ti4_engine::kernel_sync::{ability_id, event_kind};
use ti4_engine::render::{self, Render};
use ti4_engine_core::choice::reaction::{Ask, Opt as ROpt, Relation, Subtype};
use ti4_engine_core::choice::{self as core, Ask as CAsk, Opt as COpt, Subtype as CSub};

fn corpus_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("TI4_DECISION_CORPUS").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/engine_core/decision_corpus"),
        PathBuf::from,
    );
    dir.join("manifest.json").exists().then_some(dir)
}

#[test]
fn reaction_windows_render_and_parse_like_the_corpus() {
    let Some(dir) = corpus_dir() else {
        assert!(std::env::var_os("TI4_REQUIRE_CORPUS").is_none(), "decision corpus required but not found");
        eprintln!("decision corpus not found: skipped");
        return;
    };
    let manifest: Value = serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).unwrap()).unwrap();
    let (mut windows, mut options, mut labelled) = (0usize, 0usize, 0usize);
    for (_, entry) in manifest["sources"].as_object().unwrap() {
        let file = dir.join(entry["file"].as_str().unwrap());
        let reader = zstd::stream::read::Decoder::new(std::fs::File::open(file).unwrap()).unwrap();
        for line in BufReader::new(reader).split(b'\n') {
            let v: Value = serde_json::from_slice(&line.unwrap()).unwrap();
            let rec: Choice = serde_json::from_value(v["choice"].clone()).unwrap();
            let Some((relation, event)) = rec.prompt.split_once(' ').filter(|(r, e)| {
                matches!(*r, "when" | "after") && e.chars().all(|c| c.is_ascii_uppercase() || c == '_')
            }) else {
                continue;
            };
            if rec.options.iter().any(|o| o.kind != "ability" && o.kind != "decline") {
                continue;
            }
            windows += 1;
            let r = Render::new(vec![rec.player.as_str().to_owned()]);
            let rel = if relation == "when" { Relation::When } else { Relation::After };
            let kind = event_kind(&r, event);
            let ask = CAsk::Reaction(Ask::Window { event: kind, relation: rel });
            assert_eq!(render::prompt(&r, &ask), rec.prompt);
            if let Some(ctx) = &rec.context {
                let sub = CSub::Reaction(Subtype::Window { event: kind, relation: rel });
                if ctx.subtype.starts_with("reaction_") {
                    assert_eq!(render::subtype_text(&r, &sub), ctx.subtype);
                }
            }
            for o in &rec.options {
                options += 1;
                if o.kind == "decline" {
                    assert_eq!(render::parse(&r, &ask, &o.id), Some(COpt::Decline));
                    continue;
                }
                let typed = COpt::Reaction(ROpt::Ability(ability_id(&r, &o.id)));
                let got = render::option(&r, &ask, &typed);
                assert_eq!((got.id.as_str(), got.kind.as_str()), (o.id.as_str(), o.kind.as_str()));
                assert_eq!(render::parse(&r, &ask, &o.id), Some(typed), "{}", o.id);
                if !o.id.starts_with("reaction:") {
                    assert_eq!(got.label, o.label, "label of {}", o.id);
                    assert_eq!(got.payload.get("event"), o.payload.get("event"), "payload of {}", o.id);
                    labelled += 1;
                }
            }
            let _ = core::Area::Reaction;
        }
    }
    eprintln!("reaction windows {windows}, options {options}, labels compared {labelled}");
    assert!(windows > 0);
}
