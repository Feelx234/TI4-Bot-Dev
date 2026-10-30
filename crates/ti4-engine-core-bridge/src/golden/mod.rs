//! Golden games: frozen per-`RULES_VERSION` replays of recorded games (docs/todo/D4-core-tests.md 4, package D4-4).
//! The CLI is `bin/golden.rs`; the committed data lives in `tools/engine_core/golden/` (see its README).

pub mod bless;
pub mod corpus;
pub mod format;
pub mod record;
pub mod replay;
pub mod sets;

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use ti4_model::state::RULES_VERSION;

    use super::replay::{self, EngineReplayer, StaticOnly};
    use super::sets::{self, SetMeta};

    fn golden_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/engine_core/golden")
    }

    /// T7: a `RULES_VERSION` bump without a golden set at the new number (and a ladder game at it) fails here.
    #[test]
    fn every_version_has_a_set() {
        sets::every_version_has_a_set(&golden_root(), RULES_VERSION).unwrap();
    }

    /// The committed files are the files `set.json` lists: sha256 of the text, decision counts, header versions, and
    /// (full tier) every line's Choice round trip and V1/V2 hashes. No engine run.
    #[test]
    fn every_golden_file_matches_its_set_json() {
        let root = golden_root();
        let index = sets::read_index(&root).unwrap();
        assert!(!index.is_empty(), "sets.json is empty");
        for e in index {
            let dir = root.join(&e.dir);
            let meta = SetMeta::read(&dir).unwrap();
            assert_eq!((meta.name.as_str(), meta.games.len(), meta.decisions()), (e.name.as_str(), e.games, e.decisions), "sets.json is stale: run `golden index`");
            let results = replay::check_set(&dir, &meta, &StaticOnly, &[], false, 4).unwrap();
            for r in results {
                assert!(r.ok(), "{}/{}: {:?}", e.name, r.file, r.outcome);
            }
        }
    }

    /// The smoke subset (the smallest games of every set) reproduces on the string engine under the build's mask.
    #[test]
    fn the_smoke_subset_reproduces() {
        let root = golden_root();
        for e in sets::read_index(&root).unwrap() {
            let dir = root.join(&e.dir);
            let meta = SetMeta::read(&dir).unwrap();
            for r in replay::check_set(&dir, &meta, &EngineReplayer::default(), &[], true, 4).unwrap() {
                assert!(r.ok(), "{}/{}: {:?}", e.name, r.file, r.outcome);
            }
        }
    }

    /// The smallest recorded game of the imported template openings (60 steps: fast), read from the committed set.
    fn small_game() -> super::format::GameFile {
        let dir = golden_root().join("legacy-sources/templates");
        let meta = SetMeta::read(&dir).unwrap();
        let file = meta.smoke.first().expect("a smoke game").clone();
        super::format::GameFile::read(&dir.join(file)).unwrap().0
    }

    /// A planted difference is found, at its own step, with the right kind (the engine really replays: no fake).
    #[test]
    fn a_planted_answer_difference_is_found_at_its_step() {
        let mut g = small_game();
        assert!(g.decs.len() > 3);
        let at = g.decs[3].step;
        g.decs[3].v2 = if g.decs[3].v2.starts_with('0') { format!("1{}", &g.decs[3].v2[1..]) } else { format!("0{}", &g.decs[3].v2[1..]) };
        let d = replay::replay_game(&g).unwrap().expect("the planted difference is reported");
        assert_eq!((d.step, d.kind), (at, replay::DivKind::Answer), "{d}");
        assert_eq!(d.subtype, g.decs[3].subtype);
    }

    #[test]
    fn a_planted_state_difference_is_found_at_its_step() {
        let mut g = small_game();
        let at = g.decs[2].step;
        let h = g.decs[2].state.clone().unwrap();
        // every decision of that step carries the same state hash
        for d in g.decs.iter_mut().filter(|d| d.step == at) {
            d.state = Some(format!("{}{}", if h.starts_with('f') { "e" } else { "f" }, &h[1..]));
        }
        let d = replay::replay_game(&g).unwrap().expect("the planted difference is reported");
        assert_eq!((d.step, d.kind), (at, replay::DivKind::State), "{d}");
    }

    #[test]
    fn a_missing_recorded_question_is_reported() {
        let mut g = small_game();
        let last = g.decs.len() - 1;
        g.decs[last].step = g.header.steps + 5; // an unreachable step: the last question is "never asked"
        g.header.decisions = g.decs.len();
        let r = replay::replay_game(&g).unwrap();
        // the engine asks one question fewer in the recording than it really does at the true step: Unexpected there
        let d = r.expect("a difference");
        assert!(matches!(d.kind, replay::DivKind::Unexpected | replay::DivKind::Missing | replay::DivKind::Answer), "{d}");
    }
}
