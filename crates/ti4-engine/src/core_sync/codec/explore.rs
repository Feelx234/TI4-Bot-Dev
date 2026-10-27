use ti4_engine_core::containers::{Log, Set, SmallList, SortedMap};
use ti4_engine_core::generated::{Explore as ExploreCard, Planet, System, Wormhole};
use ti4_engine_core::state::{CoreState, Expedition, ExploreRec, Slice};
use ti4_model::id::{PlanetId, PlayerId, SystemId};
use ti4_model::state::{ExplorationRecord, GameState};

use super::{Sources, SliceCodec};
use crate::core_sync::conv::{explore_key, explore_type, id, txt};
use crate::core_sync::ctx::Ctx;
use crate::core_sync::error::{R, bad};

const S: Slice = Slice::Explore;

pub struct Explore;

const EXPEDITIONS: [(Expedition, &str); 6] = [
    (Expedition::ActionCards, "action_cards"),
    (Expedition::Influence, "influence"),
    (Expedition::Resources, "resources"),
    (Expedition::Secret, "secret"),
    (Expedition::TechPlanet, "tech_planet"),
    (Expedition::TradeGoods, "trade_goods"),
];

fn systems<'a>(texts: impl IntoIterator<Item = &'a SystemId>) -> R<Set<System, 4>> {
    let mut s = Set::new();
    for t in texts {
        s.insert(id::<System>(S, "system", t.as_str())?);
    }
    Ok(s)
}

impl SliceCodec for Explore {
    const SLICE: Slice = S;

    fn load(cx: &mut Ctx, gs: &GameState, _: Sources<'_>, core: &mut CoreState) -> R<()> {
        let e = &mut core.explore;
        e.frontier = systems(&gs.frontier_tokens)?;
        e.ingress = systems(&gs.ingress_tokens)?;
        e.breach = systems(&gs.breach_tokens)?;
        e.expedition_slices = SortedMap::new();
        for (k, owner) in &gs.expedition_slices {
            let (slice, _) = EXPEDITIONS.iter().find(|(_, t)| t == k).ok_or_else(|| bad(S, "expedition slice", k))?;
            e.expedition_slices.insert(*slice, cx.seat(S, owner.as_str())?);
        }
        e.wormhole_tokens = SortedMap::new();
        for (k, sys) in &gs.wormhole_tokens {
            e.wormhole_tokens.insert(id::<Wormhole>(S, "wormhole kind", k)?, id::<System>(S, "system", sys.as_str())?);
        }
        let mut log = Log::new();
        for r in &gs.exploration_log {
            let mut gains = SortedMap::new();
            for (k, v) in &r.gains {
                gains.insert(cx.texts.intern(S, k)?, *v);
            }
            let mut placed: SmallList<_, 2> = SmallList::new();
            for p in &r.placed {
                placed.push(cx.texts.intern(S, p)?);
            }
            log.push(ExploreRec {
                round: r.round,
                player: cx.seat(S, r.player.as_str())?,
                deck: explore_type(S, &r.deck)?,
                card: id::<ExploreCard>(S, "exploration card", &r.card)?,
                planet: r.planet.as_ref().map(|p| id::<Planet>(S, "planet", p.as_str())).transpose()?,
                outcome: cx.texts.intern(S, &r.outcome)?,
                system: r.system.as_deref().map(|s| id::<System>(S, "system", s)).transpose()?,
                // An empty result text is not written (`skip_serializing_if`): `None`.
                result: if r.result.is_empty() { None } else { Some(cx.texts.intern(S, &r.result)?) },
                gains,
                placed,
            });
        }
        e.log = log;
        Ok(())
    }

    fn store(cx: &Ctx, core: &CoreState, gs: &mut GameState) -> R<()> {
        let e = core.explore();
        gs.frontier_tokens = e.frontier.iter().map(|s| SystemId::new(txt(s))).collect();
        gs.ingress_tokens = e.ingress.iter().map(|s| SystemId::new(txt(s))).collect();
        gs.breach_tokens = e.breach.iter().map(|s| SystemId::new(txt(s))).collect();
        gs.expedition_slices = e
            .expedition_slices
            .iter()
            .map(|(k, v)| {
                let text = EXPEDITIONS.iter().find(|(x, _)| x == k).map(|(_, t)| *t).unwrap_or_default();
                Ok((text.to_owned(), PlayerId::new(cx.owner(S, *v)?)))
            })
            .collect::<R<_>>()?;
        gs.wormhole_tokens = e.wormhole_tokens.iter().map(|(k, v)| (txt(*k).to_owned(), SystemId::new(txt(*v)))).collect();
        gs.exploration_log = e
            .log
            .iter()
            .map(|r| {
                Ok(ExplorationRecord {
                    round: r.round,
                    player: PlayerId::new(cx.owner(S, r.player)?),
                    deck: explore_key(r.deck),
                    card: txt(r.card).to_owned(),
                    planet: r.planet.map(|p| PlanetId::new(txt(p))),
                    outcome: cx.texts.text(S, r.outcome)?.to_owned(),
                    system: r.system.map(|s| txt(s).to_owned()),
                    result: r.result.map(|c| cx.texts.text(S, c).map(str::to_owned)).transpose()?.unwrap_or_default(),
                    gains: r.gains.iter().map(|(k, v)| Ok((cx.texts.text(S, *k)?.to_owned(), *v))).collect::<R<_>>()?,
                    placed: r.placed.iter().map(|c| cx.texts.text(S, *c).map(str::to_owned)).collect::<R<_>>()?,
                })
            })
            .collect::<R<_>>()?;
        Ok(())
    }
}
