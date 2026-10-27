//! One codec per slice. `load` fills one slice of a [`CoreState`] from the string state; `store`
//! writes it back through the existing `GameState` structs. Slices that are not listed stay as they
//! are (default on load, untouched on store).

use ti4_content::galaxy::Galaxy;
use ti4_engine_core::state::{CoreState, Slice, SliceSet};
use ti4_model::state::GameState;

use super::ctx::Ctx;
use super::error::R;
use crate::rng::RngPositions;

mod agenda;
mod board;
mod cards;
mod decks;
mod explore;
mod ledger;
mod map;
mod marks;
mod meta;
mod objectives;
mod pending;
mod players;
pub mod rng;
mod seats;
mod strategy;
mod trade;
pub(crate) mod transient;

/// What a load reads besides the `GameState`: the string galaxy (the map slice) and the stream
/// positions of the game's random source (the rng slice). Both live outside `GameState`.
#[derive(Clone, Copy, Default)]
pub struct Sources<'a> {
    pub galaxy: Option<&'a Galaxy>,
    pub rng: Option<&'a RngPositions>,
}

/// The codec of one slice.
pub trait SliceCodec {
    const SLICE: Slice;

    /// Fill `core`'s slice from the string state.
    ///
    /// # Errors
    /// A value the typed state cannot hold.
    fn load(cx: &mut Ctx, gs: &GameState, src: Sources<'_>, core: &mut CoreState) -> R<()>;

    /// Write the slice back into the string state.
    ///
    /// # Errors
    /// A typed value without a text (a code the table never issued).
    fn store(cx: &Ctx, core: &CoreState, gs: &mut GameState) -> R<()>;
}

macro_rules! dispatch {
    ($slice:expr, $codec:ident :: $f:ident ( $($a:expr),* )) => {
        match $slice {
            Slice::Meta => meta::Meta::$f($($a),*),
            Slice::Seats => seats::Seats::$f($($a),*),
            Slice::Players => players::Players::$f($($a),*),
            Slice::Board => board::Board::$f($($a),*),
            Slice::Map => map::Map::$f($($a),*),
            Slice::Decks => decks::Decks::$f($($a),*),
            Slice::Cards => cards::Cards::$f($($a),*),
            Slice::Objectives => objectives::Objectives::$f($($a),*),
            Slice::Agenda => agenda::Agenda::$f($($a),*),
            Slice::Strategy => strategy::Strategy::$f($($a),*),
            Slice::Explore => explore::Explore::$f($($a),*),
            Slice::Trade => trade::Trade::$f($($a),*),
            Slice::Marks => marks::Marks::$f($($a),*),
            Slice::Transient => transient::Transient::$f($($a),*),
            Slice::Pending => pending::Pending::$f($($a),*),
            Slice::Rng => rng::Rng::$f($($a),*),
            Slice::Ledger => ledger::Ledger::$f($($a),*),
        }
    };
}

/// The facade over the per-slice codecs.
pub struct Codec;

impl Codec {
    /// One slice, loaded into `core` (marked loaded).
    ///
    /// # Errors
    /// A value the typed state cannot hold.
    pub fn load_slice(cx: &mut Ctx, gs: &GameState, src: Sources<'_>, slice: Slice, core: &mut CoreState) -> R<()> {
        dispatch!(slice, SliceCodec::load(cx, gs, src, core))?;
        core.mark_loaded(slice);
        Ok(())
    }

    /// The slices of `slices`, loaded into a fresh state; the others stay default and unloaded.
    ///
    /// # Errors
    /// A value the typed state cannot hold.
    pub fn load(cx: &mut Ctx, gs: &GameState, src: Sources<'_>, slices: SliceSet) -> R<CoreState> {
        let mut core = CoreState::with_loaded(SliceSet::EMPTY);
        for slice in Slice::ALL {
            if slices.has(slice) {
                Self::load_slice(cx, gs, src, slice, &mut core)?;
            }
        }
        Ok(core)
    }

    /// One slice, written back into `gs`.
    ///
    /// # Errors
    /// A typed value without a text.
    pub fn store_slice(cx: &Ctx, core: &CoreState, gs: &mut GameState, slice: Slice) -> R<()> {
        dispatch!(slice, SliceCodec::store(cx, core, gs))
    }

    /// The slices of `slices`, written back into `gs`.
    ///
    /// # Errors
    /// A typed value without a text.
    pub fn store(cx: &Ctx, core: &CoreState, gs: &mut GameState, slices: SliceSet) -> R<()> {
        for slice in Slice::ALL {
            if slices.has(slice) {
                Self::store_slice(cx, core, gs, slice)?;
            }
        }
        Ok(())
    }
}
