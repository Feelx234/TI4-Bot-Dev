use ti4_engine_core::containers::Deck;
use ti4_engine_core::generated::{ActionCard, Agenda, Explore, ExploreType, PublicObjective, Relic, SecretObjective};
use ti4_engine_core::state::{CoreState, Slice};
use ti4_model::id::{ActionCardId, ObjectiveId, RelicId, SecretObjectiveId};
use ti4_model::state::GameState;

use super::{Sources, SliceCodec};
use crate::core_sync::conv::{explore_key, explore_type, id, txt};
use crate::core_sync::ctx::Ctx;
use crate::core_sync::error::R;
use crate::generated::IdStr;

const S: Slice = Slice::Decks;

pub struct Decks;

fn deck<T: IdStr + Copy, I: AsRef<str>>(what: &'static str, cards: impl IntoIterator<Item = I>) -> R<Deck<T>> {
    Ok(Deck::new(cards.into_iter().map(|c| id::<T>(S, what, c.as_ref())).collect::<R<Vec<_>>>()?))
}

impl SliceCodec for Decks {
    const SLICE: Slice = S;

    fn load(_: &mut Ctx, gs: &GameState, _: Sources<'_>, core: &mut CoreState) -> R<()> {
        let d = &mut core.decks;
        d.action = deck::<ActionCard, _>("action card", gs.action_card_deck.iter().map(ActionCardId::as_str))?;
        d.secret = deck::<SecretObjective, _>("secret objective", gs.secret_deck.iter().map(SecretObjectiveId::as_str))?;
        d.agenda = deck::<Agenda, _>("agenda", gs.agenda_deck.iter())?;
        d.relic = deck::<Relic, _>("relic", gs.relic_deck.iter().map(RelicId::as_str))?;
        d.objective = deck::<PublicObjective, _>("public objective", gs.objective_deck.iter().map(ObjectiveId::as_str))?;
        d.explore_present = 0;
        for (key, cards) in &gs.exploration_decks {
            let t = explore_type(S, key)?;
            d.explore[t as usize] = deck::<Explore, _>("exploration card", cards.iter())?;
            d.explore_present |= 1 << t as u8;
        }
        Ok(())
    }

    fn store(_: &Ctx, core: &CoreState, gs: &mut GameState) -> R<()> {
        let d = core.decks();
        gs.action_card_deck = d.action.cards().iter().map(|c| ActionCardId::new(txt(*c))).collect();
        gs.secret_deck = d.secret.cards().iter().map(|c| SecretObjectiveId::new(txt(*c))).collect();
        gs.agenda_deck = d.agenda.cards().iter().map(|c| txt(*c).to_owned()).collect();
        gs.relic_deck = d.relic.cards().iter().map(|c| RelicId::new(txt(*c))).collect();
        gs.objective_deck = d.objective.cards().iter().map(|c| ObjectiveId::new(txt(*c))).collect();
        gs.exploration_decks.clear();
        for t in ExploreType::ALL {
            if d.explore_present & (1 << t as u8) != 0 {
                gs.exploration_decks
                    .insert(explore_key(t), d.explore[t as usize].cards().iter().map(|c| txt(*c).to_owned()).collect());
            }
        }
        Ok(())
    }
}
