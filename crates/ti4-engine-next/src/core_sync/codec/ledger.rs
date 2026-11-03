use ti4_engine_core::generated::Relic;
use ti4_engine_core::state::{CoreState, RelicRec, Slice, VpRec};
use ti4_model::id::PlayerId;
use ti4_model::state::{GameState, RelicGainRecord};

use super::{Sources, SliceCodec};
use crate::core_sync::conv::{id, txt};
use crate::core_sync::ctx::Ctx;
use crate::core_sync::error::{R, bad};
use crate::core_sync::keys::{parse_ledger, render_ledger};

const S: Slice = Slice::Ledger;

/// The victory point ledger and the relic log, the two logs `GameState` carries. The event text log
/// (`Game.events`, `Resolver.log`) and the dice history live in `Game`, not in `GameState`: they stay
/// default here and are lent by the game-level caller when an area needs them. Reason texts are package X1's
/// typed [`LedgerReason`](ti4_engine_core::keys::LedgerReason); a text it does not type is `Unmapped`.
pub struct Ledger;

impl SliceCodec for Ledger {
    const SLICE: Slice = S;

    fn load(cx: &mut Ctx, gs: &GameState, _: Sources<'_>, core: &mut CoreState) -> R<()> {
        core.ledger.vp = ti4_engine_core::containers::Log::new();
        for (p, delta, reason) in &gs.vp_ledger {
            let player = cx.seat(S, p.as_str())?;
            let reason = parse_ledger(reason)
                .filter(|r| render_ledger(*r) == *reason)
                .ok_or_else(|| bad(S, "ledger reason not typed", reason))?;
            core.ledger.vp.push(VpRec { player, delta: *delta, reason });
        }
        core.ledger.relics = ti4_engine_core::containers::Log::new();
        for r in &gs.relic_log {
            core.ledger.relics.push(RelicRec {
                round: r.round,
                player: cx.seat(S, r.player.as_str())?,
                relic: id::<Relic>(S, "relic", &r.relic)?,
                victory_points: r.victory_points,
            });
        }
        Ok(())
    }

    fn store(cx: &Ctx, core: &CoreState, gs: &mut GameState) -> R<()> {
        let l = core.ledger();
        gs.vp_ledger = l
            .vp
            .iter()
            .map(|r| {
                Ok((PlayerId::new(cx.owner(S, r.player)?), r.delta, render_ledger(r.reason)))
            })
            .collect::<R<_>>()?;
        gs.relic_log = l
            .relics
            .iter()
            .map(|r| {
                Ok(RelicGainRecord {
                    round: r.round,
                    player: PlayerId::new(cx.owner(S, r.player)?),
                    relic: txt(r.relic).to_owned(),
                    victory_points: r.victory_points,
                })
            })
            .collect::<R<_>>()?;
        Ok(())
    }
}
