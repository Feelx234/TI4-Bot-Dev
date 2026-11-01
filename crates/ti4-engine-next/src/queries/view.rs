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
