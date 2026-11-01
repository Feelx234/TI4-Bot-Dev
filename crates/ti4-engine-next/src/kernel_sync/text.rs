//! Text of the kernel's closed sets: event names, payload keys, ability ids. The core holds none of it.
//!
//! An event name is the UPPER_SNAKE_CASE of the core `EventKind` variant (`ActionCardPlayed` <->
//! `ACTION_CARD_PLAYED`); a name outside the closed set is `EventKind::Other(code)` with the code
//! interned in the game's [`Render`]. Payload keys are the same with lower_snake_case. An ability id is
//! opaque text to the core until its owner package types it: `AbilityId { kind, key }` where `key` is the
//! interned full id (`ability:empyrean:aetherpassage:SYSTEM_ACTIVATED:after`).

use std::sync::OnceLock;

use ti4_engine_core::choice::reaction::{AbilityId, AbilityKind, Relation};
use ti4_engine_core::state::{EventKind, Key};

use crate::render::Render;

pub(crate) fn snake(debug: &str, upper: bool) -> String {
    let mut out = String::with_capacity(debug.len() + 6);
    for (i, ch) in debug.chars().enumerate() {
        if ch.is_ascii_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(if upper { ch.to_ascii_uppercase() } else { ch.to_ascii_lowercase() });
    }
    out
}

fn event_names() -> &'static Vec<String> {
    static NAMES: OnceLock<Vec<String>> = OnceLock::new();
    NAMES.get_or_init(|| EventKind::ALL.iter().map(|k| snake(&format!("{k:?}"), true)).collect())
}

fn key_names() -> &'static Vec<String> {
    static NAMES: OnceLock<Vec<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        Key::ALL
            .iter()
            .map(|k| match k {
                // The one key whose variant name differs from its text.
                Key::OtherParty => "other".to_owned(),
                k => snake(&format!("{k:?}"), false),
            })
            .collect()
    })
}

/// The event name of a kind.
#[must_use]
pub fn event_text(r: &Render, kind: EventKind) -> String {
    match kind {
        EventKind::Other(code) => r.code_text(code),
        closed => {
            let i = EventKind::ALL.iter().position(|k| *k == closed).expect("a closed kind is in ALL");
            event_names()[i].clone()
        }
    }
}

/// The kind of an event name (interns names outside the closed set).
#[must_use]
pub fn event_kind(r: &Render, text: &str) -> EventKind {
    event_names()
        .iter()
        .position(|n| n == text)
        .map_or_else(|| EventKind::Other(r.code(text)), |i| EventKind::ALL[i])
}

/// The payload key text.
#[must_use]
pub fn key_text(r: &Render, key: Key) -> String {
    match key {
        Key::Other(code) => r.code_text(code),
        closed => {
            let i = Key::ALL.iter().position(|k| *k == closed).expect("a closed key is in ALL");
            key_names()[i].clone()
        }
    }
}

/// The payload key of a text.
#[must_use]
pub fn key_of(r: &Render, text: &str) -> Key {
    key_names().iter().position(|n| n == text).map_or_else(|| Key::Other(r.code(text)), |i| Key::ALL[i])
}

/// The ability kind a full id text starts with.
#[must_use]
pub fn ability_kind(id: &str) -> AbilityKind {
    match id.split(':').next().unwrap_or("") {
        "ability" => AbilityKind::Ability,
        "leader" => AbilityKind::Leader,
        "breakthrough" => AbilityKind::Breakthrough,
        "technology" => AbilityKind::Technology,
        "unit" => AbilityKind::Unit,
        "promissory" => AbilityKind::Promissory,
        "reaction" => AbilityKind::Reaction,
        _ => AbilityKind::Other,
    }
}

/// The typed id of a registered ability's text (interned).
#[must_use]
pub fn ability_id(r: &Render, text: &str) -> AbilityId {
    AbilityId { kind: ability_kind(text), key: r.code(text) }
}

/// The text of a typed ability id.
#[must_use]
pub fn ability_text(r: &Render, id: AbilityId) -> String {
    r.code_text(id.key)
}

/// `"when"` / `"after"`.
#[must_use]
pub const fn relation_text(relation: Relation) -> &'static str {
    match relation {
        Relation::When => "when",
        Relation::After => "after",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_names_round_trip_and_match_the_known_texts() {
        let r = Render::new(vec!["a".to_owned()]);
        assert_eq!(event_text(&r, EventKind::ActionCardPlayed), "ACTION_CARD_PLAYED");
        assert_eq!(event_text(&r, EventKind::TurnEndedByMinisterOfPeace), "TURN_ENDED_BY_MINISTER_OF_PEACE");
        for k in EventKind::ALL {
            assert_eq!(event_kind(&r, &event_text(&r, k)), k);
        }
        let odd = event_kind(&r, "SOMETHING_ELSE");
        assert!(matches!(odd, EventKind::Other(_)));
        assert_eq!(event_text(&r, odd), "SOMETHING_ELSE");
        assert_eq!(event_kind(&r, "SOMETHING_ELSE"), odd, "the same text gets the same code");
    }

    #[test]
    fn payload_keys_round_trip() {
        let r = Render::new(vec!["a".to_owned()]);
        assert_eq!(key_text(&r, Key::DuringSpaceCombat), "during_space_combat");
        assert_eq!(key_text(&r, Key::OtherParty), "other");
        for k in Key::ALL {
            assert_eq!(key_of(&r, &key_text(&r, k)), k);
        }
    }

    #[test]
    fn ability_ids_keep_their_kind() {
        let r = Render::new(vec!["a".to_owned()]);
        let id = ability_id(&r, "ability:empyrean:aetherpassage:SYSTEM_ACTIVATED:after");
        assert_eq!(id.kind, AbilityKind::Ability);
        assert_eq!(ability_text(&r, id), "ability:empyrean:aetherpassage:SYSTEM_ACTIVATED:after");
        assert_eq!(ability_kind("reaction:a:x:Y:when"), AbilityKind::Reaction);
        assert_eq!(ability_kind("relic:foo"), AbilityKind::Other);
    }
}
