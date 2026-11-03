//! The hidden-card reads of the surface (`factions::hooks_cards` and `promissory::has_commander_ability` of the legacy crate, as far as
//! `SeatObservation::{revealed_cards, agenda_deck_ends}` use them): what other seats have shown the bound seat, the standing permissions of
//! the hooks (M'aban), and the Naalu commander's ability. The recorded reveals are a plain read of the public state's marks; the hooks and the
//! commander ability are the core kernel's. A position the core cannot hold answers no standing permission and no ability.

use ti4_engine_core::generated::Leader;
use ti4_engine_core::hooks::RevealKind as CoreKind;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use super::objectives::ObjView;
use crate::generated::IdStr;

/// What kind of hidden card a reveal shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RevealKind {
    /// Action cards in a hand (ids are action card aliases).
    ActionCards,
    /// Promissory notes in a hand (ids are note ids), faceup notes excluded: those are public.
    PromissoryNotes,
    /// Secret objectives in a hand (ids are secret objective aliases).
    SecretObjectives,
}

impl RevealKind {
    fn from_token(token: &str) -> Option<Self> {
        match token {
            "action_cards" => Some(Self::ActionCards),
            "promissory_notes" => Some(Self::PromissoryNotes),
            "secret_objectives" => Some(Self::SecretObjectives),
            _ => None,
        }
    }

    const fn core(self) -> CoreKind {
        match self {
            Self::ActionCards => CoreKind::ActionCards,
            Self::PromissoryNotes => CoreKind::PromissoryNotes,
            Self::SecretObjectives => CoreKind::SecretObjectives,
        }
    }
}

/// Cards of one kind that `owner` has shown to a viewer, as the viewer's observation reports them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revealed {
    /// The holder whose hand was shown.
    pub owner: PlayerId,
    /// Which kind of card.
    pub kind: RevealKind,
    /// The shown ids, in the order they were revealed, only those the owner still holds.
    pub ids: Vec<String>,
}

const REVEAL_PREFIX: &str = "cards:reveal:";

/// Whether `owner` currently holds `id` as a hidden card of `kind`.
fn held(state: &GameState, owner: &PlayerId, kind: RevealKind, id: &str) -> bool {
    match kind {
        RevealKind::ActionCards => state.player(owner).is_some_and(|seat| seat.action_cards.iter().any(|card| card.as_str() == id)),
        RevealKind::SecretObjectives => state.player(owner).is_some_and(|seat| seat.secret_objectives.iter().any(|card| card.as_str() == id)),
        RevealKind::PromissoryNotes => state.promissory_notes.get(id) == Some(owner) && !state.promissory_faceup.contains(id),
    }
}

/// What other seats have shown `viewer` of their hidden cards, and nothing of anyone else's: the recorded rows, a card its owner no longer
/// holds dropped. Ordered by owner id then kind, ids in reveal order.
#[must_use]
pub fn revealed_to(state: &GameState, viewer: &PlayerId) -> Vec<Revealed> {
    let mut merged: std::collections::BTreeMap<(PlayerId, RevealKind), Vec<String>> = std::collections::BTreeMap::new();
    for (key, row) in state.faction_marks.range(REVEAL_PREFIX.to_owned()..) {
        let Some(detail) = key.strip_prefix(REVEAL_PREFIX) else {
            break;
        };
        let parts: Vec<&str> = detail.split('|').collect();
        let [scope, _source, row_viewer, owner, kind] = parts[..] else {
            continue;
        };
        if !matches!(scope, "choice" | "action" | "standing") || row_viewer != viewer.as_str() {
            continue;
        }
        let Some(kind) = RevealKind::from_token(kind) else {
            continue;
        };
        let owner = PlayerId::new(owner);
        let ids = merged.entry((owner.clone(), kind)).or_default();
        for id in row.split(',') {
            if held(state, &owner, kind, id) && !ids.iter().any(|seen| seen == id) {
                ids.push(id.to_owned());
            }
        }
    }
    merged.into_iter().filter(|(_, ids)| !ids.is_empty()).map(|((owner, kind), ids)| Revealed { owner, kind, ids }).collect()
}

/// Whether any module currently lets `viewer` inspect `owner`'s hidden hand of `kind` (the core kernel's hook table).
#[must_use]
pub fn may_view_hand(view: Option<&ObjView>, viewer: &PlayerId, owner: &PlayerId, kind: RevealKind) -> bool {
    // M'aban reads the neighbours of the viewer, which needs the map: without one nobody may look (the core's hook would read a slice that
    // was not loaded).
    let Some(view) = view.filter(|view| view.has_map()) else { return false };
    let owned = view.owned();
    let (Some(viewer), Some(owner)) = (owned.seat(viewer), owned.seat(owner)) else { return false };
    viewer != owner && owned.kernel().hooks.cards.may_view_hand(owned.core(), owned.kernel(), viewer, owner, kind.core())
}

/// Whether `player` has the ability of the named commander (`naalucommander`): their own unlocked commander or a durable grant, Mahact's
/// Imperia, or a faceup Alliance whose owner has it unlocked.
#[must_use]
pub fn has_commander_ability(view: Option<&ObjView>, player: &PlayerId, commander: &str) -> bool {
    let Some(view) = view else { return false };
    let owned = view.owned();
    let (Some(seat), Some(leader)) = (owned.seat(player), <Leader as IdStr>::from_id(commander)) else { return false };
    ti4_engine_core_factions::cc_ground::borrowed::query::has_commander_ability(owned.core(), owned.kernel(), seat, leader)
}
