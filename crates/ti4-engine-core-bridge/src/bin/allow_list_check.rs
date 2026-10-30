//! Validate the allow-list file and run its stale check (package D2-6a).
//!
//! ```text
//! allow_list_check [--file PATH] [--rules-version N] [--hits FILE]... [--check-stale]
//! ```
//! Without options it parses `tools/engine_core/allow_list.json` and prints the entries. `--hits` adds up the hits files
//! the shards of a run wrote (`Tally::to_json`) and prints the hits per entry; `--check-stale` (full runs only) then fails
//! on an entry with 0 hits, an expired entry, or hits for an id the file does not have. `--rules-version` defaults to
//! the current `RULES_VERSION`. Exit status 1 on a malformed file or a failed check, 2 on bad arguments.

use ti4_engine_core_bridge::allow_list::{AllowList, DEFAULT_PATH, Tally};

fn usage() -> ! {
    eprintln!("usage: allow_list_check [--file PATH] [--rules-version N] [--hits FILE]... [--check-stale]");
    std::process::exit(2);
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(1);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut file = DEFAULT_PATH.to_owned();
    let mut rules_version = ti4_model::state::RULES_VERSION;
    let mut hits: Vec<String> = Vec::new();
    let mut check_stale = false;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--file" => file = it.next().unwrap_or_else(|| usage()).clone(),
            "--rules-version" => rules_version = it.next().and_then(|v| v.parse().ok()).unwrap_or_else(|| usage()),
            "--hits" => hits.push(it.next().unwrap_or_else(|| usage()).clone()),
            "--check-stale" => check_stale = true,
            _ => usage(),
        }
    }
    let list = AllowList::load(&file).unwrap_or_else(|e| fail(&e));
    let mut tally = Tally::new(&list);
    for path in &hits {
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| fail(&format!("{path}: {e}")));
        tally.merge(&Tally::from_json(&text).unwrap_or_else(|e| fail(&format!("{path}: {e}"))));
    }
    print!("{}", tally.render(&list));
    if check_stale {
        if let Err(problems) = tally.check_stale(&list, rules_version) {
            fail(problems.trim_end());
        }
        println!("stale check: ok");
    }
}
