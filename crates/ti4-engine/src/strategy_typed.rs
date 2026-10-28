//! The typed answers of `Area::Strategy` as plain `Option`s, whether or not `typed-core` is compiled in:
//! without it every function answers `None` and the string side stands (docs/ENGINE_CORE_STATE.md 6.2).
//! Each is called only from a `dual_typed!` typed branch, so a string-only run builds no view.

use ti4_model::id::{PlayerId, StrategyCardId};
use ti4_model::state::GameState;

/// `phase::strategy_pick_order`.
pub(crate) fn pick_order(state: &GameState) -> Option<Vec<PlayerId>> {
    #[cfg(feature = "typed-core")]
    {
        crate::strategy_view::StratView::light(state)?.pick_order()
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = state;
        None
    }
}

/// `phase::next_strategy_picker`.
pub(crate) fn next_picker(state: &GameState) -> Option<Option<PlayerId>> {
    #[cfg(feature = "typed-core")]
    {
        crate::strategy_view::StratView::light(state)?.next_picker()
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = state;
        None
    }
}

/// The picker and the unclaimed cards of `draft::strategy_options`.
pub(crate) fn draft(state: &GameState) -> Option<Option<(PlayerId, Vec<StrategyCardId>)>> {
    #[cfg(feature = "typed-core")]
    {
        crate::strategy_view::StratView::light(state)?.draft()
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = state;
        None
    }
}

/// `Player::unused_strategy_cards`, owned.
pub(crate) fn unused_cards(state: &GameState, player: &PlayerId) -> Option<Vec<StrategyCardId>> {
    #[cfg(feature = "typed-core")]
    {
        crate::strategy_view::StratView::light(state)?.unused_cards(player)
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (state, player);
        None
    }
}

/// `strategy_cards::commodity_limit`.
pub(crate) fn commodity_limit(state: &GameState, player: &PlayerId) -> Option<i32> {
    #[cfg(feature = "typed-core")]
    {
        crate::strategy_view::commodity_limit(state, player)
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (state, player);
        None
    }
}

/// `strategy::secondary_costs_token`.
pub(crate) fn secondary_costs_token(card: &StrategyCardId) -> Option<bool> {
    #[cfg(feature = "typed-core")]
    {
        crate::strategy_view::secondary_costs_token(card)
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = card;
        None
    }
}

/// The view held across the follower checks of one position.
#[cfg(feature = "typed-core")]
pub(crate) type View<'a> = Option<crate::strategy_view::StratView<'a>>;
/// The view held across the follower checks of one position (never built without `typed-core`).
#[cfg(not(feature = "typed-core"))]
pub(crate) type View<'a> = Option<std::marker::PhantomData<&'a ()>>;

/// Open the view of a position; `None` when the area runs on the string side.
pub(crate) fn open_view<'a>(
    state: &'a GameState,
    content: &'a ti4_content::ContentStore,
    sources: ti4_model::content_types::SourceSet,
) -> View<'a> {
    #[cfg(feature = "typed-core")]
    {
        crate::strategy_view::StratView::open(state, content, sources)
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (state, content, sources);
        None
    }
}

/// `strategy::secondary_eligible` on an open view.
pub(crate) fn eligible(view: &View<'_>, player: &PlayerId, card: &StrategyCardId) -> Option<bool> {
    #[cfg(feature = "typed-core")]
    {
        view.as_ref()?.secondary_eligible(player, card)
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (view, player, card);
        None
    }
}
