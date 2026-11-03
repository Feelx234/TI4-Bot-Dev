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

use crate::observed::Observed;
use crate::core_sync::{CoreView, Ctx, Sources, Unmapped};

/// The kernel of the view for a source scope: the full hook table (every faction module, the seams of the laws and
/// relics), with the scope of the game it reads (units, planets and systems of an expansion the game does not play are
/// not in the tables the rules read, as in the string `catalogue(content, sources)`). Built once per scope and process.
fn kernel_for(sources: ti4_model::content_types::SourceSet) -> &'static Kernel {
    static KERNELS: [std::sync::OnceLock<Kernel>; 128] = [const { std::sync::OnceLock::new() }; 128];
    let slot = usize::try_from(sources.as_u32()).ok().and_then(|i| KERNELS.get(i)).unwrap_or(&KERNELS[127]);
    slot.get_or_init(|| {
        let mut k = crate::ties::production_view::kernel().clone();
        k.sources = ti4_engine_core::kernel::Sources(u8::try_from(crate::ties::core_adapter::scope(sources).0).unwrap_or(u8::MAX));
        k
    })
}

/// One position as the typed view reads it, with the owner table that maps player ids to seats.
#[derive(Clone)]
pub struct SeenOwned {
    core: CoreState,
    cx: Ctx,
    kernel: &'static Kernel,
    blanked: bool,
    /// The players whose secret objectives were markers in the position (a redacted view): the core holds placeholders for them.
    masked: Vec<PlayerId>,
}

impl SeenOwned {
    /// Load the typed copy of the position `observed` shows.
    ///
    /// # Errors
    /// [`Unmapped`] for a value the typed state cannot hold.
    pub fn of(observed: &Observed<'_>) -> Result<Self, Unmapped> {
        let (state, _content, sources, galaxy) = observed.parts();
        Self::from_parts(state, sources, galaxy)
    }

    /// Load the typed copy of a position given by its parts (the objectives queries of the surface build theirs from a state, a
    /// source scope and a map, without an `Observed`).
    ///
    /// # Errors
    /// [`Unmapped`] for a value the typed state cannot hold.
    pub fn from_parts(state: &ti4_model::state::GameState, sources: ti4_model::content_types::SourceSet, galaxy: Option<&ti4_content::galaxy::Galaxy>) -> Result<Self, Unmapped> {
        // A redacted view holds markers where other seats' hands are: they are loaded as placeholders (queries::view::loadable).
        let (loadable, masked) = crate::queries::view::loadable(state);
        let state = &*loadable;
        let mut cx = Ctx::of(state)?;
        // A decision asked without a map (some nested windows build their `Observed` without one) loads without the
        // map slice; the movement accessors of the view then answer as the string side does without a galaxy.
        let reads = if galaxy.is_some() { READS } else { SliceSet::of_without(READS, Slice::Map) };
        let core = CoreView::of(state, &mut cx, Sources { galaxy, rng: None }, reads)?.into_state();
        Ok(Self { core, cx, kernel: kernel_for(sources), blanked: crate::ties::laws::blanked(state), masked })
    }

    /// The loaded typed state.
    #[must_use]
    pub(crate) const fn core(&self) -> &CoreState {
        &self.core
    }

    /// The hook table of the position's source scope.
    #[must_use]
    pub(crate) const fn kernel(&self) -> &'static Kernel {
        self.kernel
    }

    /// Whether the secret objectives of `player` were hidden markers in the position (their placeholders must not be read).
    #[must_use]
    pub(crate) fn secrets_masked(&self, player: &PlayerId) -> bool {
        self.masked.contains(player)
    }

    /// Whether all laws are blank this turn (Law's Order).
    #[must_use]
    pub(crate) const fn blanked(&self) -> bool {
        self.blanked
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

impl SeenOwned {
    /// The string model of one system from the typed view, for the string helpers the bot and the learned policies
    /// share (`fleet_strength::descriptors`, `valuation`, `objectives::*Position`): they keep their string signatures in
    /// D3 (trap T17), so the same function runs on the same data and its `f64` arithmetic stays byte-identical. Filled:
    /// `units`, `command_tokens`, `planet_control`, `planet_units` (non-empty lists), which is every field those helpers
    /// read; the coexistence and purge maps stay empty.
    #[must_use]
    pub fn to_model_system(&self, system: ti4_engine_core::generated::System) -> ti4_model::state::SystemState {
        use crate::generated::IdStr;
        use ti4_model::id::{PlanetId, UnitTypeId};
        use ti4_model::units::Unit;
        let seen = Seen::new(&self.core, self.kernel, Seat(0));
        let view = seen.system(system);
        let owner = |seat: Seat| PlayerId::new(self.cx.owners.id(seat).unwrap_or_default());
        let unit = |p: &ti4_engine_core::state::Piece| Unit {
            type_id: UnitTypeId::new(p.kind().as_str()),
            owner: owner(p.owner()),
            sustained_damage: p.sustained(),
            galvanized: p.galvanized(),
        };
        let mut out = ti4_model::state::SystemState::default();
        out.units = view.units().iter().map(unit).collect();
        out.command_tokens = view.tokens().iter().map(owner).collect();
        for planet in view.planets().as_slice() {
            let cell = view.planet(*planet);
            if let Some(seat) = cell.control {
                out.planet_control.insert(PlanetId::new(planet.as_str()), owner(seat));
            }
            if !cell.units.is_empty() {
                out.planet_units.insert(PlanetId::new(planet.as_str()), cell.units.iter().map(unit).collect());
            }
        }
        out
    }

    /// A copy whose hidden information is scrambled, for the perturbation test: the identities and order of every
    /// other seat's hand (action cards, secrets) change, their sizes do not (hand sizes are public). The decks, the
    /// RNG streams, the ledger and the pending machine are not loaded at all (`seen::READS`), so there is nothing to
    /// scramble there. Every accessor of a [`Seen`] over the copy must answer as over the original.
    #[must_use]
    pub fn scrambled(&self, me: &PlayerId) -> Self {
        use ti4_engine_core::generated::{ActionCard, SecretObjective};
        let mut out = self.clone();
        let Some(me) = self.seat(me) else { return out };
        let players = out.core.players().len();
        for i in 0..players {
            let seat = out.core.players().get(i).seat;
            if seat == me {
                continue;
            }
            let p = out.core.players_mut().edit(i);
            let n = p.cards.hand.len();
            p.cards.hand.clear();
            for k in 0..n {
                p.cards.hand.push(ActionCard::ALL[(k * 7 + usize::from(seat.0) * 13 + 3) % ActionCard::COUNT]);
            }
            let m = p.cards.secrets.len();
            p.cards.secrets.clear();
            for k in 0..m {
                p.cards.secrets.push(SecretObjective::ALL[(k * 5 + usize::from(seat.0) * 11 + 1) % SecretObjective::COUNT]);
            }
        }
        out
    }
}
