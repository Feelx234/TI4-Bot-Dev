//! What one remote seat is allowed to see.
//!
//! The host's window is omniscient — it is the reviewer's view. A remote player is not: everything
//! that leaves the host for a seat passes through [`frame_for`] or [`header_for`] first. Redaction is
//! done here, on the host, so a modified client cannot get back what was never sent.
//!
//! `ti4_model::view::view_for` covers the hands and unscored secrets of other players. A remote seat
//! needs more than that, because the whole state crosses the wire:
//!
//! - every deck's order (objectives, agendas, action cards, secrets, relics, exploration) would tell
//!   a player what comes next, so each card becomes a [`HIDDEN`] marker and only the size survives;
//! - `rng_seed` and the manifest's seeds would let a player replay the game and recover all of it;
//! - a promissory note held face down by someone else keeps its holder but loses its identity.
//!
//! The frame around the state also talks. Other seats' decisions list their options — their hand —
//! so only the viewer's own decisions are kept. Events, rolls and action summaries are scrubbed of
//! every identity that was redacted from the state. That scrub is by id: a label that spells out a
//! private card's *name* is not caught, which is why other seats' decisions are dropped outright
//! rather than scrubbed.

use std::collections::BTreeSet;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;
use ti4_model::view::{HIDDEN, view_for};
use ti4_review::{ReviewFrame, ReviewSession};

/// Ids shorter than this are not scrubbed inside longer strings: they would match ordinary words.
const MIN_TOKEN: usize = 3;

/// The session header a remote seat receives: no frames, no seeds, no host paths.
#[must_use]
pub fn header_for(header: &ReviewSession) -> ReviewSession {
    let mut header = header.clone();
    header.frames.clear();
    let manifest = &mut header.manifest;
    manifest.seed = 0;
    manifest.tile_seed = 0;
    manifest.checkpoint_path = String::new();
    manifest.map_pool_path = String::new();
    header
}

/// One frame as `viewer` may see it.
#[must_use]
pub fn frame_for(frame: &ReviewFrame, viewer: &PlayerId) -> ReviewFrame {
    let private = private_ids(&frame.state, viewer);
    ReviewFrame {
        index: frame.index,
        engine_step: frame.engine_step,
        decision_count: frame.decision_count,
        action_count: frame.action_count,
        round: frame.round,
        phase: frame.phase,
        active: frame.active.clone(),
        resolved_choice: frame.resolved_choice,
        action_completed: frame.action_completed,
        finished: frame.finished,
        error: frame
            .error
            .as_ref()
            .map(|error| scrub_text(error, &private)),
        new_events: scrubbed(&frame.new_events, &private).unwrap_or_default(),
        structured_events: scrubbed(&frame.structured_events, &private).unwrap_or_default(),
        rolls: scrubbed(&frame.rolls, &private).unwrap_or_default(),
        decisions: frame
            .decisions
            .iter()
            .filter(|decision| decision.player == viewer.as_str())
            .cloned()
            .collect(),
        action_summary: frame
            .action_summary
            .as_ref()
            .and_then(|summary| scrubbed(summary, &private)),
        action_in_progress: frame
            .action_in_progress
            .as_ref()
            .and_then(|summary| scrubbed(summary, &private)),
        state: state_for(&frame.state, viewer),
    }
}

/// The state as `viewer` may see it: [`view_for`], plus decks, seed and face-down notes.
#[must_use]
pub fn state_for(state: &GameState, viewer: &PlayerId) -> GameState {
    let mut view = view_for(state, viewer);
    view.rng_seed = 0;
    hide_all(&mut view.objective_deck);
    hide_all(&mut view.relic_deck);
    hide_all(&mut view.action_card_deck);
    hide_all(&mut view.secret_deck);
    for card in &mut view.agenda_deck {
        HIDDEN.clone_into(card);
    }
    for deck in view.exploration_decks.values_mut() {
        for card in deck.iter_mut() {
            HIDDEN.clone_into(card);
        }
    }
    // A note's key is its identity. Keys must stay unique, so a hidden one is numbered: the holder
    // and the count of notes they hold stay public, as they are at a real table.
    let mut hidden = 0_usize;
    view.promissory_notes = std::mem::take(&mut view.promissory_notes)
        .into_iter()
        .map(|(note, holder)| {
            if &holder == viewer || view.promissory_faceup.contains(&note) {
                (note, holder)
            } else {
                hidden += 1;
                (format!("{HIDDEN}{hidden}"), holder)
            }
        })
        .collect();
    view
}

