use ti4_engine_core::state::{CoreState, PendingTag, Slice};
use ti4_model::state::GameState;

use super::{Sources, SliceCodec};
use crate::core_sync::ctx::Ctx;
use crate::core_sync::error::{R, bad};

const S: Slice = Slice::Pending;

/// `GameState.pending` (the only text is `"move"`). The window stack and the open question live in
/// private `Game` fields until the driver is ported; they are not in `GameState` and stay default.
pub struct Pending;

impl SliceCodec for Pending {
    const SLICE: Slice = S;

    fn load(_: &mut Ctx, gs: &GameState, _: Sources<'_>, core: &mut CoreState) -> R<()> {
        core.pending.tag = match gs.pending.as_deref() {
            None => None,
            Some("move") => Some(PendingTag::Move),
            Some(other) => return Err(bad(S, "pending tag", other)),
        };
        Ok(())
    }

    fn store(_: &Ctx, core: &CoreState, gs: &mut GameState) -> R<()> {
        gs.pending = core.pending().tag.map(|PendingTag::Move| "move".to_owned());
        Ok(())
    }
}
