use ti4_engine_core::containers::{Cow, DecimalTextOrd, SortedMap};
use ti4_engine_core::state::{CoreState, MarkFamily, MarkKey, MarkVal, Slice};
use ti4_model::state::GameState;

use super::{Sources, SliceCodec};
use crate::core_sync::ctx::Ctx;
use crate::core_sync::error::R;

const S: Slice = Slice::Marks;

/// `faction_marks`: until package X1 pins the key families, every key text is one interned family
/// (`MarkKey { family: code(key), a: 0, b: 0, seq: 0 }`) and every value text an interned code
/// (`MarkVal(code(value))`). Lossless, but the typed order is the order of first use, not the text
/// order: the string side re-sorts on store, and core code must not iterate marks in order until the
/// typed keys land.
pub struct Marks;

impl SliceCodec for Marks {
    const SLICE: Slice = S;

    fn load(cx: &mut Ctx, gs: &GameState, _: Sources<'_>, core: &mut CoreState) -> R<()> {
        let mut m = SortedMap::new();
        for (k, v) in &gs.faction_marks {
            let family = MarkFamily(cx.texts.intern(S, k)?.0);
            let val = MarkVal(i32::from(cx.texts.intern(S, v)?.0));
            m.insert(MarkKey { family, a: 0, b: 0, seq: DecimalTextOrd(0) }, val);
        }
        core.marks.tail = Cow::new(m);
        Ok(())
    }

    fn store(cx: &Ctx, core: &CoreState, gs: &mut GameState) -> R<()> {
        gs.faction_marks = core
            .marks()
            .tail
            .iter()
            .map(|(k, v)| {
                let key = cx.texts.text(S, ti4_engine_core::state::Code(k.family.0))?.to_owned();
                let val = cx.texts.text(S, ti4_engine_core::state::Code(v.0 as u16))?.to_owned();
                Ok((key, val))
            })
            .collect::<R<_>>()?;
        Ok(())
    }
}
