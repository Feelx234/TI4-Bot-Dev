//! Write a copy of a checkpoint whose vocabulary knows more feature names.
//!
//! Names are appended into the bundle's preallocated rows, which are zero and are checked to be
//! zero, so the written policy scores every option exactly as the source did until PPO trains the
//! new rows. The list usually comes from `vocab_census`. Names the vocabulary already places are
//! skipped. The source is never modified and the destination must not exist; the written bundle is
//! read back through the ordinary loader before the tool reports success.
//!
//! ```text
//! GIT_COMMIT=$(git rev-parse HEAD) cargo run --release -p ti4-mlp --example migrate_bundle_append_names -- \
//!   --bundle <checkpoint> --names <file, one name per line> --out <new checkpoint>
//! ```

use std::path::Path;

fn argument(name: &str) -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == name {
            return args.next();
        }
    }
    None
}

fn refuse(message: &str) -> ! {
    eprintln!("REFUSED: {message}");
    std::process::exit(2)
}

fn main() {
    let source = argument("--bundle").unwrap_or_else(|| refuse("--bundle is required"));
    let names_path = argument("--names").unwrap_or_else(|| refuse("--names is required"));
    let destination = argument("--out").unwrap_or_else(|| refuse("--out is required"));
    let destination = Path::new(&destination);
    if destination.exists() {
        refuse(&format!(
            "{} already exists; bundles are never written in place",
            destination.display()
        ));
    }
    ti4_tensor::configure_deterministic(20_260_922)
        .unwrap_or_else(|error| refuse(&format!("backend: {error}")));

    let text = std::fs::read_to_string(&names_path)
        .unwrap_or_else(|error| refuse(&format!("{names_path}: {error}")));
    let names: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    for name in &names {
        if !ti4_policy::projection::admits(name) {
            refuse(&format!(
                "{name} is not in an admitted family; the MLP would never read it"
            ));
        }
    }

    let loaded = ti4_mlp::bundle::read(Path::new(&source))
        .unwrap_or_else(|error| refuse(&format!("{source}: {error}")));
    let ti4_mlp::bundle::Loaded {
        actor,
        mut vocabulary,
        critic_mode,
        update,
    } = loaded;
    let slots_before = vocabulary.slot_count();
    let fresh: Vec<&String> = names
        .iter()
        .filter(|name| !vocabulary.is_assigned(name))
        .collect();
    let added = vocabulary
        .append(fresh.iter().map(|name| name.as_str()))
        .unwrap_or_else(|error| refuse(&format!("appending names: {error}")));
    // The appended rows must be zero, or the migration changes play before any training.
    for name in &fresh {
        let column = i64::try_from(vocabulary.column_of(name)).expect("column fits");
        let row_sum = actor
            .input()
            .narrow(0, column, 1)
            .abs()
            .sum(ti4_tensor::Kind::Float)
            .double_value(&[]);
        if row_sum != 0.0 {
            refuse(&format!(
                "input row {column} for {name} is not zero ({row_sum})"
            ));
        }
    }

    let slots = vocabulary
        .to_json()
        .unwrap_or_else(|error| refuse(&format!("encoding slots.json: {error}")));
    ti4_mlp::bundle::write(
        destination,
        &actor,
        &slots,
        critic_mode,
        &ti4_mlp::bundle::Provenance {
            source: format!("{added} names appended from {names_path} to {source}"),
            git_commit: std::env::var("GIT_COMMIT").unwrap_or_else(|_| "unrecorded".to_owned()),
            update,
        },
    )
    .unwrap_or_else(|error| refuse(&format!("writing {}: {error}", destination.display())));

    let reread = ti4_mlp::bundle::read(destination).unwrap_or_else(|error| {
        refuse(&format!(
            "the written bundle does not load back: {}: {error}",
            destination.display()
        ))
    });
    println!("source   {source}  (slots {slots_before}, update {update})");
    println!(
        "written  {}  (slots {}, capacity {}, {added} of {} names new)",
        destination.display(),
        reread.vocabulary.slot_count(),
        reread.vocabulary.capacity(),
        names.len()
    );
}
