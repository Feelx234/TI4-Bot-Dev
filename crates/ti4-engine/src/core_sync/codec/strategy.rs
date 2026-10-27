use ti4_engine_core::generated::StrategyCard;
use ti4_engine_core::state::{CoreState, Slice};
use ti4_model::id::{PlayerId, StrategyCardId};
use ti4_model::state::GameState;

use super::{Sources, SliceCodec};
use crate::core_sync::conv::{id, list, txt};
use crate::core_sync::ctx::Ctx;
use crate::core_sync::error::R;

const S: Slice = Slice::Strategy;

pub struct Strategy;

impl SliceCodec for Strategy {
    const SLICE: Slice = S;

    fn load(cx: &mut Ctx, gs: &GameState, _: Sources<'_>, core: &mut CoreState) -> R<()> {
        let st = &mut core.strategy;
        st.unclaimed = list(S, "strategy card", gs.unclaimed_strategy_cards.iter().map(StrategyCardId::as_str))?;
        st.goods.clear();
        for (k, v) in &gs.strategy_card_goods {
            st.goods.insert(id::<StrategyCard>(S, "strategy card", k.as_str())?, *v);
        }
        st.initiative.clear();
        for (k, v) in &gs.card_initiative {
            st.initiative.insert(id::<StrategyCard>(S, "strategy card", k.as_str())?, *v);
        }
        st.initiative_overrides.clear();
        for (k, v) in &gs.initiative_overrides {
            st.initiative_overrides.insert(cx.seat(S, k.as_str())?, *v);
        }
        Ok(())
    }

    fn store(cx: &Ctx, core: &CoreState, gs: &mut GameState) -> R<()> {
        let st = core.strategy();
        gs.unclaimed_strategy_cards = st.unclaimed.iter().map(|c| StrategyCardId::new(txt(*c))).collect();
        gs.strategy_card_goods = st.goods.iter().map(|(k, v)| (StrategyCardId::new(txt(*k)), *v)).collect();
        gs.card_initiative = st.initiative.iter().map(|(k, v)| (StrategyCardId::new(txt(*k)), *v)).collect();
        gs.initiative_overrides = st
            .initiative_overrides
            .iter()
            .map(|(k, v)| Ok((PlayerId::new(cx.owner(S, *k)?), *v)))
            .collect::<R<_>>()?;
        Ok(())
    }
}
