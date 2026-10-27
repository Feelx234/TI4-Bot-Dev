//! Typed order = text order, for ids that are not all the same length.
//!
//! An id enum is numbered in the text order of its ids, which is also the order of keys that end
//! in the id. When more text follows the id the order can differ: `"10"` is a proper prefix of
//! `"100"`, and in `bastion:lost:10:a:cruiser` against `bastion:lost:100:a:cruiser` the byte after
//! the `10` (`:`, which is greater than every digit) decides, so the `100` key sorts first although
//! `System` 10 has the smaller index.
//!
//! [`cmp_indexed`] decides it exactly. Its input is, for every id type, the list of the pairs
//! `(a, b, c)` where id `a` is a proper prefix of id `b` and `c` is the byte of `b` after `a`
//! ([`tables`]; for promissory notes in the ranks of the texts the state writes, `an` for
//! `<color>_an`: [`NOTE_RANK`]); numbers only, computed from the generated id tables (the adapter's test
//! `prefix_tables_are_current` recomputes them and fails when a table is stale).

use core::cmp::Ordering;

pub use super::tables::{
    ACTION_CARD, AGENDA, FACTION, LEADER, NONE, NOTE_RANK, PLANET, PROMISSORY_NOTE, SYSTEM,
    TECHNOLOGY, UNIT, WORD,
};

/// The prefix pairs of one id type, sorted by `(a, b)`: `(a, b, c)`, see the module comment.
pub type PrefixPairs = &'static [(u16, u16, u8)];

/// The byte of id `b` after id `a`, when `a` is a proper prefix of `b`.
#[must_use]
pub fn next_byte(pairs: PrefixPairs, a: usize, b: usize) -> Option<u8> {
    let key = (a as u16, b as u16);
    pairs
        .binary_search_by(|(x, y, _)| (*x, *y).cmp(&key))
        .ok()
        .map(|i| pairs[i].2)
}

/// Compare the text of id `a` followed by the byte `ta` (0 for the end of the key) with the text
/// of id `b` followed by `tb`. `a` and `b` are indexes in the text order of their ids.
#[must_use]
pub fn cmp_indexed(pairs: PrefixPairs, a: usize, ta: u8, b: usize, tb: u8) -> Ordering {
    match a.cmp(&b) {
        Ordering::Equal => ta.cmp(&tb),
        Ordering::Less => match next_byte(pairs, a, b) {
            // `a` is a prefix of `b`: its follower meets the next byte of `b`.
            Some(c) => match ta.cmp(&c) {
                Ordering::Equal => Ordering::Less,
                other => other,
            },
            None => Ordering::Less,
        },
        Ordering::Greater => match next_byte(pairs, b, a) {
            Some(c) => match c.cmp(&tb) {
                Ordering::Equal => Ordering::Greater,
                other => other,
            },
            None => Ordering::Greater,
        },
    }
}
