//! Merge the files a golden dual wrote with `TI4_BAIL_PROBE=FILE` (docs/todo/D2-prereqs.md D2-5 item 5) and print the
//! rows of `ti4_engine::lend_table::BAILS`.
//!
//!   bail_table FILE..
//!
//! One line per observation, `label<TAB>slice,slice`; the output is the union per label, in label order, as Rust source.
//! Observed writes are a lower bound: widen an entry by hand when the body can write more (the footprint audit of
//! `reverse_lend::with_string` is the safety net).

use ti4_engine::lend_table::{merge_probe, render_table};

fn main() {
    let files: Vec<String> = std::env::args().skip(1).collect();
    if files.is_empty() {
        eprintln!("usage: bail_table FILE..");
        std::process::exit(2);
    }
    let mut text = String::new();
    for file in &files {
        match std::fs::read_to_string(file) {
            Ok(t) => text.push_str(&t),
            Err(e) => {
                eprintln!("{file}: {e}");
                std::process::exit(2);
            }
        }
    }
    let observed = merge_probe(text.lines());
    eprintln!("{} observations, {} labels", text.lines().count(), observed.len());
    print!("{}", render_table(&observed));
}
