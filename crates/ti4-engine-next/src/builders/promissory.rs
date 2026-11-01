//! Dealing and moving promissory notes while a position is built (legacy `promissory::{deal, take,
//! receive, support, note_id, alias_of, owner_of, faction_name, seat_of, is_play_area, held_by,
//! may_receive}`, the string bodies). Lane B's `promissory` queries are the reads of the game.

use std::collections::BTreeMap;

use ti4_content::ContentStore;
use ti4_model::content_types::{ContentType, SourceSet};
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

/// Prefix of a Support for the Throne note id.
pub const SUPPORT_PREFIX: &str = "support:";

/// The four generic notes every faction holds: Ceasefire, Political Secret, Trade Agreement,
/// Alliance (`promissory::GENERIC`).
pub const GENERIC: &[&str] = &["cf", "ps", "ta", "an"];

const GENERIC_PREFIX: &str = "<color>_";

/// This owner's Support for the Throne, by the faction name that owns it.
#[must_use]
pub fn support(owner_name: &str) -> String {
    format!("{SUPPORT_PREFIX}{owner_name}")
}

/// A note id: the printed alias, and whose copy it is (the owner's faction name).
#[must_use]
pub fn note_id(alias: &str, owner_name: &str) -> String {
    format!("{alias}:{owner_name}")
}

/// The printed alias a note id carries.
#[must_use]
pub fn alias_of(note: &str) -> &str {
    note.split_once(':').map_or(note, |(alias, _)| alias)
}

/// The faction name a note belongs to, or `None` if it is not a note id at all.
#[must_use]
pub fn owner_of(note: &str) -> Option<String> {
    if let Some(owner) = note.strip_prefix(SUPPORT_PREFIX) {
        return Some(owner.to_owned());
    }
    note.split_once(':').map(|(_, owner)| owner.to_owned())
}

/// The faction a player plays: the identity embedded in every note id that player owns.
#[must_use]
pub fn faction_name(state: &GameState, player: &PlayerId) -> String {
    state
        .player(player)
        .map(|seat| seat.faction.as_str().to_owned())
        .unwrap_or_default()
}

/// The seat playing a given faction name: the first match in seating order.
#[must_use]
pub fn seat_of(state: &GameState, name: &str) -> Option<PlayerId> {
    state
        .seating_order
        .iter()
        .find(|id| {
            state
                .player(id)
                .is_some_and(|seat| seat.faction.as_str() == name)
        })
        .cloned()
}

/// Every note this player holds, their own or received, in a stable order.
#[must_use]
pub fn held_by(state: &GameState, player: &PlayerId) -> Vec<String> {
    state
        .promissory_notes
        .iter()
        .filter(|(_, holder)| *holder == player)
        .map(|(note, _)| note.clone())
        .collect()
}

/// Whether `receiver` may be given `note`: Mahact's Hubris says "Other players cannot give you
/// their 'Alliance' promissory note."
#[must_use]
pub fn may_receive(state: &GameState, receiver: &PlayerId, note: &str) -> bool {
    !(alias_of(note) == "an"
        && state
            .player(receiver)
            .is_some_and(|seat| seat.faction.as_str() == "mahact"))
}

/// Put every player's own notes in their hand at setup (69.1).
pub fn deal(state: &mut GameState, content: &ContentStore, sources: SourceSet) {
    let mut hands = BTreeMap::new();
    for seat in &state.players {
        let mut aliases: Vec<String> = GENERIC.iter().map(|alias| (*alias).to_owned()).collect();
        // Mahact, Hubris: "During setup, purge your Alliance promissory note."
        if super::factions::purges_alliance(seat.faction.as_str()) {
            aliases.retain(|alias| alias != "an");
        }
        // A faction's own note, read from the corpus rather than a hard-coded table.
        aliases.extend(
            ti4_content::factions::get(content, seat.faction.as_str())
                .map(|faction| {
                    faction
                        .promissory_notes()
                        .into_iter()
                        .map(ToOwned::to_owned)
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default(),
        );
        for alias in aliases {
            // The id carries the faction's name; the map value stays the seat that holds it.
            hands.insert(note_id(&alias, seat.faction.as_str()), seat.id.clone());
        }
    }
    let _ = sources;
    state.promissory_notes = hands;
}

/// Whether a note lives faceup in a play area rather than in hand (69.3), read from the corpus's
/// `playArea` field (`promissory::is_play_area_text`).
#[must_use]
pub fn is_play_area(content: &ContentStore, note: &str) -> bool {
    let Some(owner) = owner_of(note) else {
        return false;
    };
    let alias = alias_of(note);
    let record = content
        .get(ContentType::PromissoryNotes, alias)
        .or_else(|| {
            content.get(
                ContentType::PromissoryNotes,
                &format!("{GENERIC_PREFIX}{alias}"),
            )
        });
    match (record, record.and_then(|r| r.text("faction"))) {
        (Some(record), Some(faction)) => record.flag("playArea") && faction == owner,
        (Some(record), None) => record.flag("playArea"),
        (None, _) => false,
    }
}

/// Whether a note's own text places it faceup with an ACTION rather than on receipt.
fn is_action_placed(state: &GameState, alias: &str) -> bool {
    matches!(
        alias,
        "convoys" | "blood_pact" | "dark_pact" | "shareknowledge" | "sever"
    ) || (alias == "pop" && state.rules_version >= ti4_model::state::RULES_FACTION_EFFECTS)
}

/// Give a note to a holder, faceup if that is where the card lives (69.3).
pub fn take(state: &mut GameState, content: &ContentStore, holder: &PlayerId, note: &str) {
    state
        .promissory_notes
        .insert(note.to_owned(), holder.clone());
    // Antivirus is a play-area note, but its text places it ("At the start of a combat: Place this
    // card faceup"): it waits in hand until the holder does.
    if is_play_area(content, note)
        && !is_action_placed(state, alias_of(note))
        && alias_of(note) != "antivirus"
    {
        state.promissory_faceup.insert(note.to_owned());
    }
}

/// Put another player's Support faceup in a play area and award its point. Returns `false` when
/// the note cannot move: nobody may hold their own Support, and a Support already lent out cannot
/// be lent again.
pub fn receive(state: &mut GameState, holder: &PlayerId, note: &str) -> bool {
    let Some(name) = owner_of(note) else {
        return false;
    };
    let Some(owner) = seat_of(state, &name) else {
        return false;
    };
    if &owner == holder || state.support_holders.contains_key(&owner) {
        return false;
    }
    state.support_holders.insert(owner, holder.clone());
    super::objectives::adjust_victory_points(state, holder, 1, "support_for_the_throne");
    true
}
