//! `action_cards.rs` of the legacy crate, as far as the product surface reads it: the hand limit (the core's rule over the kernel's hooks) and
//! what Insider Information showed a seat (plain reads of the public state's marks). A position the core cannot hold has the printed limit.

use ti4_content::ContentStore;
use ti4_engine_core::state::{Slice, SliceSet};
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use super::view::Loaded;

/// 2.4: the printed hand limit.
pub const HAND_LIMIT: usize = 7;

/// 2.4: how many action cards `player` may hold now. `usize::MAX` when there is no limit.
#[must_use]
pub fn hand_limit(state: &GameState, _content: &ContentStore, player: &PlayerId) -> usize {
    hand_limit_in(state, player).unwrap_or(HAND_LIMIT)
}

fn hand_limit_in(state: &GameState, player: &PlayerId) -> Option<usize> {
    let view = Loaded::of(state, None, ti4_model::content_types::FULL, SliceSet::of(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Agenda, Slice::Marks, Slice::Strategy, Slice::Cards]))?;
    let seat = view.seat(player)?;
    let (core, k) = (view.core(), view.kernel());
    Some(ti4_engine_core::rules::action_cards::hand_limit(core, view.blanked(), |base| k.hooks.economy.action_card_limit(core, k, seat, base)))
}

/// Whether a `faction_marks` key is an Insider Information peek (see [`insider_peek_key`]).
#[must_use]
pub fn is_insider_peek_key(key: &str) -> bool {
    key.starts_with("private:") && key.ends_with(":insider:peek")
}

/// The `faction_marks` key under which Insider Information leaves what its player saw: a `private:<player>:` row, which only that seat is ever sent.
#[must_use]
pub fn insider_peek_key(player: &PlayerId) -> String {
    format!("private:{player}:insider:peek")
}

/// What `player` was shown by Insider Information and may still see: the agenda ids they looked at (up to 3, top first) while they are still
/// the top cards of the deck in that order. A peek goes stale the moment the top of the deck moves, so it is checked against the deck on every read.
#[must_use]
pub fn insider_peek(state: &GameState, player: &PlayerId) -> Vec<String> {
    let Some(mark) = state.faction_marks.get(&insider_peek_key(player)) else {
        return Vec::new();
    };
    let seen: Vec<&str> = mark.split(',').filter(|id| !id.is_empty()).collect();
    let still_on_top = !seen.is_empty() && seen.len() <= state.agenda_deck.len() && seen.iter().zip(&state.agenda_deck).all(|(seen, card)| *seen == card.as_str());
    if still_on_top { seen.into_iter().map(str::to_owned).collect() } else { Vec::new() }
}
