use ti4_engine_core::containers::Cow;
use ti4_engine_core::state::{CoreState, MarkMap, Slice};
use ti4_model::state::GameState;

use super::{Sources, SliceCodec};
use crate::core_sync::ctx::Ctx;
use crate::core_sync::error::{R, bad};

const S: Slice = Slice::Marks;

/// `faction_marks` on package X1's typed keys: `Codec::parse_mark` per row, rendered back to check
/// the text is its own canonical form. A mark the families do not type (a test invention, an owner
/// outside the table) is `Unmapped`: the caller keeps the string state for that game. The map is in
/// key order, which is the text order of the keys.
pub struct Marks;

impl SliceCodec for Marks {
    const SLICE: Slice = S;

    fn load(cx: &mut Ctx, gs: &GameState, _: Sources<'_>, core: &mut CoreState) -> R<()> {
        let mut m = MarkMap::new();
        for (k, v) in &gs.faction_marks {
            let (key, val) = cx.keys.parse_mark(k, v).ok_or_else(|| bad(S, "mark not typed", k))?;
            if cx.keys.render_mark(&key, &val).is_none_or(|(rk, rv)| rk != *k || rv != *v) {
                return Err(bad(S, "mark text is not canonical", k));
            }
            m.insert(key, val);
        }
        core.marks.tail = Cow::new(m);
        Ok(())
    }

    fn store(cx: &Ctx, core: &CoreState, gs: &mut GameState) -> R<()> {
        gs.faction_marks = core
            .marks()
            .tail
            .iter()
            .map(|(k, v)| cx.keys.render_mark(k, v).ok_or_else(|| bad(S, "mark without a text", format!("{k:?}"))))
            .collect::<R<_>>()?;
        Ok(())
    }
}
