use ti4_engine_core::containers::Cow;
use ti4_engine_core::generated::{ActionCard, Faction, PromissoryNote};
use ti4_engine_core::state::{CoreState, NoteKey, NoteMap, Slice};
use ti4_model::id::{ActionCardId, PlayerId};
use ti4_model::state::GameState;

use super::{Sources, SliceCodec};
use crate::core_sync::conv::{id, txt};
use crate::core_sync::ctx::Ctx;
use crate::core_sync::error::{R, bad};
use crate::generated::IdStr;

const S: Slice = Slice::Cards;

pub struct Cards;

/// `"{note alias}:{faction}"`. A generic note (`cf`, `ps`, `ta`, `an`, `sftt`) is the corpus record
/// `<color>_{alias}`.
pub(super) fn note_key(text: &str) -> R<NoteKey> {
    let (alias, faction) = text.split_once(':').ok_or_else(|| bad(S, "promissory note id", text))?;
    let note = PromissoryNote::from_id(alias)
        .or_else(|| PromissoryNote::from_id(&format!("<color>_{alias}")))
        .ok_or_else(|| bad(S, "promissory note", alias))?;
    Ok(NoteKey { note, faction: id::<Faction>(S, "faction", faction)? })
}

pub(super) fn note_text(k: NoteKey) -> String {
    let id = txt(k.note);
    format!("{}:{}", id.strip_prefix("<color>_").unwrap_or(id), txt(k.faction))
}

impl SliceCodec for Cards {
    const SLICE: Slice = S;

    fn load(cx: &mut Ctx, gs: &GameState, _: Sources<'_>, core: &mut CoreState) -> R<()> {
        let c = &mut core.cards;
        c.discarded_action = Cow::new(
            gs.discarded_action_cards.iter().map(|x| id::<ActionCard>(S, "action card", x.as_str())).collect::<R<_>>()?,
        );
        c.salvage_yard =
            Cow::new(gs.salvage_yard.iter().map(|x| id::<ActionCard>(S, "action card", x.as_str())).collect::<R<_>>()?);
        // The redo-replay reserve (`#[serde(skip)]`, private fields in the model) is not convertible; a
        // replay stays on the string path.
        if gs.deck_reserve.is_some() {
            return Err(bad(S, "deck_reserve (turn-redo replay)", ""));
        }
        c.reserve = None;
        let mut notes = NoteMap::<ti4_engine_core::ids::Seat>::new();
        for (k, holder) in &gs.promissory_notes {
            notes.insert(note_key(k)?, cx.seat(S, holder.as_str())?);
        }
        c.promissory = Cow::new(notes);
        let mut faceup = NoteMap::<()>::new();
        for k in &gs.promissory_faceup {
            faceup.insert(note_key(k)?, ());
        }
        c.promissory_faceup = Cow::new(faceup);
        Ok(())
    }

    fn store(cx: &Ctx, core: &CoreState, gs: &mut GameState) -> R<()> {
        let c = core.cards();
        gs.discarded_action_cards = c.discarded_action.iter().map(|x| ActionCardId::new(txt(*x))).collect();
        gs.salvage_yard = c.salvage_yard.iter().map(|x| ActionCardId::new(txt(*x))).collect();
        gs.promissory_notes = c
            .promissory
            .iter()
            .map(|(k, v)| Ok((note_text(*k), PlayerId::new(cx.owner(S, *v)?))))
            .collect::<R<_>>()?;
        gs.promissory_faceup = c.promissory_faceup.keys().map(note_text).collect();
        Ok(())
    }
}
