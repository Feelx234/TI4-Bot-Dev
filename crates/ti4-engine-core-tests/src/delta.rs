//! `delta(before, after)`: the canonical text of what changed between two states, for scenario snapshots.
//!
//! Every changed slice (and, for the board and the players, every changed cell or player) is printed as the leaves of its `{:#?}` form
//! that differ, `path: before -> after`, in a fixed order. The deck slice and the rng streams are never printed (hidden information):
//! only "decks changed" / the stream positions. Equal states give the empty string.

use std::collections::BTreeMap;
use std::fmt::{Debug, Write as _};

use ti4_engine_core::generated::{Planet, System};
use ti4_engine_core::state::CoreState;

/// The leaves of a `{:#?}` text as `path -> text`; list items are numbered by position.
fn leaves(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut stack: Vec<(String, usize)> = Vec::new();
    let path_of = |stack: &[(String, usize)]| stack.iter().map(|(n, _)| n.as_str()).filter(|n| !n.is_empty()).collect::<Vec<_>>().join(".");
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with(['}', ']', ')']) {
            stack.pop();
            continue;
        }
        let opens = line.ends_with(['{', '[', '(']);
        let (name, value) = match line.split_once(": ") {
            Some((n, v)) if !n.contains(['{', '[', '(', ' ']) => (Some(n.to_string()), v),
            _ => (None, line),
        };
        let key = match name {
            Some(n) => n,
            None => match stack.last_mut() {
                Some((_, counter)) => {
                    let i = *counter;
                    *counter += 1;
                    format!("[{i}]")
                }
                // The root value itself: no name of its own.
                None => String::new(),
            },
        };
        if opens {
            stack.push((key, 0));
        } else {
            let mut p = path_of(&stack);
            if !p.is_empty() && !key.is_empty() {
                p.push('.');
            }
            p.push_str(&key);
            out.insert(p, value.trim_end_matches(',').to_string());
        }
    }
    out
}

/// `label.path: before -> after` for every leaf that differs between two values.
fn diff_values<T: Debug + PartialEq>(label: &str, before: &T, after: &T, out: &mut String) {
    if before == after {
        return;
    }
    let (a, b) = (leaves(&format!("{before:#?}")), leaves(&format!("{after:#?}")));
    let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
    keys.sort();
    keys.dedup();
    let mut any = false;
    for k in keys {
        let (x, y) = (a.get(k), b.get(k));
        if x != y {
            any = true;
            let _ = writeln!(out, "{label}.{k}: {} -> {}", x.map_or("-", String::as_str), y.map_or("-", String::as_str));
        }
    }
    if !any {
        // Equal leaves but unequal values: a private field the Debug form hides.
        let _ = writeln!(out, "{label}: changed (not visible in Debug)");
    }
}

/// The pieces of a list as `2xCruiserI/s0, 1xInfantryI/s1` (grouped, in list order of first appearance; `*` sustained, `^` galvanized).
fn pieces_text(units: &[ti4_engine_core::state::Piece]) -> String {
    let mut groups: Vec<(String, usize)> = Vec::new();
    for p in units {
        let label = format!("{:?}/s{}{}{}", p.kind(), p.owner().0, if p.sustained() { "*" } else { "" }, if p.galvanized() { "^" } else { "" });
        match groups.iter_mut().find(|(l, _)| *l == label) {
            Some((_, n)) => *n += 1,
            None => groups.push((label, 1)),
        }
    }
    groups.sort();
    format!("[{}]", groups.iter().map(|(l, n)| format!("{n}x{l}")).collect::<Vec<_>>().join(", "))
}

fn sys_text(c: &ti4_engine_core::state::SysCell) -> String {
    format!("units {} tokens {:?} flags {}", pieces_text(c.units.as_slice()), c.cmd, c.flags.0)
}

fn planet_text(c: &ti4_engine_core::state::PlanetCell) -> String {
    format!(
        "control {:?} units {} attach {:?} coexist {:?} placed_in {:?} flags {}",
        c.control.map(|s| s.0),
        pieces_text(c.units.as_slice()),
        c.attach.as_slice(),
        c.coexist,
        c.placed_in,
        c.flags.0
    )
}

/// What changed from `before` to `after`.
#[must_use]
pub fn delta(before: &CoreState, after: &CoreState) -> String {
    let mut out = String::new();
    diff_values("meta", &before.meta, &after.meta, &mut out);
    diff_values("seats", &before.seats, &after.seats, &mut out);
    for i in 0..before.players.len().max(after.players.len()) {
        if i < before.players.len() && i < after.players.len() {
            diff_values(&format!("player{i}"), before.players.get(i), after.players.get(i), &mut out);
        } else {
            let _ = writeln!(out, "player{i}: count {} -> {}", before.players.len(), after.players.len());
        }
    }
    for s in System::ALL {
        let (a, b) = (before.board.sys(s), after.board.sys(s));
        if a != b {
            let _ = writeln!(out, "sys[{s:?}]: {} -> {}", sys_text(a), sys_text(b));
        }
    }
    for p in Planet::ALL {
        let (a, b) = (before.board.planet(p), after.board.planet(p));
        if a != b {
            let _ = writeln!(out, "planet[{p:?}]: {} -> {}", planet_text(a), planet_text(b));
        }
    }
    if before.map != after.map {
        let _ = writeln!(out, "map: changed");
    }
    if before.decks != after.decks {
        let _ = writeln!(out, "decks: changed");
    }
    diff_values("cards", &before.cards, &after.cards, &mut out);
    diff_values("objectives", &before.objectives, &after.objectives, &mut out);
    diff_values("agenda", &before.agenda, &after.agenda, &mut out);
    diff_values("strategy", &before.strategy, &after.strategy, &mut out);
    diff_values("explore", &before.explore, &after.explore, &mut out);
    diff_values("trade", &before.trade, &after.trade, &mut out);
    diff_values("marks", &before.marks, &after.marks, &mut out);
    diff_values("transient", &before.transient, &after.transient, &mut out);
    diff_values("pending", &before.pending, &after.pending, &mut out);
    let (pa, pb): (Vec<_>, Vec<_>) = (before.rng.positions().collect(), after.rng.positions().collect());
    if pa != pb {
        let _ = writeln!(out, "rng: positions {pa:?} -> {pb:?}");
    }
    // The ledger is append-only: print what was appended.
    for (name, a, b) in [("events", before.ledger.events.len(), after.ledger.events.len()), ("dice", before.ledger.dice.len(), after.ledger.dice.len()), ("vp", before.ledger.vp.len(), after.ledger.vp.len()), ("relics", before.ledger.relics.len(), after.ledger.relics.len())] {
        if b != a {
            let _ = writeln!(out, "ledger.{name}: {a} -> {b}");
        }
    }
    for (i, e) in after.ledger.events.iter().enumerate().skip(before.ledger.events.len()) {
        let _ = writeln!(out, "event[{i}]: {e:?}");
    }
    for (i, e) in after.ledger.vp.iter().enumerate().skip(before.ledger.vp.len()) {
        let _ = writeln!(out, "vp[{i}]: {e:?}");
    }
    out
}
