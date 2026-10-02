//! `ti4-join`: a seat at somebody else's online table, and nothing else.
//!
//! Built with `--no-default-features` it carries no simulation and no libtorch: it draws what the
//! host sends and sends back what its player clicks. Every argument is optional; whatever is missing
//! is asked for in the window.
//!
//! ```text
//! ti4-join [<host:port>] [--code <code>] [--seat <seatN>] [--name <name>]
//! ```

// A double-clicked release build should not open a console next to its window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use ti4_model::id::PlayerId;
use ti4_replayer::net::{JoinRequest, remote_gui};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| {
        args.iter()
            .position(|arg| arg == name)
            .and_then(|index| args.get(index + 1))
            .cloned()
    };
    let request = JoinRequest {
        address: args
            .first()
            .filter(|arg| !arg.starts_with('-'))
            .cloned()
            .unwrap_or_default(),
        code: flag("--code").unwrap_or_default(),
        seat: flag("--seat").map(PlayerId::new),
        name: flag("--name").unwrap_or_else(|| "player".to_owned()),
        resume: None,
    };
    if let Err(error) = remote_gui::run(request) {
        eprintln!("ti4-join: the window failed: {error}");
        std::process::exit(2);
    }
}
