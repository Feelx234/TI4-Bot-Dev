//! The typed answers of `Area::ActionCards` as plain `Option`s, whether or not `typed-core` is compiled in:
//! without it every function answers `None` and the string side stands (docs/ENGINE_CORE_STATE.md 6.2).
//! Each is called only from a `dual_typed!` typed branch, so a string-only run builds no view.
#![allow(clippy::unnecessary_wraps)]

use ti4_content::ContentStore;
use ti4_model::id::{ActionCardId, PlayerId};
use ti4_model::state::GameState;

/// `action_cards::is_component_action`.
pub(crate) fn is_component_action(alias: &ActionCardId) -> Option<bool> {
    #[cfg(feature = "typed-core")]
    {
        crate::action_cards_view::is_component_action(alias)
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = alias;
        None
    }
}

/// `action_cards::hand_limit`.
pub(crate) fn hand_limit(state: &GameState, content: &ContentStore, player: &PlayerId) -> Option<usize> {
    #[cfg(feature = "typed-core")]
    {
        crate::action_cards_view::ActionView::of(state)?.hand_limit(player, content)
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (state, content, player);
        None
    }
}

/// Whether `player` may play action cards at all (the head of `is_playable`).
pub(crate) fn may_play(state: &GameState, player: &PlayerId) -> Option<bool> {
    #[cfg(feature = "typed-core")]
    {
        crate::action_cards_view::ActionView::of(state)?.may_play(player)
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (state, player);
        None
    }
}

/// `action_cards::has_legal_target` (`None` for the cards the core does not answer).
pub(crate) fn has_legal_target(state: &GameState, player: &PlayerId, alias: &ActionCardId, sources: ti4_model::content_types::SourceSet) -> Option<bool> {
    #[cfg(feature = "typed-core")]
    {
        crate::action_cards_view::ActionView::of(state)?.has_legal_target(player, alias, sources)
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (state, player, alias, sources);
        None
    }
}

/// `action_cards::reaction_can_matter` for a window that names a system (`None`: bail).
pub(crate) fn reaction_can_matter(
    state: &GameState,
    player: &PlayerId,
    alias: &ActionCardId,
    system: &str,
    actor: Option<&str>,
) -> Option<bool> {
    #[cfg(feature = "typed-core")]
    {
        crate::action_cards_view::ActionView::of(state)?.reaction_can_matter(player, alias, system, actor)
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (state, player, alias, system, actor);
        None
    }
}

/// `action_cards::cost_is_payable` (`None`: bail).
pub(crate) fn cost_is_payable(state: &GameState, player: &PlayerId, alias: &ActionCardId) -> Option<bool> {
    #[cfg(feature = "typed-core")]
    {
        crate::action_cards_view::ActionView::of(state)?.cost_is_payable(player, alias)
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (state, player, alias);
        None
    }
}

/// The own-hand question rendered by the core (`None`: bail).
pub(crate) fn own_hand_choice(
    state: &GameState,
    player: &PlayerId,
    source: &str,
    subtype: &str,
    prompt: &str,
    firsts: &[ActionCardId],
    optional: bool,
) -> Option<crate::choice::Choice> {
    #[cfg(feature = "typed-core")]
    {
        crate::action_cards_view::own_hand_choice(state, player, source, subtype, prompt, firsts, optional)
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (state, player, source, subtype, prompt, firsts, optional);
        None
    }
}

/// Play a stamp-or-flag effect on the core (`None`: the string effect runs).
pub(crate) fn play_simple(state: &mut GameState, player: &PlayerId, effect: &str, sources: &ti4_model::content_types::SourceSet) -> Option<()> {
    #[cfg(feature = "typed-core")]
    {
        crate::action_cards_view::play_simple(state, player, effect, sources)
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (state, player, effect, sources);
        None
    }
}

/// Whether the core has a window for the card's effect.
pub(crate) fn plays(alias: &ActionCardId) -> bool {
    #[cfg(feature = "typed-core")]
    {
        crate::action_cards_view::plays(alias)
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = alias;
        false
    }
}

/// What a play leaves besides the state: the faces it rolled and where the random streams stand (`None`: the card
/// rolls nothing).
pub(crate) type Rolled = (Vec<u32>, Option<crate::rng::RngPositions>);

/// Whether the card's effect rolls the game's dice (the core window needs the streams lent).
pub(crate) fn rolls(alias: &ActionCardId) -> bool {
    #[cfg(feature = "typed-core")]
    {
        crate::action_cards_view::rolls(alias)
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = alias;
        false
    }
}

/// The core half of a played card effect (`None`: bail). `before` is where the streams stood when the dice are ones the
/// core can reproduce.
pub(crate) fn play_window(
    state: &mut GameState,
    player: &PlayerId,
    alias: &ActionCardId,
    sources: &ti4_model::content_types::SourceSet,
    script: &crate::dual_window::Recording,
    before: Option<&crate::rng::RngPositions>,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
) -> Option<crate::dual::CoreWindow<Rolled>> {
    #[cfg(feature = "typed-core")]
    {
        crate::action_cards_view::play_window(state, player, alias, sources, script, before, galaxy)
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (state, player, alias, sources, script, before, galaxy);
        None
    }
}

/// The hand positions `available_actions` offers (`None`: bail).
pub(crate) fn playable_hand(state: &GameState, player: &PlayerId) -> Option<Vec<usize>> {
    #[cfg(feature = "typed-core")]
    {
        crate::action_cards_view::ActionView::of(state)?.playable_hand(player)
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (state, player);
        None
    }
}

/// `action_cards::discard` on the core (`None`: the string runs).
pub(crate) fn discard(state: &mut GameState, player: &PlayerId, index: usize) -> Option<Option<ActionCardId>> {
    #[cfg(feature = "typed-core")]
    {
        crate::action_cards_view::discard(state, player, index)
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (state, player, index);
        None
    }
}

/// The core half of a card's announcement and resolution (`None`: bail).
pub(crate) fn announce_window(
    state: &mut GameState,
    player: &PlayerId,
    alias: &ActionCardId,
    sources: &ti4_model::content_types::SourceSet,
    script: &crate::dual_window::Recording,
    before: Option<&crate::rng::RngPositions>,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
) -> Option<crate::dual::CoreWindow<(Option<bool>, Rolled)>> {
    #[cfg(feature = "typed-core")]
    {
        crate::action_cards_view::announce_window(state, player, alias, sources, script, before, galaxy)
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (state, player, alias, sources, script, before, galaxy);
        None
    }
}
