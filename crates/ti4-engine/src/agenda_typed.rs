//! The typed answers of `Area::AgendaLaws` as plain `Option`s, whether or not `typed-core` is compiled in:
//! without it every function answers `None` and the string side stands (docs/ENGINE_CORE_STATE.md 6.2).
//! Each is called only from a `dual_typed!` typed branch, so a string-only run builds no view.
#![allow(clippy::unnecessary_wraps)]

use std::collections::BTreeMap;

use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlanetId, PlayerId};
use ti4_model::state::GameState;

/// `outcomes`.
pub(crate) fn outcomes(state: &GameState, sources: SourceSet, alias: &str) -> Option<Vec<String>> {
    #[cfg(feature = "typed-core")]
    {
        crate::agenda_view::AgendaView::full(state, sources)?.outcomes(alias)
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (state, sources, alias);
        None
    }
}

/// `extra_votes`.
pub(crate) fn extra_votes(state: &GameState, player: &PlayerId) -> Option<i64> {
    #[cfg(feature = "typed-core")]
    {
        crate::agenda_view::AgendaView::light(state)?.extra_votes(player)
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (state, player);
        None
    }
}

/// `flat_vote_amount`.
pub(crate) fn flat_vote_amount(state: &GameState) -> Option<Option<i64>> {
    #[cfg(feature = "typed-core")]
    {
        crate::agenda_view::AgendaView::light(state)?.flat_vote_amount()
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (state);
        None
    }
}

/// `votable_planets`.
pub(crate) fn votable_planets(state: &GameState, sources: SourceSet, player: &PlayerId) -> Option<Vec<PlanetId>> {
    #[cfg(feature = "typed-core")]
    {
        crate::agenda_view::AgendaView::full(state, sources)?.votable_planets(player)
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (state, sources, player);
        None
    }
}

/// `reveal_order`.
pub(crate) fn reveal_order(state: &GameState) -> Option<Vec<PlayerId>> {
    #[cfg(feature = "typed-core")]
    {
        crate::agenda_view::AgendaView::light(state)?.reveal_order()
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (state);
        None
    }
}

/// `voting_order`.
pub(crate) fn voting_order(state: &GameState) -> Option<Vec<PlayerId>> {
    #[cfg(feature = "typed-core")]
    {
        crate::agenda_view::AgendaView::light(state)?.voting_order()
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (state);
        None
    }
}

/// `ready_limit`.
pub(crate) fn ready_limit(state: &GameState) -> Option<Option<usize>> {
    #[cfg(feature = "typed-core")]
    {
        crate::agenda_view::AgendaView::light(state)?.ready_limit()
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (state);
        None
    }
}

/// `political_secret_bars`.
pub(crate) fn political_secret_bars(state: &GameState, player: &PlayerId) -> Option<bool> {
    #[cfg(feature = "typed-core")]
    {
        crate::agenda_view::AgendaView::light(state)?.political_secret_bars(player)
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (state, player);
        None
    }
}

/// `undisputed_winner`.
pub(crate) fn undisputed_winner(state: &GameState, alias: &str, counts: &BTreeMap<String, i64>, choices: &[String]) -> Option<Option<String>> {
    #[cfg(feature = "typed-core")]
    {
        crate::agenda_view::AgendaView::light(state)?.undisputed_winner(alias, counts, choices)
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (state, alias, counts, choices);
        None
    }
}

/// `tiebreak_candidates`.
pub(crate) fn tiebreak_candidates(state: &GameState, alias: &str, counts: &BTreeMap<String, i64>, choices: &[String]) -> Option<Vec<String>> {
    #[cfg(feature = "typed-core")]
    {
        crate::agenda_view::AgendaView::light(state)?.tiebreak_candidates(alias, counts, choices)
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (state, alias, counts, choices);
        None
    }
}

/// `vote::is_law` of an agenda alias.
pub(crate) fn is_law(alias: &str) -> Option<bool> {
    #[cfg(feature = "typed-core")]
    {
        crate::agenda_view::is_law(alias)
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = alias;
        None
    }
}