/// Every identity [`state_for`] takes away from `viewer`.
#[must_use]
pub fn private_ids(state: &GameState, viewer: &PlayerId) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    let warrant = state.laws.get("warrant");
    for player in &state.players {
        if &player.id == viewer {
            continue;
        }
        extend(&mut ids, &player.action_cards);
        if warrant.is_none_or(|owner| *owner != player.id.to_string()) {
            extend(&mut ids, &player.secret_objectives);
        }
    }
    extend(&mut ids, &state.objective_deck);
    extend(&mut ids, &state.relic_deck);
    extend(&mut ids, &state.action_card_deck);
    extend(&mut ids, &state.secret_deck);
    ids.extend(state.agenda_deck.iter().cloned());
    for deck in state.exploration_decks.values() {
        ids.extend(deck.iter().cloned());
    }
    for (note, holder) in &state.promissory_notes {
        if holder != viewer && !state.promissory_faceup.contains(note) {
            ids.insert(note.clone());
        }
    }
    ids.remove(HIDDEN);
    ids
}

/// Replace every card with the marker, keeping the count. Every id type is a transparent string, so
/// the marker is built the same way a loaded file builds one.
fn hide_all<T: DeserializeOwned>(cards: &mut [T]) {
    for card in cards {
        if let Ok(marker) = serde_json::from_value(Value::String(HIDDEN.to_owned())) {
            *card = marker;
        }
    }
}

fn extend<T: Serialize>(ids: &mut BTreeSet<String>, items: &[T]) {
    for item in items {
        if let Ok(Value::String(id)) = serde_json::to_value(item) {
            ids.insert(id);
        }
    }
}

/// Round-trip a value through JSON with every private id replaced. `None` when the value does not
/// survive the trip, so a caller drops the field instead of sending it unscrubbed.
fn scrubbed<T: Serialize + DeserializeOwned>(value: &T, private: &BTreeSet<String>) -> Option<T> {
    let mut json = serde_json::to_value(value).ok()?;
    scrub_value(&mut json, private);
    serde_json::from_value(json).ok()
}

fn scrub_value(value: &mut Value, private: &BTreeSet<String>) {
    match value {
        Value::String(text) => *text = scrub_text(text, private),
        Value::Array(items) => {
            for item in items {
                scrub_value(item, private);
            }
        }
        Value::Object(map) => {
            let entries = std::mem::take(map);
            for (key, mut item) in entries {
                scrub_value(&mut item, private);
                map.insert(scrub_text(&key, private), item);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

/// Replace private ids in a string: the whole string, a word-like token inside it, or — for ids
/// that are not word-like themselves, such as `cf:letnev` — any occurrence.
#[must_use]
pub fn scrub_text(text: &str, private: &BTreeSet<String>) -> String {
    if private.contains(text) {
        return HIDDEN.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut token = String::new();
    let flush = |token: &mut String, out: &mut String| {
        if token.len() >= MIN_TOKEN && private.contains(token.as_str()) {
            out.push_str(HIDDEN);
        } else {
            out.push_str(token);
        }
        token.clear();
    };
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            token.push(ch);
        } else {
            flush(&mut token, &mut out);
            out.push(ch);
        }
    }
    flush(&mut token, &mut out);
    for id in private {
        if id.len() >= MIN_TOKEN
            && !id.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            && out.contains(id.as_str())
        {
            out = out.replace(id.as_str(), HIDDEN);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(ids: &[&str]) -> BTreeSet<String> {
        ids.iter().map(|id| (*id).to_owned()).collect()
    }

    #[test]
    fn whole_strings_tokens_and_punctuated_ids_are_scrubbed() {
        let private = set(&["sabotage2", "cf:letnev", "ab"]);
        assert_eq!(scrub_text("sabotage2", &private), "?");
        assert_eq!(
            scrub_text("ACTION_CARD_DRAWN:sabotage2", &private),
            "ACTION_CARD_DRAWN:?"
        );
        assert_eq!(
            scrub_text("gave cf:letnev to seat1", &private),
            "gave ? to seat1"
        );
        // A near miss is left alone, and so is a token too short to be safely matched.
        assert_eq!(scrub_text("sabotage21 ab", &private), "sabotage21 ab");
    }

    #[test]
    fn object_keys_are_scrubbed_with_their_values() {
        let mut value = serde_json::json!({"sabotage2": ["sabotage2", 1], "kept": "x"});
        scrub_value(&mut value, &set(&["sabotage2"]));
        assert_eq!(value, serde_json::json!({"?": ["?", 1], "kept": "x"}));
    }
}
