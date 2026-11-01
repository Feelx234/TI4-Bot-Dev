//! A whole position as the faction hooks read it (copy of `Pos` of the legacy `core_factions/cc.rs`): every slice but the map, the owner
//! table, and the kernel with every faction module registered. For the reads that ask a hook of the core (`hooks.direct`, the ability
//! environment) instead of the printed data.

use std::collections::BTreeSet;

use ti4_engine_core::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::state::{CoreState, Slice, SliceSet};
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use crate::core_sync::codec::Sources as CodecSources;
use crate::core_sync::{CoreView, Ctx};
use crate::ties::production_view::kernel;

/// The state as the typed core can load it: every other seat's hand in a redacted view (`observed::redacted_full_state`, what the host
/// projects from) is a row of `HIDDEN` markers, which are no cards. They are replaced by cards the tables know, so the sizes of the hands are
/// kept; the second value lists the players whose SECRET objectives were masked, whose secrets nothing may be asked about. A state with no marker
/// is borrowed as it is.
#[must_use]
pub fn loadable(state: &GameState) -> (std::borrow::Cow<'_, GameState>, Vec<PlayerId>) {
    use crate::generated::IdStr;
    use ti4_engine_core::generated::{ActionCard, SecretObjective};
    let hidden_cards = |p: &ti4_model::state::Player| p.action_cards.iter().any(|c| c.as_str() == crate::observed::HIDDEN);
    let hidden_secrets = |p: &ti4_model::state::Player| p.secret_objectives.iter().any(|c| c.as_str() == crate::observed::HIDDEN);
    if !state.players.iter().any(|p| hidden_cards(p) || hidden_secrets(p)) {
        return (std::borrow::Cow::Borrowed(state), Vec::new());
    }
    let mut owned = state.clone();
    let mut masked = Vec::new();
    for (seat, p) in owned.players.iter_mut().enumerate() {
        for (k, card) in p.action_cards.iter_mut().enumerate() {
            if card.as_str() == crate::observed::HIDDEN {
                *card = ti4_model::id::ActionCardId::new(ActionCard::ALL[(seat * 17 + k * 5 + 3) % ActionCard::COUNT].as_str());
            }
        }
        let mut secret_masked = false;
        for (k, card) in p.secret_objectives.iter_mut().enumerate() {
            if card.as_str() == crate::observed::HIDDEN {
                secret_masked = true;
                *card = ti4_model::id::SecretObjectiveId::new(SecretObjective::ALL[(seat * 11 + k * 7 + 1) % SecretObjective::COUNT].as_str());
            }
        }
        if secret_masked {
            masked.push(p.id.clone());
        }
    }
    (std::borrow::Cow::Owned(owned), masked)
}

/// One position, fully loaded but for the map.
pub struct Full {
    view: CoreView,
    cx: Ctx,
    kernel: &'static Kernel,
}

impl Full {
    /// `None` when the core cannot hold the position.
    #[must_use]
    pub fn of(state: &GameState) -> Option<Self> {
        let state = &*loadable(state).0;
        let mut cx = Ctx::of(state).ok()?;
        let view = CoreView::of(state, &mut cx, CodecSources::default(), SliceSet::of_without(SliceSet::ALL, Slice::Map)).ok()?;
        Some(Self { view, cx, kernel: kernel() })
    }

    #[must_use]
    pub fn core(&self) -> &CoreState {
        &self.view
    }

    #[must_use]
    pub const fn kernel(&self) -> &'static Kernel {
        self.kernel
    }

    #[must_use]
    pub fn seat(&self, player: &PlayerId) -> Option<Seat> {
        self.cx.owners.seat(player.as_str())
    }

    #[must_use]
    pub fn player(&self, seat: Seat) -> Option<PlayerId> {
        self.cx.owners.id(seat).map(PlayerId::new)
    }

    /// The players of some seats; `None` when one is unknown.
    #[must_use]
    pub fn players(&self, seats: impl IntoIterator<Item = Seat>) -> Option<Vec<PlayerId>> {
        seats.into_iter().map(|s| self.player(s)).collect()
    }

    /// The same, as a set.
    #[must_use]
    pub fn player_set(&self, seats: impl IntoIterator<Item = Seat>) -> Option<BTreeSet<PlayerId>> {
        seats.into_iter().map(|s| self.player(s)).collect()
    }
}

/// A position loaded with the slices a read names, with the kernel of the source scope (every faction module registered) and the scope and the
/// law's blank flag the rules read. For the reads that ask the core for one rule and do not need a whole position.
pub struct Loaded {
    view: CoreView,
    cx: Ctx,
    scope: ti4_engine_core::tables::Scope,
    blanked: bool,
    kernel: &'static Kernel,
}

impl Loaded {
    /// `None` when the core cannot hold the position (an id the tables do not know, a table with no faction seated, more than 16 owners).
    #[must_use]
    pub fn of(state: &GameState, galaxy: Option<&ti4_content::galaxy::Galaxy>, sources: ti4_model::content_types::SourceSet, reads: SliceSet) -> Option<Self> {
        let (loadable_state, _) = loadable(state);
        let state = &*loadable_state;
        let mut cx = Ctx::of(state).ok()?;
        let view = CoreView::of(state, &mut cx, CodecSources { galaxy, rng: None }, reads).ok()?;
        Some(Self { view, cx, scope: crate::ties::core_adapter::scope(sources), blanked: crate::ties::laws::blanked(state), kernel: crate::ties::core_adapter::kernel_for(sources) })
    }

    #[must_use]
    pub fn core(&self) -> &CoreState {
        &self.view
    }

    #[must_use]
    pub const fn kernel(&self) -> &'static Kernel {
        self.kernel
    }

    #[must_use]
    pub const fn scope(&self) -> ti4_engine_core::tables::Scope {
        self.scope
    }

    #[must_use]
    pub const fn blanked(&self) -> bool {
        self.blanked
    }

    #[must_use]
    pub fn seat(&self, player: &PlayerId) -> Option<Seat> {
        self.cx.owners.seat(player.as_str())
    }

    #[must_use]
    pub fn player(&self, seat: Seat) -> Option<PlayerId> {
        self.cx.owners.id(seat).map(PlayerId::new)
    }
}
