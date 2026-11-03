use ti4_engine_core::ids::Seat;
use ti4_engine_core::state::{CoreState, Slice};
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use super::{Sources, SliceCodec};
use crate::core_sync::ctx::Ctx;
use crate::core_sync::error::{R, bad};

const S: Slice = Slice::Seats;

pub struct Seats;

impl SliceCodec for Seats {
    const SLICE: Slice = S;

    fn load(cx: &mut Ctx, gs: &GameState, _: Sources<'_>, core: &mut CoreState) -> R<()> {
        if gs.players.len() > ti4_engine_core::state::MAX_PLAYERS {
            return Err(bad(S, "more than 8 players", gs.players.len()));
        }
        if gs.seating_order.len() != gs.players.len() {
            return Err(bad(S, "seating_order is not one entry per player", gs.seating_order.len()));
        }
        let players: Vec<Seat> = gs.players.iter().map(|p| cx.seat(S, p.id.as_str())).collect::<R<_>>()?;
        let order: Vec<Seat> = gs.seating_order.iter().map(|p| cx.seat(S, p.as_str())).collect::<R<_>>()?;
        core.seats = ti4_engine_core::state::Seats::new(&players, &order, cx.owners.len() as u8);
        Ok(())
    }

    fn store(cx: &Ctx, core: &CoreState, gs: &mut GameState) -> R<()> {
        gs.seating_order =
            core.seats().seating_order().iter().map(|s| cx.owner(S, *s).map(PlayerId::new)).collect::<R<_>>()?;
        Ok(())
    }
}
