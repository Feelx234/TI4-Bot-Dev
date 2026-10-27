//! `cargo xtask <command>`: repository maintenance commands.

use std::path::PathBuf;
use std::process::ExitCode;

use xtask::content_gen::{self, Config};

const USAGE: &str = "usage: cargo xtask gen-cards [--check] [--content DIR] [--core-out DIR] \
[--adapter-out DIR] [--names DIR] [--fields FILE] [--content-tables FILE] [--core-path RUST_PATH] [--only CAT,CAT]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> anyhow::Result<()> {
    let Some((command, rest)) = args.split_first() else {
        anyhow::bail!("{USAGE}");
    };
    if command != "gen-cards" {
        anyhow::bail!("unknown command {command:?}\n{USAGE}");
    }
    let mut cfg = Config::for_repo(&Config::repo_root());
    let mut check = false;
    let mut it = rest.iter();
    while let Some(arg) = it.next() {
        let mut value = || {
            it.next()
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("{arg} needs a value\n{USAGE}"))
        };
        match arg.as_str() {
            "--check" => check = true,
            "--content" => cfg.content_dir = PathBuf::from(value()?),
            "--core-out" => cfg.core_out = PathBuf::from(value()?),
            "--adapter-out" => cfg.adapter_out = PathBuf::from(value()?),
            "--names" => cfg.names_dir = PathBuf::from(value()?),
            "--fields" => cfg.fields_file = PathBuf::from(value()?),
            "--content-tables" => cfg.content_tables_out = PathBuf::from(value()?),
            "--core-path" => cfg.core_path = value()?,
            "--only" => cfg.only = Some(value()?.split(',').map(str::to_owned).collect()),
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => anyhow::bail!("unknown argument {other:?}\n{USAGE}"),
        }
    }
    if check {
        content_gen::check(&cfg)?;
        println!("generated content files are current");
        return Ok(());
    }
    let out = content_gen::generate(&cfg)?;
    for note in &out.notes {
        println!("{note}");
    }
    let changed = content_gen::write(&out)?;
    if changed.is_empty() {
        println!(
            "generated content files are current ({} files)",
            out.files.len()
        );
    } else {
        for path in &changed {
            println!("wrote {}", path.display());
        }
    }
    Ok(())
}
