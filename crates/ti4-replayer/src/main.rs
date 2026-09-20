//! The replayer's own binary: R01's board, and a hand on the table.
//!
//! With no arguments it opens the window. The two headless subcommands exist because the things an
//! operator wants to check at a terminal - "is this project honest about its inputs?" and "what
//! branches does it hold?" - should not require a desktop session, and because a script that can ask
//! them is how those answers stay true.

use std::path::Path;

use ti4_replayer::gui;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        return window(None);
    }
    // A path opens the window with that file in it. A subcommand does something at the terminal.
    let first = Path::new(args.first().map_or("", String::as_str));
    if !matches!(
        args.first().map_or("", String::as_str),
        "inspect" | "import"
    ) && first.exists()
    {
        return window(Some(first.to_path_buf()));
    }
    // A name that is not a subcommand and not a file says which of the two it expected, instead of
    // reciting a usage line at somebody who typed what they meant and simply has not recorded a game
    // under that name yet.
    let named = args.first().map_or("", String::as_str);
    if !matches!(named, "inspect" | "import") && !named.starts_with('-') {
        eprintln!(
            "ti4-replayer: {named} is not there.\n\
             \n\
             \x20 With no argument this app starts a table you can play: cargo run -p ti4-replayer\n\
             \x20 To record one with the reviewer first:\n\
             \x20   cargo run -p ti4-review -- simulate --checkpoint \
             examples/reviewer/checkpoint-473312/slots.json \\\n\
             \x20     --map-pool examples/reviewer/full_np8_12_holdout.json --out \
             out/reviews/fresh.ti4review.json.zst \\\n\
             \x20     --seed 4242 --rotation 1\n\
             \x20 A recorded game is a .ti4review.json file; a replayer file is a .r02.json file."
        );
        std::process::exit(2);
    }
    if let Err(error) = command(&args) {
        eprintln!("ti4-replayer: {error}");
        std::process::exit(2);
    }
}

/// Open the window, optionally with a session or project already loaded.
fn window(open: Option<std::path::PathBuf>) {
    if let Err(error) = gui::run_with(open) {
        eprintln!("ti4-replayer: the window failed: {error}");
        std::process::exit(2);
    }
}

fn command(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("inspect") => inspect(args),
        Some("import") => import(args),
        _ => Err(usage()),
    }
}

fn usage() -> String {
    "usage: ti4-replayer [<session-or-project>] | [inspect <project>] | [import <session>              [--checkpoint P --map-pool P --out P]]"
        .to_owned()
}

/// Read a project file and report what it claims and whether the files on disk agree.
fn inspect(args: &[String]) -> Result<(), String> {
    let path = args
        .get(1)
        .filter(|path| !path.starts_with('-'))
        .ok_or_else(usage)?;
    let project = ti4_replayer::load_project(Path::new(path)).map_err(|e| e.to_string())?;
    let base = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    println!("project  {path}");
    println!(
        "inputs   seed {} rotation {} table {} temperature {} diplomacy {}",
        project.inputs.seed,
        project.inputs.rotation,
        project.inputs.profile_table,
        project.inputs.temperature,
        project.inputs.diplomacy,
    );
    println!(
        "source   {} ({} frames)",
        project.source.session, project.source.frames
    );
    match project.verify_inputs(&base) {
        Ok(verification) => println!(
            "inputs   {} on disk; engine commit {} ({})",
            if verification.matches {
                "hash to what was recorded"
            } else {
                "DO NOT match"
            },
            verification.engine_commit,
            if verification.engine_matches {
                "same build as the recording"
            } else {
                "a different build recorded this"
            },
        ),
        Err(error) => println!("inputs   REFUSED: {error}"),
    }
    println!("branches {}", project.branches.len());
    for branch in &project.branches {
        println!(
            "  {} {} parent={} frames={} answers={} verified={} playable={}",
            branch.id,
            branch.title.as_deref().unwrap_or("(untitled)"),
            branch.parent.map_or("-".to_owned(), |p| p.to_string()),
            branch.frames,
            branch.answers.len(),
            branch.verified,
            branch.playable(
                &project
                    .verify_inputs(&base)
                    .unwrap_or(ti4_replayer::Verification {
                        matches: false,
                        engine_commit: String::new(),
                        engine_matches: false,
                        content: String::new(),
                    })
            ),
        );
    }
    Ok(())
}

/// Turn an R01 session into a project file, reading the inputs off the session's own manifest.
fn import(args: &[String]) -> Result<(), String> {
    let session = args
        .get(1)
        .filter(|path| !path.starts_with('-'))
        .ok_or_else(usage)?;
    let out = flag(args, "--out").map_or_else(
        || {
            let stem = Path::new(session)
                .file_name()
                .map_or("replay".to_owned(), |name| {
                    name.to_string_lossy().into_owned()
                });
            format!(
                "out/replays/{}.r02.json.zst",
                stem.trim_end_matches(".zst")
                    .trim_end_matches(".ti4review.json")
                    .trim_end_matches(".json")
            )
        },
        str::to_owned,
    );
    let base = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    let loaded = ti4_review::load_session(Path::new(session)).map_err(|e| e.to_string())?;
    let inputs = ti4_replayer::ReplayInputs::from_manifest(&loaded.manifest);
    let project = ti4_replayer::ReplayerProject::import_with(
        Path::new(session),
        &loaded,
        &inputs,
        &base,
        None,
        None,
    )
    .map_err(|e| e.to_string())?;
    if let Some(parent) = Path::new(&out).parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    ti4_replayer::save_project(Path::new(&out), &project).map_err(|e| e.to_string())?;
    println!(
        "imported {} frames from {session} into {out}",
        project.source.frames
    );
    Ok(())
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|arg| arg == name)
        .and_then(|index| args.get(index + 1))
        .map(String::as_str)
}
