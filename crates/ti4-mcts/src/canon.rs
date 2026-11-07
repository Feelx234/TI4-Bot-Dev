//! The candidate list of a tree decision: the options the tree branches on (`docs/MCTS_BOT_PLAN.md` 2.4, 2.5).
//!
//! * `Activate`: the hard filter of the features crate (`ti4_mcts_features::filter`: move / produce / shoot, pass-through when nothing is
//!   kept). Here the dropped options are simply not candidates (the exporter of L1 puts the rule bot's pick back; a search does not).
//! * `Move`: the canonical order: after moving a ship with key `k` in this movement step, only ships with key `>= k` (and `DoneMoving`) are
//!   candidates. Key: capacity providers first, then origin, kind, damaged (the M1 census's `canon_key`: 0.14 % of the rule bot's moved
//!   sets are reachable only out of this order).
//! * Every other tree kind: all options.
//!
//! Candidates are option indexes of the core `Choice` (option `i` is legal answer `i` of the position and option `i` of the wire form).

use ti4_engine_core::choice::{Ask, Choice, Opt, tactical};
use ti4_engine_search::Position;
use ti4_mcts_features::filter;

/// The canonical key of a movement option (`None` for `DoneMoving` and any other option).
pub type MoveKey = (bool, u16, u16, bool);

/// The key of a `Tactical(Move)` ship option.
#[must_use]
pub fn move_key(o: &Opt) -> Option<MoveKey> {
    match o {
        Opt::Tactical(tactical::Opt::Move { origin, kind, damaged, capacity, .. }) => {
            Some((*capacity <= 0, u16::try_from(origin.index()).unwrap_or(u16::MAX), u16::try_from(kind.index()).unwrap_or(u16::MAX), *damaged))
        }
        _ => None,
    }
}

/// Whether the ask is a movement step's `Move`.
#[must_use]
pub const fn is_move(choice: &Choice) -> bool {
    matches!(choice.ask, Ask::Tactical(tactical::Ask::Move { .. }))
}

/// The ship last moved in the current movement step: `(seat, key)`. What a pick leaves for the next `Move` ask: a ship option sets it,
/// anything else (done, another ask) clears it.
#[must_use]
pub fn after_pick(choice: &Choice, opt: Option<&Opt>) -> Option<(u8, MoveKey)> {
    if !is_move(choice) {
        return None;
    }
    opt.and_then(move_key).map(|k| (choice.actor.0, k))
}

/// The candidates (option indexes, ascending) of the tree decision `choice` at `pos`, given the ship last moved in this movement step.
/// Never empty while the choice has an option.
#[must_use]
pub fn candidates(pos: &Position, choice: &Choice, last_move: Option<(u8, MoveKey)>) -> Vec<usize> {
    let mut c = if filter::is_activation(&choice.ask) {
        pos.seen().map_or_else(|| (0..choice.opts.len()).collect(), |seen| filter::options(&seen, choice))
    } else {
        (0..choice.opts.len()).collect()
    };
    if is_move(choice)
        && let Some((seat, last)) = last_move
        && seat == choice.actor.0
    {
        c.retain(|&i| choice.opts.as_slice().get(i).and_then(move_key).is_none_or(|k| k >= last));
    }
    if c.is_empty() {
        c = (0..choice.opts.len()).collect();
    }
    c
}
