//! Merge the files a golden dual wrote with `TI4_BAIL_PROBE=FILE` (docs/todo/D2-prereqs.md D2-5 item 5), print the rows of
//! `ti4_engine_legacy::lend_table::BAILS`, and cross-check them against the report of the run (docs/todo/D2-remaining.md 3.1 R1, R2, R3).
//!
//! ```text
//! bail_table [--sites TSV] FILE..                       the rows (writes from the probe files; kind/args from the sites file)
//! bail_table --check REPORT [--sites TSV] [--rows] FILE..
//!                                                       the cross-check: REPORT is the merged dual report text (`dual-all.log`),
//!                                                       FILE.. the probe files of the same run. Prints label, calls, bailed, share,
//!                                                       gate runs, writes, when; exit 1 on a bail without a probe line, a probe line
//!                                                       without a bail, a bail with no counted reason and (once BAILS has rows) a
//!                                                       label without an entry or an entry nobody bails at. With --rows it also
//!                                                       prints the rows with `kind`, `args` (from the sites file) and `when` (from the reasons).
//! ```
//! One line per observation, `label<TAB>slice,slice`; the output is the union per label, in label order, as Rust source.
//! Observed writes are a lower bound: widen an entry by hand when the body can write more (the footprint audit of
//! `reverse_lend::with_string` is the safety net). The sites file is `tools/engine_core/lent_sites.tsv` (`lent_sites.py`).

use std::collections::BTreeMap;

use ti4_engine_legacy::lend_table::{BAILS, How, When, check, how_from_sites, merge_probe, parse_report, render_check, render_table_with, when_of_reason};

fn usage() -> ! {
    eprintln!("usage: bail_table [--sites TSV] FILE..\n       bail_table --check REPORT [--sites TSV] [--rows] FILE..");
    std::process::exit(2);
}

fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("{path}: {e}");
        std::process::exit(2)
    })
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut take = |flag: &str| -> Option<String> {
        let at = args.iter().position(|a| a == flag)?;
        if at + 1 >= args.len() {
            usage();
        }
        let v = args.remove(at + 1);
        args.remove(at);
        Some(v)
    };
    let sites = take("--sites").map(|p| read(&p));
    let report_path = take("--check");
    let rows = if let Some(at) = args.iter().position(|a| a == "--rows") {
        args.remove(at);
        true
    } else {
        false
    };
    if args.is_empty() && report_path.is_none() {
        usage();
    }
    let probe_text: String = args.iter().map(|f| read(f)).collect();
    let observed = merge_probe(probe_text.lines());
    let mut how: BTreeMap<String, How> = sites.as_deref().map(how_from_sites).unwrap_or_default();
    let Some(report_path) = report_path else {
        eprintln!("{} observations, {} labels", probe_text.lines().count(), observed.len());
        print!("{}", render_table_with(&observed, &how));
        return;
    };
    let report = parse_report(&read(&report_path));
    let checked = check(&report, &observed, BAILS);
    eprintln!(
        "{} labels in the report bail or have gate rows, {} probe labels, {} table entries",
        checked.rows.len(),
        observed.len(),
        BAILS.len()
    );
    if BAILS.is_empty() {
        eprintln!("BAILS is empty: the table checks (entry per label, stale entry) are skipped until the rows are pasted (R2)");
    }
    print!("{}", render_check(&checked));
    if rows {
        // `when` from the reasons of the report, per label
        for (label, reasons) in &report.bail_reasons {
            if reasons.iter().any(|(why, _)| when_of_reason(why) == When::Mid) {
                how.entry(label.clone()).or_default().when = When::Mid;
            }
        }
        // a label with only gate rows: kind Gate unless the sites file says otherwise
        let mut table = observed.clone();
        for label in report.gates.keys() {
            table.entry(label.clone()).or_insert(ti4_engine_core::state::SliceSet::ALL);
        }
        print!("\n{}", render_table_with(&table, &how));
    }
    std::process::exit(i32::from(!checked.problems.is_empty()));
}
