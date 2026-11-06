//! Parity with the trainer's fixtures (lane L4): the Rust forward pass gives the PyTorch outputs within the manifest's tolerance
//! (1e-4) on every record of a fixture directory.
//!
//! The committed `tests/fixtures/tiny` (a small model on 24 real records, written by `tools/mcts/make_tiny_fixture.py`) always
//! runs. `TI4_MCTS_FIXTURES=DIR[:DIR...]` adds fixture directories (`RUN/fixtures` of `python -m ti4mcts.train`), for example the
//! full-size model of a training run.

use std::path::{Path, PathBuf};

use ti4_mcts_features::npy;
use ti4_mcts_net::{plain_shard, Net, Scratch};

fn check(dir: &Path) -> (usize, f32) {
    let net = Net::load(&dir.join("weights.t4w")).expect("the weights load");
    let recs = plain_shard::read(&dir.join("inputs")).expect("the inputs load");
    let win = npy::read(&dir.join("out_win.npy")).expect("out_win");
    let vp = npy::read(&dir.join("out_vp.npy")).expect("out_vp");
    let logits = npy::read(&dir.join("out_logits.npy")).expect("out_logits");
    let seats = net.dims.seats;
    let mut s = Scratch::default();
    let (mut at, mut worst) = (0usize, 0f32);
    for (i, e) in recs.iter().enumerate() {
        let out = net.forward(e, &mut s);
        for k in 0..seats {
            worst = worst.max((out.win[k] - win.f[i * seats + k]).abs());
            worst = worst.max((out.vp[k] - vp.f[i * seats + k]).abs());
        }
        for (j, l) in out.logits.iter().enumerate() {
            worst = worst.max((l - logits.f[at + j]).abs());
        }
        at += out.logits.len();
    }
    assert_eq!(at, logits.f.len(), "{}: option count", dir.display());
    assert!(worst < 1e-4, "{}: max abs difference {worst}", dir.display());
    (recs.len(), worst)
}

#[test]
fn the_tiny_fixture_matches() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiny");
    let (n, worst) = check(&dir);
    assert!(n >= 8);
    eprintln!("tiny: {n} records, max abs difference {worst:e}");
}

#[test]
fn the_named_fixtures_match() {
    let Ok(dirs) = std::env::var("TI4_MCTS_FIXTURES") else {
        eprintln!("TI4_MCTS_FIXTURES not set: only the tiny fixture ran");
        return;
    };
    for d in dirs.split(':').filter(|d| !d.is_empty()) {
        let (n, worst) = check(Path::new(d));
        eprintln!("{d}: {n} records, max abs difference {worst:e}");
    }
}

#[test]
fn a_wrong_file_is_refused() {
    assert!(Net::from_bytes(b"nope").is_err());
    let mut raw = b"T4MW".to_vec();
    raw.extend_from_slice(&2u32.to_le_bytes());
    raw.extend_from_slice(&0u32.to_le_bytes());
    assert!(Net::from_bytes(&raw).unwrap_err().contains("format"));
}
