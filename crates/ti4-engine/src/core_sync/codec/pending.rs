use ti4_engine_core::state::{CoreState, PendingTag, Slice};
use ti4_model::state::GameState;

use super::{Sources, SliceCodec};
use crate::core_sync::ctx::Ctx;
use crate::core_sync::error::{R, bad};

const S: Slice = Slice::Pending;

/// `GameState.pending` (the only text is `"move"`) and `Ctx::resolver` (`pending.resolver`, which `store` leaves alone: it has no home in `GameState`). The window stack and the open question live in
/// private `Game` fields until the driver is ported; they are not in `GameState` and stay default.
pub struct Pending;

impl SliceCodec for Pending {
    const SLICE: Slice = S;

    fn load(cx: &mut Ctx, gs: &GameState, _: Sources<'_>, core: &mut CoreState) -> R<()> {
        core.pending.tag = match gs.pending.as_deref() {
            None => None,
            Some("move") => Some(PendingTag::Move),
            Some(other) => return Err(bad(S, "pending tag", other)),
        };
        // The resolver's persistent part is not in `GameState`: the caller hands it in through the `Ctx` (and reads it
        // back from there after a lend), as the stream positions are handed in through `Sources`.
        if let Some(resolver) = &cx.resolver {
            core.pending.resolver = resolver.clone();
        }
        Ok(())
    }

    fn store(_: &Ctx, core: &CoreState, gs: &mut GameState) -> R<()> {
        gs.pending = core.pending().tag.map(|PendingTag::Move| "move".to_owned());
        Ok(())
    }
}
