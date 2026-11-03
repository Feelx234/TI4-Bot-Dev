//! D2-8c: the games the host creates (`ti4_host::setup::create`: the plain tables, every start
//! preset, every scenario) are the same games after the setup moved from the legacy engine into the
//! thin crate: a digest of the position and the map of each, compared with
//! `tests/host_create.golden`.
//!
//! The golden file was recorded with the host going through the **legacy** setup and builders
//! (the commit before D2-8c; `TI4_RECORD_HOST_GOLDEN=<path>` writes one). It is never re-recorded
//! to make this test pass: a difference is a difference between the two engines.
//!
//! Run: `cargo test -p ti4-engine-core-bridge --test host_create_golden`.

use std::fmt::Write as _;

use sha2::{Digest, Sha256};
use ti4_content::ContentStore;
use ti4_host::setup::{SpeakerOrder, TableSetup, create, draw};
use ti4_model::id::PlayerId;

const GOLDEN: &str = include_str!("host_create.golden");

const PRESETS: [&str; 17] = [
    "combat", "cards", "agenda", "relics", "invasion", "techs", "leaders", "endgame", "siege",
    "bombard", "capture", "explore", "notes", "expedition", "research", "research_nopa", "world",
];

fn ids(count: usize) -> Vec<PlayerId> {
    (1..=count).map(|i| PlayerId::new(format!("p{i}"))).collect()
}

fn digest(setup: &TableSetup) -> String {
    let content = ContentStore::embedded();
    let players = ids(setup.player_count);
    let made = draw(setup, &vec![None; setup.player_count]).and_then(|drawn| {
        create(content, setup, &players, &drawn).map(|(state, galaxy)| {
            let json = serde_json::to_string(&state).expect("serialises");
            format!("{json}\n{state:?}\n{galaxy:?}")
        })
    });
    let text = match made {
        Ok(text) => text,
        Err(message) => format!("ERR {message}"),
    };
    let mut hash = Sha256::new();
    hash.update(text.as_bytes());
    let hex = hash.finalize().iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    });
    format!("{}:{}", if text.starts_with("ERR") { "err" } else { "ok" }, &hex[..32])
}

fn matrix() -> Vec<(String, TableSetup)> {
    let mut out = Vec::new();
    // Plain tables: seats, card sets, scope, starting technology, speaker order, seeds.
    for count in 2..=8 {
        for set in ["te", "pok", "base_game_codex1"] {
            for thunders_edge in [false, true] {
                for starting_tech in [false, true] {
                    for speaker in [SpeakerOrder::Random, SpeakerOrder::Seat] {
                        for seed in [1_u64, 2, 17] {
                            let mut setup = TableSetup::new(count, seed);
                            setup.strategy_cards = set.to_owned();
                            setup.thunders_edge = thunders_edge;
                            setup.starting_tech = starting_tech;
                            setup.speaker_order = speaker;
                            let key = format!(
                                "plain {count}p {set} te={thunders_edge} tech={starting_tech} {speaker:?} seed={seed}"
                            );
                            out.push((key, setup));
                        }
                    }
                }
            }
        }
    }
    // Every start preset (and its suffixes) at several table sizes.
    let suffixes = ["", "+rot", "+short", "+agenda", "+fac:sol:hacan"];
    for preset in PRESETS {
        for suffix in suffixes {
            for count in [2_usize, 3, 4, 6, 8] {
                for seed in [1_u64, 5] {
                    let mut setup = TableSetup::new(count, seed);
                    setup.preset = Some(format!("{preset}{suffix}"));
                    setup.speaker_order = SpeakerOrder::Seat;
                    out.push((format!("preset {preset}{suffix} {count}p seed={seed}"), setup));
                }
            }
        }
    }
    // Every scenario, on its own seed and on others.
    for scenario in ti4_host::view::scenario::SCENARIOS {
        for seed in [scenario.seed, 1, 9] {
            let mut setup = TableSetup::new(scenario.players, seed);
            setup.start_from = Some(scenario.id.to_owned());
            out.push((format!("scenario {} seed={seed}", scenario.id), setup));
        }
    }
    out
}

#[test]
fn the_games_the_host_creates_are_the_games_the_legacy_setup_made() {
    let lines: Vec<String> = matrix()
        .iter()
        .map(|(key, setup)| format!("{key} => {}", digest(setup)))
        .collect();
    if let Ok(path) = std::env::var("TI4_RECORD_HOST_GOLDEN") {
        std::fs::write(&path, lines.join("\n") + "\n").expect("the golden file is written");
        return;
    }
    let golden: Vec<&str> = GOLDEN.lines().collect();
    assert_eq!(
        golden.len(),
        lines.len(),
        "the golden file and the matrix differ in size (the matrix is part of the contract)"
    );
    // Integration 19 (lane E, D2-8 P1): Thunder's Edge is refused at creation (`te=true` rows, the `expedition` preset). The golden file is NOT re-recorded:
    // each such row was a game (`ok:`) at the legacy setup and is an `err:` now, which this test states; every other row still equals the golden.
    let refused = |line: &str| line.contains(" te=true ") || line.starts_with("preset expedition");
    let mut refused_rows = 0_usize;
    for (g, l) in golden.iter().zip(&lines) {
        if refused(l) {
            assert!(g.contains("=> ok:") && l.contains("=> err:"), "a Thunder's Edge table is refused at creation:\n golden {g}\n    now {l}");
            refused_rows += 1;
        }
    }
    assert_eq!(refused_rows, 302, "252 plain tables with the switch and 50 expedition presets are refused");
    let different: Vec<String> = golden
        .iter()
        .zip(&lines)
        .filter(|(g, l)| g != l && !refused(l))
        .map(|(g, l)| format!("golden {g}\n   now {l}"))
        .collect();
    assert!(
        different.is_empty(),
        "{} of {} created games differ from the legacy setup:\n{}",
        different.len(),
        lines.len(),
        different.iter().take(12).cloned().collect::<Vec<_>>().join("\n")
    );
    // The matrix is not vacuous: most of the rest creates a game.
    let made = lines.iter().filter(|l| l.contains("=> ok:")).count();
    assert!(made * 2 > lines.len(), "only {made} of {} created a game", lines.len());
}
