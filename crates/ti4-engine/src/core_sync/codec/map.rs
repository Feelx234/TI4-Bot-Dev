use ti4_engine_core::state::MapSlice;
use ti4_engine_core::state::{CoreState, Slice};
use ti4_model::state::GameState;

use super::{Sources, SliceCodec};
use crate::core_adapter::typed_map;
use crate::core_sync::ctx::Ctx;
use crate::core_sync::error::{R, bad};

const S: Slice = Slice::Map;

/// The map slice is the typed galaxy (`CoreGalaxy`). The string galaxy is not in `GameState`, so the
/// load needs it in [`Sources`]; the slice is read-only (`store` writes nothing): a galaxy edit
/// reaches the string side through the state fields that cause it (wormhole tokens, purged
/// systems), which the board and explore slices own.
pub struct Map;

impl SliceCodec for Map {
    const SLICE: Slice = S;

    fn load(_: &mut Ctx, _: &GameState, src: Sources<'_>, core: &mut CoreState) -> R<()> {
        let galaxy = src.galaxy.ok_or_else(|| bad(S, "the map slice needs the string galaxy", ""))?;
        // Cached per galaxy value: the conversion and the adjacency are built once, not per lend.
        let map = typed_map(galaxy).ok_or_else(|| bad(S, "galaxy", "the map cannot be expressed"))?;
        core.map = MapSlice::of(map.galaxy, map.adjacency);
        Ok(())
    }

    fn store(_: &Ctx, _: &CoreState, _: &mut GameState) -> R<()> {
        Ok(())
    }
}
