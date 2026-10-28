//! `Area::ActionCards` (package B6): the typed action-card reads of `ti4_engine_core::rules::action_cards`
//! behind the string interface. One [`ActionView`] is the typed view of one position (the Meta, Seats,
//! Players, Board and Agenda slices). The faction answers are passed to the rules as arguments (they come
//! from the string modules until package C). A card or player id the tables do not know makes a method
//! answer `None` and the caller keeps the string answer.

use ti4_engine_core::generated::{ActionCard, System};
use ti4_engine_core::rules::action_cards::Window;
use ti4_engine_core::rules::action_cards as rules;
use ti4_engine_core::state::{Slice, SliceSet};
use ti4_model::id::{ActionCardId, PlayerId};
use ti4_model::state::GameState;

use crate::core_sync::{CoreView, Ctx, Sources};
use crate::generated::IdStr;

/// What the action-card reads need.
pub const READS: SliceSet = SliceSet::of(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Agenda]);

/// The typed view of one position.
pub struct ActionView<'a> {
    view: CoreView,
    cx: Ctx,
    state: &'a GameState,
    blanked: bool,
}

/// `action_cards::is_component_action` of a card id (needs no view).
#[must_use]
pub fn is_component_action(alias: &ActionCardId) -> Option<bool> {
    Some(rules::is_component_action(ActionCard::from_id(alias.as_str())?))
}

impl<'a> ActionView<'a> {
    /// The view of a position; `None` when something is not mapped.
    #[must_use]
    pub fn of(state: &'a GameState) -> Option<Self> {
        let mut cx = Ctx::of(state).ok()?;
        let view = CoreView::of(state, &mut cx, Sources { galaxy: None, rng: None }, READS).ok()?;
        Some(Self { view, cx, state, blanked: crate::laws::blanked(state) })
    }

    /// The view when the area runs on the core or both (the offline check), else `None`.
    #[must_use]
    pub fn open(state: &'a GameState) -> Option<Self> {
        if crate::dual::backend(crate::dual::Area::ActionCards) == crate::dual::Backend::String {
            return None;
        }
        Self::of(state)
    }

    /// `action_cards::hand_limit`.
    #[must_use]
    pub fn hand_limit(&self, player: &PlayerId, content: &ti4_content::ContentStore) -> Option<usize> {
        self.cx.owners.seat(player.as_str())?;
        Some(rules::hand_limit(&self.view, self.blanked, |base| {
            crate::factions::hooks_economy::action_card_limit(self.state, content, player, base)
        }))
    }

    /// The head gates of `action_cards::is_playable`: whether `player` may play action cards at all.
    #[must_use]
    pub fn may_play(&self, player: &PlayerId) -> Option<bool> {
        let seat = self.cx.owners.seat(player.as_str())?;
        Some(rules::may_play(
            &self.view,
            self.blanked,
            seat,
            crate::factions::hooks_economy::action_cards_forbidden(self.state, player),
            crate::laws::political_secret_bars(self.state, player),
        ))
    }

    /// `action_cards::has_legal_target` for the cards the core answers; `None` is the documented bail.
    #[must_use]
    pub fn has_legal_target(&self, player: &PlayerId, alias: &ActionCardId) -> Option<bool> {
        let seat = self.cx.owners.seat(player.as_str())?;
        rules::has_legal_target(&self.view, self.blanked, seat, ActionCard::from_id(alias.as_str())?)
    }

    /// `action_cards::reaction_can_matter` after the cost check, for a window naming `system`; `None` is
    /// the documented bail. `actor` is the event's `player` text.
    #[must_use]
    pub fn reaction_can_matter(
        &self,
        player: &PlayerId,
        alias: &ActionCardId,
        system: &str,
        actor: Option<&str>,
    ) -> Option<bool> {
        let seat = self.cx.owners.seat(player.as_str())?;
        let actor = match actor {
            None => None,
            Some(a) => Some(self.cx.owners.seat(a)?),
        };
        let window = Window { system: System::from_id(system)?, actor };
        rules::reaction_can_matter(
            &self.view,
            crate::core_adapter::scope(ti4_model::content_types::SourceSet::all()),
            seat,
            ActionCard::from_id(alias.as_str())?,
            window,
        )
    }

    /// `action_cards::cost_is_payable`; `None` is the documented bail.
    #[must_use]
    pub fn cost_is_payable(&self, player: &PlayerId, alias: &ActionCardId) -> Option<bool> {
        let seat = self.cx.owners.seat(player.as_str())?;
        let card = ActionCard::from_id(alias.as_str())?;
        // The goods are asked only for the card that reads them (the string supply rules answer).
        let goods = if card == ActionCard::Bribery { crate::supply::potential_goods(self.state, player) } else { 0 };
        rules::cost_is_payable(&self.view, seat, card, goods)
    }
}
