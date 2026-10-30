//! The loader of the typed bot's view (docs/todo/D3-bots.md D3-1 item 3): a [`SeenOwned`] holds the typed copy of one
//! position and lends a [`ti4_engine_core::seen::Seen`] for a bound seat.
//!
//! Before the flip of the game to the typed core (D2) the typed state does not exist at decision time, so it is
//! loaded from the string `GameState` through the codec (`CoreView::of`, the slices of `seen::READS`; the decks, the
//! RNG streams and the ledger are not loaded, so the view could not leak them even by mistake). A position the typed
//! state cannot hold (an id the tables do not know: faction `generic` in the fixtures, Keleres, ...) is
//! [`Unmapped`]; the caller counts the reason and keeps the string bot for that decision. After D2 the borrow
//! `Seen::new(&game.core, kernel, seat)` replaces this loader and costs nothing.

use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::seen::{READS, Seen};
use ti4_engine_core::state::{CoreState, Slice, SliceSet};
use ti4_engine_core::Seat;
use ti4_model::id::PlayerId;

use crate::choice::Observed;
use crate::core_sync::{CoreView, Ctx, Sources, Unmapped};

/// One position as the typed view reads it, with the owner table that maps player ids to seats.
pub struct SeenOwned {
    core: CoreState,
    cx: Ctx,
    kernel: &'static Kernel,
    blanked: bool,
}

impl SeenOwned {
    /// Load the typed copy of the position `observed` shows.
    ///
    /// # Errors
    /// [`Unmapped`] for a value the typed state cannot hold.
    pub fn of(observed: &Observed<'_>) -> Result<Self, Unmapped> {
        let (state, _content, sources, galaxy) = observed.parts();
        let mut cx = Ctx::of(state)?;
        // A decision asked without a map (some nested windows build their `Observed` without one) loads without the
        // map slice; the movement accessors of the view then answer as the string side does without a galaxy.
        let reads = if galaxy.is_some() { READS } else { SliceSet::of_without(READS, Slice::Map) };
        let core = CoreView::of(state, &mut cx, Sources { galaxy, rng: None }, reads)?.into_state();
        let _ = sources;
        Ok(Self { core, cx, kernel: crate::production_view::kernel(), blanked: crate::laws::blanked(state) })
    }

    /// The view for the bound player; `None` when the id is not an owner of the game.
    #[must_use]
    pub fn seen(&self, me: &PlayerId) -> Option<Seen<'_>> {
        let seat = self.cx.owners.seat(me.as_str())?;
        Some(Seen::new(&self.core, self.kernel, seat).with_blanked(self.blanked))
    }

    /// The seat of a player id.
    #[must_use]
    pub fn seat(&self, player: &PlayerId) -> Option<Seat> {
        self.cx.owners.seat(player.as_str())
    }

    /// The player id of a seat.
    #[must_use]
    pub fn player(&self, seat: Seat) -> Option<PlayerId> {
        self.cx.owners.id(seat).map(PlayerId::new)
    }
}
