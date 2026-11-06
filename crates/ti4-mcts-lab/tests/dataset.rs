//! The dataset generator (lane L2): reproducible, noisy, subsampled, outcome-labelled.

use std::path::Path;
use std::process::Command;

fn run(out: &Path, extra: &[&str]) -> String {
    let bin = env!("CARGO_BIN_EXE_mcts_dataset");
    let o = Command::new(bin).args(["--out", out.to_str().expect("utf-8"), "--games", "2", "--first", "500", "--workers", "1", "--per-shard", "1"]).args(extra).output().expect("the generator runs");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn files(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for shard in std::fs::read_dir(dir).expect("dir").flatten() {
        if shard.path().is_dir() {
            for f in std::fs::read_dir(shard.path()).expect("shard").flatten() {
                out.push((format!("{}/{}", shard.file_name().to_string_lossy(), f.file_name().to_string_lossy()), std::fs::read(f.path()).expect("file")));
            }
        }
    }
    out.sort();
    out
}

fn field(summary: &str, key: &str) -> f64 {
    let at = summary.find(&format!("\"{key}\": ")).unwrap_or_else(|| panic!("{key} in {summary}")) + key.len() + 4;
    summary[at..].chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect::<String>().parse().expect("a number")
}

#[test]
fn two_runs_write_the_same_shards_with_noise_and_outcomes() {
    let tmp = std::env::temp_dir().join(format!("ti4-mcts-dataset-{}", std::process::id()));
    let (a, b) = (tmp.join("a"), tmp.join("b"));
    let sa = run(&a, &[]);
    let _ = run(&b, &[]);
    let (fa, fb) = (files(&a), files(&b));
    assert!(fa.len() >= 2 * 19, "two shards of 19 arrays and a schema: {}", fa.len());
    assert_eq!(fa, fb, "the same seeds give the same shards");
    assert_eq!(field(&sa, "errors"), 0.0);
    let records = field(&sa, "records");
    assert!((100.0..=400.0).contains(&records), "about 110 records per game: {records}");
    assert!(field(&sa, "noisy_plays") > 0.0, "the noise played other options");
    // No noise: fewer differences; another record rate: other record counts.
    let c = tmp.join("c");
    let sc = run(&c, &["--eps", "0", "--eps-early", "0"]);
    assert_eq!(field(&sc, "noisy_plays"), 0.0);
    let d = tmp.join("d");
    let sd = run(&d, &["--rates", "draft=0,menu=0,closing=0,activate=0,move=0,score=0,imperial=0"]);
    assert_eq!(field(&sd, "records"), 0.0);
    let _ = std::fs::remove_dir_all(&tmp);
}
